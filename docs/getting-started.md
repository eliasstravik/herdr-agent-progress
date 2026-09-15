# Getting started: see your agents' progress

Install the plugin, connect your coding client, and see task estimates in Herdr's expanded agent sidebar.

## 1. Check the prerequisites

- macOS and [Herdr](https://herdr.dev) 0.9.0 or newer. Run these commands inside a running Herdr session.
- Rust/Cargo and a C compiler. Herdr builds the executable during installation. On macOS, `xcode-select --install` installs Apple's command-line build tools if missing. Install Rust using [rustup](https://rustup.rs).
- Git and access to `eliasstravik/herdr-agent-progress`. The repository is currently private. Authenticate Git for GitHub before installing; with GitHub CLI, use `gh auth login` and `gh auth setup-git`. A repository-not-found error can mean your account lacks access.
- Claude Code or Codex on `PATH`. Live-tested versions are Claude Code 2.1.272 and Codex 0.154.0. See [Compatibility](../compatibility.md) for exact evidence; earlier versions and other platforms are unverified.

No Python, Node, hosted service, or separate API key is required by the installed plugin. Your coding client has its own prerequisites and account.

## 2. Install the plugin

```bash
herdr plugin install eliasstravik/herdr-agent-progress
```

Review the install preview. Herdr clones the repository, runs its locked Cargo release build, and registers `agent-progress`. First startup prints the Configure command; it waits for you to choose setup before changing client files.

If you already linked a developer checkout, follow [switch from a linked checkout](#switch-from-a-linked-checkout) first. Herdr refuses to overwrite a local link with a GitHub package.

## 3. Connect your clients

Install the official Herdr integration for each client you use:

```bash
herdr integration install claude
herdr integration install codex
```

Then configure progress:

```bash
herdr plugin action invoke configure --plugin agent-progress
```

Configure detects installed Claude Code and Codex executables, adds their progress hooks, adds the sidebar row, reloads Herdr's configuration, and starts the publisher. It keeps unrelated hooks, comments, and sidebar settings.

Review native hook trust prompts. Setup does not grant reporting permission or bypass a sandbox. Progress commands may need the client's normal approval for the local Herdr socket, process identity, and plugin state.

## 4. Give an agent a task

Restart or resume your Claude Code or Codex session after configuration. Give it a task with a few steps, then expand the agent list in Herdr's sidebar.

As the agent reports, you should see rows like:

```text
~15% · Reading code
~65% · Testing changes
100% · Done
```

These are examples, not fixed stages. The agent may revise its estimate or report an activity without a percentage. After five minutes without a report, an unfinished estimate is marked `stale`. Native working, blocked, and idle states stay separate.

A following task should start a new estimate. A clarification or continuation should keep the current task. Compaction and a verified native resume preserve its progress.

## Check your setup

```bash
herdr plugin list
herdr plugin action invoke doctor --plugin agent-progress
herdr plugin log --plugin agent-progress
```

Doctor reports whether the configuration marker and enabled registration match, Herdr's version, and the config/state paths. `configured: true` does not prove that a client has loaded its hooks or reported a task. Confirm that with a real agent task in the sidebar.

## Optional configuration

To select one client or use an alternate client home:

```bash
PROGRESS="$(herdr plugin config-dir agent-progress)/herdr-progress"
"$PROGRESS" configure --clients codex --codex-home /absolute/path/to/codex-home
```

The stable launcher exists after the first Configure. For initial setup with alternate homes, set `CODEX_HOME` or `CLAUDE_CONFIG_DIR` in the environment that starts Herdr, or run the installed executable's Configure command directly with the flags below. Plugin actions inherit the server's environment, not arbitrary variables from the shell invoking the action.

| Option or environment | Purpose |
|---|---|
| `--clients claude,codex` | Explicit clients; default is detected installed clients |
| `--claude-home`, `CLAUDE_CONFIG_DIR` | Claude settings directory |
| `--codex-home`, `CODEX_HOME` | Codex hooks directory |
| `--herdr-config`, `HERDR_CONFIG_PATH` | Herdr configuration file |
| `XDG_CONFIG_HOME`, `XDG_STATE_HOME` | Configuration and state base directories |
| `HERDR_PLUGIN_CONFIG_DIR`, `HERDR_PLUGIN_STATE_DIR` | Plugin directories, supplied by Herdr actions |

Configure copies the executable into the plugin config directory and writes a stable launcher. Hooks do not depend on your shell's `PATH` or an old checkout location. No extra configuration is needed for the default sidebar row. See [Operations](operations.md) for custom tokens, command syntax, identity checks, and development.

## Upgrade

Stop the publisher in **each running Herdr session** before replacing the package:

```bash
herdr plugin action invoke stop --plugin agent-progress
herdr plugin install eliasstravik/herdr-agent-progress
herdr plugin action invoke configure --plugin agent-progress
```

Restart or resume your agents afterward. Stop clears the display and revokes launch bindings. SessionStart restores the matching native session's task with a fresh binding. Configure updates the copied executable, but cannot replace a publisher that you left running.

For a package installed before the Stop action was added, use its stable launcher:

```bash
"$(herdr plugin config-dir agent-progress)/herdr-progress" stop
```

Use `herdr plugin action invoke start --plugin agent-progress` to start a configured publisher again. A valid reporting command or hook also starts it if needed.

### Switch from a linked checkout

Stop the publisher in each session, then replace the registration and configure again:

```bash
"$(herdr plugin config-dir agent-progress)/herdr-progress" stop
herdr plugin unlink agent-progress
herdr plugin install eliasstravik/herdr-agent-progress
herdr plugin action invoke configure --plugin agent-progress
```

Do not unconfigure for this migration. The existing ownership journal lets Configure preserve your settings. Resume or restart agents when finished.

## Remove

```bash
herdr plugin action invoke unconfigure --plugin agent-progress
herdr plugin uninstall agent-progress
```

For a linked checkout, use `herdr plugin unlink agent-progress` instead of uninstall. Unconfigure removes matching owned hooks and rows, preserves user edits, and revokes bindings. The stable launcher and local state remain for diagnostics. Removing only the package leaves inert configuration entries.

## Troubleshooting

- **Install says repository not found:** Confirm your GitHub account has access and Git can authenticate. This repository is not yet a public distribution.
- **Cargo or a linker is missing:** Install Rust and Apple's command-line tools, then rerun the install.
- **Startup asks you to configure:** Run step 3. This is expected on a fresh install.
- **No progress row:** Check that the plugin is enabled, run Configure, install the official client integration, restart or resume the client, and expand the agent sidebar. Start a real task and allow its normal reporting permission if prompted.
- **Doctor says configured but no estimates appear:** Doctor checks plugin registration, not client trust or live reporting. Inspect `publisher.log` in the state directory it prints and the client's hook/permission messages.
- **The estimate is stale or moves backwards:** Both are expected. Stale means no report for five minutes; a lower estimate means the agent discovered more work.
- **Setup refuses a file or sidebar row:** Inspect the reported file. Configure refuses symlinks, malformed files, edited conflicting progress rows, and full 16-row layouts. It does not silently overwrite them.
- **A reporting command rejects a binding:** Resume or restart that client. Do not copy a binding from another agent or override the endpoint on a reporting command.
- **An upgrade still runs old code:** Stop publishers in every session before reinstalling, configure again, then resume clients.

For more help, [open an issue](https://github.com/eliasstravik/herdr-agent-progress/issues) with client/Herdr versions and a redacted error message.
