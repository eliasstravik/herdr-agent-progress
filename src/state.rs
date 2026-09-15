use anyhow::{Result, bail, ensure};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, time::Duration};
use unicode_width::UnicodeWidthChar;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Identity {
    pub endpoint: String,
    pub terminal: String,
    pub session: String,
    pub agent: String,
    pub pid: u32,
    pub started: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    pub percent: Option<u8>,
    pub activity: Option<String>,
    pub reported_at: Option<i64>,
    pub cleared: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Slot {
    pub identity: Identity,
    pub binding: String,
    pub task: Option<Task>,
    pub revoked: bool,
    pub reminded_at: i64,
    pub published: String,
}

pub fn open(path: &Path) -> Result<Connection> {
    let c = Connection::open(path)?;
    c.busy_timeout(Duration::from_secs(10))?;
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;
        CREATE TABLE IF NOT EXISTS slots (endpoint TEXT NOT NULL, terminal TEXT NOT NULL, body TEXT NOT NULL, PRIMARY KEY(endpoint,terminal));
        CREATE TABLE IF NOT EXISTS sequences (endpoint TEXT PRIMARY KEY, value INTEGER NOT NULL);
        CREATE TABLE IF NOT EXISTS ownership (path TEXT PRIMARY KEY, body TEXT NOT NULL);")?;
    Ok(c)
}

pub fn transaction(c: &mut Connection) -> Result<Transaction<'_>> {
    Ok(c.transaction_with_behavior(TransactionBehavior::Immediate)?)
}

pub fn slot(c: &Connection, endpoint: &str, terminal: &str) -> Result<Option<Slot>> {
    let s: Option<String> = c
        .query_row(
            "SELECT body FROM slots WHERE endpoint=? AND terminal=?",
            params![endpoint, terminal],
            |r| r.get(0),
        )
        .optional()?;
    s.map(|s| Ok(serde_json::from_str(&s)?)).transpose()
}

pub fn slots(c: &Connection, endpoint: &str) -> Result<Vec<Slot>> {
    let mut q = c.prepare("SELECT body FROM slots WHERE endpoint=?")?;
    q.query_map([endpoint], |r| r.get::<_, String>(0))?
        .map(|s| Ok(serde_json::from_str(&s?)?))
        .collect()
}

pub fn save(c: &Connection, s: &Slot) -> Result<()> {
    c.execute("INSERT INTO slots VALUES (?,?,?) ON CONFLICT(endpoint,terminal) DO UPDATE SET body=excluded.body",
        params![s.identity.endpoint, s.identity.terminal, serde_json::to_string(s)?])?;
    Ok(())
}

pub fn bootstrap(c: &Connection, identity: Identity) -> Result<Slot> {
    let old = slot(c, &identity.endpoint, &identity.terminal)?;
    if let Some(s) = &old
        && !s.revoked
        && s.identity == identity
    {
        return Ok(s.clone());
    }
    // Only a verified native session can carry a task across a launch generation.
    let task = old
        .filter(|s| s.identity.session == identity.session && s.identity.agent == identity.agent)
        .and_then(|s| s.task);
    let s = Slot {
        identity,
        binding: uuid::Uuid::new_v4().to_string(),
        task,
        revoked: false,
        reminded_at: 0,
        published: String::new(),
    };
    save(c, &s)?;
    Ok(s)
}

pub fn bound(c: &Connection, identity: &Identity, binding: &str) -> Result<Slot> {
    let s = slot(c, &identity.endpoint, &identity.terminal)?
        .ok_or_else(|| anyhow::anyhow!("No bound agent. Resume with the progress hook enabled."))?;
    ensure!(
        !s.revoked && s.binding == binding && &s.identity == identity,
        "Obsolete or mismatched agent binding"
    );
    Ok(s)
}

pub fn begin(s: &mut Slot, expected: &str, title: &str) -> Result<()> {
    ensure!(
        s.task.as_ref().map(|t| t.id.as_str()).unwrap_or("none") == expected,
        "Task conflict. Re-read context and reassess the request."
    );
    let title = clean(title, 80);
    ensure!(!title.is_empty(), "Task title is empty");
    s.task = Some(Task {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        percent: None,
        activity: None,
        reported_at: None,
        cleared: false,
    });
    Ok(())
}

fn task<'a>(s: &'a mut Slot, id: &str) -> Result<&'a mut Task> {
    match s.task.as_mut() {
        Some(t) if t.id == id && !t.cleared => Ok(t),
        _ => bail!("Obsolete or cleared task"),
    }
}

pub fn report(s: &mut Slot, id: &str, percent: Option<u8>, activity: &str, now: i64) -> Result<()> {
    ensure!(
        percent.is_none_or(|p| p <= 100),
        "Percent must be 0 through 100"
    );
    let activity = clean(activity, 40);
    ensure!(!activity.is_empty(), "Activity is empty");
    let t = task(s, id)?;
    ensure!(
        t.percent != Some(100),
        "Completed task: begin a new generation before further work"
    );
    t.percent = percent;
    t.activity = Some(if percent == Some(100) {
        "Done".into()
    } else {
        activity
    });
    t.reported_at = Some(now);
    Ok(())
}

pub fn clear(s: &mut Slot, id: &str) -> Result<()> {
    task(s, id)?.cleared = true;
    Ok(()) // Keep the generation as a compare-and-swap tombstone.
}

pub fn clean(input: &str, columns: usize) -> String {
    let mut width = 0;
    input
        .chars()
        .filter(|c| {
            !c.is_control() && !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(80)
        .take_while(|c| {
            width += c.width().unwrap_or(0);
            width <= columns
        })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn tokens(s: &Slot, now: i64) -> [Option<String>; 3] {
    let Some(t) = s
        .task
        .as_ref()
        .filter(|t| !t.cleared && !s.revoked && t.reported_at.is_some())
    else {
        return [None, None, None];
    };
    [
        t.percent.map(|p| {
            if p == 100 {
                "100%".into()
            } else {
                format!("~{p}%")
            }
        }),
        (t.percent != Some(100) && now.saturating_sub(t.reported_at.unwrap()) >= 300)
            .then(|| "stale".into()),
        t.activity.clone(),
    ]
}

pub fn next_sequence(c: &Connection, endpoint: &str) -> Result<i64> {
    Ok(c.query_row("INSERT INTO sequences VALUES (?,1) ON CONFLICT(endpoint) DO UPDATE SET value=value+1 RETURNING value", [endpoint], |r| r.get(0))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> Identity {
        Identity {
            endpoint: "e".into(),
            terminal: "t".into(),
            session: "s".into(),
            agent: "codex".into(),
            pid: 12,
            started: "launch1".into(),
        }
    }
    fn initial() -> Slot {
        bootstrap(&open(Path::new(":memory:")).unwrap(), identity()).unwrap()
    }
    #[test]
    fn generations_survive_clear_and_reject_delayed_begin() {
        let mut s = initial();
        begin(&mut s, "none", "First").unwrap();
        let first = s.task.as_ref().unwrap().id.clone();
        assert!(begin(&mut s, "none", "Delayed").is_err());
        clear(&mut s, &first).unwrap();
        assert!(begin(&mut s, "none", "Delayed").is_err());
        assert!(report(&mut s, &first, Some(20), "Old report", 0).is_err());
        begin(&mut s, &first, "Next").unwrap();
        let next = s.task.as_ref().unwrap().id.clone();
        assert_ne!(first, next);
        assert!(clear(&mut s, &first).is_err());
        assert!(begin(&mut s, &first, "Delayed").is_err());
        assert_eq!(next, s.task.unwrap().id);
    }
    #[test]
    fn unknown_decreasing_stale_and_finished() {
        let mut s = initial();
        begin(&mut s, "none", "Task").unwrap();
        let id = s.task.as_ref().unwrap().id.clone();
        report(&mut s, &id, None, "Assessing task", 100).unwrap();
        assert_eq!(
            tokens(&s, 400),
            [None, Some("stale".into()), Some("Assessing task".into())]
        );
        report(&mut s, &id, Some(65), "Testing changes", 401).unwrap();
        report(&mut s, &id, Some(40), "Found more work", 402).unwrap();
        assert_eq!(tokens(&s, 701)[1], None);
        assert_eq!(tokens(&s, 702)[1], Some("stale".into()));
        assert_eq!(s.task.as_ref().unwrap().reported_at, Some(402));
        report(&mut s, &id, Some(100), "All checks passed", 703).unwrap();
        assert_eq!(
            tokens(&s, 9999),
            [Some("100%".into()), None, Some("Done".into())]
        );
        assert!(report(&mut s, &id, Some(70), "More work", 704).is_err());
    }
    #[test]
    fn launch_rotation_compaction_and_endpoint_isolation() {
        let c = open(Path::new(":memory:")).unwrap();
        let mut s = bootstrap(&c, identity()).unwrap();
        begin(&mut s, "none", "Task").unwrap();
        save(&c, &s).unwrap();
        assert_eq!(bootstrap(&c, identity()).unwrap().binding, s.binding);
        let mut resumed = identity();
        resumed.started = "launch2".into();
        let new = bootstrap(&c, resumed.clone()).unwrap();
        assert_ne!(new.binding, s.binding);
        assert_eq!(new.task.as_ref().unwrap().id, s.task.unwrap().id);
        assert!(bound(&c, &resumed, &s.binding).is_err());
        assert!(bound(&c, &identity(), &s.binding).is_err());
        resumed.session = "different".into();
        assert!(bootstrap(&c, resumed.clone()).unwrap().task.is_none());
        resumed.endpoint = "other".into();
        assert!(bootstrap(&c, resumed).unwrap().task.is_none());
    }
    #[test]
    fn activity_has_display_width_cap_and_no_controls() {
        assert_eq!(clean("Hi\n\u{202e}世界", 6), "Hi世界");
        assert_eq!(clean("界界界", 5), "界界");
        assert!(
            clean(&format!("a{}", "\u{0301}".repeat(100)), 40)
                .chars()
                .count()
                <= 80
        );
    }

    #[test]
    fn concurrent_begins_have_exactly_one_winner() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.sqlite3");
        let c = open(&path).unwrap();
        bootstrap(&c, identity()).unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let workers: Vec<_> = (0..8)
            .map(|_| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    let mut c = open(&path).unwrap();
                    barrier.wait();
                    let tx = transaction(&mut c).unwrap();
                    let mut s = slot(&tx, "e", "t").unwrap().unwrap();
                    if begin(&mut s, "none", "Competing task").is_err() {
                        return false;
                    }
                    save(&tx, &s).unwrap();
                    tx.commit().unwrap();
                    true
                })
            })
            .collect();
        assert_eq!(
            workers
                .into_iter()
                .filter(|w| w.thread().id() != std::thread::current().id())
                .map(|w| usize::from(w.join().unwrap()))
                .sum::<usize>(),
            1
        );
    }
}
