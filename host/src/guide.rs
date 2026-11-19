//! `collide guide` (ts-0140-gk): a short guide for an agent that runs in the user's own terminal, or in a run. It is
//! plain text, needs no daemon and writes nothing. It holds no path, no task text and no setting of a board: it is a
//! function of the CLI's own tables, so it is the same on every machine.
//!
//! The commands it lists are looked up in [`crate::commands::COMMANDS`], and the forms of `collide task` it names are
//! each an example that [`crate::task_cli::parse_task`] reads, so a command that leaves the tree or a form that leaves
//! the parser fails a test.

use crate::auth::PersonOnlyOp;
use crate::commands;
use crate::task_cli::{Command as Parsed, exit};

/// The commands the guide lists, in order. Each is looked up in [`crate::commands::COMMANDS`].
pub const LISTED_COMMANDS: [&str; 3] = ["run", "tasks", "task"];

/// One form of `collide task`: the line the guide prints, words after `collide task` that the parser reads as that
/// form, and what the form is.
pub struct Form {
    pub usage: &'static str,
    pub example: &'static [&'static str],
    pub is_read_as: fn(&Parsed) -> bool,
    pub meaning: &'static str,
}

/// The forms of `collide task` the guide names, in order.
pub const TASK_FORMS: [Form; 4] = [
    Form {
        usage: "collide task create <title> [epic=<epic>] [kind=<kind>]",
        example: &["create", "A title", "epic=ep-2"],
        is_read_as: |parsed| matches!(parsed, Parsed::Create(_)),
        meaning: "makes an item of the plan, in a file. This is how you file a follow-up.",
    },
    Form {
        usage: "collide task new <prompt>",
        example: &["new", "A prompt"],
        is_read_as: |parsed| matches!(parsed, Parsed::New(_)),
        meaning: "makes a scratch task in Collide's store, in no file. It is not a follow-up.",
    },
    Form {
        usage: "collide task <id>",
        example: &["12"],
        is_read_as: |parsed| matches!(parsed, Parsed::Task(task) if task.op.is_none()),
        meaning: "shows one item.",
    },
    Form {
        usage: "collide task <id> link <target> type=<type>",
        example: &["12", "link", "13", "type=blocked-by"],
        is_read_as: |parsed| matches!(parsed, Parsed::Task(task) if task.op.as_deref() == Some("link")),
        meaning: "links one item to another.",
    },
];

/// The ops only a person does: the daemon refuses them to an agent (0.10).
pub const PERSON_ONLY_OPS: [PersonOnlyOp; 5] = [
    PersonOnlyOp::Accept,
    PersonOnlyOp::Defer,
    PersonOnlyOp::Reject,
    PersonOnlyOp::Resolve,
    PersonOnlyOp::Reopen,
];

/// What `collide guide` prints: plain text, lines of at most 120 columns, the same on every machine.
pub fn text() -> String {
    let mut text = String::from(
        "Collide: a guide for an agent\n\
         \n\
         Collide plans and runs software work. The plan is a set of items (epics, tasks, proposals, spikes and\n\
         questions) kept beside the code. You work with it through the `collide` command. A person may have the\n\
         Collide app open: it shows each change as it lands. The commands start the daemon when none runs.\n\
         \n\
         Commands you may use:\n",
    );
    for name in LISTED_COMMANDS {
        let summary =
            commands::find(name).map_or("(no longer a command)", |command| command.summary);
        text.push_str(&format!("  collide {name}: {summary}\n"));
    }
    text.push_str(
        "\nForms of `collide task` (<id> is an item's key or a short form such as 12):\n",
    );
    for form in &TASK_FORMS {
        text.push_str(&format!("  {}\n      {}\n", form.usage, form.meaning));
    }
    text.push_str(&format!(
        "\nExit codes, besides 0 for ok:\n\
         \x20 {view} a view has an error of its own: `collide tasks` lists no task and prints the error.\n\
         \x20 {queued} a write is queued, because its checkout is busy (a rebase, a merge, a live run). It is not\n\
         \x20    applied yet. Do not repeat it.\n\
         \x20 {confirm} a person-only op: nothing was written. A confirmation waits for the person in the app.\n\
         Any other code beyond 0 is a failure: `collide --help` lists them.\n",
        view = exit::VIEW_FAILED,
        queued = exit::QUEUED,
        confirm = exit::CONFIRMATION,
    ));
    let ops: Vec<&str> = PERSON_ONLY_OPS.iter().map(|op| op.name()).collect();
    text.push_str(&format!(
        "\nOnly a person does these ops: {}.\n\
         `collide task <id> <op>` with one of them writes nothing and exits {}. The person answers in the app.\n\
         Say so in your reply, and go on; do not retry it.\n\
         \n\
         In a Collide run, an item you create is private to the person, who decides whether to keep it, and\n\
         `collide run` is refused: a run's agent does not start runs.\n",
        ops.join(", "),
        exit::CONFIRMATION,
    ));
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_cli::parse_task;

    fn words(example: &[&str]) -> Vec<String> {
        example.iter().map(|word| word.to_string()).collect()
    }

    #[test]
    fn every_command_the_guide_lists_is_in_the_tree_and_the_guide_gives_its_summary() {
        let text = text();
        for name in LISTED_COMMANDS {
            let command = commands::find(name).unwrap_or_else(|| panic!("`{name}` left COMMANDS"));
            assert!(
                text.contains(&format!("collide {name}: {}", command.summary)),
                "{name} in {text}"
            );
        }
    }

    #[test]
    fn the_guide_lists_the_three_commands_the_plan_names() {
        assert_eq!(LISTED_COMMANDS, ["run", "tasks", "task"]);
    }

    #[test]
    fn every_form_of_task_the_guide_names_is_read_by_the_parser_as_that_form() {
        for form in &TASK_FORMS {
            let parsed = parse_task(&words(form.example))
                .unwrap_or_else(|error| panic!("{}: {}", form.usage, error.message));
            assert!((form.is_read_as)(&parsed), "{}: {parsed:?}", form.usage);
        }
    }

    #[test]
    fn a_form_whose_example_is_the_other_form_is_caught() {
        // The check above would pass for any parseable example: `create` read as a show of an item is not a create.
        let parsed = parse_task(&words(&["12"])).unwrap();
        assert!(!(TASK_FORMS[0].is_read_as)(&parsed));
        assert!(!(TASK_FORMS[3].is_read_as)(&parsed));
    }

    #[test]
    fn the_guide_prints_each_form_with_its_meaning() {
        let text = text();
        for form in &TASK_FORMS {
            assert!(text.contains(form.usage), "{} in {text}", form.usage);
            assert!(text.contains(form.meaning), "{} in {text}", form.meaning);
        }
    }

    #[test]
    fn the_guide_gives_the_three_exit_codes_as_the_cli_exits_with_them() {
        let text = text();
        for code in [exit::VIEW_FAILED, exit::QUEUED, exit::CONFIRMATION] {
            assert!(
                text.lines()
                    .any(|line| line.trim_start().starts_with(&format!("{code} "))),
                "{code} in {text}"
            );
        }
        assert_eq!(
            [exit::VIEW_FAILED, exit::QUEUED, exit::CONFIRMATION],
            [10, 11, 12]
        );
    }

    #[test]
    fn the_guide_names_the_five_person_only_ops_and_says_they_wait_in_the_app() {
        let names: Vec<&str> = PERSON_ONLY_OPS.iter().map(|op| op.name()).collect();
        assert_eq!(names, ["accept", "defer", "reject", "resolve", "reopen"]);
        let text = text();
        assert!(text.contains(&names.join(", ")), "{text}");
        assert!(text.contains("in the app"), "{text}");
    }

    #[test]
    fn the_guide_is_plain_text_with_no_path_in_it_and_lines_of_at_most_120_columns() {
        let text = text();
        assert!(!text.contains('/'), "{text}");
        assert!(
            text.lines().all(|line| line.chars().count() <= 120),
            "{text}"
        );
        assert!(!text.contains('\u{1b}'), "{text}");
    }
}
