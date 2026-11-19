//! The roles of the built-in `collide` pack that the daemon runs itself (0.4, The built-in `collide` pack; ts-0065-kx).
//! The pack's manifest names the role `builder` and ships no role files until the pack's own files come (Stage 3), so
//! the one role a file-task run needs is compiled in here: its definition, which is the prompt, and its [`Role`].
//!
//! The role's model is an alias, resolved per user from `models.yaml` ([`BUILDER_MODEL_ALIAS`]). Nothing else is set:
//! no budget, no environment, and the context options and the config sources are the tool's own defaults.

use collide_core::pack::Role;

/// The role as a workflow names it, `pack/role`.
pub const BUILDER_ROLE_NAME: &str = "collide/builder";

/// The model alias of the builder when the caller names none. The plan's example pack gives its builder `fast` and its
/// reviewer `strong`, so the builder is the cheaper of the two (the owner may change it).
pub const BUILDER_MODEL_ALIAS: &str = "fast";

/// The builder's agent definition: a Markdown file whose front matter has a `description` and whose body is the
/// agent's system prompt. It is the text of the `agents.json` entry of the phase's snapshot.
pub const BUILDER_DEFINITION: &str = "---
description: Builds one task in its own worktree.
---
You are the builder. The first message is your task. Do it in this worktree, which is a git checkout on a branch of
its own, and nowhere else.

Work in small steps. Read the code around a change before you make it, follow the conventions you find, and run the
project's own tests and checks before you say you are done. Commit your work to this branch in commits that each build
and pass. Do not push, do not touch another branch, and do not change anything outside this worktree.

If you find something outside this task, or leave part of it undone, file it as a follow-up with `collide task create`
and a short title, one item for each, and do not leave it only in your last reply. The item is private: the person
decides whether to keep it. `collide task new` makes a scratch task, which is not a follow-up. `collide guide` lists
the commands.

When you are done, end with a short report: what you changed, what you ran and what it said, and anything you left
undone and why.
";

/// The `collide/builder` role.
pub fn builder_role() -> Role {
    serde_json::from_value(serde_json::json!({
        "agent": "claude",
        "model": BUILDER_MODEL_ALIAS,
    }))
    .expect("the built-in builder role is a valid role")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builder_runs_on_claude_with_the_default_alias_and_nothing_else_set() {
        let role = builder_role();
        assert_eq!(role.agent, "claude");
        assert_eq!(role.model.as_deref(), Some(BUILDER_MODEL_ALIAS));
        assert!(role.env.is_empty() && role.budget.is_none() && role.compact_at.is_none());
    }

    #[test]
    fn the_definition_tells_the_builder_to_file_follow_ups_with_collide_task_create() {
        assert!(BUILDER_DEFINITION.contains("`collide task create`"));
    }

    #[test]
    fn the_definition_says_a_scratch_task_is_no_follow_up() {
        assert!(BUILDER_DEFINITION.contains("`collide task new`"));
    }

    #[test]
    fn the_definition_keeps_the_final_report_as_the_place_for_what_was_left_undone() {
        let flat = BUILDER_DEFINITION
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(flat.contains("anything you left undone and why"));
    }

    #[test]
    fn the_definition_is_wrapped_at_120_columns() {
        assert!(
            BUILDER_DEFINITION
                .lines()
                .all(|line| line.chars().count() <= 120)
        );
    }

    #[test]
    fn the_definition_has_a_description_in_its_front_matter() {
        let mut lines = BUILDER_DEFINITION.lines();
        assert_eq!(lines.next(), Some("---"));
        assert!(lines.next().unwrap().starts_with("description: "));
        assert_eq!(lines.next(), Some("---"));
    }
}
