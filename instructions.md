# Agent progress

When a verified binding is supplied below, report the progress of the user's whole current task through `herdr-progress`. This is your estimate, not a timer or a count of tools. Reporting failures must never stop the actual work. Give one short diagnostic and continue, without retry loops.

Reporting needs access to the local Herdr socket, OS process identity and the plugin's state directory. When the client's sandbox requires approval for that access, use its normal tool permission request for this reporting command. Never disable the sandbox or bypass hook trust. If permission is denied or requests are unavailable, continue the actual task without progress reporting.

Only the top-level agent that owns this pane reports. Same-pane helpers and subagents must not use or share its binding. Outside Herdr, do nothing.

At genuinely new work, read bound context, then call `begin` with the exact observed task ID, or `none` when no task exists. Do this bookkeeping before task tools or a blocking clarification question, so a prior 100% does not describe new work. Report an initial estimate or unknown activity. This comparison protects against delayed task starts. If it conflicts, re-read context and reconsider which request owns the current task. Never blindly replace the expected ID and retry.

After clarification, continuation or compaction, reuse the existing task ID. Do not start a new task merely because another reply began. If more work is requested after completion, deliberately begin a new generation. Ordinary questions about finished results do not require a task.

Report a rough percentage in five-point increments and a two-to-four-word activity, such as `Reading code`, `Testing changes`, or `Waiting for you`. Revise the estimate downward when you discover more work. Use `--unknown` while the scope is unclear. Activity can change while the percentage stays the same.

Report after meaningful milestones, changes of activity, blockers, and before a substantive reply. During active work, aim for one check-in per minute at a natural tool boundary. Long tools may take longer. Never invent progress to satisfy a timer. Await each reporting command; do not launch competing background reports.

Use 100 only when the entire requested outcome and relevant checks are finished. A tool finishing, a clarification question, or Herdr saying idle/done does not establish completion. Reported 100 displays `Done` and stays complete. Clear only an explicitly cancelled or unwanted task. Stale markers describe the age of your actual last report; rereading context never refreshes it.

The bound command examples below include the executable path, caller binding and current generation. Replace only task-specific arguments. Keep the binding for this live launch. Never discover a binding from the focused pane or borrow another agent's binding.
