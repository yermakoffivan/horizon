mod composer;
mod dashboard;
mod render;
mod state;

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use horizon_core::{AgentPanelLink, AgentSquad, PerformerSlot, WorkItem, WorkStatus};

use self::render::render_agent_squad;
pub(super) use self::state::SquadPanelState;
use self::state::{SquadRunDraft, SquadView};
use super::HorizonApp;

#[derive(Clone, Debug, PartialEq, Eq)]
enum SquadAction {
    None,
    Close,
    Dashboard,
    NewRun,
    StartRun(SquadRunDraft),
}

impl HorizonApp {
    pub(super) fn open_agent_squad_dashboard(&mut self) {
        if let Some(state) = &mut self.squad_panel {
            state.view = SquadView::Dashboard;
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
        self.apply_squad_action(action);
    }

    fn apply_squad_action(&mut self, action: SquadAction) {
        match action {
            SquadAction::None => {}
            SquadAction::Close => self.squad_panel = None,
            SquadAction::Dashboard => {
                if let Some(state) = &mut self.squad_panel {
                    state.view = SquadView::Dashboard;
                }
            }
            SquadAction::NewRun => {
                self.squad_panel = Some(SquadPanelState::composer());
            }
            SquadAction::StartRun(draft) => {
                self.create_stub_squad_run(&draft);
                if let Some(state) = &mut self.squad_panel {
                    state.view = SquadView::Dashboard;
                    state.composer.goal.clear();
                }
            }
        }
    }

    fn create_stub_squad_run(&mut self, draft: &SquadRunDraft) {
        let scratch_root = self.session_store.home().root().join("squad-tmp");
        let now = current_unix_millis();
        let result = self.update_agent_squad(|squad| {
            let run = squad.create_run(draft.goal.clone(), now);
            run.start_decomposing(AgentPanelLink::new(draft.researcher_kind, None));
            run.reviewer = Some(AgentPanelLink::new(draft.reviewer_kind, None));
            let run_scratch_root = scratch_root.join(&run.id);
            run.set_primary_worktree(run_scratch_root.join("_review"));
            let performers = stub_performer_slots(draft, &run_scratch_root);
            run.queue_plan(String::new(), performers);
            Ok(())
        });

        if let Err(error) = result {
            tracing::warn!("failed to persist Agent Squad run: {error}");
        }
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

fn stub_performer_slots(draft: &SquadRunDraft, scratch_root: &Path) -> Vec<PerformerSlot> {
    (1..=draft.performer_count)
        .map(|index| {
            let slot_id = format!("s{index}");
            let work_item = WorkItem {
                id: slot_id.clone(),
                title: format!("Slot {index}"),
                request: draft.goal.clone(),
                status: WorkStatus::Queued,
                ..WorkItem::default()
            };
            PerformerSlot::new(
                slot_id.clone(),
                work_item,
                draft.performer_kind,
                scratch_root.join(&slot_id),
            )
        })
        .collect()
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
