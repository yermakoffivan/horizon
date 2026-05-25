# macOS/OSX Agent Squad Smoke Plan

This temporary smoke-test plan covers every implementation slice for issue
203, from worktree primitives through cleanup. Run the section for each slice
as that slice lands. Before marking the PR ready after slice 5 or slice 6, rerun
the full plan on macOS with the final branch or commit.

Keep this file in the PR while the epic is under active validation. Remove it
after the final UI validation pass unless the reviewer asks to keep it.

## Environment

- macOS with Xcode Command Line Tools installed.
- Rust stable 1.88 or newer.
- The branch or commit under test checked out locally.
- A disposable Git repository with at least one committed file.
- An isolated Horizon runtime home:

```bash
export HORIZON_SMOKE_HOME="$(mktemp -d)"
export HORIZON_HOME="$HORIZON_SMOKE_HOME/.horizon"
mkdir -p "$HORIZON_HOME"
```

Prepare the disposable repository:

```bash
export SQUAD_REPO="$HORIZON_SMOKE_HOME/repo"
mkdir -p "$SQUAD_REPO"
cd "$SQUAD_REPO"
git init
git config user.name "Horizon Smoke"
git config user.email "horizon-smoke@example.invalid"
printf 'one\n' > smoke.txt
git add smoke.txt
git commit -m "seed smoke repository"
cd -
```

Use debug builds for UI smoke work:

```bash
cargo build
HORIZON_HOME="$HORIZON_HOME" target/debug/horizon
```

Capture screenshots with:

```bash
screencapture -x "$HORIZON_SMOKE_HOME/<name>.png"
```

## Shared Validation Gate

Run this gate after each slice before committing or pushing that slice:

```bash
cargo fmt --all -- --check
./scripts/check-maintainability.sh
cargo test --workspace
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --workspace --lib --bins --examples -- -D warnings -D clippy::unwrap_used -D clippy::expect_used
cargo clippy --workspace --all-targets --all-features -- -D warnings -W clippy::pedantic
```

For UI slices, also launch the debug app and capture screenshots after launch,
after opening the Squad surface, after resizing narrower, and after resizing
back wider.

## Slice 1: Worktree Primitives

Scope:

- `crates/horizon-core/src/worktree.rs`
- `WorktreeManager::create`, `remove`, `diff`, and `apply_to`
- `tempfile` Git repository tests

Validation:

```bash
cargo test -p horizon-core worktree
```

Manual smoke:

1. In the disposable repository, create three slot worktrees under
   `$HORIZON_HOME/squad-tmp/<run-id>/s1`, `s2`, and `s3`.
2. Edit `smoke.txt` in `s1` and collect the slot diff.
3. Create a primary review worktree under
   `$HORIZON_HOME/squad-tmp/<run-id>/_review`.
4. Apply the `s1` diff into `_review`.
5. Remove `s1` and confirm the worktree registration and directory are gone.

Expected:

- The source repository remains clean.
- Applying a slot diff changes `_review`, not the performer worktree.
- Empty diffs are accepted as no-ops.
- Invalid diffs fail without modifying `_review`.

Evidence:

- Focused test output.
- `git worktree list` before and after removal.
- `git -C "$SQUAD_REPO" status --short`.

## Slice 2: Data Model And Persistence

Scope:

- `crates/horizon-core/src/squad.rs`
- `AgentSquad`, `SquadRun`, `PerformerSlot`, `WorkItem`,
  `PerformerReport`
- Per-session `~/.horizon/sessions/<sid>/squad.json`
- Atomic save on state transitions

Validation:

```bash
cargo test -p horizon-core squad
cargo test -p horizon-core session_store::tests::agent_squad
```

Manual smoke:

1. Create or update one `AgentSquad` run through `SessionStore`.
2. Confirm `squad.json` exists under the active session directory.
3. Exercise at least one transition through `SessionStore::update_agent_squad`.
4. Reload the same session and confirm the transition persists.

Expected `squad.json` fields:

- `version`
- `runs`
- `goal`, `status`, `created_at_millis`
- researcher and reviewer `AgentPanelLink` objects
- performer `work_item`, `assigned_kind`, `panel_local_id`, `scratch`, and
  `report`
- `primary_worktree`
- `plan_text`

Evidence:

- Focused test output.
- Redacted `cat "$HORIZON_HOME/sessions/<sid>/squad.json"`.
- Before/after status values from the transition.

## Slice 3: UI Shell, Composer, And Dashboard

Scope:

- `crates/horizon-ui/src/app/squad/{mod,render,state,composer,dashboard}.rs`
- Toolbar `Squad` entry
- Command palette `CommandId::OpenSquad`
- Dashboard and composer backed by the persisted model
- Start Run stub that creates persisted run state without spawning panels

Validation:

```bash
cargo test -p horizon-ui command_registry
cargo test -p horizon-ui squad
cargo test -p horizon-ui root_chrome
```

Manual smoke:

1. Launch the debug app with `HORIZON_HOME` set to the isolated runtime.
2. Open Squad from the toolbar.
3. Verify the dashboard opens above the canvas and does not get hidden by the
   empty-canvas card, HUD, minimap, or toolbar.
4. Click `New run` and verify the composer shows:
   - goal editor
   - researcher selector
   - reviewer selector
   - performer kind selector
   - performer count selector
   - worktree isolation label
   - advanced toggles
5. Enter a goal, keep three performers, and click `Start Run`.
6. Verify the dashboard shows a new run row and no performer panels are spawned
   yet.
7. Reopen Squad from the command palette by searching for `squad` and verify it
   focuses the same Squad surface rather than creating duplicates.
8. Restart Horizon with the same `HORIZON_HOME` and confirm the run row is
   restored from `squad.json`.

Resize checks:

- 1280x800: composer content remains readable and controls do not overlap.
- 1728x1117: dashboard remains compact and does not stretch into unreadable
  spacing.
- Narrow window: Squad remains usable or scrollable; no text overlaps.

Evidence:

- Screenshot after launch.
- Screenshot of empty dashboard.
- Screenshot of composer at the narrowest tested size.
- Screenshot of dashboard after the Start Run stub.
- Redacted `squad.json` showing the stubbed run.

## Slice 4: Spawn And Fanout

Scope:

- Start Run creates N performer worktrees and one primary review worktree.
- Performer panels spawn with `cwd` set to each slot worktree.
- Per-slot brief prompts are written to each performer panel.
- Scenario A run lane renders live slot chips.
- Manual `Mark Done` and `Block` transitions are available.

Manual smoke:

1. Launch Horizon with the disposable repository as the active workspace or
   panel cwd.
2. Open Squad composer from the toolbar.
3. Enter a goal that can be split into three independent file edits.
4. Set performers to three and keep worktree isolation.
5. Start the run.
6. Verify directories:
   - `$HORIZON_HOME/squad-tmp/<run-id>/s1`
   - `$HORIZON_HOME/squad-tmp/<run-id>/s2`
   - `$HORIZON_HOME/squad-tmp/<run-id>/s3`
   - `$HORIZON_HOME/squad-tmp/<run-id>/_review`
7. In each performer panel, run `pwd`.
8. Confirm each `pwd` matches the slot worktree path.
9. Confirm each performer panel received the generated brief.
10. Use `Focus` on each slot and verify focus returns to the correct panel.
11. Mark one slot Done and one slot Blocked.
12. Confirm the run lane and `squad.json` reflect both transitions.

Expected:

- Performer worktrees never share the same directory.
- Start Run fails visibly if there is no active source checkout for the current
  workspace.
- The primary checkout remains clean unless the user edits it directly.
- Slot chips show queued, working, done, and blocked states correctly.
- Manual status changes persist immediately.

Evidence:

- Screenshot of run lane with three performers.
- `find "$HORIZON_HOME/squad-tmp/<run-id>" -maxdepth 2 -type d | sort`.
- `pwd` output from each performer panel.
- Redacted `squad.json` after status changes.

## Slice 5: Reviewer Auto-Spawn And Slot Detail

Scope:

- Reviewer auto-spawns after all slots are Done.
- Reviewer panel cwd is the primary review worktree.
- Reviewer receives consolidated context: original goal, plan text, every slot
  report, and every slot diff.
- Blocked-slot prompt appears instead of blindly spawning reviewer.
- Scenario C slot drill-down renders slot worktree, brief, report, diff, and
  reviewer notes.

Manual smoke, all-done path:

1. Start a three-performer run from the disposable repository.
2. Make a distinct edit in each performer worktree.
3. Open each slot detail, use Refresh Diff, then mark each performer slot Done
   with a short report and validation command.
4. Confirm the reviewer panel auto-spawns only after the last Done transition.
5. In the reviewer panel, run `pwd` and confirm it is `_review`.
6. Confirm reviewer context includes:
   - the original goal
   - the researcher plan
   - all performer reports
   - all slot diffs
7. Apply or inspect the slot diffs in `_review`.

Manual smoke, blocked path:

1. Start another run with at least two performers.
2. Mark one slot Blocked and all remaining slots Done.
3. Confirm Horizon surfaces the blocked decision banner in the run lane.
4. Confirm the reviewer is not auto-spawned before the user chooses a path.
5. Choose `Review Done Slots`.
6. Confirm the reviewer starts from `_review` and the skipped blocked slot is
   represented in reviewer context.

Slot detail smoke:

1. Open a slot from the run lane.
2. Verify the detail surface shows status, panel link, scratch path, work item,
   performer report, diff, and reviewer notes.
3. Edit the report fields and verify `Mark Done` persists the report to
   `squad.json`.
4. Use the slot detail `Focus` or equivalent action and verify it selects the
   correct performer panel.

Evidence:

- Screenshot of reviewer waiting state.
- Screenshot after reviewer auto-spawn.
- Screenshot of blocked-slot decision prompt.
- Screenshot of slot detail.
- Redacted reviewer prompt/context.
- `git -C "$HORIZON_HOME/squad-tmp/<run-id>/_review" diff --stat`.

## Slice 6: Polish, Cleanup, And Regression Pass

Scope:

- Run deletion removes only that run's scratch worktrees.
- Existing runs restore without duplicate performer or reviewer panels.
- Optional done-hint detection remains advisory, not an automatic status change.
- Final maintainability and pedantic clippy gates are green.

Manual smoke:

1. Create two Squad runs in the same Horizon session.
2. Delete one run.
3. Confirm only that run's scratch directory is removed.
4. Confirm the other run's worktrees and `squad.json` entry remain.
5. Restart Horizon with the same `HORIZON_HOME`.
6. Confirm existing runs restore once and do not duplicate panels.
7. If done-hint detection is present, print the sentinel text in a performer
   panel and confirm Horizon shows a hint without changing the slot to Done
   until the user confirms.
8. Resize the app, detach and reattach a workspace if relevant, and reopen
   Squad. Confirm overlays stay layered above the canvas and below modal
   dialogs.

Expected:

- Deleting a run cannot remove the source checkout or another run's worktree.
- Restart restore is idempotent.
- Cleanup errors are visible and leave state recoverable.
- The dashboard remains accurate after restart, delete, and restore.

Evidence:

- Before/after `find "$HORIZON_HOME/squad-tmp" -maxdepth 3 -type d | sort`.
- Redacted `squad.json` before and after deletion.
- Screenshot after restart restore.
- Final validation gate output.

## Full End-To-End Pass

Run this after slice 5 and again after slice 6:

1. Open Squad composer from toolbar and command palette.
2. Set a goal and three performers.
3. Start the run.
4. Verify three performer worktrees and one primary worktree exist.
5. Verify each performer panel starts in the correct cwd and receives a brief.
6. Complete all performers with reports and validation results.
7. Verify reviewer auto-spawns in `_review` with consolidated context.
8. Restart Horizon and confirm the run, slot statuses, worktree paths, reports,
   and reviewer link persist.
9. Create a second run with one blocked slot and confirm the blocked decision
   prompt appears.
10. Delete one run and confirm only that run's scratch data is removed.

## Regression Checklist

- Slot worktree creation never writes into the source checkout.
- Applying a slot diff to `_review` never mutates performer worktrees.
- Empty diffs are accepted without changing `_review`.
- Invalid diffs produce a visible failure and leave `_review` unchanged.
- Restarting Horizon does not duplicate existing performer or reviewer panels.
- Deleting a run removes only that run's scratch worktrees.
- Dashboard state always matches `squad.json`.
- Toolbar and command palette open the same Squad surface.
- Squad overlays do not sit behind foreground canvas helpers.
- Text and controls do not overlap at 1280x800 or 1728x1117.

## PR Evidence Checklist

- Exact commit SHA tested.
- Terminal output for the shared validation gate.
- Focused test output for touched slice modules.
- Screenshots for dashboard, composer, run lane, slot detail, and reviewer
  states as the corresponding slices land.
- Directory listing for `$HORIZON_HOME/squad-tmp`.
- Redacted `squad.json`.
- Any known smoke limitations, including skipped macOS checks and why.
