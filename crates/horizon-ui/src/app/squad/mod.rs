mod composer;
mod dashboard;
mod lane;
mod render;
mod review;
mod slot_detail;
mod state;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use horizon_core::{
    AgentPanelLink, AgentSquad, PanelKind, PanelOptions, PanelResume, PerformerReport, PerformerSlot, WorkItem,
    WorkStatus, WorkspaceId, WorktreeManager,
};

use self::render::render_agent_squad;
use self::review::{
    apply_slot_diffs, blocked_slots, collect_review_contexts, ready_for_blocked_decision, ready_for_review,
    reviewer_prompt,
};
pub(super) use self::state::SquadPanelState;
use self::state::{SquadRunDraft, SquadSlotDetailState, SquadView};
use super::HorizonApp;

#[derive(Clone, Debug, PartialEq, Eq)]
enum SquadAction {
    None,
    Close,
    Dashboard,
    NewRun,
    OpenRun(String),
    OpenSlot {
        run_id: String,
        slot_id: String,
    },
    RefreshSlotDetail,
    FocusPanel(String),
    StartRun(SquadRunDraft),
    MarkSlotDone {
        run_id: String,
        slot_id: String,
    },
    MarkSlotDoneWithReport {
        run_id: String,
        slot_id: String,
        report: PerformerReport,
    },
    MarkSlotBlocked {
        run_id: String,
        slot_id: String,
        follow_up: String,
    },
    ReviewDoneSlots(String),
}

struct CreatedSquadRun {
    run_id: String,
    slots: Vec<CreatedPerformerSlot>,
}

struct CreatedPerformerSlot {
    slot_id: String,
    kind: PanelKind,
    scratch: PathBuf,
    prompt: String,
}

struct SpawnedSlot {
    slot_id: String,
    panel_local_id: String,
}

impl HorizonApp {
    pub(super) fn open_agent_squad_dashboard(&mut self) {
        if let Some(state) = &mut self.squad_panel {
            state.view = SquadView::Dashboard;
            state.slot_detail = None;
            state.error_message = None;
        } else {
            self.squad_panel = Some(SquadPanelState::dashboard());
        }
    }

    pub(super) fn toggle_agent_squad(&mut self) {
        self.squad_panel = if self.squad_panel.is_some() {
            None
        } else {
            Some(SquadPanelState::dashboard())
        };
    }

    pub(super) fn render_agent_squad(&mut self, ctx: &egui::Context) {
        let Some(state) = &mut self.squad_panel else {
            return;
        };

        let action = render_agent_squad(ctx, state, &self.agent_squad);
        self.apply_squad_action(ctx, action);
    }

    fn apply_squad_action(&mut self, ctx: &egui::Context, action: SquadAction) {
        match action {
            SquadAction::None => {}
            SquadAction::Close => self.squad_panel = None,
            SquadAction::Dashboard => {
                if let Some(state) = &mut self.squad_panel {
                    state.view = SquadView::Dashboard;
                    state.slot_detail = None;
                    state.error_message = None;
                }
            }
            SquadAction::NewRun => {
                self.squad_panel = Some(SquadPanelState::composer());
            }
            SquadAction::OpenRun(run_id) => {
                if let Some(state) = &mut self.squad_panel {
                    state.view = SquadView::RunLane { run_id };
                    state.slot_detail = None;
                    state.error_message = None;
                }
            }
            SquadAction::OpenSlot { run_id, slot_id } => self.open_squad_slot_detail(&run_id, &slot_id),
            SquadAction::RefreshSlotDetail => self.refresh_squad_slot_detail(),
            SquadAction::FocusPanel(panel_local_id) => {
                self.focus_squad_panel(ctx, &panel_local_id);
            }
            SquadAction::StartRun(draft) => match self.start_squad_run(ctx, &draft) {
                Ok(run_id) => {
                    if let Some(state) = &mut self.squad_panel {
                        state.view = SquadView::RunLane { run_id };
                        state.composer.goal.clear();
                        state.slot_detail = None;
                        state.error_message = None;
                    }
                }
                Err(error) => {
                    tracing::warn!("failed to start Agent Squad run: {error}");
                    if let Some(state) = &mut self.squad_panel {
                        state.error_message = Some(error.to_string());
                    }
                }
            },
            SquadAction::MarkSlotDone { run_id, slot_id } => self.mark_squad_slot_done(
                ctx,
                &run_id,
                &slot_id,
                PerformerReport {
                    summary: "Marked done manually from the Squad lane.".to_string(),
                    validation_result: "Manual status update.".to_string(),
                    ..PerformerReport::default()
                },
            ),
            SquadAction::MarkSlotDoneWithReport {
                run_id,
                slot_id,
                report,
            } => self.mark_squad_slot_done(ctx, &run_id, &slot_id, report),
            SquadAction::MarkSlotBlocked {
                run_id,
                slot_id,
                follow_up,
            } => self.mark_squad_slot_blocked(&run_id, &slot_id, follow_up),
            SquadAction::ReviewDoneSlots(run_id) => self.review_done_squad_slots(ctx, &run_id),
        }
    }

    fn start_squad_run(&mut self, ctx: &egui::Context, draft: &SquadRunDraft) -> horizon_core::Result<String> {
        let workspace_id = self.ensure_workspace_visible(ctx);
        let source_repo = self.squad_source_repo(workspace_id)?;
        let scratch_root = self.session_store.home().root().join("squad-tmp");
        let now = current_unix_millis();
        let mut created_paths = Vec::new();
        let mut created_run = None;

        let create_result = self.update_agent_squad(|squad| {
            let run = squad.create_run(draft.goal.clone(), now);
            run.start_decomposing(AgentPanelLink::new(draft.researcher_kind, None));
            run.reviewer = Some(AgentPanelLink::new(draft.reviewer_kind, None));
            let run_id = run.id.clone();
            let run_scratch_root = scratch_root.join(&run.id);
            let primary_worktree = WorktreeManager::create(&source_repo, "HEAD", &run_scratch_root, "_review")?;
            created_paths.push(primary_worktree.clone());
            let performers = create_performer_slots(draft, &source_repo, &run_scratch_root, &mut created_paths)?;
            let slots = performers
                .iter()
                .map(|slot| CreatedPerformerSlot {
                    slot_id: slot.id.clone(),
                    kind: slot.assigned_kind,
                    scratch: slot.scratch.clone(),
                    prompt: performer_prompt(&run_id, draft, slot),
                })
                .collect();
            run.set_primary_worktree(primary_worktree);
            run.queue_plan(plan_text(draft), performers);
            created_run = Some(CreatedSquadRun { run_id, slots });
            Ok(())
        });

        if let Err(error) = create_result {
            cleanup_created_worktrees(&created_paths);
            return Err(error);
        }

        let created_run =
            created_run.ok_or_else(|| horizon_core::Error::State("squad run was not created".to_string()))?;
        let spawned = match self.spawn_squad_performers(workspace_id, &created_run) {
            Ok(spawned) => spawned,
            Err(error) => {
                self.fail_squad_run(&created_run.run_id, error.to_string());
                return Err(error);
            }
        };
        if let Err(error) = self.dispatch_squad_slots(&created_run.run_id, &spawned) {
            self.fail_squad_run(&created_run.run_id, error.to_string());
            return Err(error);
        }
        self.mark_runtime_dirty();
        Ok(created_run.run_id)
    }

    fn spawn_squad_performers(
        &mut self,
        workspace_id: WorkspaceId,
        run: &CreatedSquadRun,
    ) -> horizon_core::Result<Vec<SpawnedSlot>> {
        let mut spawned = Vec::with_capacity(run.slots.len());
        for slot in &run.slots {
            let options = PanelOptions {
                name: Some(format!("Squad {} {}", short_run_id(&run.run_id), slot.slot_id)),
                cwd: Some(slot.scratch.clone()),
                kind: slot.kind,
                resume: PanelResume::Fresh,
                ..PanelOptions::default()
            };
            let panel_id = self.create_panel_with_options(options, workspace_id)?;
            let panel = self
                .board
                .panel_mut(panel_id)
                .ok_or_else(|| horizon_core::Error::State(format!("panel {} was not created", panel_id.0)))?;
            panel.write_input(slot.prompt.as_bytes());
            spawned.push(SpawnedSlot {
                slot_id: slot.slot_id.clone(),
                panel_local_id: panel.local_id.clone(),
            });
        }
        Ok(spawned)
    }

    fn dispatch_squad_slots(&mut self, run_id: &str, spawned: &[SpawnedSlot]) -> horizon_core::Result<()> {
        self.update_agent_squad(|squad| {
            let run = squad.run_mut(run_id)?;
            for slot in spawned {
                run.dispatch_slot(&slot.slot_id, slot.panel_local_id.clone())?;
            }
            Ok(())
        })
    }

    fn focus_squad_panel(&mut self, ctx: &egui::Context, panel_local_id: &str) {
        let Some(panel_id) = self.board.panel_id_by_local_id(panel_local_id) else {
            return;
        };
        self.board.focus(panel_id);
        if let Some(workspace_id) = self.board.panel_workspace_id(panel_id)
            && let Some((min, max)) = self.board.workspace_bounds(workspace_id)
        {
            self.focus_workspace_bounds(ctx, min, max, true);
        }
    }

    fn open_squad_slot_detail(&mut self, run_id: &str, slot_id: &str) {
        match self.build_slot_detail_state(run_id, slot_id) {
            Ok(detail) => {
                if let Some(state) = &mut self.squad_panel {
                    state.view = SquadView::SlotDetail;
                    state.slot_detail = Some(detail);
                    state.error_message = None;
                }
            }
            Err(error) => self.set_squad_error(error.to_string()),
        }
    }

    fn refresh_squad_slot_detail(&mut self) {
        let Some((run_id, slot_id)) = self.squad_panel.as_ref().and_then(|state| {
            state
                .slot_detail
                .as_ref()
                .map(|detail| (detail.run_id.clone(), detail.slot_id.clone()))
        }) else {
            return;
        };
        self.open_squad_slot_detail(&run_id, &slot_id);
    }

    fn build_slot_detail_state(&self, run_id: &str, slot_id: &str) -> horizon_core::Result<SquadSlotDetailState> {
        let run = self
            .agent_squad
            .runs
            .iter()
            .find(|run| run.id == run_id)
            .ok_or_else(|| horizon_core::Error::State(format!("squad run {run_id} was not found")))?;
        let slot = run
            .performers
            .iter()
            .find(|slot| slot.id == slot_id)
            .ok_or_else(|| horizon_core::Error::State(format!("performer slot {slot_id} was not found")))?;
        let (diff, diff_error) = match WorktreeManager::diff(&slot.scratch) {
            Ok(diff) => (diff, None),
            Err(error) => (String::new(), Some(error.to_string())),
        };
        Ok(SquadSlotDetailState::from_slot(
            run_id.to_string(),
            slot,
            diff,
            diff_error,
        ))
    }

    fn mark_squad_slot_done(&mut self, ctx: &egui::Context, run_id: &str, slot_id: &str, report: PerformerReport) {
        if let Err(error) = self.update_agent_squad(|squad| squad.run_mut(run_id)?.mark_slot_done(slot_id, report)) {
            tracing::warn!("failed to mark Squad slot done: {error}");
            self.set_squad_error(error.to_string());
            return;
        }

        if let Err(error) = self.maybe_start_squad_review(ctx, run_id) {
            tracing::warn!("failed to start Squad review: {error}");
            self.set_squad_error(error.to_string());
        }
    }

    fn mark_squad_slot_blocked(&mut self, run_id: &str, slot_id: &str, follow_up: String) {
        if let Err(error) =
            self.update_agent_squad(|squad| squad.run_mut(run_id)?.mark_slot_blocked(slot_id, follow_up))
        {
            tracing::warn!("failed to mark Squad slot blocked: {error}");
            self.set_squad_error(error.to_string());
            return;
        }

        if let Some(run) = self.agent_squad.runs.iter().find(|run| run.id == run_id)
            && ready_for_blocked_decision(run)
        {
            self.set_squad_error(blocked_review_message(run));
        }
    }

    fn review_done_squad_slots(&mut self, ctx: &egui::Context, run_id: &str) {
        let blocked_slot_ids = self
            .agent_squad
            .runs
            .iter()
            .find(|run| run.id == run_id)
            .map(|run| {
                blocked_slots(run)
                    .into_iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if blocked_slot_ids.is_empty() {
            return;
        }

        let result = self.update_agent_squad(|squad| {
            let run = squad.run_mut(run_id)?;
            for slot_id in &blocked_slot_ids {
                run.mark_slot_done(
                    slot_id,
                    PerformerReport {
                        summary: "Skipped blocked slot for reviewer pass.".to_string(),
                        validation_result: "Skipped by manual review decision.".to_string(),
                        follow_up: "Original slot was blocked; reviewer should inspect remaining context.".to_string(),
                        ..PerformerReport::default()
                    },
                )?;
            }
            Ok(())
        });

        if let Err(error) = result {
            tracing::warn!("failed to skip blocked Squad slots: {error}");
            self.set_squad_error(error.to_string());
            return;
        }

        if let Err(error) = self.maybe_start_squad_review(ctx, run_id) {
            tracing::warn!("failed to start Squad review after skip: {error}");
            self.set_squad_error(error.to_string());
        }
    }

    fn maybe_start_squad_review(&mut self, ctx: &egui::Context, run_id: &str) -> horizon_core::Result<()> {
        let Some(run) = self.agent_squad.runs.iter().find(|run| run.id == run_id).cloned() else {
            return Err(horizon_core::Error::State(format!("squad run {run_id} was not found")));
        };

        if matches!(
            run.status,
            horizon_core::RunStatus::Reviewing | horizon_core::RunStatus::Done | horizon_core::RunStatus::Failed
        ) {
            self.clear_squad_error();
            return Ok(());
        }
        if ready_for_blocked_decision(&run) {
            self.set_squad_error(blocked_review_message(&run));
            return Ok(());
        }
        if !ready_for_review(&run) {
            self.clear_squad_error();
            return Ok(());
        }

        self.start_squad_review(ctx, &run)
    }

    fn start_squad_review(&mut self, ctx: &egui::Context, run: &horizon_core::SquadRun) -> horizon_core::Result<()> {
        let primary_worktree = run
            .primary_worktree
            .clone()
            .ok_or_else(|| horizon_core::Error::State(format!("squad run {} has no review worktree", run.id)))?;
        let contexts = collect_review_contexts(run)?;
        apply_slot_diffs(&contexts, &primary_worktree)?;
        let prompt = reviewer_prompt(run, &contexts);
        let reviewer_kind = run.reviewer.as_ref().map_or(PanelKind::Claude, |link| link.kind);
        let workspace_id = self
            .squad_run_workspace_id(run)
            .unwrap_or_else(|| self.ensure_workspace_visible(ctx));
        let options = PanelOptions {
            name: Some(format!("Squad {} review", short_run_id(&run.id))),
            cwd: Some(primary_worktree),
            kind: reviewer_kind,
            resume: PanelResume::Fresh,
            ..PanelOptions::default()
        };
        let panel_id = self.create_panel_with_options(options, workspace_id)?;
        let panel = self
            .board
            .panel_mut(panel_id)
            .ok_or_else(|| horizon_core::Error::State(format!("panel {} was not created", panel_id.0)))?;
        panel.write_input(prompt.as_bytes());
        let panel_local_id = panel.local_id.clone();
        let reviewer = AgentPanelLink::new(reviewer_kind, Some(panel_local_id));
        self.update_agent_squad(|squad| squad.run_mut(&run.id)?.start_reviewing(reviewer))?;
        self.mark_runtime_dirty();
        self.clear_squad_error();
        Ok(())
    }

    fn squad_run_workspace_id(&self, run: &horizon_core::SquadRun) -> Option<WorkspaceId> {
        for slot in &run.performers {
            if let Some(panel_local_id) = &slot.panel_local_id
                && let Some(workspace_id) = self.workspace_id_for_panel_local_id(panel_local_id)
            {
                return Some(workspace_id);
            }
        }
        if let Some(panel_local_id) = run.reviewer.as_ref().and_then(|link| link.panel_local_id.as_ref()) {
            return self.workspace_id_for_panel_local_id(panel_local_id);
        }
        None
    }

    fn workspace_id_for_panel_local_id(&self, panel_local_id: &str) -> Option<WorkspaceId> {
        let panel_id = self.board.panel_id_by_local_id(panel_local_id)?;
        self.board.panel_workspace_id(panel_id)
    }

    fn set_squad_error(&mut self, message: String) {
        if let Some(state) = &mut self.squad_panel {
            state.error_message = Some(message);
        }
    }

    fn clear_squad_error(&mut self) {
        if let Some(state) = &mut self.squad_panel {
            state.error_message = None;
        }
    }

    fn fail_squad_run(&mut self, run_id: &str, reason: String) {
        if let Err(error) = self.update_agent_squad(|squad| {
            squad.run_mut(run_id)?.fail(reason);
            Ok(())
        }) {
            tracing::warn!("failed to persist failed Squad run: {error}");
        }
    }

    fn squad_source_repo(&self, workspace_id: WorkspaceId) -> horizon_core::Result<PathBuf> {
        if let Some(panel_id) = self.board.focused
            && self.board.panel_workspace_id(panel_id) == Some(workspace_id)
            && let Some(cwd) = self.board.panel(panel_id).and_then(|panel| panel.launch_cwd.clone())
        {
            return Ok(cwd);
        }
        if let Some(cwd) = self
            .board
            .workspace(workspace_id)
            .and_then(|workspace| workspace.cwd.clone())
        {
            return Ok(cwd);
        }
        if let Some(cwd) = self
            .board
            .workspace(workspace_id)
            .and_then(|workspace| workspace.panels.iter().find_map(|panel_id| self.board.panel(*panel_id)))
            .and_then(|panel| panel.launch_cwd.clone())
        {
            return Ok(cwd);
        }
        Err(horizon_core::Error::State(
            "Agent Squad needs an active workspace or panel directory inside a Git repository".to_string(),
        ))
    }

    fn update_agent_squad<F>(&mut self, update: F) -> horizon_core::Result<()>
    where
        F: FnOnce(&mut AgentSquad) -> horizon_core::Result<()>,
    {
        if let Some(active_session) = &self.active_session
            && active_session.persistent
        {
            let session_id = active_session.session_id.clone();
            self.agent_squad = self.session_store.update_agent_squad(&session_id, update)?;
            return Ok(());
        }

        update(&mut self.agent_squad)
    }
}

fn create_performer_slots(
    draft: &SquadRunDraft,
    source_repo: &Path,
    scratch_root: &Path,
    created_paths: &mut Vec<PathBuf>,
) -> horizon_core::Result<Vec<PerformerSlot>> {
    (1..=draft.performer_count)
        .map(|index| {
            let slot_id = format!("s{index}");
            let scratch = WorktreeManager::create(source_repo, "HEAD", scratch_root, &slot_id)?;
            created_paths.push(scratch.clone());
            let work_item = WorkItem {
                id: slot_id.clone(),
                title: format!("Slot {index}: {}", truncated_goal(&draft.goal)),
                request: slot_request(draft, index),
                acceptance_criteria: vec![
                    "Keep the change isolated to this slot's worktree.".to_string(),
                    "Run the most relevant validation available for the touched files.".to_string(),
                    "Report summary, validation, and any follow-up before marking done.".to_string(),
                ],
                status: WorkStatus::Queued,
                ..WorkItem::default()
            };
            Ok(PerformerSlot::new(slot_id, work_item, draft.performer_kind, scratch))
        })
        .collect()
}

fn cleanup_created_worktrees(paths: &[PathBuf]) {
    for path in paths.iter().rev() {
        if let Err(error) = WorktreeManager::remove(path) {
            tracing::warn!(path = %path.display(), "failed to clean up Squad worktree after start failure: {error}");
        }
    }
}

fn slot_request(draft: &SquadRunDraft, index: usize) -> String {
    format!(
        "Work independently on performer slot {index} for this goal:\n\n{}",
        draft.goal
    )
}

fn plan_text(draft: &SquadRunDraft) -> String {
    (1..=draft.performer_count)
        .map(|index| format!("{index}. {}", slot_request(draft, index).replace('\n', " ")))
        .collect::<Vec<_>>()
        .join("\n")
}

fn performer_prompt(run_id: &str, draft: &SquadRunDraft, slot: &PerformerSlot) -> String {
    let criteria = slot
        .work_item
        .acceptance_criteria
        .iter()
        .map(|criterion| format!("- {criterion}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "You are Agent Squad performer {slot_id} for run {run_short}.\n\nGoal:\n{goal}\n\nWork item:\n{title}\n\nRequest:\n{request}\n\nAcceptance criteria:\n{criteria}\n\nWhen finished, leave a concise report with summary, validation, and follow-up.\n",
        slot_id = slot.id,
        run_short = short_run_id(run_id),
        goal = draft.goal,
        title = slot.work_item.title,
        request = slot.work_item.request,
        criteria = criteria,
    )
}

fn blocked_review_message(run: &horizon_core::SquadRun) -> String {
    let slots = blocked_slots(run).join(", ");
    format!("Review paused for blocked slots: {slots}. Use Review Done Slots to skip blocked work.")
}

fn truncated_goal(goal: &str) -> String {
    let trimmed = goal.trim();
    if trimmed.chars().count() <= 42 {
        return trimmed.to_string();
    }
    let mut value = trimmed.chars().take(39).collect::<String>();
    value.push_str("...");
    value
}

fn short_run_id(run_id: &str) -> String {
    run_id.get(..4).unwrap_or(run_id).to_string()
}

fn current_unix_millis() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    i64::try_from(millis).unwrap_or(i64::MAX)
}

pub(super) fn empty_squad() -> AgentSquad {
    AgentSquad::new()
}
