use crate::{
    publisher,
    runtime::{Paths, Runtime, output, quote},
    state,
};
use anyhow::{Context, Result, ensure};
use jsonc_parser::cst::{CstInputValue, CstRootNode};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json as serde_json_value};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};
use toml_edit::{Array, DocumentMut, InlineTable, Item};

#[derive(clap::Args)]
pub struct Configure {
    /// Comma-separated clients to configure. Defaults to installed Claude/Codex.
    #[arg(long,value_delimiter=',',value_parser=["claude","codex"])]
    pub clients: Vec<String>,
    #[arg(long)]
    pub claude_home: Option<PathBuf>,
    #[arg(long)]
    pub codex_home: Option<PathBuf>,
    #[arg(long)]
    pub herdr_config: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
struct Owned {
    before: Option<String>,
    after: String,
    kind: String,
    command: Option<String>,
}

fn removal_baseline(previous: Owned, current: &Owned) -> Result<Option<String>> {
    if current.before.as_ref() == Some(&previous.after) {
        return Ok(previous.before);
    }
    // An upgrade must retain edits made since the previous Configure. Saving
    // only the first-install snapshot would erase those edits on Unconfigure.
    current
        .before
        .as_deref()
        .map(|text| {
            if current.kind == "sidebar" {
                sidebar(text, true)
            } else {
                hooks(
                    text,
                    current.command.as_deref().context("Missing owned hook")?,
                    true,
                )
            }
        })
        .transpose()
}

fn home() -> PathBuf {
    PathBuf::from(env::var_os("HOME").unwrap_or_default())
}
fn config_path() -> PathBuf {
    env::var_os("HERDR_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            env::var_os("XDG_CONFIG_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| home().join(".config"))
                .join("herdr/config.toml")
        })
}

fn read(path: &Path) -> Result<Option<String>> {
    // Refuse symlink files and parents, rather than silently editing another home.
    for p in path.ancestors() {
        if let Ok(m) = fs::symlink_metadata(p) {
            ensure!(
                !m.file_type().is_symlink(),
                "Refusing symlink configuration path {}",
                p.display()
            );
        }
    }
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn replace(path: &Path, before: &Option<String>, after: &str) -> Result<()> {
    ensure!(
        &read(path)? == before,
        "Configuration changed concurrently: {}",
        path.display()
    );
    fs::create_dir_all(path.parent().context("Config path has no parent")?)?;
    let tmp = path.with_file_name(format!(".agent-progress-{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&tmp, after)?;
    if path.exists() {
        fs::set_permissions(&tmp, fs::metadata(path)?.permissions())?;
    }
    if &read(path)? != before {
        fs::remove_file(&tmp)?;
        anyhow::bail!("Configuration changed concurrently: {}", path.display());
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

fn row() -> toml_edit::Value {
    // A single value lets Herdr truncate only the tail. Separate tokens receive
    // separate width budgets and can truncate "stale" before the activity.
    let mut a = Array::new();
    let mut style = InlineTable::new();
    style.insert("token", "$agent_progress_summary".into());
    style.insert("dim", true.into());
    a.push(style);
    a.into()
}

fn legacy_row() -> toml_edit::Value {
    let mut a = Array::new();
    a.push("$agent_progress_percent");
    let mut style = InlineTable::new();
    style.insert("token", "$agent_progress_freshness".into());
    style.insert("dim", true.into());
    a.push(style);
    a.push("$agent_progress_activity");
    a.into()
}

pub fn sidebar(input: &str, remove: bool) -> Result<String> {
    let mut doc = input.parse::<DocumentMut>()?;
    if remove
        && doc
            .get("ui")
            .and_then(|v| v.get("sidebar"))
            .and_then(|v| v.get("agents"))
            .is_none()
    {
        return Ok(input.into());
    }
    let agents = &mut doc["ui"]["sidebar"]["agents"];
    if agents.get("rows").is_none() && !remove {
        let mut defaults = Array::new();
        let mut first = Array::new();
        for t in ["state_icon", "machine", "workspace", "tab"] {
            first.push(t);
        }
        defaults.push(first);
        let mut second = Array::new();
        second.push("agent");
        defaults.push(second);
        agents["rows"] = toml_edit::value(defaults);
    }
    fn edit(item: &mut Item, remove: bool) -> Result<()> {
        let rows = item.as_array_mut().context("Sidebar rows must be arrays")?;
        let expected = row();
        let legacy = legacy_row();
        let is_legacy = |v: &toml_edit::Value| {
            v.to_string().split_whitespace().collect::<String>()
                == legacy.to_string().split_whitespace().collect::<String>()
        };
        rows.retain(|v| !is_legacy(v));
        let same = |v: &toml_edit::Value| {
            v.to_string().split_whitespace().collect::<String>()
                == expected.to_string().split_whitespace().collect::<String>()
        };
        if remove {
            rows.retain(|v| !same(v));
        } else if !rows.iter().any(same) {
            ensure!(
                !rows
                    .iter()
                    .any(|v| v.to_string().contains("$agent_progress_")),
                "An edited progress row already exists; restore or remove it before configuring"
            );
            ensure!(
                rows.len() < 16,
                "Sidebar already uses all 16 rows; remove one row before configuring"
            );
            rows.push(expected);
        }
        Ok(())
    }
    if let Some(rows) = agents.get_mut("rows").filter(|v| !v.is_none()) {
        edit(rows, remove)?;
    }
    if let Some(overrides) = agents.get_mut("rows_by_agent").filter(|v| !v.is_none()) {
        for (_, rows) in overrides
            .as_table_like_mut()
            .context("rows_by_agent must be a table")?
            .iter_mut()
        {
            edit(rows, remove)?;
        }
    }
    Ok(doc.to_string())
}

fn hook_entry(command: &str) -> Value {
    serde_json_value!({"matcher":"*","hooks":[{"type":"command","command":command,"timeout":10}]})
}
pub fn hooks(input: &str, command: &str, remove: bool) -> Result<String> {
    let root = CstRootNode::parse(input, &Default::default())?;
    let obj = root
        .object_value()
        .context("Hook configuration must be an object")?;
    let hooks = match obj.get("hooks") {
        Some(p) => p.object_value().context("hooks must be an object")?,
        None if remove => return Ok(input.into()),
        None => obj
            .append("hooks", CstInputValue::Object(vec![]))
            .object_value()
            .unwrap(),
    };
    let expected = hook_entry(command);
    for event in ["SessionStart", "PostToolUse", "UserPromptSubmit"] {
        let entries = match hooks.get(event) {
            Some(p) => p.array_value().context("Hook event must be an array")?,
            None if remove => continue,
            None => hooks
                .append(event, CstInputValue::Array(vec![]))
                .array_value()
                .unwrap(),
        };
        let mut found = false;
        for entry in entries.elements() {
            if entry.to_serde_value().as_ref() == Some(&expected) {
                if remove {
                    entry.remove();
                } else {
                    found = true;
                }
            }
        }
        if !remove && !found {
            entries.append(CstInputValue::Object(vec![
                ("matcher".into(), "*".into()),
                (
                    "hooks".into(),
                    CstInputValue::Array(vec![CstInputValue::Object(vec![
                        ("type".into(), "command".into()),
                        ("command".into(), command.into()),
                        ("timeout".into(), 10u64.into()),
                    ])]),
                ),
            ]));
        }
    }
    Ok(root.to_string())
}

pub fn configure(options: &Configure, rt: &Runtime, paths: &Paths) -> Result<()> {
    let registry = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(".config"))
        .join("herdr/plugins.json");
    let plugins: Value = serde_json::from_str(
        &fs::read_to_string(&registry)
            .context("Install or link the Herdr plugin before Configure")?,
    )?;
    let plugin = plugins
        .as_array()
        .and_then(|p| {
            p.iter()
                .find(|p| p["plugin_id"] == "agent-progress" && p["enabled"] == true)
        })
        .context("Enable the agent-progress Herdr plugin before Configure")?;
    let mut c = state::open(&paths.db())?;
    let tx = state::transaction(&mut c)?;
    let mut edits: Vec<(PathBuf, Owned)> = vec![];
    let config = options.herdr_config.clone().unwrap_or_else(config_path);
    let before = read(&config)?;
    let after = sidebar(before.as_deref().unwrap_or(""), false)?;
    edits.push((
        config.clone(),
        Owned {
            before,
            after,
            kind: "sidebar".into(),
            command: None,
        },
    ));
    let selected: Vec<_> = if options.clients.is_empty() {
        ["claude", "codex"]
            .into_iter()
            .filter(|client| Command::new(client).arg("--version").output().is_ok())
            .map(str::to_owned)
            .collect()
    } else {
        options.clients.clone()
    };
    for client in selected {
        let dir = if client == "claude" {
            options
                .claude_home
                .clone()
                .or_else(|| env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from))
                .unwrap_or_else(|| home().join(".claude"))
        } else {
            options
                .codex_home
                .clone()
                .or_else(|| env::var_os("CODEX_HOME").map(PathBuf::from))
                .unwrap_or_else(|| home().join(".codex"))
        };
        let file = dir.join(if client == "claude" {
            "settings.json"
        } else {
            "hooks.json"
        });
        let command = format!(
            "{} hook --agent {client}",
            quote(&paths.config.join("herdr-progress").to_string_lossy())
        );
        let before = read(&file)?;
        let after = hooks(before.as_deref().unwrap_or("{}"), &command, false)?;
        edits.push((
            file,
            Owned {
                before,
                after,
                kind: "hooks".into(),
                command: Some(command),
            },
        ));
    }
    // Parse and check the complete candidate before touching any live config.
    let candidate = paths
        .config
        .join(format!("check-{}.toml", uuid::Uuid::new_v4()));
    fs::write(&candidate, &edits[0].1.after)?;
    let checked = output(
        Command::new(&rt.bin)
            .env("HERDR_CONFIG_PATH", &candidate)
            .args(["config", "check"]),
    );
    fs::remove_file(&candidate)?;
    checked
        .context("Herdr rejected the proposed sidebar config; no live configuration was changed")?;
    for (path, edit) in &edits {
        let previous: Option<String> = tx
            .query_row(
                "SELECT body FROM ownership WHERE path=?",
                [path.to_string_lossy().as_ref()],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(previous) = previous {
            let old: Owned = serde_json::from_str(&previous)?;
            ensure!(
                old.kind == edit.kind && old.command == edit.command,
                "Setup ownership differs at {}",
                path.display()
            );
        } else {
            ensure!(
                !edit.before.as_ref().is_some_and(|text| {
                    if edit.kind == "sidebar" {
                        text.contains("$agent_progress_")
                    } else {
                        edit.command
                            .as_ref()
                            .is_some_and(|command| text.contains(command))
                    }
                }),
                "Existing progress entries have no ownership record at {}; inspect them before configuring",
                path.display()
            );
        }
    }
    // Install a stable copied binary and launcher, independent of checkout paths.
    let binary = paths.config.join("herdr-progress-bin");
    let staged = paths.config.join("herdr-progress-bin.new");
    fs::copy(env::current_exe()?, &staged)?;
    fs::rename(staged, &binary)?;
    let launcher = format!(
        "#!/bin/sh\nexport HERDR_PLUGIN_CONFIG_DIR={}\nexport HERDR_PLUGIN_STATE_DIR={}\nexec {} \"$@\"\n",
        quote(&paths.config.to_string_lossy()),
        quote(&paths.state.to_string_lossy()),
        quote(&binary.to_string_lossy())
    );
    let launcher_path = paths.config.join("herdr-progress");
    fs::write(&launcher_path, launcher)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&launcher_path, fs::Permissions::from_mode(0o755))?;
    }
    // Persist the recovery journal before editing user files. A killed installer
    // must not leave hooks that Unconfigure cannot identify as its own.
    for (path, edit) in &edits {
        let mut owned = Owned {
            before: edit.before.clone(),
            after: edit.after.clone(),
            kind: edit.kind.clone(),
            command: edit.command.clone(),
        };
        if let Some(old) = tx
            .query_row(
                "SELECT body FROM ownership WHERE path=?",
                [path.to_string_lossy().as_ref()],
                |r| r.get::<_, String>(0),
            )
            .optional()?
        {
            owned.before = removal_baseline(serde_json::from_str(&old)?, &owned)?;
        }
        tx.execute(
            "INSERT INTO ownership VALUES (?,?) ON CONFLICT(path) DO UPDATE SET body=excluded.body",
            params![path.to_string_lossy(), serde_json::to_string(&owned)?],
        )?;
    }
    tx.commit()?;
    for (index, (path, edit)) in edits.iter().enumerate() {
        if let Err(e) = replace(path, &edit.before, &edit.after) {
            for (path, edit) in edits[..index].iter().rev() {
                if read(path)?.as_ref() == Some(&edit.after) {
                    match &edit.before {
                        Some(text) => replace(path, &Some(edit.after.clone()), text)?,
                        None => fs::remove_file(path)?,
                    }
                }
            }
            return Err(e);
        }
    }
    fs::write(
        paths.config.join("enabled"),
        serde_json::to_string(
            &serde_json_value!({"registry":registry,"root":plugin["plugin_root"]}),
        )?,
    )?;
    output(
        Command::new(&rt.bin)
            .env("HERDR_SOCKET_PATH", &rt.socket)
            .args(["server", "reload-config"]),
    )?;
    publisher::start(rt, paths)?;
    println!(
        "Configured. Restart/resume the selected clients and review their native hook trust prompts. Existing hook permissions are unchanged."
    );
    Ok(())
}

pub fn unconfigure(rt: &Runtime, paths: &Paths) -> Result<()> {
    if paths.config.join("enabled").exists() {
        fs::remove_file(paths.config.join("enabled"))?;
    }
    publisher::stop(rt, paths)?;
    let mut c = state::open(&paths.db())?;
    publisher::publish(rt, &mut c, true)?;
    let tx = state::transaction(&mut c)?;
    let rows: Vec<(String, String)> = tx
        .prepare("SELECT path,body FROM ownership")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<std::result::Result<_, _>>()?;
    for (path, body) in rows {
        let path = PathBuf::from(path);
        let owned: Owned = serde_json::from_str(&body)?;
        let current = read(&path)?;
        if current.as_ref() == Some(&owned.after) {
            match owned.before {
                Some(before) => replace(&path, &current, &before)?,
                None => fs::remove_file(&path)?,
            }
        } else if let Some(current) = current {
            let revised = if owned.kind == "sidebar" {
                sidebar(&current, true)?
            } else {
                hooks(&current, owned.command.as_deref().unwrap(), true)?
            };
            replace(&path, &Some(current), &revised)?;
        }
        tx.execute(
            "DELETE FROM ownership WHERE path=?",
            [path.to_string_lossy().as_ref()],
        )?;
    }
    tx.commit()?;
    output(
        Command::new(&rt.bin)
            .env("HERDR_SOCKET_PATH", &rt.socket)
            .args(["server", "reload-config"]),
    )?;
    println!(
        "Removed matching progress hooks and rows. User edits were preserved. The package can now be uninstalled."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upgrade_removal_keeps_settings_added_after_first_install() {
        let command = "'/stable/herdr-progress' hook --agent codex";
        let original =
            "{\"hooks\":{\"SessionStart\":[{\"hooks\":[{\"command\":\"native-hook\"}]}]}}";
        let configured = hooks(original, command, false).unwrap();
        let edited = configured.replacen('{', "{\n// keep my comment\n\"user_setting\":true,", 1);
        let previous = Owned {
            before: Some(original.into()),
            after: configured,
            kind: "hooks".into(),
            command: Some(command.into()),
        };
        let current = Owned {
            before: Some(edited.clone()),
            after: hooks(&edited, command, false).unwrap(),
            kind: "hooks".into(),
            command: Some(command.into()),
        };
        let restored = removal_baseline(previous, &current).unwrap().unwrap();
        assert!(restored.contains("user_setting"));
        assert!(restored.contains("keep my comment"));
        assert!(restored.contains("native-hook"));
        assert!(!restored.contains("herdr-progress"));

        let configured = sidebar("[ui]\nsidebar_width = 26\n", false).unwrap();
        let edited = configured.replace("26", "32") + "\n# keep this too\n";
        let previous = Owned {
            before: Some("[ui]\nsidebar_width = 26\n".into()),
            after: configured,
            kind: "sidebar".into(),
            command: None,
        };
        let current = Owned {
            before: Some(edited.clone()),
            after: sidebar(&edited, false).unwrap(),
            kind: "sidebar".into(),
            command: None,
        };
        let restored = removal_baseline(previous, &current).unwrap().unwrap();
        assert!(restored.contains("sidebar_width = 32"));
        assert!(restored.contains("keep this too"));
        assert!(!restored.contains("$agent_progress_"));
    }

    #[test]
    fn unchanged_upgrade_keeps_original_snapshot_but_deleted_files_stay_deleted() {
        for before in [None, Some("original".to_owned())] {
            let previous = Owned {
                before: before.clone(),
                after: "configured".into(),
                kind: "hooks".into(),
                command: None,
            };
            let current = Owned {
                before: Some("configured".into()),
                after: "configured".into(),
                kind: "hooks".into(),
                command: None,
            };
            assert_eq!(removal_baseline(previous, &current).unwrap(), before);
        }
        let previous = Owned {
            before: Some("original".into()),
            after: "configured".into(),
            kind: "hooks".into(),
            command: None,
        };
        let current = Owned {
            before: None,
            after: "configured".into(),
            kind: "hooks".into(),
            command: None,
        };
        assert_eq!(removal_baseline(previous, &current).unwrap(), None);
    }

    #[test]
    fn existing_hooks_comments_and_user_edits_survive() {
        let original = "{\n// user's comment\n\"theme\": \"dark\",\"hooks\":{\"SessionStart\":[{\"hooks\":[{\"command\":\"keep\"}]}]}}";
        let added = hooks(
            original,
            "'/path with space/herdr-progress' hook --agent codex",
            false,
        )
        .unwrap();
        assert!(added.contains("// user's comment"));
        assert!(added.contains("keep"));
        assert_eq!(
            hooks(
                &added,
                "'/path with space/herdr-progress' hook --agent codex",
                false
            )
            .unwrap(),
            added
        );
        let removed = hooks(
            &added,
            "'/path with space/herdr-progress' hook --agent codex",
            true,
        )
        .unwrap();
        assert!(!removed.contains("herdr-progress"));
        assert!(removed.contains("keep"));
    }
    #[test]
    fn sidebar_defaults_overrides_and_budget() {
        assert!(
            sidebar("", false)
                .unwrap()
                .contains("$agent_progress_summary")
        );
        assert!(
            sidebar("[ui.sidebar.agents]\nrows=[[\"agent\"]]", false)
                .unwrap()
                .contains("$agent_progress_summary")
        );
        let original = "# preserved\n[ui.sidebar.agents]\nrows=[[\"agent\"]]\n[ui.sidebar.agents.rows_by_agent]\nclaude=[[\"state_icon\",\"agent\"]]\n";
        let added = sidebar(original, false).unwrap();
        assert!(added.contains("# preserved"));
        assert_eq!(added.matches("$agent_progress_summary").count(), 2);
        assert_eq!(sidebar(&added, false).unwrap(), added);
        assert!(!sidebar(&added, true).unwrap().contains("$agent_progress"));
        let full = format!(
            "[ui.sidebar.agents]\nrows=[{}]",
            vec!["[\"agent\"]"; 16].join(",")
        );
        assert!(sidebar(&full, false).is_err());
    }
    #[test]
    fn upgrade_replaces_legacy_row_without_using_an_extra_slot() {
        let mut rows = vec!["[\"agent\"]".to_string(); 15];
        rows.push(legacy_row().to_string());
        let old = format!("[ui.sidebar.agents]\nrows=[{}]", rows.join(","));
        let upgraded = sidebar(&old, false).unwrap();
        assert_eq!(upgraded.matches("$agent_progress_summary").count(), 1);
        assert!(!upgraded.contains("$agent_progress_percent"));
        assert_eq!(sidebar(&upgraded, false).unwrap(), upgraded);
        let removed = sidebar(&upgraded, true).unwrap();
        assert!(!removed.contains("$agent_progress_"));
        assert_eq!(removed.matches("\"agent\"").count(), 15);
    }
    #[test]
    fn detects_concurrent_edits() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("config");
        fs::write(&p, "new user edit").unwrap();
        assert!(replace(&p, &Some("old".into()), "replacement").is_err());
        assert_eq!(fs::read_to_string(p).unwrap(), "new user edit");
    }

    #[cfg(unix)]
    #[test]
    fn symlink_config_is_refused_without_touching_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.json");
        let link = dir.path().join("settings.json");
        fs::write(&target, "{\"user\":true}").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(read(&link).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "{\"user\":true}");
    }

    #[test]
    fn uninstall_keeps_user_modified_hook_and_other_rows() {
        let command = "'/plugin/herdr-progress' hook --agent codex";
        let added = hooks("{}", command, false).unwrap();
        let edited = added.replace("\"timeout\": 10", "\"timeout\": 20");
        assert!(
            hooks(&edited, command, true)
                .unwrap()
                .contains("herdr-progress")
        );
        let added = sidebar(
            "[ui.sidebar.agents]\nrows=[[\"agent\",\"state_text\"]]",
            false,
        )
        .unwrap();
        let edited = format!("{added}\n[ui.sound]\nenabled = false\n");
        let removed = sidebar(&edited, true).unwrap();
        assert!(removed.contains("state_text"));
        assert!(removed.contains("enabled = false"));
        assert!(!removed.contains("$agent_progress_"));
    }
}
