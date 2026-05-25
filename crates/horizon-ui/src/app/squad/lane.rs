use egui::{Align, Layout, Margin, RichText, Stroke, Vec2};
use horizon_core::{AgentPanelLink, AgentSquad, PerformerSlot, RunStatus, SquadRun, WorkStatus};

use crate::app::util;
use crate::theme;

use super::SquadAction;

pub(super) fn render_run_lane(ui: &mut egui::Ui, squad: &AgentSquad, run_id: &str) -> SquadAction {
    let Some(run) = squad.runs.iter().find(|run| run.id == run_id) else {
        ui.label(RichText::new("Run not found").color(theme::FG_DIM()));
        return if ui.add(util::chrome_button("Dashboard")).clicked() {
            SquadAction::Dashboard
        } else {
            SquadAction::None
        };
    };

    let mut action = SquadAction::None;
    ui.horizontal(|ui| {
        ui.heading(
            RichText::new(format!("Run {} - {}", short_run_id(&run.id), status_label(run.status)))
                .size(16.0)
                .color(theme::FG()),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(util::chrome_button("Dashboard")).clicked() {
                action = SquadAction::Dashboard;
            }
        });
    });
    ui.add_space(8.0);
    ui.label(RichText::new(&run.goal).color(theme::FG_SOFT()));
    ui.add_space(12.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        render_researcher(ui, run);
        ui.add_space(10.0);
        render_performer_slots(ui, run, &mut action);
        ui.add_space(10.0);
        render_reviewer(ui, run);
    });

    action
}

fn render_researcher(ui: &mut egui::Ui, run: &SquadRun) {
    render_panel_link(
        ui,
        "Researcher",
        run.researcher.as_ref(),
        &format!("Plan: {}", plan_summary(run)),
    );
}

fn render_reviewer(ui: &mut egui::Ui, run: &SquadRun) {
    let detail = match run.status {
        RunStatus::Reviewing => "Reviewing slot reports and diffs".to_string(),
        RunStatus::Done => "Review complete".to_string(),
        RunStatus::Failed => run
            .failure_reason
            .clone()
            .unwrap_or_else(|| "Run failed before review completed".to_string()),
        _ => format!("Waiting on {}", waiting_summary(run)),
    };
    render_panel_link(ui, "Reviewer", run.reviewer.as_ref(), &detail);
}

fn render_panel_link(ui: &mut egui::Ui, title: &str, link: Option<&AgentPanelLink>, detail: &str) {
    card_frame().show(ui, |ui| {
        ui.label(RichText::new(title).size(12.5).strong().color(theme::ACCENT()));
        let agent = link.map_or("Not started", |link| link.kind.display_name());
        let panel = link
            .and_then(|link| link.panel_local_id.as_deref())
            .map_or("no panel", |panel_id| panel_id);
        ui.label(RichText::new(format!("{agent} - {panel}")).color(theme::FG_SOFT()));
        ui.label(RichText::new(detail).color(theme::FG_DIM()));
    });
}

fn render_performer_slots(ui: &mut egui::Ui, run: &SquadRun, action: &mut SquadAction) {
    ui.label(RichText::new("Performers").size(10.5).strong().color(theme::FG_DIM()));
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
        for slot in &run.performers {
            let next = render_slot_card(ui, run, slot);
            if !matches!(next, SquadAction::None) {
                *action = next;
            }
        }
    });
}

fn render_slot_card(ui: &mut egui::Ui, run: &SquadRun, slot: &PerformerSlot) -> SquadAction {
    let mut action = SquadAction::None;
    card_frame().show(ui, |ui| {
        ui.set_min_width(220.0);
        ui.set_max_width(260.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(&slot.id).monospace().strong().color(theme::ACCENT()));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(status_text(slot.work_item.status));
            });
        });
        ui.label(RichText::new(&slot.work_item.title).color(theme::FG()).strong());
        ui.label(RichText::new(slot.assigned_kind.display_name()).color(theme::FG_SOFT()));
        ui.label(RichText::new(panel_text(slot)).color(theme::FG_DIM()));
        ui.label(
            RichText::new(path_text(&slot.scratch))
                .monospace()
                .color(theme::FG_DIM()),
        );
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if let Some(panel_local_id) = &slot.panel_local_id
                && ui.add(util::chrome_button("Focus")).clicked()
            {
                action = SquadAction::FocusPanel(panel_local_id.clone());
            }
            if slot.work_item.status != WorkStatus::Done && ui.add(util::primary_button("Mark Done")).clicked() {
                action = SquadAction::MarkSlotDone {
                    run_id: run.id.clone(),
                    slot_id: slot.id.clone(),
                };
            }
            if slot.work_item.status != WorkStatus::Blocked && ui.add(util::chrome_button("Block")).clicked() {
                action = SquadAction::MarkSlotBlocked {
                    run_id: run.id.clone(),
                    slot_id: slot.id.clone(),
                };
            }
        });
    });
    action
}

fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::alpha(theme::PANEL_BG(), 238))
        .stroke(Stroke::new(1.0, theme::BORDER_SUBTLE()))
        .corner_radius(6)
        .inner_margin(Margin::same(10))
}

fn status_text(status: WorkStatus) -> RichText {
    let color = match status {
        WorkStatus::Queued => theme::FG_DIM(),
        WorkStatus::Dispatched => theme::PALETTE_YELLOW(),
        WorkStatus::Done => theme::PALETTE_GREEN(),
        WorkStatus::Blocked => theme::PALETTE_RED(),
    };
    RichText::new(work_status_label(status)).monospace().color(color)
}

fn work_status_label(status: WorkStatus) -> &'static str {
    match status {
        WorkStatus::Queued => "queued",
        WorkStatus::Dispatched => "working",
        WorkStatus::Done => "done",
        WorkStatus::Blocked => "blocked",
    }
}

fn status_label(status: RunStatus) -> &'static str {
    match status {
        RunStatus::Draft => "draft",
        RunStatus::Decomposing => "decomposing",
        RunStatus::FanningOut => "fanning out",
        RunStatus::Working => "working",
        RunStatus::Reviewing => "reviewing",
        RunStatus::Done => "done",
        RunStatus::Failed => "failed",
    }
}

fn plan_summary(run: &SquadRun) -> String {
    if run.plan_text.trim().is_empty() {
        format!("{} slots queued", run.performers.len())
    } else {
        let plan_items = run.plan_text.lines().filter(|line| !line.trim().is_empty()).count();
        format!("{plan_items} plan items, {} slots queued", run.performers.len())
    }
}

fn waiting_summary(run: &SquadRun) -> String {
    let waiting = run
        .performers
        .iter()
        .filter(|slot| slot.work_item.status != WorkStatus::Done)
        .map(|slot| slot.id.as_str())
        .collect::<Vec<_>>();
    if waiting.is_empty() {
        "reviewer start".to_string()
    } else {
        waiting.join(", ")
    }
}

fn panel_text(slot: &PerformerSlot) -> String {
    slot.panel_local_id
        .as_ref()
        .map_or_else(|| "no panel".to_string(), |panel_id| format!("panel {panel_id}"))
}

fn path_text(path: &std::path::Path) -> String {
    let value = path.display().to_string();
    let char_count = value.chars().count();
    if char_count <= 38 {
        return value;
    }
    let tail = value
        .chars()
        .rev()
        .take(35)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<String>();
    format!("...{tail}")
}

fn short_run_id(run_id: &str) -> String {
    format!("#{}", run_id.get(..4).unwrap_or(run_id))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::path_text;

    #[test]
    fn path_text_keeps_short_paths_verbatim() {
        assert_eq!(path_text(&PathBuf::from("/tmp/s1")), "/tmp/s1");
    }

    #[test]
    fn path_text_truncates_from_front() {
        let text = path_text(&PathBuf::from("/very/long/path/that/keeps/the/slot/worktree/s1"));

        assert!(text.starts_with("..."));
        assert!(text.ends_with("/s1"));
    }
}
