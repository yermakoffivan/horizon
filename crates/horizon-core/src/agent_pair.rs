#[cfg(test)]
mod tests;

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{Error, Result};

const AGENT_PAIR_QUEUE_VERSION: u32 = 2;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct AgentPairQueue {
    pub version: u32,
    pub goal: String,
    pub plan: String,
    pub researcher: Option<AgentPanelLink>,
    pub performer: Option<AgentPanelLink>,
    pub work_items: Vec<AgentWorkItem>,
}

impl Default for AgentPairQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentPairQueue {
    #[must_use]
    pub fn new() -> Self {
        Self {
            version: AGENT_PAIR_QUEUE_VERSION,
            goal: String::new(),
            plan: String::new(),
            researcher: None,
            performer: None,
            work_items: Vec::new(),
        }
    }

    pub fn normalize(&mut self) {
        if self.version == 0 {
            self.version = AGENT_PAIR_QUEUE_VERSION;
        }
    }

    /// Set the shared research or planning goal for the pair.
    ///
    /// # Errors
    ///
    /// Returns an error when the goal is blank.
    pub fn set_goal(&mut self, goal: impl Into<String>) -> Result<()> {
        self.goal = normalized_required(&goal.into(), "research goal")?;
        Ok(())
    }

    pub fn set_plan(&mut self, plan: impl Into<String>) {
        self.plan = plan.into().trim().to_string();
    }

    #[must_use]
    pub fn link_for(&self, role: AgentPairRole) -> Option<&AgentPanelLink> {
        match role {
            AgentPairRole::Researcher => self.researcher.as_ref(),
            AgentPairRole::Performer => self.performer.as_ref(),
        }
    }

    /// Link a stable panel identity to one side of the agent pair.
    ///
    /// # Errors
    ///
    /// Returns an error when `panel_local_id` is blank.
    pub fn link_panel(&mut self, role: AgentPairRole, panel_local_id: impl Into<String>) -> Result<()> {
        let panel_local_id = panel_local_id.into();
        if panel_local_id.trim().is_empty() {
            return Err(Error::State("panel link requires a stable local id".to_string()));
        }

        let link = AgentPanelLink::new(role, panel_local_id);
        match role {
            AgentPairRole::Researcher => self.researcher = Some(link),
            AgentPairRole::Performer => self.performer = Some(link),
        }
        Ok(())
    }

    pub fn unlink_panel(&mut self, role: AgentPairRole) {
        match role {
            AgentPairRole::Researcher => self.researcher = None,
            AgentPairRole::Performer => self.performer = None,
        }
    }

    /// Queue a performer work request produced by the researcher.
    ///
    /// # Errors
    ///
    /// Returns an error when the goal, title, or request body is blank.
    pub fn queue_work_request(
        &mut self,
        title: impl Into<String>,
        request: impl Into<String>,
        context: impl Into<String>,
        acceptance_criteria: Vec<String>,
        suggested_commands: Vec<String>,
    ) -> Result<String> {
        if self.goal.trim().is_empty() {
            return Err(Error::State(
                "set a research goal before queueing performer work".to_string(),
            ));
        }

        let title = normalized_required(&title.into(), "work title")?;
        let request = normalized_required(&request.into(), "work request")?;
        let now = current_unix_millis();
        let id = Uuid::new_v4().to_string();
        self.work_items.push(AgentWorkItem {
            id: id.clone(),
            title,
            request,
            context: context.into().trim().to_string(),
            acceptance_criteria: normalize_lines(acceptance_criteria),
            suggested_commands: normalize_lines(suggested_commands),
            status: WorkItemStatus::Queued,
            requested_by: AgentPairRole::Researcher,
            assigned_performer_panel_local_id: None,
            performer_report: None,
            created_at_millis: now,
            updated_at_millis: now,
        });
        Ok(id)
    }

    /// Generate the performer handoff and mark queued work as dispatched.
    ///
    /// # Errors
    ///
    /// Returns an error when no performer is linked, the work item does not
    /// exist, or the work item is not queued.
    pub fn dispatch_to_performer(&mut self, work_item_id: &str) -> Result<String> {
        let performer = self
            .performer
            .as_ref()
            .ok_or_else(|| Error::State("no performer panel is linked".to_string()))?
            .panel_local_id
            .clone();
        let goal = self.goal.clone();
        let item = self.work_item_mut(work_item_id)?;
        if item.status != WorkItemStatus::Queued {
            return Err(Error::State(format!(
                "only queued work can be dispatched; {} is {}",
                item.id,
                item.status.label()
            )));
        }

        let prompt = item.performer_prompt(&goal);
        item.status = WorkItemStatus::Dispatched;
        item.assigned_performer_panel_local_id = Some(performer);
        item.updated_at_millis = current_unix_millis();
        Ok(prompt)
    }

    /// Attach a performer report and mark dispatched work as done.
    ///
    /// # Errors
    ///
    /// Returns an error when the report is incomplete, the work item does not
    /// exist, or the work item is not dispatched.
    pub fn complete_work(&mut self, work_item_id: &str, report: PerformerWorkReport) -> Result<()> {
        if !report.is_complete() {
            return Err(Error::State("performer report is incomplete".to_string()));
        }

        let item = self.work_item_mut(work_item_id)?;
        if item.status != WorkItemStatus::Dispatched {
            return Err(Error::State(format!(
                "only dispatched work can be completed; {} is {}",
                item.id,
                item.status.label()
            )));
        }

        item.performer_report = Some(report);
        item.status = WorkItemStatus::Done;
        item.updated_at_millis = current_unix_millis();
        Ok(())
    }

    /// Attach a performer report and mark dispatched work as blocked.
    ///
    /// # Errors
    ///
    /// Returns an error when the report lacks a summary, the work item does not
    /// exist, or the work item is not dispatched.
    pub fn block_work(&mut self, work_item_id: &str, mut report: PerformerWorkReport) -> Result<()> {
        if report.summary.trim().is_empty() {
            return Err(Error::State("blocked work requires a performer summary".to_string()));
        }

        let item = self.work_item_mut(work_item_id)?;
        if item.status != WorkItemStatus::Dispatched {
            return Err(Error::State(format!(
                "only dispatched work can be blocked; {} is {}",
                item.id,
                item.status.label()
            )));
        }

        report.summary = report.summary.trim().to_string();
        item.performer_report = Some(report);
        item.status = WorkItemStatus::Blocked;
        item.updated_at_millis = current_unix_millis();
        Ok(())
    }

    #[must_use]
    pub fn work_item(&self, work_item_id: &str) -> Option<&AgentWorkItem> {
        self.work_items.iter().find(|item| item.id == work_item_id)
    }

    #[must_use]
    pub fn researcher_brief_prompt(&self) -> String {
        format!(
            "You are the Researcher in a Horizon paired-agent workflow.\n\nResearch goal:\n{}\n\nResearch, plan, and break the goal into concrete performer work requests. Do not edit files directly from this role. When execution is needed, produce a queue-ready work request with these fields:\n\nTitle:\nRequest:\nContext:\nAcceptance criteria:\nSuggested commands:\n\nKeep the plan current as new information appears.",
            goal_or_placeholder(&self.goal),
        )
    }

    #[must_use]
    pub fn performer_brief_prompt(&self) -> String {
        format!(
            "You are the Performer in a Horizon paired-agent workflow.\n\nResearch goal:\n{}\n\nWait for dispatched work requests from the Horizon Agent Pair queue. For each request, verify it is credible and relevant before editing. Report what changed, validation commands, validation result, and any follow-up work.",
            goal_or_placeholder(&self.goal),
        )
    }

    #[must_use]
    pub fn plan_handoff_prompt(&self) -> String {
        let mut prompt = format!(
            "Start a fresh agent session from this Horizon Agent Pair plan.\n\nResearch goal:\n{}\n\nPlan:\n{}\n",
            goal_or_placeholder(&self.goal),
            text_or_placeholder(&self.plan, "No plan has been written yet."),
        );

        if !self.work_items.is_empty() {
            prompt.push_str("\nQueue state:\n");
            for item in &self.work_items {
                let _ = writeln!(
                    prompt,
                    "- [{}] {} ({})",
                    item.status.label(),
                    item.title,
                    shorten_id(&item.id)
                );
                let _ = writeln!(prompt, "  Request: {}", item.request);
                if let Some(report) = &item.performer_report {
                    let _ = writeln!(prompt, "  Performer report: {}", report.summary);
                }
            }
        }

        prompt.push_str("\nUse this as the starting context. Re-verify assumptions before editing.");
        prompt
    }

    fn work_item_mut(&mut self, work_item_id: &str) -> Result<&mut AgentWorkItem> {
        self.work_items
            .iter_mut()
            .find(|item| item.id == work_item_id)
            .ok_or_else(|| Error::State(format!("work item {work_item_id} was not found")))
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPairRole {
    Researcher,
    Performer,
}

impl AgentPairRole {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Researcher => "Researcher",
            Self::Performer => "Performer",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct AgentPanelLink {
    pub role: AgentPairRole,
    pub panel_local_id: String,
}

impl AgentPanelLink {
    #[must_use]
    pub fn new(role: AgentPairRole, panel_local_id: String) -> Self {
        Self { role, panel_local_id }
    }
}

impl Default for AgentPanelLink {
    fn default() -> Self {
        Self {
            role: AgentPairRole::Researcher,
            panel_local_id: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct AgentWorkItem {
    pub id: String,
    pub title: String,
    pub request: String,
    pub context: String,
    pub acceptance_criteria: Vec<String>,
    pub suggested_commands: Vec<String>,
    pub status: WorkItemStatus,
    pub requested_by: AgentPairRole,
    pub assigned_performer_panel_local_id: Option<String>,
    pub performer_report: Option<PerformerWorkReport>,
    pub created_at_millis: i64,
    pub updated_at_millis: i64,
}

impl AgentWorkItem {
    #[must_use]
    pub fn performer_prompt(&self, goal: &str) -> String {
        format!(
            "Execute work request {} from the Horizon Agent Pair queue.\n\nResearch goal: {}\nTitle: {}\nRequest: {}\nContext from researcher: {}\nAcceptance criteria: {}\nSuggested commands: {}\n\nVerify the request first. Execute only after confirming it is credible and relevant. Report what changed, validation commands, validation result, and any follow-up work before marking the work done.",
            self.id,
            goal_or_placeholder(goal),
            self.title,
            self.request,
            text_or_placeholder(&self.context, "None provided"),
            format_list(&self.acceptance_criteria),
            format_list(&self.suggested_commands),
        )
    }

    #[must_use]
    pub fn assignment_label(&self, performer_title: Option<&str>) -> String {
        match self.status {
            WorkItemStatus::Queued => "Queued for Performer".to_string(),
            WorkItemStatus::Dispatched => performer_title.map_or_else(
                || "Dispatched to linked performer".to_string(),
                |title| format!("Dispatched to {title}"),
            ),
            WorkItemStatus::Done => "Done".to_string(),
            WorkItemStatus::Blocked => "Blocked".to_string(),
        }
    }
}

impl Default for AgentWorkItem {
    fn default() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            request: String::new(),
            context: String::new(),
            acceptance_criteria: Vec::new(),
            suggested_commands: Vec::new(),
            status: WorkItemStatus::Queued,
            requested_by: AgentPairRole::Researcher,
            assigned_performer_panel_local_id: None,
            performer_report: None,
            created_at_millis: 0,
            updated_at_millis: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkItemStatus {
    #[default]
    Queued,
    Dispatched,
    Done,
    Blocked,
}

impl WorkItemStatus {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Dispatched => "dispatched",
            Self::Done => "done",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(default)]
pub struct PerformerWorkReport {
    pub summary: String,
    pub validation_commands: Vec<String>,
    pub validation_result: String,
    pub follow_up: String,
}

impl PerformerWorkReport {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.summary.trim().is_empty()
            && self
                .validation_commands
                .iter()
                .any(|command| !command.trim().is_empty())
            && !self.validation_result.trim().is_empty()
    }
}

fn normalized_required(value: &str, field: &str) -> Result<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(Error::State(format!("{field} cannot be empty")));
    }
    Ok(trimmed.to_string())
}

fn normalize_lines(lines: Vec<String>) -> Vec<String> {
    lines
        .into_iter()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn format_list(values: &[String]) -> String {
    if values.is_empty() {
        "None listed".to_string()
    } else {
        values.join(", ")
    }
}

fn goal_or_placeholder(goal: &str) -> &str {
    text_or_placeholder(goal, "No research goal has been set.")
}

fn text_or_placeholder<'a>(value: &'a str, placeholder: &'a str) -> &'a str {
    let trimmed = value.trim();
    if trimmed.is_empty() { placeholder } else { trimmed }
}

fn shorten_id(id: &str) -> String {
    id.chars().take(8).collect()
}

fn current_unix_millis() -> i64 {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    i64::try_from(millis).unwrap_or(i64::MAX)
}
