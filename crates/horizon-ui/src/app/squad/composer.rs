use egui::{Align, ComboBox, Layout, RichText};
use horizon_core::PanelKind;

use crate::app::util;
use crate::theme;

use super::SquadAction;
use super::state::SquadComposerState;

const AGENT_KINDS: [PanelKind; 5] = [
    PanelKind::Codex,
    PanelKind::Claude,
    PanelKind::OpenCode,
    PanelKind::Gemini,
    PanelKind::KiloCode,
];

pub(super) fn render_composer(ui: &mut egui::Ui, composer: &mut SquadComposerState) -> SquadAction {
    let mut action = SquadAction::None;

    ui.horizontal(|ui| {
        ui.heading(RichText::new("New Squad Run").size(16.0).color(theme::FG()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(util::chrome_button("Dashboard")).clicked() {
                action = SquadAction::Dashboard;
            }
        });
    });
    ui.add_space(10.0);

    ui.label(RichText::new("Goal").size(10.5).strong().color(theme::FG_DIM()));
    ui.add(
        egui::TextEdit::multiline(&mut composer.goal)
            .desired_rows(4)
            .desired_width(f32::INFINITY),
    );
    ui.add_space(12.0);

    ui.label(RichText::new("Roles").size(10.5).strong().color(theme::FG_DIM()));
    egui::Grid::new("agent_squad_roles_grid")
        .num_columns(2)
        .spacing([14.0, 8.0])
        .show(ui, |ui| {
            role_label(ui, "Researcher");
            kind_combo(ui, "agent_squad_researcher_kind", &mut composer.researcher_kind);
            ui.end_row();

            role_label(ui, "Reviewer");
            kind_combo(ui, "agent_squad_reviewer_kind", &mut composer.reviewer_kind);
            ui.end_row();

            role_label(ui, "Performers");
            ui.horizontal(|ui| {
                kind_combo(ui, "agent_squad_performer_kind", &mut composer.performer_kind);
                ComboBox::from_id_salt("agent_squad_performer_count")
                    .selected_text(composer.performer_count.to_string())
                    .show_ui(ui, |ui| {
                        for count in SquadComposerState::MIN_PERFORMERS..=SquadComposerState::MAX_PERFORMERS {
                            ui.selectable_value(&mut composer.performer_count, count, count.to_string());
                        }
                    });
            });
            ui.end_row();
        });
    ui.add_space(12.0);

    ui.label(RichText::new("Isolation").size(10.5).strong().color(theme::FG_DIM()));
    ui.label(RichText::new("Worktree").monospace().color(theme::FG_SOFT()));
    ui.add_space(12.0);

    ui.label(RichText::new("Advanced").size(10.5).strong().color(theme::FG_DIM()));
    ui.checkbox(&mut composer.auto_start_reviewer, "Auto-start reviewer");
    ui.checkbox(&mut composer.reviewer_commits, "Reviewer commits");
    ui.checkbox(&mut composer.auto_close_performers, "Auto-close performers");
    ui.add_space(16.0);

    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let start_enabled = composer.draft().is_some();
        if ui
            .add_enabled(start_enabled, util::primary_button("Start Run"))
            .clicked()
            && let Some(draft) = composer.draft()
        {
            action = SquadAction::StartRun(draft);
        }
        if ui.add(util::chrome_button("Cancel")).clicked() {
            action = SquadAction::Dashboard;
        }
    });

    action
}

fn role_label(ui: &mut egui::Ui, label: &str) {
    ui.label(RichText::new(label).color(theme::FG_DIM()));
}

fn kind_combo(ui: &mut egui::Ui, id: &'static str, value: &mut PanelKind) {
    ComboBox::from_id_salt(id)
        .selected_text(value.display_name())
        .show_ui(ui, |ui| {
            for kind in AGENT_KINDS {
                ui.selectable_value(value, kind, kind.display_name());
            }
        });
}
