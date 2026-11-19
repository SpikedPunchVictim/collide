//! What a run's agent creates (0.10, Follow-ups; 0.2, Who is asking): an item it files goes to the board's private
//! root, in the folder of its run's task's epic, with a `raised-by` link to that task and the run id in `run`. The
//! agent's own word on any of that is replaced, not trusted. This is the pure shaping; the daemon finds the run's task.

use collide_core::provider::ApplyParams;
use serde_json::json;

/// The task a run works on, as far as the rule needs it: found in the planning source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTask {
    pub id: String,
    /// The epic the task is part of, when it has one.
    pub epic: Option<String>,
}

/// The kinds that are not made in an epic: the provider refuses an `epic` for them, so it is left as sent.
const WITHOUT_EPIC: [&str; 2] = ["epic", "spike"];

/// Rewrites a `create` made by the agent of `run`.
///
/// - `root` is `private`, and `checkout` is dropped: the private root is the board's, written from the main checkout.
/// - `run` is the run's id, whatever the agent sent.
/// - `raisedBy` is the run's task when it is in the planning source, and is dropped otherwise, whatever was sent.
/// - `epic` is the run's task's epic, for a kind that has one, when the task has one.
pub fn shape_create(apply: &mut ApplyParams, run: &str, task: Option<&RunTask>) {
    apply.checkout = None;
    apply.args.insert("root".to_string(), json!("private"));
    apply.args.insert("run".to_string(), json!(run));
    apply.args.remove("raisedBy");
    let Some(task) = task else {
        return;
    };
    apply.args.insert("raisedBy".to_string(), json!(task.id));
    let kind = apply
        .args
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("task");
    if let (Some(epic), false) = (&task.epic, WITHOUT_EPIC.contains(&kind)) {
        apply.args.insert("epic".to_string(), json!(epic));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::collections::BTreeMap;

    fn create(args: Value) -> ApplyParams {
        ApplyParams {
            actor: None,
            op: "create".to_string(),
            id: None,
            args: serde_json::from_value::<BTreeMap<String, Value>>(args).unwrap(),
            version: None,
            checkout: Some("/elsewhere".to_string()),
        }
    }

    fn task() -> RunTask {
        RunTask {
            id: "ts-0002-bb".to_string(),
            epic: Some("ep-002-bb".to_string()),
        }
    }

    #[test]
    fn the_agents_root_checkout_run_and_raised_by_are_replaced_and_the_epic_is_the_tasks() {
        let mut apply = create(json!({
            "title": "t", "epic": "ep-001-aa", "root": "shared", "run": "forged", "raisedBy": "ts-0001-aa"
        }));
        shape_create(&mut apply, "R-1", Some(&task()));
        assert_eq!(
            serde_json::to_value(&apply.args).unwrap(),
            json!({
                "title": "t", "epic": "ep-002-bb", "root": "private", "run": "R-1", "raisedBy": "ts-0002-bb"
            })
        );
        assert_eq!(apply.checkout, None);
    }

    #[test]
    fn without_a_task_only_the_run_is_written_and_a_sent_raised_by_is_dropped() {
        let mut apply =
            create(json!({"title": "t", "epic": "ep-001-aa", "raisedBy": "ts-0001-aa"}));
        shape_create(&mut apply, "R-1", None);
        assert_eq!(
            serde_json::to_value(&apply.args).unwrap(),
            json!({"title": "t", "epic": "ep-001-aa", "root": "private", "run": "R-1"})
        );
    }

    #[test]
    fn a_kind_made_outside_an_epic_keeps_the_epic_it_was_sent() {
        for kind in ["epic", "spike"] {
            let mut apply = create(json!({"title": "t", "kind": kind}));
            shape_create(&mut apply, "R-1", Some(&task()));
            assert_eq!(apply.args.get("epic"), None, "{kind}");
        }
    }

    #[test]
    fn a_task_with_no_epic_leaves_the_epic_as_sent() {
        let mut apply = create(json!({"title": "t", "epic": "ep-001-aa"}));
        let bare = RunTask {
            id: "ep-002-bb".to_string(),
            epic: None,
        };
        shape_create(&mut apply, "R-1", Some(&bare));
        assert_eq!(apply.args.get("epic"), Some(&json!("ep-001-aa")));
    }
}
