<p align="center"><img src="https://img.shields.io/badge/Agent%20Progress-Task%20progress%20for%20Herdr-2ea44f?style=flat-square&labelColor=24292f" alt="Agent Progress | Task progress for Herdr" /></p>

<h3 align="center">Stay on top of your coding agents without opening every terminal or asking how far they've got</h3>

<p align="center">Agent Progress lets you check on your <a href="https://herdr.dev">Herdr</a> coding agents when several tasks are running and you don't know which one needs your attention, by putting each agent's estimated progress and current activity beside its session so you can see who's getting started, who's testing, and whose work is ready for a closer look.</p>

<p align="center"><img src="assets/agent-progress-sidebar.svg" width="88%" alt="Illustration: a coding task leads to an agent's progress report, then an estimate and current activity in the Herdr sidebar" /></p>

<p align="center"><a href="https://github.com/eliasstravik/herdr-agent-progress/blob/main/docs/getting-started.md"><img src="assets/buttons/see-your-agents-progress.svg" alt="See your agents' progress" /></a></p>

<p align="center"><sub>✓&nbsp;Free,&nbsp;MIT&nbsp;licensed &nbsp; ✓&nbsp;No&nbsp;extra&nbsp;API&nbsp;key &nbsp; ✓&nbsp;Claude&nbsp;Code&nbsp;and&nbsp;Codex&nbsp;on&nbsp;macOS</sub></p>

<br />

## Keep track of the work while your agents get on with it

An agent that's reading code, running tests, or wrapping up shows you where it stands beside its session. You open the terminal that needs a closer look instead of working through every conversation to find out how far things have got.

## Choose between watching status dots, opening terminals, asking for updates, or checking the sidebar

| | **Agent Progress** | Native status dots | Opening terminals | Asking for updates |
|---|:---:|:---:|:---:|:---:|
| **No extra software fee** | ✅ | ✅ | ✅ | ✅ |
| **Visible beside the session** | ✅ | ✅ | ❌ | ❌ |
| **No extra API key** | ✅ | ✅ | ✅ | ✅ |
| **Read a reported task estimate** | ✅ | ❌ | ✅ | ✅ |
| **Current task activity** | ✅ | ❌ | ✅ | ✅ |
| **No repeated questions** | ✅ | ✅ | ✅ | ❌ |
| **Old estimates marked stale** | ✅ | ❌ | ❌ | ❌ |
| **No switching between panes** | ✅ | ✅ | ❌ | ❌ |
| **Full conversation and tool output** | ❌ | ❌ | ✅ | ❌ |

Keep your attention on the work that needs it. Agent Progress shows the estimate, your agents report as they work, and you choose which conversation to open next.

## Give your agents a task. See how far they've got in the sidebar.

### 📈 See the work at a glance

Each reporting Claude Code or Codex session shows an estimate and a short activity beside its name. Read `~65% · Testing changes` and know what the agent is doing before opening its terminal.

### ⚡ Stop asking for another progress update

Give an agent a task and let it report as it works. The sidebar keeps its latest estimate in view while you work elsewhere, and marks an unfinished report stale after five minutes without an update.

### 💬 Know when to take a closer look

An agent that reports 100% shows `Done`, so you can open the conversation and review its work. The next task starts a fresh estimate. You still decide whether the result is good enough.

## Set up your first progress update in three steps

<table>
<tr>
<td align="center" valign="top" width="33%"><h3>1️⃣</h3><b>Install the plugin</b><br /><sub>Run <code>herdr plugin install eliasstravik/herdr-agent-progress</code>. The plugin builds itself with Cargo. You'll need access to the private repository.</sub></td>
<td align="center" valign="top" width="33%"><h3>2️⃣</h3><b>Connect your coding clients</b><br /><sub>Install your client's Herdr integration, then run <code>herdr plugin action invoke configure --plugin agent-progress</code>. Review the normal trust prompts.</sub></td>
<td align="center" valign="top" width="33%"><h3>3️⃣</h3><b>Give an agent a task</b><br /><sub>Restart or resume your client and give it work. Expand the agent sidebar to see its estimate and current activity as it reports.</sub></td>
</tr>
</table>

## Get everything included, free

<table align="center">
<tr>
<td align="center" valign="top"><sub>For developers who run Claude Code or Codex in Herdr on macOS</sub><br /><h2>Free</h2><div align="left">&nbsp;&nbsp;&nbsp;✓&nbsp; Task estimates beside each reporting session<br />&nbsp;&nbsp;&nbsp;✓&nbsp; Current activity without opening the terminal<br />&nbsp;&nbsp;&nbsp;✓&nbsp; Stale markers when an update gets old<br />&nbsp;&nbsp;&nbsp;✓&nbsp; Done when an agent reports its task complete<br />&nbsp;&nbsp;&nbsp;✓&nbsp; Progress restored when a verified session resumes<br />&nbsp;&nbsp;&nbsp;✓&nbsp; Setup that keeps your existing hooks and settings</div></td>
</tr>
<tr>
<td align="center"><a href="https://github.com/eliasstravik/herdr-agent-progress/blob/main/docs/getting-started.md"><img src="assets/buttons/see-your-agents-progress.svg" alt="See your agents' progress" /></a></td>
</tr>
</table>

## Get your questions answered

### Do I need to know how to code?

No. The plugin builds itself on install, and setup connects your coding clients and adds the sidebar row. You'll need macOS, Herdr 0.9.0 or newer, Rust/Cargo, and repository access. The [getting-started guide](docs/getting-started.md) covers the prerequisites and supported clients.

### How do I check that Agent Progress is running?

Expand the agent sidebar after giving a configured client a task. If no estimate appears, run:

```bash
herdr plugin action invoke doctor --plugin agent-progress
```

The command checks the plugin's setup. The [getting-started guide](docs/getting-started.md#check-your-setup) walks through missing updates, restarting clients, and checking their hooks.

### What permissions does progress reporting need?

Access to the **local Herdr socket**, process information, and the plugin's state directory. Review your client's normal hook trust and sandbox prompts. Setup doesn't grant those permissions for you.

### Does the plugin send progress to a hosted service?

No. The plugin stores progress on your machine and sends the display values to your local Herdr session. Your coding client still uses its own service as usual. [Operations](docs/operations.md) covers how reporting works.

### Will setup change my existing hooks and settings?

It adds its own hooks and sidebar row while keeping unrelated settings. If setup finds a conflicting row or a file it can't safely edit, it stops and tells you why. [Removal](docs/getting-started.md#remove) takes out the plugin's entries and preserves your edits.

### Where does my progress live?

In a local database in the plugin's state directory. A verified session resume restores that session's task, so you can pick up where you left off. A new task starts its own estimate.

### Do I have to ask agents to report on every task?

No. Once configured, Claude Code and Codex load the reporting instructions when a session starts and receive reminders as they use tools. Restart or resume your client after setup. [Compatibility](compatibility.md) lists the tested versions.

### What if an estimate is wrong or stops updating?

The percentage is the agent's estimate, so it can go down when the agent finds more work. An unfinished report gets a `stale` label after five minutes without an update. Open the conversation to check; `Done` still needs your review.

### What does it cost?

Agent Progress is free and [MIT licensed](LICENSE). You need access to the private repository to install it. Your coding client's usual usage charges still apply, including its work to report progress.

## See your first progress update in three steps

<p align="center">Your first progress update starts with an install, client setup, and a task. Agent Progress keeps the updates in view. You choose which conversation needs a closer look.</p>

<p align="center"><a href="https://github.com/eliasstravik/herdr-agent-progress/blob/main/docs/getting-started.md"><img src="assets/buttons/see-your-agents-progress.svg" alt="See your agents' progress" /></a></p>

<p align="center"><sub>✓&nbsp;Free,&nbsp;MIT&nbsp;licensed &nbsp; ✓&nbsp;No&nbsp;extra&nbsp;API&nbsp;key &nbsp; ✓&nbsp;Claude&nbsp;Code&nbsp;and&nbsp;Codex&nbsp;on&nbsp;macOS</sub></p>
