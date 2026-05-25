use std::cmp::Reverse;

use egui::{Align, Color32, Context, Layout, Margin, RichText, ScrollArea, Stroke, TextEdit, Ui, Vec2};
use horizon_core::{AgentPairRole, AgentWorkItem, WorkItemStatus};

use super::state::{AgentPresetChoice, LinkablePanel, dispatch_enabled, role_heading, shorten_middle};
use super::{
    AGENT_PAIR_REVIEW_QUEUE_DEFAULT_WIDTH, AGENT_PAIR_REVIEW_QUEUE_MAX_WIDTH, AGENT_PAIR_REVIEW_QUEUE_MIN_WIDTH,
    AGENT_PAIR_REVIEW_QUEUE_PANEL_ID, HorizonApp, agent_kind_label, work_item_status_order,
};
use crate::app::util::{chrome_button, danger_button, primary_button};
use crate::theme;

enum QueueAction {
    Close,
    SaveGoal,
    SavePlan,
    StartPair,
    SendBrief(AgentPairRole),
    Link(AgentPairRole, Option<String>),
    SelectResearcherPreset(usize),
    SelectPerformerPreset(usize),
    SelectHandoffPreset(usize),
    QueueWork,
    Dispatch(String),
    Complete(String),
    Block(String),
    LaunchPlanHandoff,
    Focus(String),
}

impl HorizonApp {
    pub(in crate::app) fn render_agent_pair_review_queue(&mut self, ctx: &Context) {
        if !self.agent_pair_review_queue_open {
            return;
        }

        let linkable_panels = self.linkable_agent_panels();
        let agent_presets = self.agent_preset_choices();
        self.agent_pair_ui.ensure_default_presets(&agent_presets);
        let mut actions = Vec::new();

        egui::SidePanel::right(AGENT_PAIR_REVIEW_QUEUE_PANEL_ID)
            .resizable(true)
            .default_width(AGENT_PAIR_REVIEW_QUEUE_DEFAULT_WIDTH)
            .width_range(AGENT_PAIR_REVIEW_QUEUE_MIN_WIDTH..=AGENT_PAIR_REVIEW_QUEUE_MAX_WIDTH)
            .frame(
                egui::Frame::default()
                    .fill(theme::PANEL_BG())
                    .stroke(Stroke::new(1.0, theme::alpha(theme::BORDER_SUBTLE(), 210)))
                    .inner_margin(Margin::same(14)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(330.0);
                render_title(ui, &mut actions);
                ui.add_space(10.0);
                self.render_goal_and_start(ui, &agent_presets, &mut actions);
                ui.add_space(12.0);
                self.render_agent_links(ui, &linkable_panels, &mut actions);
                ui.add_space(12.0);
                self.render_work_request_form(ui, &mut actions);
                ui.add_space(12.0);
                self.render_plan_handoff(ui, &agent_presets, &mut actions);
                ui.add_space(12.0);
                self.render_work_items(ui, &mut actions);
            });

        for action in actions {
            self.apply_queue_action(ctx, action);
        }
    }

    fn render_goal_and_start(
        &mut self,
        ui: &mut Ui,
        agent_presets: &[AgentPresetChoice],
        actions: &mut Vec<QueueAction>,
    ) {
        egui::Frame::default()
            .fill(theme::PANEL_BG_ALT())
            .stroke(Stroke::new(1.0, theme::alpha(theme::BORDER_SUBTLE(), 190)))
            .corner_radius(8)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.label(RichText::new("Shared Goal").color(theme::FG()).size(13.0).strong());
                ui.add_space(6.0);
                labeled_multiline(ui, "Goal", &mut self.agent_pair_ui.goal, 3);
                ui.add_space(8.0);
                render_agent_preset_picker(
                    ui,
                    "Researcher",
                    agent_presets,
                    self.agent_pair_ui.researcher_preset_index,
                    QueueAction::SelectResearcherPreset,
                    actions,
                );
                render_agent_preset_picker(
                    ui,
                    "Performer",
                    agent_presets,
                    self.agent_pair_ui.performer_preset_index,
                    QueueAction::SelectPerformerPreset,
                    actions,
                );
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    let goal_ready = !self.agent_pair_ui.goal.trim().is_empty();
                    if ui
                        .add_enabled(goal_ready, primary_button("Start Pair").min_size(Vec2::new(96.0, 28.0)))
                        .on_disabled_hover_text("Enter a shared goal before starting agents.")
                        .clicked()
                    {
                        actions.push(QueueAction::StartPair);
                    }
                    if ui
                        .add_enabled(goal_ready, chrome_button("Save Goal").min_size(Vec2::new(94.0, 28.0)))
                        .clicked()
                    {
                        actions.push(QueueAction::SaveGoal);
                    }
                });
            });
    }

    fn render_agent_links(&self, ui: &mut Ui, panels: &[LinkablePanel], actions: &mut Vec<QueueAction>) {
        ui.horizontal_wrapped(|ui| {
            render_agent_link_chip(self, ui, AgentPairRole::Researcher, panels, actions);
            render_agent_link_chip(self, ui, AgentPairRole::Performer, panels, actions);
        });
    }

    fn render_work_request_form(&mut self, ui: &mut Ui, actions: &mut Vec<QueueAction>) {
        egui::CollapsingHeader::new(
            RichText::new("Queue Performer Work")
                .color(theme::FG())
                .size(13.0)
                .strong(),
        )
        .default_open(true)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            labeled_singleline(ui, "Title", &mut self.agent_pair_ui.work_request.title);
            labeled_multiline(ui, "Request", &mut self.agent_pair_ui.work_request.request, 3);
            labeled_multiline(ui, "Research context", &mut self.agent_pair_ui.work_request.context, 3);
            labeled_multiline(
                ui,
                "Acceptance criteria",
                &mut self.agent_pair_ui.work_request.acceptance_criteria,
                3,
            );
            labeled_multiline(
                ui,
                "Suggested commands",
                &mut self.agent_pair_ui.work_request.suggested_commands,
                3,
            );

            let ready = self.agent_pair_ui.work_request.is_ready(&self.agent_pair_ui.goal);
            if ui
                .add_enabled(ready, primary_button("Queue Work").min_size(Vec2::new(112.0, 30.0)))
                .on_disabled_hover_text("Goal, title, and request are required.")
                .clicked()
            {
                actions.push(QueueAction::QueueWork);
            }
        });
    }

    fn render_plan_handoff(
        &mut self,
        ui: &mut Ui,
        agent_presets: &[AgentPresetChoice],
        actions: &mut Vec<QueueAction>,
    ) {
        egui::CollapsingHeader::new(RichText::new("Plan Handoff").color(theme::FG()).size(13.0).strong())
            .default_open(false)
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                labeled_multiline(ui, "Plan", &mut self.agent_pair_ui.plan, 5);
                render_agent_preset_picker(
                    ui,
                    "New session",
                    agent_presets,
                    self.agent_pair_ui.handoff_preset_index,
                    QueueAction::SelectHandoffPreset,
                    actions,
                );
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add(chrome_button("Save Plan").min_size(Vec2::new(90.0, 28.0)))
                        .clicked()
                    {
                        actions.push(QueueAction::SavePlan);
                    }
                    let enabled = !self.agent_pair_ui.goal.trim().is_empty();
                    if ui
                        .add_enabled(
                            enabled,
                            primary_button("Open With Plan").min_size(Vec2::new(126.0, 28.0)),
                        )
                        .on_disabled_hover_text("Enter a goal before launching a handoff session.")
                        .clicked()
                    {
                        actions.push(QueueAction::LaunchPlanHandoff);
                    }
                });
            });
    }

    fn render_work_items(&mut self, ui: &mut Ui, actions: &mut Vec<QueueAction>) {
        if let Some(error) = &self.agent_pair_ui.error {
            ui.label(RichText::new(error).color(theme::PALETTE_RED()).size(11.0));
            ui.add_space(8.0);
        }

        let mut items = self.agent_pair_queue.work_items.clone();
        items.sort_by_key(|item| (work_item_status_order(item.status), Reverse(item.updated_at_millis)));

        ui.horizontal(|ui| {
            ui.label(RichText::new("Performer Queue").color(theme::FG()).size(13.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(items.len().to_string())
                        .monospace()
                        .color(theme::FG_DIM())
                        .size(11.0),
                );
            });
        });
        ui.add_space(6.0);

        ScrollArea::vertical()
            .id_salt("agent_pair_collaboration_work_items")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if items.is_empty() {
                    ui.label(
                        RichText::new("No performer work queued.")
                            .color(theme::FG_DIM())
                            .size(11.0),
                    );
                    return;
                }

                for item in items {
                    self.render_work_item(ui, &item, actions);
                    ui.add_space(10.0);
                }
            });
    }

    fn render_work_item(&mut self, ui: &mut Ui, item: &AgentWorkItem, actions: &mut Vec<QueueAction>) {
        egui::Frame::default()
            .fill(theme::PANEL_BG_ALT())
            .stroke(Stroke::new(1.0, theme::alpha(theme::BORDER_SUBTLE(), 190)))
            .corner_radius(8)
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(status_badge(item.status));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(shorten_middle(&item.id, 12))
                                .monospace()
                                .color(theme::FG_DIM())
                                .size(10.0),
                        );
                    });
                });
                ui.add_space(6.0);
                ui.add(egui::Label::new(RichText::new(&item.title).color(theme::FG()).size(13.0).strong()).wrap());
                ui.add_space(4.0);
                wrapping_label(ui, &item.request, theme::FG_SOFT());
                if !item.context.trim().is_empty() {
                    ui.add_space(6.0);
                    wrapping_label(ui, &item.context, theme::FG_DIM());
                }
                render_list(ui, "Acceptance", &item.acceptance_criteria);
                render_list(ui, "Commands", &item.suggested_commands);

                let performer_title = self.performer_title_for_work_item(&item.id);
                ui.add_space(8.0);
                ui.label(
                    RichText::new(item.assignment_label(performer_title.as_deref()))
                        .color(theme::FG_SOFT())
                        .size(11.0),
                );
                if let Some(report) = &item.performer_report {
                    ui.add_space(6.0);
                    wrapping_label(ui, &report.summary, theme::FG_DIM());
                }
                ui.add_space(8.0);
                self.render_work_item_actions(ui, item, actions);
            });
    }

    fn render_work_item_actions(&mut self, ui: &mut Ui, item: &AgentWorkItem, actions: &mut Vec<QueueAction>) {
        match item.status {
            WorkItemStatus::Queued => {
                let enabled = dispatch_enabled(&self.agent_pair_queue, item);
                if ui
                    .add_enabled(enabled, primary_button("Dispatch").min_size(Vec2::new(96.0, 28.0)))
                    .on_disabled_hover_text("Link a performer panel before dispatch.")
                    .clicked()
                {
                    actions.push(QueueAction::Dispatch(item.id.clone()));
                }
            }
            WorkItemStatus::Dispatched => self.render_report_form(ui, item, actions),
            WorkItemStatus::Done | WorkItemStatus::Blocked => {}
        }
    }

    fn render_report_form(&mut self, ui: &mut Ui, item: &AgentWorkItem, actions: &mut Vec<QueueAction>) {
        let draft = self.agent_pair_ui.report_draft_mut(item);
        ui.separator();
        ui.add_space(4.0);
        labeled_multiline(ui, "Summary", &mut draft.summary, 2);
        labeled_multiline(ui, "Validation commands", &mut draft.validation_commands, 3);
        labeled_multiline(ui, "Validation result", &mut draft.validation_result, 2);
        labeled_multiline(ui, "Follow-up", &mut draft.follow_up, 2);
        let complete = draft.report().is_complete();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(complete, primary_button("Mark Done").min_size(Vec2::new(104.0, 28.0)))
                .clicked()
            {
                actions.push(QueueAction::Complete(item.id.clone()));
            }
            let blocked_ready = !draft.summary.trim().is_empty();
            if ui
                .add_enabled(blocked_ready, danger_button("Blocked").min_size(Vec2::new(92.0, 28.0)))
                .clicked()
            {
                actions.push(QueueAction::Block(item.id.clone()));
            }
        });
    }

    fn apply_queue_action(&mut self, ctx: &Context, action: QueueAction) {
        match action {
            QueueAction::Close => self.agent_pair_review_queue_open = false,
            QueueAction::SaveGoal => self.save_agent_pair_goal(),
            QueueAction::SavePlan => self.save_agent_pair_plan(),
            QueueAction::StartPair => self.start_agent_pair(ctx),
            QueueAction::SendBrief(role) => self.send_role_brief(role),
            QueueAction::Link(role, panel_local_id) => self.link_agent_panel(role, panel_local_id),
            QueueAction::SelectResearcherPreset(index) => self.agent_pair_ui.researcher_preset_index = Some(index),
            QueueAction::SelectPerformerPreset(index) => self.agent_pair_ui.performer_preset_index = Some(index),
            QueueAction::SelectHandoffPreset(index) => self.agent_pair_ui.handoff_preset_index = Some(index),
            QueueAction::QueueWork => self.create_agent_pair_work_request(),
            QueueAction::Dispatch(work_item_id) => self.dispatch_agent_pair_work_item(&work_item_id),
            QueueAction::Complete(work_item_id) => self.complete_agent_pair_work_item(&work_item_id),
            QueueAction::Block(work_item_id) => self.block_agent_pair_work_item(&work_item_id),
            QueueAction::LaunchPlanHandoff => self.launch_plan_handoff(ctx),
            QueueAction::Focus(panel_local_id) => self.focus_linked_agent_panel(ctx, &panel_local_id),
        }
    }
}

fn render_title(ui: &mut Ui, actions: &mut Vec<QueueAction>) {
    ui.horizontal(|ui| {
        ui.label(RichText::new("Agent Pair").color(theme::FG()).size(16.0).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add(chrome_button("Close").min_size(Vec2::new(64.0, 28.0))).clicked() {
                actions.push(QueueAction::Close);
            }
        });
    });
}

fn render_agent_preset_picker(
    ui: &mut Ui,
    label: &str,
    presets: &[AgentPresetChoice],
    selected: Option<usize>,
    action: fn(usize) -> QueueAction,
    actions: &mut Vec<QueueAction>,
) {
    ui.label(RichText::new(label).color(theme::FG_DIM()).size(10.5));
    let selected_text = selected
        .and_then(|index| presets.iter().find(|choice| choice.index == index))
        .map_or_else(|| "No agent preset".to_string(), preset_choice_label);
    egui::ComboBox::from_id_salt(("agent_pair_preset", label))
        .selected_text(selected_text)
        .width(220.0)
        .show_ui(ui, |ui| {
            for choice in presets {
                let is_selected = selected == Some(choice.index);
                if ui.selectable_label(is_selected, preset_choice_label(choice)).clicked() {
                    actions.push(action(choice.index));
                }
            }
        });
}

fn render_agent_link_chip(
    app: &HorizonApp,
    ui: &mut Ui,
    role: AgentPairRole,
    panels: &[LinkablePanel],
    actions: &mut Vec<QueueAction>,
) {
    let linked_id = app
        .agent_pair_queue
        .link_for(role)
        .map(|link| link.panel_local_id.as_str());
    let current = linked_id.and_then(|local_id| panels.iter().find(|panel| panel.local_id == local_id));

    egui::Frame::default()
        .fill(theme::alpha(theme::BG_ELEVATED(), 210))
        .stroke(Stroke::new(1.0, theme::alpha(theme::BORDER_SUBTLE(), 190)))
        .corner_radius(8)
        .inner_margin(Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(200.0);
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.label(RichText::new(role_heading(role)).color(theme::FG()).size(11.5).strong());
                ui.add_space(2.0);
                ui.label(link_label(current, linked_id).color(theme::FG_SOFT()).size(10.5));
                ui.label(RichText::new(link_detail(current)).color(theme::FG_DIM()).size(10.0));
                ui.add_space(6.0);
                egui::ComboBox::from_id_salt(("agent_pair_link", role.label()))
                    .selected_text(current.map_or("Disconnected".to_string(), |panel| shorten_middle(&panel.title, 22)))
                    .width(176.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(linked_id.is_none(), "Disconnected").clicked() {
                            actions.push(QueueAction::Link(role, None));
                        }
                        for panel in panels {
                            let selected = Some(panel.local_id.as_str()) == linked_id;
                            let label = format!("{} · {}", shorten_middle(&panel.title, 24), panel.kind.display_name());
                            if ui.selectable_label(selected, label).clicked() {
                                actions.push(QueueAction::Link(role, Some(panel.local_id.clone())));
                            }
                        }
                    });
                ui.horizontal_wrapped(|ui| {
                    let focus_enabled = current.is_some();
                    if ui
                        .add_enabled(focus_enabled, chrome_button("Focus").min_size(Vec2::new(62.0, 26.0)))
                        .clicked()
                        && let Some(panel) = current
                    {
                        actions.push(QueueAction::Focus(panel.local_id.clone()));
                    }
                    if ui
                        .add_enabled(focus_enabled, chrome_button("Brief").min_size(Vec2::new(58.0, 26.0)))
                        .clicked()
                    {
                        actions.push(QueueAction::SendBrief(role));
                    }
                });
            });
        });
}

fn link_label(current: Option<&LinkablePanel>, linked_id: Option<&str>) -> RichText {
    match (current, linked_id) {
        (Some(panel), _) => RichText::new(format!(
            "{} · {}",
            panel.kind.display_name(),
            shorten_middle(&panel.title, 24)
        )),
        (None, Some(_) | None) => RichText::new("Disconnected"),
    }
}

fn link_detail(current: Option<&LinkablePanel>) -> String {
    current.map_or_else(
        || "No linked panel".to_string(),
        |panel| {
            let terminal = if panel.terminal_backed {
                "terminal"
            } else {
                "not terminal"
            };
            format!(
                "{} · panel {} · {terminal}",
                shorten_middle(&panel.workspace_name, 24),
                panel.panel_id.0
            )
        },
    )
}

fn labeled_singleline(ui: &mut Ui, label: &str, value: &mut String) {
    ui.label(RichText::new(label).color(theme::FG_DIM()).size(10.5));
    ui.add(
        TextEdit::singleline(value)
            .desired_width(f32::INFINITY)
            .font(egui::FontId::proportional(12.0)),
    );
}

fn labeled_multiline(ui: &mut Ui, label: &str, value: &mut String, rows: usize) {
    ui.label(RichText::new(label).color(theme::FG_DIM()).size(10.5));
    ui.add(
        TextEdit::multiline(value)
            .desired_rows(rows)
            .desired_width(f32::INFINITY)
            .font(egui::FontId::proportional(12.0)),
    );
}

fn wrapping_label(ui: &mut Ui, text: &str, color: Color32) {
    ui.add(egui::Label::new(RichText::new(text).color(color).size(11.0)).wrap());
}

fn render_list(ui: &mut Ui, label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }

    ui.add_space(6.0);
    ui.label(RichText::new(label).color(theme::FG_DIM()).size(10.5));
    for value in values {
        wrapping_label(ui, &shorten_middle(value, 72), theme::FG_SOFT());
    }
}

fn status_badge(status: WorkItemStatus) -> RichText {
    let color = match status {
        WorkItemStatus::Queued => theme::ACCENT(),
        WorkItemStatus::Dispatched => theme::PALETTE_YELLOW(),
        WorkItemStatus::Done => theme::PALETTE_GREEN(),
        WorkItemStatus::Blocked => theme::PALETTE_RED(),
    };
    RichText::new(status.label())
        .monospace()
        .color(color)
        .size(10.5)
        .strong()
}

fn preset_choice_label(choice: &AgentPresetChoice) -> String {
    format!(
        "{} · {}",
        shorten_middle(&choice.name, 28),
        agent_kind_label(choice.kind)
    )
}
