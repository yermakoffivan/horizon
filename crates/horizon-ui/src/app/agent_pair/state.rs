use std::collections::HashMap;

use horizon_core::{AgentPairQueue, AgentWorkItem, PanelId, PanelKind, PerformerWorkReport, WorkItemStatus};

#[derive(Clone, Debug)]
pub(super) struct LinkablePanel {
    pub panel_id: PanelId,
    pub local_id: String,
    pub title: String,
    pub kind: PanelKind,
    pub workspace_name: String,
    pub terminal_backed: bool,
}

#[derive(Clone, Debug)]
pub(super) struct AgentPresetChoice {
    pub index: usize,
    pub name: String,
    pub kind: PanelKind,
}

#[derive(Clone, Debug, Default)]
pub(super) struct WorkRequestDraft {
    pub title: String,
    pub request: String,
    pub context: String,
    pub acceptance_criteria: String,
    pub suggested_commands: String,
}

impl WorkRequestDraft {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    #[must_use]
    pub fn is_ready(&self, goal: &str) -> bool {
        !goal.trim().is_empty() && !self.title.trim().is_empty() && !self.request.trim().is_empty()
    }

    #[must_use]
    pub fn acceptance_criteria_lines(&self) -> Vec<String> {
        split_lines(&self.acceptance_criteria)
    }

    #[must_use]
    pub fn suggested_command_lines(&self) -> Vec<String> {
        split_lines(&self.suggested_commands)
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct WorkReportDraft {
    pub summary: String,
    pub validation_commands: String,
    pub validation_result: String,
    pub follow_up: String,
}

impl WorkReportDraft {
    #[must_use]
    pub fn report(&self) -> PerformerWorkReport {
        PerformerWorkReport {
            summary: self.summary.trim().to_string(),
            validation_commands: split_lines(&self.validation_commands),
            validation_result: self.validation_result.trim().to_string(),
            follow_up: self.follow_up.trim().to_string(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(in crate::app) struct AgentPairUiState {
    pub(super) goal: String,
    pub(super) plan: String,
    pub(super) work_request: WorkRequestDraft,
    pub(super) report_by_work_item: HashMap<String, WorkReportDraft>,
    pub(super) researcher_preset_index: Option<usize>,
    pub(super) performer_preset_index: Option<usize>,
    pub(super) handoff_preset_index: Option<usize>,
    pub(super) error: Option<String>,
}

impl AgentPairUiState {
    pub(super) fn reset_for_queue(&mut self, queue: &AgentPairQueue) {
        self.goal.clone_from(&queue.goal);
        self.plan.clone_from(&queue.plan);
        self.report_by_work_item
            .retain(|work_item_id, _| queue.work_items.iter().any(|item| &item.id == work_item_id));
    }

    pub(super) fn report_draft_mut(&mut self, item: &AgentWorkItem) -> &mut WorkReportDraft {
        self.report_by_work_item.entry(item.id.clone()).or_insert_with(|| {
            item.performer_report
                .as_ref()
                .map_or_else(WorkReportDraft::default, |report| WorkReportDraft {
                    summary: report.summary.clone(),
                    validation_commands: report.validation_commands.join("\n"),
                    validation_result: report.validation_result.clone(),
                    follow_up: report.follow_up.clone(),
                })
        })
    }

    pub(super) fn ensure_default_presets(&mut self, choices: &[AgentPresetChoice]) {
        self.researcher_preset_index = ensure_valid_choice(self.researcher_preset_index, choices)
            .or_else(|| first_kind_choice(choices, PanelKind::Claude))
            .or_else(|| choices.first().map(|choice| choice.index));
        self.performer_preset_index = ensure_valid_choice(self.performer_preset_index, choices)
            .or_else(|| first_kind_choice(choices, PanelKind::Codex))
            .or_else(|| choices.first().map(|choice| choice.index));
        self.handoff_preset_index = ensure_valid_choice(self.handoff_preset_index, choices)
            .or_else(|| first_kind_choice(choices, PanelKind::Codex))
            .or_else(|| choices.first().map(|choice| choice.index));
    }
}

#[must_use]
pub(super) fn dispatch_enabled(queue: &AgentPairQueue, item: &AgentWorkItem) -> bool {
    item.status == WorkItemStatus::Queued && queue.performer.is_some()
}

#[must_use]
pub(super) fn role_heading(role: horizon_core::AgentPairRole) -> &'static str {
    role.label()
}

#[must_use]
pub(super) fn shorten_middle(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }

    let keep = max_chars - 3;
    let left = keep / 2 + keep % 2;
    let right = keep / 2;
    let prefix = value.chars().take(left).collect::<String>();
    let suffix = value
        .chars()
        .rev()
        .take(right)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("{prefix}...{suffix}")
}

fn ensure_valid_choice(selected: Option<usize>, choices: &[AgentPresetChoice]) -> Option<usize> {
    selected.filter(|index| choices.iter().any(|choice| choice.index == *index))
}

fn first_kind_choice(choices: &[AgentPresetChoice], kind: PanelKind) -> Option<usize> {
    choices
        .iter()
        .find(|choice| choice.kind == kind)
        .map(|choice| choice.index)
}

fn split_lines(value: &str) -> Vec<String> {
    value
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use horizon_core::{AgentPairQueue, AgentPairRole, PanelKind, WorkItemStatus};

    use super::{AgentPairUiState, AgentPresetChoice, dispatch_enabled, role_heading, shorten_middle};

    #[test]
    fn role_headings_match_queue_roles() {
        assert_eq!(role_heading(AgentPairRole::Researcher), "Researcher");
        assert_eq!(role_heading(AgentPairRole::Performer), "Performer");
    }

    #[test]
    fn disconnected_performer_disables_dispatch() {
        let mut queue = AgentPairQueue::new();
        queue.set_goal("Plan a feature").expect("goal");
        let id = queue
            .queue_work_request("Title", "Request", "", Vec::new(), Vec::new())
            .expect("work");
        let item = queue.work_item(&id).expect("work item");

        assert_eq!(item.status, WorkItemStatus::Queued);
        assert!(!dispatch_enabled(&queue, item));
    }

    #[test]
    fn connected_performer_enables_queued_dispatch() {
        let mut queue = AgentPairQueue::new();
        queue.set_goal("Plan a feature").expect("goal");
        queue
            .link_panel(AgentPairRole::Performer, "performer-local-id")
            .expect("link");
        let id = queue
            .queue_work_request("Title", "Request", "", Vec::new(), Vec::new())
            .expect("work");

        assert!(dispatch_enabled(&queue, queue.work_item(&id).expect("work item")));
    }

    #[test]
    fn default_presets_prefer_claude_researcher_and_codex_performer() {
        let choices = vec![
            AgentPresetChoice {
                index: 0,
                name: "Shell".to_string(),
                kind: PanelKind::Shell,
            },
            AgentPresetChoice {
                index: 1,
                name: "Codex".to_string(),
                kind: PanelKind::Codex,
            },
            AgentPresetChoice {
                index: 2,
                name: "Claude Code".to_string(),
                kind: PanelKind::Claude,
            },
        ];
        let mut state = AgentPairUiState::default();

        state.ensure_default_presets(&choices);

        assert_eq!(state.researcher_preset_index, Some(2));
        assert_eq!(state.performer_preset_index, Some(1));
        assert_eq!(state.handoff_preset_index, Some(1));
    }

    #[test]
    fn shorten_middle_preserves_edges_for_long_titles_and_paths() {
        let value = "crates/horizon-ui/src/app/agent_pair/really_long_collaboration_queue_path.rs";
        let shortened = shorten_middle(value, 32);

        assert!(shortened.starts_with("crates/horizon-"));
        assert!(shortened.ends_with("ue_path.rs"));
        assert!(shortened.len() <= 32);
    }
}
