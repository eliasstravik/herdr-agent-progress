use crate::state::Identity;
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn output(command: &mut Command) -> Result<String> {
    // Bound every external invocation; an unavailable Herdr must not hang a hook.
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let read = |mut stream: Box<dyn std::io::Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            stream.read_to_end(&mut bytes).map(|_| bytes)
        })
    };
    let out = read(Box::new(stdout));
    let err = read(Box::new(stderr));
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        if let Some(status) = child.try_wait()? {
            let stdout = out.join().unwrap()?;
            let stderr = err.join().unwrap()?;
            ensure!(
                status.success(),
                "Command failed: {}",
                String::from_utf8_lossy(&stderr).trim()
            );
            return Ok(String::from_utf8(stdout)?);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Command timed out after four seconds");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[derive(Clone)]
pub struct Runtime {
    pub endpoint: String,
    pub socket: String,
    pub bin: String,
}
impl Runtime {
    pub fn new(socket: Option<String>) -> Result<Self> {
        let socket = socket
            .or_else(|| env::var("HERDR_SOCKET_PATH").ok())
            .context("HERDR_SOCKET_PATH is missing")?;
        let endpoint = endpoint_identity(Path::new(&socket))?;
        Ok(Self {
            endpoint,
            socket,
            bin: env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".into()),
        })
    }
    pub fn call(&self, args: &[&str]) -> Result<Value> {
        ensure!(
            endpoint_identity(Path::new(&self.socket))? == self.endpoint,
            "Herdr endpoint was replaced"
        );
        let s = output(
            Command::new(&self.bin)
                .env("HERDR_SOCKET_PATH", &self.socket)
                .args(args),
        )?;
        // Metadata writes acknowledge success with exit status alone.
        if args.starts_with(&["pane", "report-metadata"]) && s.trim().is_empty() {
            return Ok(serde_json::json!({"type":"ok"}));
        }
        let v: Value = serde_json::from_str(&s).context("Herdr did not return JSON")?;
        ensure!(v.get("error").is_none(), "Herdr API error: {}", v["error"]);
        Ok(v["result"].clone())
    }
    pub fn current(&self) -> Result<Value> {
        ensure!(
            env::var("HERDR_ENV").as_deref() == Ok("1") && env::var("HERDR_PANE_ID").is_ok(),
            "Reporting requires the caller's own Herdr pane environment"
        );
        Ok(self.call(&["pane", "current", "--current"])?["pane"].clone())
    }
    pub fn identity(&self, pane: &Value, ancestor: bool) -> Result<Identity> {
        let agent = string(pane, "agent")?;
        ensure!(
            matches!(agent.as_str(), "claude" | "codex"),
            "Runtime binding is unavailable for {agent}"
        );
        let session = &pane["agent_session"];
        ensure!(
            session["agent"] == agent
                && session["kind"] == "id"
                && session["source"] == format!("herdr:{agent}"),
            "Official native session identity is unavailable"
        );
        let info = self.call(&["pane", "process-info", "--pane", &string(pane, "pane_id")?])?;
        let table = processes()?;
        let candidates = info["process_info"]["foreground_processes"]
            .as_array()
            .context("Foreground process information unavailable")?;
        let ids: Vec<u32> = candidates
            .iter()
            .filter(|p| {
                let name = p["name"].as_str().unwrap_or("");
                let argv = p["argv0"].as_str().unwrap_or("");
                Path::new(name).file_name().and_then(|x| x.to_str()) == Some(&agent)
                    || Path::new(argv).file_name().and_then(|x| x.to_str()) == Some(&agent)
            })
            .filter_map(|p| p["pid"].as_u64().map(|p| p as u32))
            .collect();
        ensure!(ids.len() == 1, "Ambiguous foreground agent process");
        let pid = ids[0];
        let process = table.get(&pid).context("Agent process exited")?;
        if ancestor {
            ensure!(
                is_ancestor(&table, pid, std::process::id()),
                "Caller is not a descendant of the active agent launch"
            );
        }
        Ok(Identity {
            endpoint: self.endpoint.clone(),
            terminal: string(pane, "terminal_id")?,
            session: string(session, "value")?,
            agent,
            pid,
            started: process.started.clone(),
        })
    }
    pub fn live(&self, id: &Identity, pane: &Value) -> bool {
        let fresh = string(pane, "pane_id").and_then(|pane| self.call(&["pane", "get", &pane]));
        fresh
            .and_then(|result| self.identity(&result["pane"], false))
            .is_ok_and(|live| live == *id)
    }
}

pub fn string(v: &Value, key: &str) -> Result<String> {
    v[key]
        .as_str()
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .with_context(|| format!("Missing {key}"))
}

#[cfg(unix)]
fn endpoint_identity(path: &Path) -> Result<String> {
    use std::os::unix::fs::{FileTypeExt, MetadataExt};
    let meta = fs::metadata(path).context("Herdr socket unavailable")?;
    ensure!(
        meta.file_type().is_socket(),
        "Endpoint is not a Unix socket"
    );
    Ok(format!(
        "{}:{}:{}:{}:{}",
        fs::canonicalize(path)?.display(),
        meta.dev(),
        meta.ino(),
        meta.ctime(),
        meta.ctime_nsec()
    ))
}
#[cfg(not(unix))]
fn endpoint_identity(_: &Path) -> Result<String> {
    bail!("This build cannot verify the endpoint and process launch identity on this OS")
}

pub struct Process {
    parent: u32,
    started: String,
}
pub fn processes() -> Result<HashMap<u32, Process>> {
    let text = output(
        Command::new("ps")
            .env("LC_ALL", "C")
            .args(["-axo", "pid=,ppid=,lstart="]),
    )?;
    let mut table = HashMap::new();
    for line in text.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 7 {
            continue;
        }
        if let (Ok(pid), Ok(parent)) = (fields[0].parse(), fields[1].parse()) {
            table.insert(
                pid,
                Process {
                    parent,
                    started: fields[2..].join(" "),
                },
            );
        }
    }
    ensure!(!table.is_empty(), "OS process start times unavailable");
    Ok(table)
}
fn is_ancestor(table: &HashMap<u32, Process>, expected: u32, mut child: u32) -> bool {
    for _ in 0..128 {
        if child == expected {
            return true;
        }
        let Some(p) = table.get(&child) else {
            return false;
        };
        if p.parent == child {
            return false;
        }
        child = p.parent;
    }
    false
}

#[derive(Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub state: PathBuf,
}
impl Paths {
    pub fn discover() -> Result<Self> {
        let base = env::var_os("HERDR_CONFIG_PATH")
            .map(PathBuf::from)
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| {
                env::var_os("XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        PathBuf::from(env::var_os("HOME").unwrap_or_default()).join(".config")
                    })
                    .join("herdr")
            });
        Ok(Self {
            config: env::var_os("HERDR_PLUGIN_CONFIG_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| base.join("plugins/config/agent-progress")),
            state: env::var_os("HERDR_PLUGIN_STATE_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    env::var_os("XDG_STATE_HOME")
                        .map(PathBuf::from)
                        .unwrap_or_else(|| {
                            PathBuf::from(env::var_os("HOME").unwrap_or_default())
                                .join(".local/state")
                        })
                        .join("herdr/plugins/agent-progress")
                }),
        })
    }
    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.config)?;
        fs::create_dir_all(&self.state)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.config, fs::Permissions::from_mode(0o700))?;
            fs::set_permissions(&self.state, fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }
    pub fn db(&self) -> PathBuf {
        self.state.join("progress.sqlite3")
    }
    pub fn enabled(&self) -> bool {
        let read = |path: &Path| -> Option<Value> {
            serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
        };
        let Some(marker) = read(&self.config.join("enabled")) else {
            return false;
        };
        let Some(registry) = marker["registry"].as_str().and_then(|p| read(Path::new(p))) else {
            return false;
        };
        registry.as_array().is_some_and(|plugins| {
            plugins.iter().any(|p| {
                p["plugin_id"] == "agent-progress"
                    && p["enabled"] == true
                    && p["plugin_root"] == marker["root"]
                    && p["manifest_path"]
                        .as_str()
                        .is_some_and(|p| Path::new(p).is_file())
            })
        })
    }
}

pub fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ancestry_requires_the_actual_parent_chain() {
        let table = HashMap::from([
            (
                1,
                Process {
                    parent: 0,
                    started: "a".into(),
                },
            ),
            (
                2,
                Process {
                    parent: 1,
                    started: "b".into(),
                },
            ),
            (
                3,
                Process {
                    parent: 0,
                    started: "c".into(),
                },
            ),
        ]);
        assert!(is_ancestor(&table, 1, 2));
        assert!(!is_ancestor(&table, 3, 2));
    }

    #[cfg(unix)]
    #[test]
    fn replacement_socket_invalidates_the_endpoint_even_at_the_same_path() {
        use std::os::unix::net::UnixListener;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("herdr.sock");
        let first = UnixListener::bind(&path).unwrap();
        let old = Runtime::new(Some(path.to_string_lossy().into())).unwrap();
        fs::remove_file(&path).unwrap();
        let _second = UnixListener::bind(&path).unwrap();
        let new = Runtime::new(Some(path.to_string_lossy().into())).unwrap();
        assert_ne!(old.endpoint, new.endpoint);
        assert!(
            old.call(&["api", "snapshot"])
                .unwrap_err()
                .to_string()
                .contains("replaced")
        );
        drop(first);
    }
}
