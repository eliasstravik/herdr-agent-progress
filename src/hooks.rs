use crate::{
    publisher,
    runtime::{Paths, Runtime, now, quote, string},
    state,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    io::Read,
    time::{Duration, Instant},
};

pub const INSTRUCTIONS: &str = include_str!("../instructions.md");

pub fn context(s: &state::Slot, paths: &Paths) -> Result<String> {
    let exe = quote(&paths.config.join("herdr-progress").to_string_lossy());
    let binding = &s.binding;
    let task = s.task.as_ref().map(|t| t.id.as_str()).unwrap_or("none");
    Ok(format!(
        "{INSTRUCTIONS}\n\nVerified binding: {binding}\nCurrent state: {}\n\nCommands:\n{exe} context --binding {binding}\n{exe} begin --binding {binding} --expected-task {task} --title 'Task title'\n{exe} report --binding {binding} --task TASK_ID --percent 25 --activity 'Reading code'\n{exe} report --binding {binding} --task TASK_ID --unknown --activity 'Assessing task'\n{exe} status --binding {binding}\n{exe} clear --binding {binding} --task TASK_ID\n",
        serde_json::to_string(&s.task)?
    ))
}

pub fn run(agent: &str, paths: &Paths) -> Result<()> {
    if std::env::var("HERDR_ENV").as_deref() != Ok("1") || !paths.enabled() {
        return Ok(());
    }
    let mut input = String::new();
    std::io::stdin()
        .take(1_048_576)
        .read_to_string(&mut input)?;
    let event: Value = serde_json::from_str(&input)?;
    if !eligible(&event) {
        return Ok(());
    }
    let kind = string(&event, "hook_event_name")?;
    let native = string(&event, "session_id")?;
    if agent == "codex"
        && std::env::var("CODEX_THREAD_ID").is_ok_and(|id| !id.is_empty() && id != native)
    {
        return Ok(());
    }
    let rt = Runtime::new(None)?;
    let mut c = state::open(&paths.db())?;
    let deadline = Instant::now() + Duration::from_secs(2);
    let identity = loop {
        let result = rt
            .current()
            .and_then(|p| rt.identity(&p, true))
            .and_then(|id| {
                ensure!(
                    id.agent == agent && id.session == native,
                    "Hook session does not match Herdr's official session report"
                );
                Ok(id)
            });
        match result {
            Ok(id) => break id,
            Err(e) if kind != "SessionStart" || Instant::now() >= deadline => return Err(e),
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    let tx = state::transaction(&mut c)?;
    // Recheck after acquiring the store lock, including process ancestry.
    ensure!(
        rt.identity(&rt.current()?, true)? == identity,
        "Agent launch changed while hook waited"
    );
    let mut slot = if kind == "SessionStart" {
        state::bootstrap(&tx, identity.clone())?
    } else {
        let s = state::slot(&tx, &identity.endpoint, &identity.terminal)?
            .context("SessionStart has not established a progress binding")?;
        ensure!(
            !s.revoked && s.identity == identity,
            "Hook belongs to an obsolete launch"
        );
        s
    };
    let due = now() - slot.reminded_at >= 60
        && slot.task.as_ref().is_none_or(|t| {
            t.percent != Some(100) && !t.cleared && t.reported_at.is_none_or(|at| now() - at >= 60)
        });
    let text = if kind == "SessionStart" {
        Some(context(&slot, paths)?)
    } else if kind == "UserPromptSubmit" {
        Some(format!(
            "Before task tools or a blocking clarification question, check whether this request starts genuinely new work. If so, begin a new generation using the observed current ID; a previous 100% must not describe new work. For continuation, reuse the existing generation. Then report an estimate or --unknown activity before waiting for the user, for example 'Waiting for you'. Current task: {}. Bound context: {} context --binding {}",
            serde_json::to_string(&slot.task)?,
            quote(&paths.config.join("herdr-progress").to_string_lossy()),
            slot.binding
        ))
    } else if due {
        Some(format!(
            "Progress check-in is due if this is a natural boundary. Reassess the current task; do not invent progress. {} context --binding {}",
            quote(&paths.config.join("herdr-progress").to_string_lossy()),
            slot.binding
        ))
    } else {
        None
    };
    if text.is_some() {
        slot.reminded_at = now();
        state::save(&tx, &slot)?;
    }
    tx.commit()?;
    publisher::start(&rt, paths)?;
    if let Some(text) = text {
        println!(
            "{}",
            json!({"hookSpecificOutput":{"hookEventName":kind,"additionalContext":text}})
        );
    }
    Ok(())
}

fn eligible(event: &Value) -> bool {
    if ["agent_id", "subagent_id", "agent_transcript_path"]
        .iter()
        .any(|key| event.get(key).is_some_and(|v| !v.is_null()))
    {
        return false;
    }
    if event["transcript_path"]
        .as_str()
        .is_some_and(|p| p.contains("/subagents/"))
    {
        return false;
    }
    let name = event["hook_event_name"].as_str().unwrap_or("");
    if !matches!(name, "SessionStart" | "PostToolUse" | "UserPromptSubmit") {
        return false;
    }
    if name == "PostToolUse" && event["tool_input"].to_string().contains("herdr-progress") {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn helpers_and_reporter_calls_do_not_trigger_reminders() {
        assert!(!eligible(
            &json!({"hook_event_name":"PostToolUse","agent_id":"child"})
        ));
        assert!(!eligible(
            &json!({"hook_event_name":"PostToolUse","tool_input":{"cmd":"/path/herdr-progress report"}})
        ));
        assert!(eligible(
            &json!({"hook_event_name":"SessionStart","source":"compact"})
        ));
    }
}
