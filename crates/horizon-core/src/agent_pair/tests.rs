use super::{AgentPairQueue, AgentPairRole, PerformerWorkReport, WorkItemStatus};

fn queue_with_goal() -> AgentPairQueue {
    let mut queue = AgentPairQueue::new();
    queue
        .set_goal("Plan a safer detached-window restore feature.")
        .expect("goal");
    queue
}

fn queue_with_work() -> (AgentPairQueue, String) {
    let mut queue = queue_with_goal();
    let id = queue
        .queue_work_request(
            "Inspect restore path",
            "Trace runtime-state restore and identify where stale detached geometry is replayed.",
            "The researcher saw a snap-back during relaunch.",
            vec!["Name the exact restore seam.".to_string()],
            vec!["cargo test --workspace".to_string()],
        )
        .expect("work");
    (queue, id)
}

fn complete_report() -> PerformerWorkReport {
    PerformerWorkReport {
        summary: "Confirmed the restore path and updated the stale replay guard.".to_string(),
        validation_commands: vec![
            "cargo test --workspace".to_string(),
            "cargo clippy --all-targets --all-features -- -D warnings".to_string(),
        ],
        validation_result: "All validation passed.".to_string(),
        follow_up: "Run native resize smoke on macOS.".to_string(),
    }
}

#[test]
fn research_goal_is_required_before_queueing_work() {
    let mut queue = AgentPairQueue::new();

    let error = queue
        .queue_work_request("Inspect", "Find the seam.", "", Vec::new(), Vec::new())
        .expect_err("goal should be required");

    assert!(error.to_string().contains("research goal"));
    assert!(queue.work_items.is_empty());
}

#[test]
fn researcher_can_queue_performer_work() {
    let (queue, id) = queue_with_work();

    let item = queue.work_item(&id).expect("work item");
    assert_eq!(item.status, WorkItemStatus::Queued);
    assert_eq!(item.requested_by, AgentPairRole::Researcher);
    assert_eq!(item.acceptance_criteria, vec!["Name the exact restore seam."]);
}

#[test]
fn queued_work_can_dispatch_to_linked_performer() {
    let (mut queue, id) = queue_with_work();
    queue
        .link_panel(AgentPairRole::Performer, "performer-panel-local-id")
        .expect("link performer");

    let prompt = queue.dispatch_to_performer(&id).expect("dispatch");

    let item = queue.work_item(&id).expect("work item");
    assert_eq!(item.status, WorkItemStatus::Dispatched);
    assert_eq!(
        item.assigned_performer_panel_local_id.as_deref(),
        Some("performer-panel-local-id")
    );
    assert!(prompt.contains(&format!("Execute work request {id}")));
}

#[test]
fn non_queued_work_cannot_dispatch() {
    let (mut dispatched, dispatched_id) = queue_with_work();
    dispatched
        .link_panel(AgentPairRole::Performer, "performer")
        .expect("link performer");
    dispatched.dispatch_to_performer(&dispatched_id).expect("dispatch");
    assert!(dispatched.dispatch_to_performer(&dispatched_id).is_err());

    let (mut done, done_id) = queue_with_work();
    done.link_panel(AgentPairRole::Performer, "performer")
        .expect("link performer");
    done.dispatch_to_performer(&done_id).expect("dispatch");
    done.complete_work(&done_id, complete_report()).expect("complete");
    assert!(done.dispatch_to_performer(&done_id).is_err());
}

#[test]
fn dispatched_work_can_be_completed_with_report() {
    let (mut queue, id) = queue_with_work();
    queue
        .link_panel(AgentPairRole::Performer, "performer")
        .expect("link performer");
    queue.dispatch_to_performer(&id).expect("dispatch");

    queue.complete_work(&id, complete_report()).expect("complete");

    let item = queue.work_item(&id).expect("work item");
    assert_eq!(item.status, WorkItemStatus::Done);
    assert!(item.performer_report.is_some());
}

#[test]
fn incomplete_report_is_rejected() {
    let (mut queue, id) = queue_with_work();
    queue
        .link_panel(AgentPairRole::Performer, "performer")
        .expect("link performer");
    queue.dispatch_to_performer(&id).expect("dispatch");

    let report = PerformerWorkReport {
        summary: "Changed the guard.".to_string(),
        validation_commands: Vec::new(),
        validation_result: "Passed".to_string(),
        follow_up: String::new(),
    };

    assert!(queue.complete_work(&id, report).is_err());
    assert_eq!(
        queue.work_item(&id).expect("work item").status,
        WorkItemStatus::Dispatched
    );
}

#[test]
fn dispatched_work_can_be_marked_blocked_with_summary() {
    let (mut queue, id) = queue_with_work();
    queue
        .link_panel(AgentPairRole::Performer, "performer")
        .expect("link performer");
    queue.dispatch_to_performer(&id).expect("dispatch");

    queue
        .block_work(
            &id,
            PerformerWorkReport {
                summary: "Cannot reproduce without a native macOS window trace.".to_string(),
                ..PerformerWorkReport::default()
            },
        )
        .expect("block");

    assert_eq!(queue.work_item(&id).expect("work item").status, WorkItemStatus::Blocked);
}

#[test]
fn performer_prompt_includes_goal_and_structured_work_fields() {
    let (queue, id) = queue_with_work();
    let item = queue.work_item(&id).expect("work item");

    let prompt = item.performer_prompt(&queue.goal);

    assert!(prompt.contains(&id));
    assert!(prompt.contains("Plan a safer detached-window restore feature"));
    assert!(prompt.contains("Inspect restore path"));
    assert!(prompt.contains("Trace runtime-state restore"));
    assert!(prompt.contains("The researcher saw a snap-back"));
    assert!(prompt.contains("Name the exact restore seam"));
    assert!(prompt.contains("cargo test --workspace"));
}

#[test]
fn plan_handoff_includes_goal_plan_and_queue_state() {
    let (mut queue, id) = queue_with_work();
    queue.set_plan("1. Inspect restore. 2. Patch stale replay. 3. Smoke macOS.");
    queue
        .link_panel(AgentPairRole::Performer, "performer")
        .expect("link performer");
    queue.dispatch_to_performer(&id).expect("dispatch");
    queue.complete_work(&id, complete_report()).expect("complete");

    let prompt = queue.plan_handoff_prompt();

    assert!(prompt.contains("Plan a safer detached-window restore feature"));
    assert!(prompt.contains("Patch stale replay"));
    assert!(prompt.contains("Inspect restore path"));
    assert!(prompt.contains("done"));
    assert!(prompt.contains("Confirmed the restore path"));
}

#[test]
fn startup_briefs_explain_agent_roles_and_goal() {
    let queue = queue_with_goal();

    let researcher = queue.researcher_brief_prompt();
    let performer = queue.performer_brief_prompt();

    assert!(researcher.contains("Researcher"));
    assert!(researcher.contains("Title:"));
    assert!(researcher.contains("Plan a safer detached-window restore feature"));
    assert!(performer.contains("Performer"));
    assert!(performer.contains("Wait for dispatched work requests"));
}

#[test]
fn panel_links_use_stable_panel_local_ids() {
    let mut queue = AgentPairQueue::new();

    queue
        .link_panel(AgentPairRole::Researcher, "researcher-local-id")
        .expect("link researcher");
    queue
        .link_panel(AgentPairRole::Performer, "performer-local-id")
        .expect("link performer");

    assert_eq!(
        queue
            .link_for(AgentPairRole::Researcher)
            .expect("researcher")
            .panel_local_id,
        "researcher-local-id"
    );
    assert_eq!(
        queue
            .link_for(AgentPairRole::Performer)
            .expect("performer")
            .panel_local_id,
        "performer-local-id"
    );
}
