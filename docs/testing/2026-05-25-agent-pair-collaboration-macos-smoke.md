# Agent Pair Collaboration macOS Smoke

Temporary validation plan for the Agent Pair collaboration queue.

## Environment

- macOS desktop session.
- Build with `cargo build`.
- Launch `target/debug/horizon` with an isolated `HOME`.
- Use test panels backed by visible terminal input. A `/bin/cat` command panel is enough when proving prompt dispatch.

## Steps

1. Open `Agent Pair` from the root toolbar.
2. Close and reopen `Agent Pair` from the command palette.
3. Enter a long shared goal describing a feature-planning workflow.
4. Select Claude Code as Researcher and Codex as Performer where both presets exist.
5. Click `Start Pair`.
6. Confirm two terminal-backed panels appear and the Agent Pair header links them as Researcher and Performer.
7. Confirm both panels visibly receive their startup briefs.
8. Queue a performer work request with a long title, long context, long acceptance criteria, and long command/file paths.
9. Confirm dispatch is enabled only when a performer panel is linked.
10. Dispatch the work request and confirm the performer terminal receives the generated prompt.
11. Fill a performer report with summary, validation commands, validation result, and follow-up.
12. Mark the work request done.
13. Write a plan in `Plan Handoff`.
14. Launch a new Codex or Claude session with `Open With Plan`.
15. Confirm the new terminal receives the goal, plan, queue state, and performer report.
16. Close and relaunch Horizon with the same isolated `HOME`.
17. Confirm goal, plan, linked panel local ids, work status, and report persist.
18. Resize narrow and wide. Confirm goal text, role chips, long work titles, paths, report text, and plan handoff controls wrap without overlap.

## Evidence

- Capture screenshots with `screencapture` after pair startup, after dispatch, after plan handoff launch, and after relaunch.
- If prompt dispatch or resize behavior looks motion-sensitive, capture a short video with `screencapture -V`.
