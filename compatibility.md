# Compatibility and verification

This file distinguishes implemented adapters from live support. Documentation discovery does not establish automatic support.

## Priority clients

| Client | Native mechanism | Configuration | Restore and reminders | Runtime evidence | Validation status |
| --- | --- | --- | --- | --- | --- |
| Claude Code | SessionStart `additionalContext`; PostToolUse and UserPromptSubmit | `CLAUDE_CONFIG_DIR/settings.json`, otherwise `~/.claude/settings.json` | SessionStart restores after resume/compact; reminders are throttled to once a minute | Official `herdr:claude` session ID, foreground process PID/start time, verified hook ancestry; native `agent_id` helpers excluded | Adapter implemented; live sign-off pending. Inspected 2.1.270; smoke-test executable updated to 2.1.272. |
| Codex | SessionStart `additionalContext`; PostToolUse and UserPromptSubmit | `CODEX_HOME/hooks.json`, otherwise `~/.codex/hooks.json` | SessionStart restores after resume/compact; reminders are throttled to once a minute | Official `herdr:codex` session ID, foreground process PID/start time, verified hook ancestry; inherited thread mismatch and native helper indicators excluded | 0.154.0 automatically loaded the instructions. Default sandbox denied the Herdr socket; full reporting sign-off pending native permission approval. |

Sources: [Claude hooks](https://code.claude.com/docs/en/hooks), [Codex hooks](https://learn.chatgpt.com/docs/hooks). Inspected 2026-09-15. Native trust must be reviewed; setup does not enable bypass flags. The versions above are validation targets, not an assertion that older clients cannot work. No older-version support floor has been established by testing.

## Other Herdr agents

Each row is `unverified/unavailable` for progress reporting in this build. `--instructions` provides the common reporting convention, but cannot establish caller identity. These clients have no supported progress version, verified compaction restoration or verified reminders yet. Configure leaves their files untouched.

The [current Herdr agent list](https://herdr.dev/docs/agents/) defines the coverage inventory. Herdr's own session integration does not by itself prove this plugin can inject instructions or identify the hook's launching process.

| Agent / upstream | Candidate mechanism or known gap | Config location or override evidence | Local validation boundary |
| --- | --- | --- | --- |
| Pi, badlogic/pi-mono | Native extension events are the appropriate candidate | Herdr's Pi extension integration; progress path not implemented | Executable absent |
| OMP, can1357/oh-my-pi | Extensions and context files | [OMP settings](https://github.com/can1357/oh-my-pi/blob/main/docs/settings.md); relocatable agent config | Executable absent |
| GitHub Copilot CLI, GitHub | User instructions; native session hooks need progress-context verification | User-level `copilot-instructions.md`; exact override not validated | Executable absent |
| Devin CLI, Cognition | Native SessionStart integration exists in Herdr; context output not verified | Herdr integration documentation; progress path not selected | Executable absent |
| Kimi Code CLI, Moonshot AI | Native plugin `sessionStart.skill` is a candidate | [Kimi plugins](https://www.kimi.com/code/docs/en/kimi-code-cli/customization/plugins); plugin enablement must be verified | Executable absent |
| Hermes Agent, Nous Research | Extension mechanism needs selection; personality files are unsuitable | Herdr session integration; no progress config selected | Executable absent |
| Qoder CLI, Qoder | SessionStart context capability needs verification | Herdr native integration; override not validated | Executable absent |
| Qwen Code, QwenLM | SessionStart hooks are a candidate | Herdr native integration; override not validated | Executable absent |
| Droid, Factory | AGENTS.md can supply instructions, but not a binding | Native config and runtime identity adapter not validated | Executable absent |
| OpenCode, Anomaly | Configured instruction files and native plugins | [`~/.config/opencode/opencode.json` instructions](https://opencode.ai/docs/rules/) | Executable installed; client/server launch identity and progress adapter not verified |
| Kilo Code CLI, Kilo | Native lifecycle plugin is a candidate | Herdr lifecycle integration; progress path not selected | Executable absent |
| MastraCode, Mastra | Native hooks; context-output contract needs verification | `~/.mastracode/hooks.json` per Herdr integration docs | Executable absent |
| Cursor Agent CLI, Cursor | Hooks need session/context verification | Native `hooks.json`; config override not validated | `agent` executable installed; progress binding not verified |
| Amp, Sourcegraph | Global instructions alone cannot bootstrap a binding | Global AGENTS.md mechanism; path/override not validated here | Executable absent; Herdr lists no session integration |
| Grok CLI | Exact upstream identity must be confirmed before choosing a context adapter | Herdr session integration exists; progress config not selected | Executable absent |
| Antigravity CLI | Exact native customization contract needs verification | Herdr has a session integration for `agy`; progress config not selected | Executable absent |
| Kiro CLI, AWS | Instruction loading and independent launch evidence need verification | No progress config selected | Executable absent; Herdr lists no session integration |
| Maki, maki.sh | Global instructions plus a separately verified startup adapter | [`~/.config/maki/AGENTS.md`](https://maki.sh/docs/context/) | Executable absent; Herdr lists no session integration |
| Muse | Upstream unresolved; executable name alone is insufficient evidence | No config selected | Executable absent; Herdr lists no session integration |
| Gemini CLI, Google | Global GEMINI.md instructions; runtime adapter still needed | `~/.gemini/GEMINI.md`; overrides not validated | Executable installed; Herdr marks Gemini less thoroughly tested |
| Cline, Cline | Native CLI loading and launch identity need verification | No progress config selected | Executable absent; Herdr marks Cline less thoroughly tested |

## Platform and test boundary

- macOS is the package target. The runtime verifies Unix socket identity and uses `ps` PID, parent PID and launch time. Herdr 0.9.0 exposes the required terminal/session/process and sequenced metadata capabilities.
- Linux has not been live tested and is not listed in the plugin manifest.
- Windows reporting is unavailable because this implementation does not yet verify named-pipe server identity and caller launch identity. Merely forwarding transport to Herdr would not establish those facts.
- A missing, mismatched or ambiguous identity is refused. There is no fallback to the focused pane or current occupant for an unbound agent command.
- Multi-pane, narrow-sidebar, stale, resume and removal evidence must be completed before automatic-support sign-off. Unit tests do not substitute for those live checks.
