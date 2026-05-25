use super::{Config, HorizonHome, RuntimeState, SessionOpenDisposition, SessionStore, StartupDecision};
use crate::agent_pair::{AgentPairQueue, AgentPairRole, PerformerWorkReport, WorkItemStatus};

#[test]
fn empty_store_creates_new_session() {
    let root = test_root("empty-store");
    let home = HorizonHome::from_root(root.clone());
    let store = SessionStore::new(home.clone(), home.config_path());

    let decision = store.prepare_startup(&Config::default()).expect("startup decision");

    match decision {
        StartupDecision::Open {
            disposition: SessionOpenDisposition::New,
            session,
        } => {
            assert!(session.runtime_state_path.exists());
            assert!(session.transcript_root.starts_with(root.join("sessions")));
        }
        other => panic!("unexpected decision: {other:?}"),
    }
}

#[test]
fn second_startup_resumes_previous_session() {
    let root = test_root("resume-store");
    let home = HorizonHome::from_root(root.clone());
    let store = SessionStore::new(home.clone(), home.config_path());
    let created = store.create_new_session(&Config::default()).expect("create session");
    store
        .save_runtime_state(&created.session_id, &RuntimeState::from_config(&Config::default()))
        .expect("save state");

    let decision = store.prepare_startup(&Config::default()).expect("startup decision");

    match decision {
        StartupDecision::Open {
            disposition: SessionOpenDisposition::Resume,
            session,
        } => assert_eq!(session.session_id, created.session_id),
        other => panic!("unexpected decision: {other:?}"),
    }
}

#[test]
fn list_profile_sessions_returns_saved_session_summaries() {
    let root = test_root("list-store");
    let home = HorizonHome::from_root(root.clone());
    let store = SessionStore::new(home.clone(), home.config_path());
    let created = store.create_new_session(&Config::default()).expect("create session");

    let sessions = store.list_profile_sessions().expect("list sessions");

    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, created.session_id);
    assert_eq!(sessions[0].label, "Horizon session");
}

#[test]
fn delete_session_removes_saved_state_and_updates_index() {
    let root = test_root("delete-store");
    let home = HorizonHome::from_root(root.clone());
    let store = SessionStore::new(home.clone(), home.config_path());
    let first = store
        .create_new_session(&Config::default())
        .expect("create first session");
    let second = store
        .create_new_session(&Config::default())
        .expect("create second session");

    store.delete_session(&first.session_id).expect("delete session");

    let sessions = store.list_profile_sessions().expect("list sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, second.session_id);
    assert!(!home.session_dir(&first.session_id).exists());

    let decision = store.prepare_startup(&Config::default()).expect("startup decision");
    match decision {
        StartupDecision::Open { session, .. } => assert_eq!(session.session_id, second.session_id),
        other => panic!("unexpected decision: {other:?}"),
    }
}

#[test]
fn delete_session_rejects_live_sessions() {
    let root = test_root("delete-live-store");
    let home = HorizonHome::from_root(root.clone());
    let store = SessionStore::new(home.clone(), home.config_path());
    let created = store.create_new_session(&Config::default()).expect("create session");
    let _lease = store.acquire_lease(&created.session_id).expect("acquire lease");

    let error = store
        .delete_session(&created.session_id)
        .expect_err("live session should not delete");

    assert!(error.to_string().contains("cannot delete live session"));
    assert!(home.session_dir(&created.session_id).exists());
}

#[test]
fn agent_pair_state_defaults_to_empty_when_file_is_missing() {
    let root = test_root("missing-agent-pair");
    let home = HorizonHome::from_root(root);
    let store = SessionStore::new(home.clone(), home.config_path());
    let created = store.create_new_session(&Config::default()).expect("create session");

    let queue = store.load_agent_pair_queue(&created.session_id).expect("load queue");

    assert!(queue.work_items.is_empty());
    assert!(queue.goal.is_empty());
    assert!(queue.researcher.is_none());
    assert!(queue.performer.is_none());
}

#[test]
fn agent_pair_state_persists_goal_links_work_status_report_and_plan() {
    let root = test_root("persist-agent-pair");
    let home = HorizonHome::from_root(root);
    let store = SessionStore::new(home.clone(), home.config_path());
    let created = store.create_new_session(&Config::default()).expect("create session");
    let mut queue = AgentPairQueue::new();
    queue
        .set_goal("Plan a feature with a researcher and performer.")
        .expect("goal");
    queue.set_plan("Research first, queue execution work, then launch a fresh implementation session.");
    queue
        .link_panel(AgentPairRole::Researcher, "researcher-local-id")
        .expect("link researcher");
    queue
        .link_panel(AgentPairRole::Performer, "performer-local-id")
        .expect("link performer");
    let id = queue
        .queue_work_request(
            "Prototype handoff",
            "Create a performer prompt for the first implementation task.",
            "The researcher identified the first slice.",
            vec!["Prompt includes the shared goal.".to_string()],
            vec!["cargo test --workspace".to_string()],
        )
        .expect("work request");
    queue.dispatch_to_performer(&id).expect("dispatch");
    queue
        .complete_work(
            &id,
            PerformerWorkReport {
                summary: "Generated and sent the prompt.".to_string(),
                validation_commands: vec!["cargo test --workspace".to_string()],
                validation_result: "Passed.".to_string(),
                follow_up: "Smoke the plan handoff UI.".to_string(),
            },
        )
        .expect("complete");

    store
        .save_agent_pair_queue(&created.session_id, &queue)
        .expect("save queue");
    let loaded = store.load_agent_pair_queue(&created.session_id).expect("load queue");

    assert_eq!(loaded.goal, "Plan a feature with a researcher and performer.");
    assert!(loaded.plan.contains("launch a fresh implementation session"));
    assert_eq!(
        loaded
            .link_for(AgentPairRole::Researcher)
            .expect("researcher")
            .panel_local_id,
        "researcher-local-id"
    );
    assert_eq!(
        loaded
            .link_for(AgentPairRole::Performer)
            .expect("performer")
            .panel_local_id,
        "performer-local-id"
    );
    let item = loaded.work_item(&id).expect("work item");
    assert_eq!(item.status, WorkItemStatus::Done);
    assert!(item.performer_report.is_some());
}

fn test_root(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("horizon-session-store-{label}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("create temp root");
    root
}
