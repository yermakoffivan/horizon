use egui::{Align, Layout, RichText};
use horizon_core::{AgentSquad, RunStatus, SquadRun, WorkStatus};

use crate::app::util;
use crate::theme;

use super::SquadAction;

pub(super) fn render_dashboard(ui: &mut egui::Ui, squad: &AgentSquad) -> SquadAction {
    let mut action = SquadAction::None;

    ui.horizontal(|ui| {
        ui.heading(RichText::new("Squad Dashboard").size(16.0).color(theme::FG()));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(util::primary_button("New run")).clicked() {
                action = SquadAction::NewRun;
            }
        });
    });
    ui.add_space(10.0);

    if squad.runs.is_empty() {
        render_empty_dashboard(ui);
        return action;
    }

    egui::Grid::new("agent_squad_dashboard_grid")
        .num_columns(6)
        .spacing([18.0, 8.0])
        .striped(true)
        .show(ui, |ui| {
            header(ui, "Run");
            header(ui, "Goal");
            header(ui, "Roles");
            header(ui, "Status");
            header(ui, "Progress");
            header(ui, "");
            ui.end_row();

            for run in squad.runs.iter().rev() {
                ui.label(RichText::new(short_run_id(&run.id)).monospace().color(theme::ACCENT()));
                ui.label(RichText::new(run_goal(run)).color(theme::FG()));
                ui.label(RichText::new(role_summary(run)).color(theme::FG_DIM()));
                ui.label(status_text(run.status));
                ui.label(RichText::new(progress_text(run)).monospace().color(theme::FG_SOFT()));
                if ui.add(util::chrome_button("Open")).clicked() {
                    action = SquadAction::OpenRun(run.id.clone());
                }
                ui.end_row();
            }
        });

    action
}

fn render_empty_dashboard(ui: &mut egui::Ui) {
    let available = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::Vec2::new(available, 120.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 6.0, theme::PANEL_BG());
    painter.rect_stroke(
        rect,
        6.0,
        egui::Stroke::new(1.0, theme::BORDER_SUBTLE()),
        egui::StrokeKind::Outside,
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "No squad runs",
        egui::FontId::proportional(13.0),
        theme::FG_DIM(),
    );
}

fn header(ui: &mut egui::Ui, label: &str) {
    ui.label(RichText::new(label).size(10.0).strong().color(theme::FG_DIM()));
}

fn short_run_id(run_id: &str) -> String {
    format!("#{}", run_id.get(..4).unwrap_or(run_id))
}

fn run_goal(run: &SquadRun) -> String {
    let count = run.performers.len();
    if count == 0 {
        return truncate(&run.goal, 44);
    }
    format!("{} (x{count})", truncate(&run.goal, 36))
}

fn role_summary(run: &SquadRun) -> String {
    let researcher = run.researcher.as_ref().map_or("None", |link| link.kind.display_name());
    let reviewer = run.reviewer.as_ref().map_or("None", |link| link.kind.display_name());
    let performer = run
        .performers
        .first()
        .map_or("None", |slot| slot.assigned_kind.display_name());
    format!("{researcher} -> {performer} -> {reviewer}")
}

fn status_text(status: RunStatus) -> RichText {
    let color = match status {
        RunStatus::Done => theme::PALETTE_GREEN(),
        RunStatus::Failed => theme::PALETTE_RED(),
        RunStatus::Reviewing => theme::ACCENT(),
        RunStatus::Working | RunStatus::FanningOut | RunStatus::Decomposing => theme::PALETTE_YELLOW(),
        RunStatus::Draft => theme::FG_DIM(),
    };
    RichText::new(format!("{status:?}").to_lowercase())
        .monospace()
        .color(color)
}

fn progress_text(run: &SquadRun) -> String {
    if run.performers.is_empty() {
        return "-".to_string();
    }

    run.performers
        .iter()
        .map(|slot| match slot.work_item.status {
            WorkStatus::Queued => "q",
            WorkStatus::Dispatched => "w",
            WorkStatus::Done => "d",
            WorkStatus::Blocked => "b",
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value.chars().take(max_chars.saturating_sub(3)).collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use horizon_core::SquadRun;

    use super::run_goal;

    #[test]
    fn run_goal_includes_performer_count() {
        let mut run = SquadRun::new("run-1", "Fix flaky tests", 1);
        run.performers = vec![
            horizon_core::PerformerSlot::default(),
            horizon_core::PerformerSlot::default(),
        ];

        assert_eq!(run_goal(&run), "Fix flaky tests (x2)");
    }
}
