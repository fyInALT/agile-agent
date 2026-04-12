use crate::backlog::TaskItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionDecision {
    ReadyForVerification,
    Incomplete { reason: String },
}

pub fn continuation_prompt(text: &str) -> Option<String> {
    let lowered = text.to_lowercase();

    if text.trim().is_empty() {
        return Some("Continue the task. No meaningful result was produced yet.".to_string());
    }

    if contains_any(
        &lowered,
        &[
            "not tested",
            "have not tested",
            "haven't tested",
            "need to test",
            "should test",
            "remaining",
            "next step",
            "next steps",
            "still need",
            "still have to",
            "follow-up",
            "todo",
        ],
    ) {
        return Some(
            "Continue the same task. Finish the remaining implementation and testing work before stopping."
                .to_string(),
        );
    }

    None
}

pub fn judge_completion(task: &TaskItem, text: &str) -> CompletionDecision {
    if text.trim().is_empty() {
        return CompletionDecision::Incomplete {
            reason: "provider returned no assistant result".to_string(),
        };
    }

    if continuation_prompt(text).is_some() {
        return CompletionDecision::Incomplete {
            reason: "assistant output still implies remaining work".to_string(),
        };
    }

    if task.objective.trim().is_empty() {
        return CompletionDecision::Incomplete {
            reason: "task objective is empty".to_string(),
        };
    }

    if task.verification_plan.is_empty() {
        return CompletionDecision::Incomplete {
            reason: "task is missing a verification plan".to_string(),
        };
    }

    CompletionDecision::ReadyForVerification
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

#[cfg(test)]
mod tests {
    use super::CompletionDecision;
    use super::continuation_prompt;
    use super::judge_completion;
    use crate::backlog::TaskItem;
    use crate::backlog::TaskStatus;

    fn task() -> TaskItem {
        TaskItem {
            id: "task-1".to_string(),
            todo_id: "todo-1".to_string(),
            objective: "write tests".to_string(),
            scope: ".".to_string(),
            constraints: Vec::new(),
            verification_plan: vec!["cargo check passes".to_string()],
            status: TaskStatus::Running,
            result_summary: None,
        }
    }

    #[test]
    fn continuation_detects_unfinished_testing() {
        assert!(continuation_prompt("I made the change but have not tested it yet.").is_some());
    }

    #[test]
    fn completion_marks_done_message_complete() {
        assert_eq!(
            judge_completion(&task(), "Implemented the change and completed the work."),
            CompletionDecision::ReadyForVerification
        );
    }

    #[test]
    fn completion_requires_verification_plan() {
        let mut task = task();
        task.verification_plan.clear();

        assert_eq!(
            judge_completion(&task, "Implemented the change and completed the work."),
            CompletionDecision::Incomplete {
                reason: "task is missing a verification plan".to_string()
            }
        );
    }
}
