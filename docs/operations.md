# Operations and development

For the first install, follow [Getting started](getting-started.md). For supported clients and live-test boundaries, see [Compatibility](../compatibility.md).

## How it works

`herdr-progress` is one Rust executable with bundled instructions. Claude Code and Codex SessionStart hooks load those instructions automatically. The agent calls the CLI after milestones and before substantive replies. Tool-boundary hooks provide throttled reminders. No separate skill activation is required.

A plugin-owned process publishes namespaced metadata into Herdr's native sidebar. It checks for overdue reports every 15 seconds and notices queued reports within approximately 250 milliseconds. It does no work in Herdr's render loop.

Automatic instruction loading does not grant tool permissions. A sandbox may require normal approval for the CLI to access Herdr's local socket, inspect process identity and write plugin state. Denied reporting does not stop the agent's actual task. The installer does not change sandbox settings or bypass native hook trust.

## Development status

The implementation targets Herdr 0.9.0 on macOS. Live validation and the client support matrix are recorded in [compatibility.md](../compatibility.md). Other operating systems and agents are not advertised as automatically supported.

## Install from a checkout

Build prerequisites are Rust/Cargo and the platform C compiler. SQLite is compiled into the executable; installed runtime commands need no Python or Node runtime. The native adapters also require Herdr's official session integrations.

```sh
cargo build --release --locked
herdr plugin link "$PWD"
herdr integration install claude
herdr integration install codex
herdr plugin action invoke configure --plugin agent-progress
```

Review Herdr's and the clients' normal trust prompts. Resume or restart existing agent sessions after configuration. Configure discovers installed Claude Code and Codex executables. To select clients or alternate homes explicitly:

```sh
target/release/herdr-progress configure --clients codex --codex-home /absolute/path/to/codex-home
```

`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `HERDR_CONFIG_PATH`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `HERDR_PLUGIN_CONFIG_DIR` and `HERDR_PLUGIN_STATE_DIR` are respected. Herdr plugin actions supply the plugin directories. Configure writes a stable launcher and copied executable in the plugin config directory, so hooks do not depend on PATH changes or an old checkout path.

The installer appends its own hooks and sidebar row, including existing per-agent row overrides. It preserves unrelated configuration and comments. Symlink configuration paths, malformed files, conflicting edited progress rows and full 16-row layouts are refused before any live config edit. Concurrent edits are checked before replacement.

For an upgrade, follow [the upgrade steps](getting-started.md#upgrade). Stop the publisher in each running Herdr session before replacing the package, run Configure again, then resume or restart the agents. Stop clears the display and revokes launch bindings; a fresh SessionStart restores tasks for matching native sessions. Configure updates the copied executable, but does not replace an already-running publisher process.

## Commands

```text
herdr-progress --instructions                 # --skill is an alias; prints only
herdr-progress context --binding B
herdr-progress status --binding B
herdr-progress begin --binding B --expected-task none --title 'Task title'
herdr-progress report --binding B --task ID --percent 65 --activity 'Testing changes'
herdr-progress report --binding B --task ID --unknown --activity 'Assessing task'
herdr-progress clear --binding B --task ID
herdr-progress doctor
herdr-progress start
herdr-progress stop
herdr-progress unconfigure
```

Use the absolute launcher path printed by the hook. `begin` returns the opaque task ID as JSON. On continuation or clarification, keep that ID. To replace a task, pass the exact current ID as `--expected-task`, including after clearing it. There is no unconditional task replacement. A completed task must get a new generation before further work is reported.

For manual activation by a human, `herdr-progress activate --pane PANE_ID` prints bound instructions after checking a live Claude/Codex process and its native session. It does not type into the pane. Other clients remain unavailable until their runtime identity can be verified. Printing `--instructions` alone never creates a binding.

Administrative commands accept `--endpoint SOCKET`. Reporting commands require the invoking agent's inherited Herdr environment and reject endpoint overrides.

## Identity and publication

Every bound operation checks the endpoint's socket identity, stable terminal ID, official native session, foreground agent PID and OS process start time. Agent-facing calls must descend from that process. The binding is an opaque consistency token, not a security boundary against other processes under the same OS account.

Compaction within a launch reuses the binding. A verified resume in a new process rotates it and restores only the matching native session's task. Old bindings cannot read, replace, report or clear the successor's task. Pane moves resolve by terminal ID. Ambiguous identity is invalidated.

SQLite transactions serialize task generation changes and publication decisions. Publication sequence numbers are persisted before sends, so a timeout or crash cannot reuse a sequence. The publisher uses one stable metadata source, `agent-progress`, and explicitly clears its four tokens. It does not use TTL to implement stale reports.

The default dim row uses `$agent_progress_summary`, with percentage and freshness before the activity so shortening the tail preserves them. The separate `$agent_progress_percent`, `$agent_progress_freshness` and `$agent_progress_activity` tokens are also available for custom layouts. The summary is capped at 80 characters and display columns; the activity is capped at 40 columns.

State lives in the plugin state directory, including `progress.sqlite3` and `publisher.log`. Per-endpoint OS file locks prevent competing background publishers. A crash releases the lock; Start, a valid hook or a reporting command restarts the publisher. Endpoint loss uses bounded backoff. Disabling or removing the Herdr registration makes hooks inert and causes the publisher to clear its tokens and exit when the endpoint remains reachable.

## Remove

```sh
herdr plugin action invoke unconfigure --plugin agent-progress
herdr plugin unlink agent-progress
```

Use Herdr's `plugin uninstall` command for a GitHub-installed package. Unconfigure removes matching owned hooks and rows while preserving user edits. It retains the local state and stable launcher for diagnostics. Removing only the package leaves inert configuration entries; it is not full configuration cleanup.

## Verification

```sh
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
```

Tests cover task replacement races, tombstones after clear, unknown/decreasing estimates, stale/completed behavior, launch rotation, endpoint isolation, delayed publication after task replacement, failed-send sequence reservation, config preservation, row limits, helper filtering and caller ancestry. See the compatibility report for the separate live-test boundary.
