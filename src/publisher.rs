use crate::{
    runtime::{Paths, Runtime, now},
    state,
};
use anyhow::{Result, ensure};
use fs2::FileExt;
use std::{
    fs::{self, File, OpenOptions},
    hash::{DefaultHasher, Hash, Hasher},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn name(rt: &Runtime) -> String {
    let mut h = DefaultHasher::new();
    rt.endpoint.hash(&mut h);
    format!("publisher-{:x}", h.finish())
}
fn lock(rt: &Runtime, paths: &Paths) -> Result<File> {
    Ok(OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(paths.state.join(format!("{}.lock", name(rt))))?)
}
pub fn start(rt: &Runtime, paths: &Paths) -> Result<()> {
    ensure!(paths.enabled(), "Plugin is not configured");
    let stop = paths.state.join(format!("{}.stop", name(rt)));
    if stop.exists() {
        fs::remove_file(stop)?;
    }
    let l = lock(rt, paths)?;
    if l.try_lock_exclusive().is_err() {
        return Ok(());
    }
    FileExt::unlock(&l)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.state.join("publisher.log"))?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .args(["--endpoint", &rt.socket, "serve"])
        .env("HERDR_PLUGIN_CONFIG_DIR", &paths.config)
        .env("HERDR_PLUGIN_STATE_DIR", &paths.state)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log);
    // Hook/tool runners clean up their process group when the entrypoint exits.
    // The endpoint publisher must survive that entrypoint.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn()?;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if l.try_lock_exclusive().is_err() {
            return Ok(());
        }
        FileExt::unlock(&l)?;
        ensure!(
            child.try_wait()?.is_none(),
            "Publisher exited during startup; inspect publisher.log"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    anyhow::bail!("Publisher did not acquire its endpoint lock; inspect publisher.log")
}
pub fn stop(rt: &Runtime, paths: &Paths) -> Result<()> {
    fs::write(paths.state.join(format!("{}.stop", name(rt))), b"stop")?;
    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline {
        let l = lock(rt, paths)?;
        if l.try_lock_exclusive().is_ok() {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!("Publisher has not stopped; inspect publisher.log")
}
pub fn serve(rt: &Runtime, paths: &Paths) -> Result<()> {
    let l = lock(rt, paths)?;
    if l.try_lock_exclusive().is_err() {
        return Ok(());
    }
    let stop = paths.state.join(format!("{}.stop", name(rt)));
    let mut c = state::open(&paths.db())?;
    let mut last_scan = Instant::now() - Duration::from_secs(15);
    let mut version = -1i64;
    let mut failures = 0;
    loop {
        let ending = !paths.enabled() || stop.exists();
        let changed: i64 = c.query_row("PRAGMA data_version", [], |r| r.get(0))?;
        if ending || changed != version || last_scan.elapsed() >= Duration::from_secs(15) {
            match publish(rt, &mut c, ending) {
                Ok(()) => {
                    failures = 0;
                    version = changed;
                    last_scan = Instant::now();
                }
                Err(e) => {
                    eprintln!("Progress publication: {e:#}");
                    failures += 1;
                    if failures >= 4 {
                        return Err(e);
                    }
                    std::thread::sleep(Duration::from_secs(1 << failures));
                }
            }
        }
        if ending {
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    Ok(())
}

pub fn publish(rt: &Runtime, c: &mut rusqlite::Connection, clear: bool) -> Result<()> {
    let snapshot = rt.call(&["api", "snapshot"])?;
    let panes = snapshot["snapshot"]["panes"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Snapshot has no panes"))?;
    // Mutations, stale decisions and sends share this transaction. A queued old
    // timer never carries cached tokens across the serialization boundary.
    let terminals: Vec<_> = state::slots(c, &rt.endpoint)?
        .into_iter()
        .map(|s| s.identity.terminal)
        .collect();
    for terminal in terminals {
        // Reserve durably before any external effect. Even a timeout or crash
        // after Herdr accepted a send must never cause sequence reuse.
        let seq = state::next_sequence(c, &rt.endpoint)?.to_string();
        let tx = state::transaction(c)?;
        let Some(mut s) = state::slot(&tx, &rt.endpoint, &terminal)? else {
            continue;
        };
        let pane = panes
            .iter()
            .find(|p| p["terminal_id"] == s.identity.terminal);
        let Some(pane) = pane else {
            // Herdr never reuses terminal IDs. Closed panes cannot be resumed
            // through this binding, so their state need not be scanned forever.
            tx.execute(
                "DELETE FROM slots WHERE endpoint=? AND terminal=?",
                rusqlite::params![rt.endpoint, terminal],
            )?;
            tx.commit()?;
            continue;
        };
        if clear || !rt.live(&s.identity, pane) {
            s.revoked = true;
        }
        let tokens = state::tokens(&s, now());
        let summary = state::summary(&tokens);
        let rendered = serde_json::to_string(&(&tokens, &summary))?;
        if s.published == rendered {
            state::save(&tx, &s)?;
            tx.commit()?;
            continue;
        }
        let pane_id = pane["pane_id"].as_str().unwrap();
        let mut args = vec![
            "pane".to_string(),
            "report-metadata".into(),
            pane_id.into(),
            "--source".into(),
            "agent-progress".into(),
            "--seq".into(),
            seq,
        ];
        for (key, value) in [
            "agent_progress_percent",
            "agent_progress_freshness",
            "agent_progress_activity",
            "agent_progress_summary",
        ]
        .iter()
        .zip(tokens.into_iter().chain([summary]))
        {
            match value {
                Some(v) => {
                    args.push("--token".into());
                    args.push(format!("{key}={v}"));
                }
                None => {
                    args.push("--clear-token".into());
                    args.push(key.to_string());
                }
            }
        }
        rt.call(&args.iter().map(String::as_str).collect::<Vec<_>>())?;
        s.published = rendered;
        state::save(&tx, &s)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::runtime::quote;
    use serde_json::json;
    use std::os::unix::{fs::PermissionsExt, net::UnixListener};

    #[test]
    fn publisher_revalidates_tasks_and_reserves_sequences_before_failed_sends() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("herdr.sock");
        let _listener = UnixListener::bind(&socket).unwrap();
        let fixture = dir.path().join("herdr-fixture");
        let pane = json!({"agent":"codex","agent_session":{"agent":"codex","source":"herdr:codex","kind":"id","value":"session"},"terminal_id":"terminal","pane_id":"w1:p1"});
        let info = json!({"process_info":{"foreground_processes":[{"name":"codex","pid":std::process::id()}]}});
        let log = dir.path().join("calls");
        let fail = dir.path().join("fail");
        fs::write(&fixture,format!("#!/bin/sh\ncase \"$1 $2\" in\n'api snapshot') printf '%s' {};;\n'pane get') printf '%s' {};;\n'pane process-info') printf '%s' {};;\n'pane report-metadata') printf '%s\\n' \"$*\" >> {}; if test -f {}; then exit 1; fi;;\nesac\n",
            quote(&json!({"result":{"snapshot":{"panes":[pane.clone()]}}}).to_string()),
            quote(&json!({"result":{"pane":pane}}).to_string()),
            quote(&json!({"result":info}).to_string()),quote(&log.to_string_lossy()),quote(&fail.to_string_lossy()))).unwrap();
        fs::set_permissions(&fixture, fs::Permissions::from_mode(0o755)).unwrap();
        let mut rt = Runtime::new(Some(socket.to_string_lossy().into())).unwrap();
        rt.bin = fixture.to_string_lossy().into();
        let identity = rt.identity(&pane, false).unwrap();
        let mut c = state::open(&dir.path().join("state.sqlite3")).unwrap();
        let mut s = state::bootstrap(&c, identity).unwrap();
        state::begin(&mut s, "none", "First").unwrap();
        let first = s.task.as_ref().unwrap().id.clone();
        state::report(&mut s, &first, Some(35), "Waiting for you", now() - 301).unwrap();
        state::save(&c, &s).unwrap();
        fs::write(&fail, "fail").unwrap();
        assert!(publish(&rt, &mut c, false).is_err());
        fs::remove_file(&fail).unwrap();
        state::begin(&mut s, &first, "Replacement").unwrap();
        let next = s.task.as_ref().unwrap().id.clone();
        state::report(&mut s, &next, Some(70), "Testing changes", now()).unwrap();
        state::save(&c, &s).unwrap();
        publish(&rt, &mut c, false).unwrap();
        let calls = fs::read_to_string(&log).unwrap();
        let lines: Vec<_> = calls.lines().collect();
        assert!(lines[0].contains("--seq 1"));
        assert!(lines[0].contains("agent_progress_freshness=stale"));
        assert!(lines[1].contains("--seq 2"));
        assert!(lines[1].contains("agent_progress_percent=~70%"));
        assert!(!lines[1].contains("=stale"));
        publish(&rt, &mut c, false).unwrap();
        assert_eq!(fs::read_to_string(&log).unwrap().lines().count(), 2);
        publish(&rt, &mut c, true).unwrap();
        let calls = fs::read_to_string(&log).unwrap();
        assert!(
            calls
                .lines()
                .last()
                .unwrap()
                .contains("--clear-token agent_progress_percent")
        );
    }
}
