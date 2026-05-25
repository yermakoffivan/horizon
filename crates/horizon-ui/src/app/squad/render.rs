use egui::{Id, Order, RichText, Vec2};
use horizon_core::AgentSquad;

use crate::theme;

use super::SquadAction;
use super::composer::render_composer;
use super::dashboard::render_dashboard;
use super::lane::render_run_lane;
use super::slot_detail::render_slot_detail;
use super::state::{SquadPanelState, SquadView};

pub(super) fn render_agent_squad(ctx: &egui::Context, state: &mut SquadPanelState, squad: &AgentSquad) -> SquadAction {
    let mut open = true;
    let mut action = SquadAction::None;

    egui::Window::new(RichText::new("Agent Squad").color(theme::FG()).strong())
        .id(Id::new("agent_squad_window"))
        .open(&mut open)
        .default_width(820.0)
        .min_width(620.0)
        .min_height(360.0)
        .order(Order::Debug)
        .resizable(true)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.set_min_size(Vec2::new(580.0, 320.0));
            if let Some(message) = &state.error_message {
                ui.label(RichText::new(message).color(theme::PALETTE_RED()).strong());
                ui.add_space(8.0);
            }
            let next = match &state.view {
                SquadView::Dashboard => render_dashboard(ui, squad),
                SquadView::Composer => render_composer(ui, &mut state.composer),
                SquadView::RunLane { run_id } => render_run_lane(ui, squad, run_id),
                SquadView::SlotDetail => {
                    if let Some(detail) = &mut state.slot_detail {
                        render_slot_detail(ui, squad, detail)
                    } else {
                        SquadAction::Dashboard
                    }
                }
            };
            if !matches!(next, SquadAction::None) {
                action = next;
            }
        });

    if open { action } else { SquadAction::Close }
}
