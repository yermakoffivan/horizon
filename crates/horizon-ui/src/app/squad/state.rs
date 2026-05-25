use horizon_core::{PanelKind, PerformerReport, PerformerSlot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SquadView {
    Dashboard,
    Composer,
    RunLane { run_id: String },
    SlotDetail,
}

#[derive(Clone, Debug)]
pub(in crate::app) struct SquadPanelState {
    pub(super) view: SquadView,
    pub(super) composer: SquadComposerState,
    pub(super) slot_detail: Option<SquadSlotDetailState>,
    pub(super) error_message: Option<String>,
}

impl SquadPanelState {
    pub fn dashboard() -> Self {
        Self {
            view: SquadView::Dashboard,
            composer: SquadComposerState::default(),
            slot_detail: None,
            error_message: None,
        }
    }

    pub fn composer() -> Self {
        Self {
            view: SquadView::Composer,
            composer: SquadComposerState::default(),
            slot_detail: None,
            error_message: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SquadComposerState {
    pub goal: String,
    pub researcher_kind: PanelKind,
    pub reviewer_kind: PanelKind,
    pub performer_kind: PanelKind,
    pub performer_count: usize,
    pub auto_start_reviewer: bool,
    pub reviewer_commits: bool,
    pub auto_close_performers: bool,
}

impl SquadComposerState {
    pub const MIN_PERFORMERS: usize = 1;
    pub const MAX_PERFORMERS: usize = 8;

    pub fn draft(&self) -> Option<SquadRunDraft> {
        let goal = self.goal.trim();
        if goal.is_empty() {
            return None;
        }

        Some(SquadRunDraft {
            goal: goal.to_string(),
            researcher_kind: self.researcher_kind,
            reviewer_kind: self.reviewer_kind,
            performer_kind: self.performer_kind,
            performer_count: self.performer_count.clamp(Self::MIN_PERFORMERS, Self::MAX_PERFORMERS),
        })
    }
}

impl Default for SquadComposerState {
    fn default() -> Self {
        Self {
            goal: String::new(),
            researcher_kind: PanelKind::Claude,
            reviewer_kind: PanelKind::Claude,
            performer_kind: PanelKind::Codex,
            performer_count: 3,
            auto_start_reviewer: true,
            reviewer_commits: true,
            auto_close_performers: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SquadRunDraft {
    pub goal: String,
    pub researcher_kind: PanelKind,
    pub reviewer_kind: PanelKind,
    pub performer_kind: PanelKind,
    pub performer_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SquadSlotDetailState {
    pub run_id: String,
    pub slot_id: String,
    pub diff: String,
    pub diff_error: Option<String>,
    pub report_summary: String,
    pub validation_commands: String,
    pub validation_result: String,
    pub follow_up: String,
}

impl SquadSlotDetailState {
    pub fn from_slot(
        run_id: impl Into<String>,
        slot: &PerformerSlot,
        diff: String,
        diff_error: Option<String>,
    ) -> Self {
        let report = slot.report.clone().unwrap_or_default();
        Self {
            run_id: run_id.into(),
            slot_id: slot.id.clone(),
            diff,
            diff_error,
            report_summary: report.summary,
            validation_commands: report.validation_commands.join("\n"),
            validation_result: report.validation_result,
            follow_up: report.follow_up,
        }
    }

    pub fn report(&self) -> PerformerReport {
        PerformerReport {
            summary: self.report_summary.trim().to_string(),
            validation_commands: self
                .validation_commands
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToString::to_string)
                .collect(),
            validation_result: self.validation_result.trim().to_string(),
            follow_up: self.follow_up.trim().to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SquadComposerState;

    #[test]
    fn composer_draft_requires_goal() {
        assert!(SquadComposerState::default().draft().is_none());
    }

    #[test]
    fn composer_draft_clamps_performer_count() {
        let state = SquadComposerState {
            goal: "Fix bugs".to_string(),
            performer_count: 99,
            ..SquadComposerState::default()
        };

        let draft = state.draft().expect("draft");

        assert_eq!(draft.performer_count, SquadComposerState::MAX_PERFORMERS);
    }
}
