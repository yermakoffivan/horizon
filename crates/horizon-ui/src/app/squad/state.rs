use horizon_core::PanelKind;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SquadView {
    Dashboard,
    Composer,
    RunLane { run_id: String },
}

#[derive(Clone, Debug)]
pub(in crate::app) struct SquadPanelState {
    pub(super) view: SquadView,
    pub(super) composer: SquadComposerState,
    pub(super) error_message: Option<String>,
}

impl SquadPanelState {
    pub fn dashboard() -> Self {
        Self {
            view: SquadView::Dashboard,
            composer: SquadComposerState::default(),
            error_message: None,
        }
    }

    pub fn composer() -> Self {
        Self {
            view: SquadView::Composer,
            composer: SquadComposerState::default(),
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
