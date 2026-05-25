mod render;
mod state;

use egui::Context;
use horizon_core::{AgentPairQueue, AgentPairRole, PanelId, PanelKind, PresetConfig, WorkItemStatus};

use super::HorizonApp;
use crate::input;

pub(super) use state::AgentPairUiState;
use state::{AgentPresetChoice, LinkablePanel};

pub(super) const AGENT_PAIR_REVIEW_QUEUE_PANEL_ID: &str = "agent_pair_collaboration";
pub(super) const AGENT_PAIR_REVIEW_QUEUE_DEFAULT_WIDTH: f32 = 450.0;
pub(super) const AGENT_PAIR_REVIEW_QUEUE_MIN_WIDTH: f32 = 360.0;
pub(super) const AGENT_PAIR_REVIEW_QUEUE_MAX_WIDTH: f32 = 600.0;

impl HorizonApp {
    pub(in crate::app) fn toggle_agent_pair_review_queue(&mut self) {
        self.agent_pair_review_queue_open = !self.agent_pair_review_queue_open;
    }

    pub(in crate::app) fn open_agent_pair_review_queue(&mut self) {
        self.agent_pair_review_queue_open = true;
    }

    pub(super) fn load_agent_pair_queue_for_active_session(&mut self) {
        let Some(active_session) = self.active_session.as_ref().filter(|session| session.persistent) else {
            self.agent_pair_queue = AgentPairQueue::new();
            self.agent_pair_ui.error = None;
            self.agent_pair_ui.reset_for_queue(&self.agent_pair_queue);
            return;
        };

        match self.session_store.load_agent_pair_queue(&active_session.session_id) {
            Ok(queue) => {
                self.agent_pair_queue = queue;
                self.agent_pair_ui.error = None;
            }
            Err(error) => {
                tracing::warn!(
                    session_id = %active_session.session_id,
                    %error,
                    "failed to load agent pair collaboration state"
                );
                self.agent_pair_queue = AgentPairQueue::new();
                self.agent_pair_ui.error = Some(format!("Failed to load Agent Pair state: {error}"));
            }
        }
        self.agent_pair_ui.reset_for_queue(&self.agent_pair_queue);
    }

    fn save_agent_pair_queue(&mut self) {
        let Some(active_session) = self.active_session.as_ref().filter(|session| session.persistent) else {
            return;
        };

        if let Err(error) = self
            .session_store
            .save_agent_pair_queue(&active_session.session_id, &self.agent_pair_queue)
        {
            tracing::warn!(
                session_id = %active_session.session_id,
                %error,
                "failed to save agent pair collaboration state"
            );
            self.agent_pair_ui.error = Some(format!("Failed to save Agent Pair state: {error}"));
        }
    }

    fn save_agent_pair_goal(&mut self) {
        match self.agent_pair_queue.set_goal(self.agent_pair_ui.goal.clone()) {
            Ok(()) => {
                self.agent_pair_ui.error = None;
                self.save_agent_pair_queue();
            }
            Err(error) => self.agent_pair_ui.error = Some(error.to_string()),
        }
    }

    fn save_agent_pair_plan(&mut self) {
        self.agent_pair_queue.set_plan(self.agent_pair_ui.plan.clone());
        self.agent_pair_ui.error = None;
        self.save_agent_pair_queue();
    }

    fn start_agent_pair(&mut self, ctx: &Context) {
        if let Err(error) = self.agent_pair_queue.set_goal(self.agent_pair_ui.goal.clone()) {
            self.agent_pair_ui.error = Some(error.to_string());
            return;
        }

        let Some(researcher_preset) = self.selected_agent_preset(self.agent_pair_ui.researcher_preset_index) else {
            self.agent_pair_ui.error = Some("Select a researcher agent preset.".to_string());
            return;
        };
        let Some(performer_preset) = self.selected_agent_preset(self.agent_pair_ui.performer_preset_index) else {
            self.agent_pair_ui.error = Some("Select a performer agent preset.".to_string());
            return;
        };

        match self.spawn_agent_pair_panel(ctx, AgentPairRole::Researcher, &researcher_preset) {
            Ok(researcher_panel_id) => {
                if let Err(error) = self.spawn_and_link_performer(ctx, &performer_preset, researcher_panel_id) {
                    self.agent_pair_ui.error = Some(error);
                }
            }
            Err(error) => self.agent_pair_ui.error = Some(error),
        }
    }

    fn spawn_and_link_performer(
        &mut self,
        ctx: &Context,
        performer_preset: &PresetConfig,
        researcher_panel_id: PanelId,
    ) -> std::result::Result<(), String> {
        let performer_panel_id = self.spawn_agent_pair_panel(ctx, AgentPairRole::Performer, performer_preset)?;
        let researcher_local_id = self
            .board
            .panel(researcher_panel_id)
            .map(|panel| panel.local_id.clone())
            .ok_or_else(|| "Researcher panel disappeared after launch.".to_string())?;
        let performer_local_id = self
            .board
            .panel(performer_panel_id)
            .map(|panel| panel.local_id.clone())
            .ok_or_else(|| "Performer panel disappeared after launch.".to_string())?;

        self.agent_pair_queue
            .link_panel(AgentPairRole::Researcher, researcher_local_id)
            .map_err(|error| error.to_string())?;
        self.agent_pair_queue
            .link_panel(AgentPairRole::Performer, performer_local_id)
            .map_err(|error| error.to_string())?;
        self.send_prompt_to_panel(researcher_panel_id, &self.agent_pair_queue.researcher_brief_prompt())?;
        self.send_prompt_to_panel(performer_panel_id, &self.agent_pair_queue.performer_brief_prompt())?;
        self.agent_pair_ui.error = None;
        self.save_agent_pair_queue();
        Ok(())
    }

    fn spawn_agent_pair_panel(
        &mut self,
        ctx: &Context,
        role: AgentPairRole,
        preset: &PresetConfig,
    ) -> std::result::Result<PanelId, String> {
        let workspace_id = self.ensure_workspace_visible(ctx);
        let mut options = preset.to_panel_options();
        options.name = Some(format!("{} · {}", role.label(), preset.name));
        let panel_id = self
            .create_panel_with_options(options, workspace_id)
            .map_err(|error| format!("Failed to start {}: {error}", role.label()))?;
        self.mark_runtime_dirty();
        Ok(panel_id)
    }

    fn selected_agent_preset(&self, index: Option<usize>) -> Option<PresetConfig> {
        index
            .and_then(|index| self.presets.get(index))
            .filter(|preset| preset.kind.is_agent())
            .cloned()
    }

    fn link_agent_panel(&mut self, role: AgentPairRole, panel_local_id: Option<String>) {
        let result = if let Some(panel_local_id) = panel_local_id {
            self.agent_pair_queue.link_panel(role, panel_local_id)
        } else {
            self.agent_pair_queue.unlink_panel(role);
            Ok(())
        };

        match result {
            Ok(()) => {
                self.agent_pair_ui.error = None;
                self.save_agent_pair_queue();
            }
            Err(error) => self.agent_pair_ui.error = Some(error.to_string()),
        }
    }

    fn send_role_brief(&mut self, role: AgentPairRole) {
        let Some(local_id) = self
            .agent_pair_queue
            .link_for(role)
            .map(|link| link.panel_local_id.clone())
        else {
            self.agent_pair_ui.error = Some(format!("Link a {} panel before sending the brief.", role.label()));
            return;
        };
        let prompt = match role {
            AgentPairRole::Researcher => self.agent_pair_queue.researcher_brief_prompt(),
            AgentPairRole::Performer => self.agent_pair_queue.performer_brief_prompt(),
        };
        match self.send_prompt_to_panel_by_local_id(&local_id, &prompt) {
            Ok(()) => self.agent_pair_ui.error = None,
            Err(error) => self.agent_pair_ui.error = Some(error),
        }
    }

    fn create_agent_pair_work_request(&mut self) {
        if let Err(error) = self.agent_pair_queue.set_goal(self.agent_pair_ui.goal.clone()) {
            self.agent_pair_ui.error = Some(error.to_string());
            return;
        }

        let result = self.agent_pair_queue.queue_work_request(
            self.agent_pair_ui.work_request.title.clone(),
            self.agent_pair_ui.work_request.request.clone(),
            self.agent_pair_ui.work_request.context.clone(),
            self.agent_pair_ui.work_request.acceptance_criteria_lines(),
            self.agent_pair_ui.work_request.suggested_command_lines(),
        );

        match result {
            Ok(_) => {
                self.agent_pair_ui.work_request.clear();
                self.agent_pair_ui.error = None;
                self.save_agent_pair_queue();
            }
            Err(error) => self.agent_pair_ui.error = Some(error.to_string()),
        }
    }

    fn dispatch_agent_pair_work_item(&mut self, work_item_id: &str) {
        let Some(performer_local_id) = self
            .agent_pair_queue
            .link_for(AgentPairRole::Performer)
            .map(|link| link.panel_local_id.clone())
        else {
            self.agent_pair_ui.error = Some("Link a performer panel before dispatch.".to_string());
            return;
        };

        let prompt = match self.agent_pair_queue.dispatch_to_performer(work_item_id) {
            Ok(prompt) => prompt,
            Err(error) => {
                self.agent_pair_ui.error = Some(error.to_string());
                return;
            }
        };

        match self.send_prompt_to_panel_by_local_id(&performer_local_id, &prompt) {
            Ok(()) => {
                self.agent_pair_ui.error = None;
                self.save_agent_pair_queue();
            }
            Err(error) => {
                if let Some(item) = self
                    .agent_pair_queue
                    .work_items
                    .iter_mut()
                    .find(|item| item.id == work_item_id)
                {
                    item.status = WorkItemStatus::Queued;
                    item.assigned_performer_panel_local_id = None;
                }
                self.agent_pair_ui.error = Some(error);
            }
        }
    }

    fn complete_agent_pair_work_item(&mut self, work_item_id: &str) {
        let Some(item) = self.agent_pair_queue.work_item(work_item_id).cloned() else {
            self.agent_pair_ui.error = Some(format!("Work item {work_item_id} was not found."));
            return;
        };
        let report = self.agent_pair_ui.report_draft_mut(&item).report();

        match self.agent_pair_queue.complete_work(work_item_id, report) {
            Ok(()) => {
                self.agent_pair_ui.error = None;
                self.save_agent_pair_queue();
            }
            Err(error) => self.agent_pair_ui.error = Some(error.to_string()),
        }
    }

    fn block_agent_pair_work_item(&mut self, work_item_id: &str) {
        let Some(item) = self.agent_pair_queue.work_item(work_item_id).cloned() else {
            self.agent_pair_ui.error = Some(format!("Work item {work_item_id} was not found."));
            return;
        };
        let report = self.agent_pair_ui.report_draft_mut(&item).report();

        match self.agent_pair_queue.block_work(work_item_id, report) {
            Ok(()) => {
                self.agent_pair_ui.error = None;
                self.save_agent_pair_queue();
            }
            Err(error) => self.agent_pair_ui.error = Some(error.to_string()),
        }
    }

    fn launch_plan_handoff(&mut self, ctx: &Context) {
        if let Err(error) = self.agent_pair_queue.set_goal(self.agent_pair_ui.goal.clone()) {
            self.agent_pair_ui.error = Some(error.to_string());
            return;
        }
        self.agent_pair_queue.set_plan(self.agent_pair_ui.plan.clone());
        let Some(preset) = self.selected_agent_preset(self.agent_pair_ui.handoff_preset_index) else {
            self.agent_pair_ui.error = Some("Select an agent preset for the handoff session.".to_string());
            return;
        };

        let workspace_id = self.ensure_workspace_visible(ctx);
        let mut options = preset.to_panel_options();
        options.name = Some(format!("Plan Handoff · {}", preset.name));
        match self.create_panel_with_options(options, workspace_id) {
            Ok(panel_id) => {
                self.mark_runtime_dirty();
                match self.send_prompt_to_panel(panel_id, &self.agent_pair_queue.plan_handoff_prompt()) {
                    Ok(()) => {
                        self.agent_pair_ui.error = None;
                        self.save_agent_pair_queue();
                    }
                    Err(error) => self.agent_pair_ui.error = Some(error),
                }
            }
            Err(error) => self.agent_pair_ui.error = Some(format!("Failed to start handoff session: {error}")),
        }
    }

    fn send_prompt_to_panel_by_local_id(
        &mut self,
        panel_local_id: &str,
        prompt: &str,
    ) -> std::result::Result<(), String> {
        let panel_id = self
            .board
            .panel_id_by_local_id(panel_local_id)
            .ok_or_else(|| "The linked panel is not open.".to_string())?;
        self.send_prompt_to_panel(panel_id, prompt)
    }

    fn send_prompt_to_panel(&mut self, panel_id: PanelId, prompt: &str) -> std::result::Result<(), String> {
        let Some(mode) = self
            .board
            .panel(panel_id)
            .and_then(|panel| panel.terminal().map(horizon_core::Terminal::mode))
        else {
            return Err("The target panel cannot receive terminal input.".to_string());
        };
        let Some(panel) = self.board.panel_mut(panel_id) else {
            return Err("The target panel is not open.".to_string());
        };

        let mut bytes = input::paste_bytes(prompt, mode, true);
        bytes.push(b'\r');
        panel.write_input(&bytes);
        Ok(())
    }

    fn focus_linked_agent_panel(&mut self, ctx: &Context, panel_local_id: &str) {
        if let Some(panel_id) = self.board.panel_id_by_local_id(panel_local_id) {
            self.focus_panel_visible(ctx, panel_id, true);
        }
    }

    fn linkable_agent_panels(&self) -> Vec<LinkablePanel> {
        self.board
            .panels
            .iter()
            .filter(|panel| panel.terminal().is_some())
            .map(|panel| {
                let workspace_name = self
                    .board
                    .workspace(panel.workspace_id)
                    .map_or_else(|| "Unknown workspace".to_string(), |workspace| workspace.name.clone());
                LinkablePanel {
                    panel_id: panel.id,
                    local_id: panel.local_id.clone(),
                    title: panel.display_title().into_owned(),
                    kind: panel.kind,
                    workspace_name,
                    terminal_backed: panel.terminal().is_some(),
                }
            })
            .collect()
    }

    fn agent_preset_choices(&self) -> Vec<AgentPresetChoice> {
        self.presets
            .iter()
            .enumerate()
            .filter(|(_, preset)| preset.kind.is_agent())
            .map(|(index, preset)| AgentPresetChoice {
                index,
                name: preset.name.clone(),
                kind: preset.kind,
            })
            .collect()
    }

    fn performer_title_for_work_item(&self, work_item_id: &str) -> Option<String> {
        let local_id = self
            .agent_pair_queue
            .work_item(work_item_id)?
            .assigned_performer_panel_local_id
            .as_deref()?;
        self.board
            .panel_id_by_local_id(local_id)
            .and_then(|panel_id| self.board.panel(panel_id))
            .map(|panel| panel.display_title().into_owned())
    }
}

fn work_item_status_order(status: WorkItemStatus) -> usize {
    match status {
        WorkItemStatus::Queued => 0,
        WorkItemStatus::Dispatched => 1,
        WorkItemStatus::Blocked => 2,
        WorkItemStatus::Done => 3,
    }
}

fn agent_kind_label(kind: PanelKind) -> &'static str {
    kind.display_name()
}
