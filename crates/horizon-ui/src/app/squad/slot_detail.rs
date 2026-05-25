use egui::{Align, Layout, Margin, RichText, Stroke, TextEdit};
use horizon_core::{AgentSquad, PerformerSlot, SquadRun, WorkStatus};

use crate::app::util;
use crate::theme;

use super::SquadAction;
use super::state::SquadSlotDetailState;

pub(super) fn render_slot_detail(
    ui: &mut egui::Ui,
    squad: &AgentSquad,
    detail: &mut SquadSlotDetailState,
) -> SquadAction {
    let Some((run, slot)) = find_slot(squad, &detail.run_id, &detail.slot_id) else {
        ui.label(RichText::new("Slot not found").color(theme::FG_DIM()));
        return if ui.add(util::chrome_button("Dashboard")).clicked() {
            SquadAction::Dashboard
        } else {
            SquadAction::None
        };
    };

    let mut action = SquadAction::None;
    ui.horizontal(|ui| {
        ui.heading(
            RichText::new(format!(
                "Slot {} - {}",
                slot.id,
                work_status_label(slot.work_item.status)
            ))
            .size(16.0)
            .color(theme::FG()),
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(util::chrome_button("Run Lane")).clicked() {
                action = SquadAction::OpenRun(run.id.clone());
            }
            if ui.add(util::chrome_button("Refresh Diff")).clicked() {
                action = SquadAction::RefreshSlotDetail;
            }
        });
    });
    ui.add_space(8.0);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        render_slot_summary(ui, slot, &mut action);
        ui.add_space(10.0);
        render_work_item(ui, slot);
        ui.add_space(10.0);
        render_report_editor(ui, detail, slot, &mut action);
        ui.add_space(10.0);
        render_diff(ui, detail);
        ui.add_space(10.0);
        render_reviewer_notes(ui, run);
    });

    action
}

fn render_slot_summary(ui: &mut egui::Ui, slot: &PerformerSlot, action: &mut SquadAction) {
    card_frame().show(ui, |ui| {
        ui.label(RichText::new("Panel").size(12.5).strong().color(theme::ACCENT()));
        ui.label(RichText::new(slot.assigned_kind.display_name()).color(theme::FG_SOFT()));
        ui.label(
            RichText::new(slot.scratch.display().to_string())
                .monospace()
                .color(theme::FG_DIM()),
        );
        if let Some(panel_local_id) = &slot.panel_local_id
            && ui.add(util::chrome_button("Focus")).clicked()
        {
            *action = SquadAction::FocusPanel(panel_local_id.clone());
        }
    });
}

fn render_work_item(ui: &mut egui::Ui, slot: &PerformerSlot) {
    card_frame().show(ui, |ui| {
        ui.label(RichText::new("Brief").size(12.5).strong().color(theme::ACCENT()));
        ui.label(RichText::new(&slot.work_item.title).strong().color(theme::FG()));
        ui.label(RichText::new(&slot.work_item.request).color(theme::FG_SOFT()));
        if !slot.work_item.acceptance_criteria.is_empty() {
            ui.add_space(6.0);
            for criterion in &slot.work_item.acceptance_criteria {
                ui.label(RichText::new(format!("- {criterion}")).color(theme::FG_DIM()));
            }
        }
    });
}

fn render_report_editor(
    ui: &mut egui::Ui,
    detail: &mut SquadSlotDetailState,
    slot: &PerformerSlot,
    action: &mut SquadAction,
) {
    card_frame().show(ui, |ui| {
        ui.label(RichText::new("Report").size(12.5).strong().color(theme::ACCENT()));
        labeled_multiline(ui, "Summary", &mut detail.report_summary, 2);
        labeled_multiline(ui, "Validation commands", &mut detail.validation_commands, 2);
        labeled_multiline(ui, "Validation result", &mut detail.validation_result, 2);
        labeled_multiline(ui, "Follow-up", &mut detail.follow_up, 2);
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if slot.work_item.status != WorkStatus::Done && ui.add(util::primary_button("Mark Done")).clicked() {
                *action = SquadAction::MarkSlotDoneWithReport {
                    run_id: detail.run_id.clone(),
                    slot_id: detail.slot_id.clone(),
                    report: detail.report(),
                };
            }
            if slot.work_item.status != WorkStatus::Blocked && ui.add(util::chrome_button("Block")).clicked() {
                *action = SquadAction::MarkSlotBlocked {
                    run_id: detail.run_id.clone(),
                    slot_id: detail.slot_id.clone(),
                    follow_up: block_reason(detail),
                };
            }
        });
    });
}

fn render_diff(ui: &mut egui::Ui, detail: &mut SquadSlotDetailState) {
    card_frame().show(ui, |ui| {
        ui.label(RichText::new("Diff").size(12.5).strong().color(theme::ACCENT()));
        if let Some(error) = &detail.diff_error {
            ui.label(RichText::new(error).color(theme::PALETTE_RED()));
        }
        if detail.diff.trim().is_empty() {
            ui.label(RichText::new("Empty diff").color(theme::FG_DIM()));
        } else {
            ui.add(
                TextEdit::multiline(&mut detail.diff)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(12)
                    .interactive(false),
            );
        }
    });
}

fn render_reviewer_notes(ui: &mut egui::Ui, run: &SquadRun) {
    card_frame().show(ui, |ui| {
        ui.label(
            RichText::new("Reviewer Notes")
                .size(12.5)
                .strong()
                .color(theme::ACCENT()),
        );
        let text = if let Some(reason) = &run.failure_reason {
            reason.as_str()
        } else if run
            .reviewer
            .as_ref()
            .and_then(|link| link.panel_local_id.as_ref())
            .is_some()
        {
            "Reviewer panel is active; final notes are captured in that panel."
        } else {
            "Reviewer has not started."
        };
        ui.label(RichText::new(text).color(theme::FG_SOFT()));
    });
}

fn labeled_multiline(ui: &mut egui::Ui, label: &str, value: &mut String, rows: usize) {
    ui.label(RichText::new(label).size(10.5).strong().color(theme::FG_DIM()));
    ui.add(
        TextEdit::multiline(value)
            .desired_width(f32::INFINITY)
            .desired_rows(rows),
    );
}

fn card_frame() -> egui::Frame {
    egui::Frame::new()
        .fill(theme::alpha(theme::PANEL_BG(), 238))
        .stroke(Stroke::new(1.0, theme::BORDER_SUBTLE()))
        .corner_radius(6)
        .inner_margin(Margin::same(10))
}

fn find_slot<'a>(squad: &'a AgentSquad, run_id: &str, slot_id: &str) -> Option<(&'a SquadRun, &'a PerformerSlot)> {
    let run = squad.runs.iter().find(|run| run.id == run_id)?;
    let slot = run.performers.iter().find(|slot| slot.id == slot_id)?;
    Some((run, slot))
}

fn block_reason(detail: &SquadSlotDetailState) -> String {
    let follow_up = detail.follow_up.trim();
    if follow_up.is_empty() {
        "Marked blocked manually from the slot detail.".to_string()
    } else {
        follow_up.to_string()
    }
}

fn work_status_label(status: WorkStatus) -> &'static str {
    match status {
        WorkStatus::Queued => "queued",
        WorkStatus::Dispatched => "working",
        WorkStatus::Done => "done",
        WorkStatus::Blocked => "blocked",
    }
}

#[cfg(test)]
mod tests {
    use super::block_reason;
    use crate::app::squad::state::SquadSlotDetailState;

    #[test]
    fn block_reason_falls_back_when_follow_up_is_empty() {
        let detail = SquadSlotDetailState {
            run_id: "run-1".to_string(),
            slot_id: "s1".to_string(),
            diff: String::new(),
            diff_error: None,
            report_summary: String::new(),
            validation_commands: String::new(),
            validation_result: String::new(),
            follow_up: String::new(),
        };

        assert!(block_reason(&detail).contains("slot detail"));
    }
}
