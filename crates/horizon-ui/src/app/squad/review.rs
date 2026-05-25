use std::fmt::Write as _;
use std::path::Path;

use horizon_core::{PerformerReport, SquadRun, WorkStatus, WorktreeManager};

pub(super) struct SlotReviewContext {
    pub slot_id: String,
    pub title: String,
    pub report: PerformerReport,
    pub diff: String,
}

pub(super) fn collect_review_contexts(run: &SquadRun) -> horizon_core::Result<Vec<SlotReviewContext>> {
    run.performers
        .iter()
        .map(|slot| {
            let diff = WorktreeManager::diff(&slot.scratch)?;
            Ok(SlotReviewContext {
                slot_id: slot.id.clone(),
                title: slot.work_item.title.clone(),
                report: slot.report.clone().unwrap_or_default(),
                diff,
            })
        })
        .collect()
}

pub(super) fn apply_slot_diffs(contexts: &[SlotReviewContext], primary_worktree: &Path) -> horizon_core::Result<()> {
    for context in contexts {
        WorktreeManager::apply_to(&context.diff, primary_worktree)?;
    }
    Ok(())
}

pub(super) fn reviewer_prompt(run: &SquadRun, contexts: &[SlotReviewContext]) -> String {
    let mut prompt = format!(
        "You are the Agent Squad reviewer for run {run_id}.\n\nGoal:\n{goal}\n\nResearcher plan:\n{plan}\n\nReview the consolidated changes in this checkout and produce final review notes.\n",
        run_id = short_run_id(&run.id),
        goal = run.goal,
        plan = if run.plan_text.trim().is_empty() {
            "(no plan text recorded)"
        } else {
            run.plan_text.as_str()
        },
    );

    for context in contexts {
        prompt.push_str("\n---\n");
        let _ = writeln!(prompt, "Slot {}: {}", context.slot_id, context.title);
        let _ = writeln!(prompt, "Summary: {}", empty_label(&context.report.summary));
        let validation_commands = if context.report.validation_commands.is_empty() {
            "(none)".to_string()
        } else {
            context.report.validation_commands.join(", ")
        };
        let _ = writeln!(prompt, "Validation commands: {validation_commands}");
        let _ = writeln!(
            prompt,
            "Validation result: {}",
            empty_label(&context.report.validation_result)
        );
        if !context.report.follow_up.trim().is_empty() {
            let _ = writeln!(prompt, "Follow-up: {}", context.report.follow_up.trim());
        }
        prompt.push_str("Diff:\n");
        if context.diff.trim().is_empty() {
            prompt.push_str("(empty diff)\n");
        } else {
            prompt.push_str(&context.diff);
            if !context.diff.ends_with('\n') {
                prompt.push('\n');
            }
        }
    }

    prompt.push_str("\nWhen finished, report issues found, validation performed, and final merge recommendation.\n");
    prompt
}

pub(super) fn blocked_slots(run: &SquadRun) -> Vec<&str> {
    run.performers
        .iter()
        .filter(|slot| slot.work_item.status == WorkStatus::Blocked)
        .map(|slot| slot.id.as_str())
        .collect()
}

pub(super) fn ready_for_blocked_decision(run: &SquadRun) -> bool {
    let has_blocked = run
        .performers
        .iter()
        .any(|slot| slot.work_item.status == WorkStatus::Blocked);
    has_blocked
        && run
            .performers
            .iter()
            .all(|slot| matches!(slot.work_item.status, WorkStatus::Done | WorkStatus::Blocked))
}

pub(super) fn ready_for_review(run: &SquadRun) -> bool {
    !run.performers.is_empty()
        && run
            .performers
            .iter()
            .all(|slot| slot.work_item.status == WorkStatus::Done)
}

fn empty_label(value: &str) -> &str {
    if value.trim().is_empty() {
        "(none)"
    } else {
        value.trim()
    }
}

fn short_run_id(run_id: &str) -> String {
    format!("#{}", run_id.get(..4).unwrap_or(run_id))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use horizon_core::{PanelKind, PerformerReport, PerformerSlot, SquadRun, WorkItem};

    use super::{ready_for_blocked_decision, ready_for_review, reviewer_prompt};

    #[test]
    fn readiness_requires_all_done_and_no_blocked_slots() {
        let mut run = SquadRun::new("run-1234", "Fix issues", 1);
        run.queue_plan("Plan", vec![slot("s1"), slot("s2")]);

        run.mark_slot_done("s1", PerformerReport::default()).unwrap();
        run.mark_slot_blocked("s2", "needs fixture").unwrap();

        assert!(!ready_for_review(&run));
        assert!(ready_for_blocked_decision(&run));
    }

    #[test]
    fn reviewer_prompt_includes_each_slot_report_and_diff() {
        let mut run = SquadRun::new("run-1234", "Fix issues", 1);
        run.plan_text = "1. Do s1".to_string();
        let contexts = vec![super::SlotReviewContext {
            slot_id: "s1".to_string(),
            title: "Task s1".to_string(),
            report: horizon_core::PerformerReport {
                summary: "Changed parser".to_string(),
                validation_commands: vec!["cargo test".to_string()],
                validation_result: "passed".to_string(),
                follow_up: String::new(),
            },
            diff: "diff --git a/file b/file\n+changed\n".to_string(),
        }];

        let prompt = reviewer_prompt(&run, &contexts);

        assert!(prompt.contains("Changed parser"));
        assert!(prompt.contains("cargo test"));
        assert!(prompt.contains("+changed"));
    }

    fn slot(id: &str) -> PerformerSlot {
        PerformerSlot::new(
            id,
            WorkItem::new(id, format!("Task {id}"), "Do the thing"),
            PanelKind::Codex,
            PathBuf::from(format!("/tmp/squad/{id}")),
        )
    }
}
