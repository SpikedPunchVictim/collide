//! The command tree of `collide` (ts-0041-xf): every command the plan names, with the one line `collide --help` prints
//! for it and, for a command that is not built yet, the tag and stage that fill it in. `schema`, `tasks`, `task` and
//! `sections` are real and keep their own modules ([`crate::task_cli`], [`crate::schema_list`]); the others are
//! outlines.
//!
//! **A command that is not built yet.** Its handler is a function whose body is a tagged `todo!`, so the stub check
//! counts it like any other stub. [`execute_stub`] runs the handler, catches that one panic and prints
//! `collide <command>: not built yet (<tag>, Stage <n>)` with the exit code [`exit::NOT_BUILT`], not a backtrace. It
//! catches only the panic whose message is that command's own tag: any other panic, or another stub's, is not caught.
//! The tag and stage in the table are checked against `stub-tags.json` by a test.
//!
//! | tag | command | what fills it in |
//! |---|---|---|
//! | `cli: run` | `collide run` | built: [`crate::run_cli`], one Claude phase on a task file, in a new worktree |
//! | `cli: stop` | `collide stop` | built: [`crate::stop_cli`], the stop of 0.5 ([`crate::stop`]) through the daemon |
//! | `cli: report` | `collide report` | report files for shell scripts (`span`, `metric`) |
//! | `cli: config` | `collide config` | built: `get`, `set`, `set --secret` ([`crate::config_set_cli`]) |
//! | `cli: pack` | `collide pack` | built: `add`, `link`, `enable`, `disable`, `list`, `update` ([`crate::pack_cli`]) |
//! | `cli: init` | `collide init` | built: [`crate::board_cli`], the board file of Share with team (0.11) |
//! | `cli: trust` | `collide trust` | built: [`crate::board_cli`], the review and the trust of a line (0.9) |
//! | `cli: board` | `collide board` | built: `remove` ([`crate::board_cli`], ts-0222-gx), a board hidden, not deleted |

use crate::task_cli::exit;
use std::any::Any;
use std::panic::{self, AssertUnwindSafe};

/// The handler of a command that is not built yet: the arguments after the command's name, the exit code out.
pub type Handler = fn(&[String]) -> u8;

/// A command whose handler is a tagged stub.
#[derive(Debug, Clone, Copy)]
pub struct Stub {
    /// The tag in the handler's `todo!`, and in `stub-tags.json`.
    pub tag: &'static str,
    /// The stage that fills the tag in, as `stub-tags.json` gives it.
    pub stage: &'static str,
    pub handler: Handler,
}

/// Whether a command works.
#[derive(Debug, Clone, Copy)]
pub enum Build {
    /// Real: `collide` runs it through its own module.
    Built,
    /// An outline: running it exits with [`exit::NOT_BUILT`].
    NotBuilt(Stub),
}

/// One command of the tree.
#[derive(Debug, Clone, Copy)]
pub struct Command {
    pub name: &'static str,
    /// What `collide --help` prints after the name.
    pub summary: &'static str,
    pub build: Build,
}

/// The tree, in the order `collide --help` lists it.
pub const COMMANDS: [Command; 13] = [
    Command {
        name: "run",
        summary: "start one agent phase on a task, in a new worktree, and record it",
        build: Build::Built,
    },
    Command {
        name: "stop",
        summary: "stop a run: its processes, in order, then a report of the survivors",
        build: Build::Built,
    },
    Command {
        name: "tasks",
        summary: "list the items (epics, tasks and proposals)",
        build: Build::Built,
    },
    Command {
        name: "task",
        summary: "show one item, or change it with an op",
        build: Build::Built,
    },
    Command {
        name: "schema",
        summary: "list the named points and events of the contract",
        build: Build::Built,
    },
    Command {
        name: "report",
        summary: "write a span or a metric to the report file of the hook that runs this",
        build: Build::NotBuilt(Stub {
            tag: "cli: report",
            stage: "7",
            handler: report,
        }),
    },
    Command {
        name: "config",
        summary: "show a setting and the layer it came from (`get`), or set one (`set`, `set --secret`)",
        build: Build::Built,
    },
    Command {
        name: "pack",
        summary: "add, link, enable, disable, list and update packs",
        build: Build::Built,
    },
    Command {
        name: "init",
        summary: "write a short .collide/board.yaml in this checkout, and nothing else",
        build: Build::Built,
    },
    Command {
        name: "trust",
        summary: "review what a line's packs run, and trust it (asks, unless --yes)",
        build: Build::Built,
    },
    Command {
        name: "board",
        summary: "take a board out of Collide (`remove`): it is hidden and nothing is deleted",
        build: Build::Built,
    },
    Command {
        name: "sections",
        summary: "list the sections in use in the proposals' features",
        build: Build::Built,
    },
    Command {
        name: "guide",
        summary: "print a short guide for an agent that drives collide (starts no daemon)",
        build: Build::Built,
    },
];

/// The command named `name`, if the tree has one.
pub fn find(name: &str) -> Option<&'static Command> {
    COMMANDS.iter().find(|command| command.name == name)
}

/// What `collide --help` prints about the tree: one line for each command, and the exit code of one that is not
/// built yet.
pub fn tree_text() -> String {
    let width = COMMANDS.iter().map(|c| c.name.len()).max().unwrap_or(0);
    let mut text = String::from("commands:\n");
    for command in &COMMANDS {
        let note = match command.build {
            Build::Built => String::new(),
            Build::NotBuilt(stub) => format!(" (not built yet: Stage {})", stub.stage),
        };
        text.push_str(&format!(
            "  {:width$}  {}{note}\n",
            command.name, command.summary
        ));
    }
    text.push_str(&format!(
        "\nA command that is not built yet exits {} and names its tag and stage.",
        exit::NOT_BUILT
    ));
    text
}

/// Runs `name` if it is a command that is not built yet, and returns the exit code. `None` when `name` is built, or no
/// command.
pub fn execute_stub(name: &str, args: &[String]) -> Option<u8> {
    let command = find(name)?;
    let Build::NotBuilt(stub) = command.build else {
        return None;
    };
    silence_panic_message_of(stub.tag);
    Some(run_catching(command.name, &stub, args))
}

/// Calls the handler and turns its own stub's panic into the exit message. Any other panic goes on unwinding.
fn run_catching(name: &str, stub: &Stub, args: &[String]) -> u8 {
    match panic::catch_unwind(AssertUnwindSafe(|| (stub.handler)(args))) {
        Ok(code) => code,
        Err(payload) if is_stub_of(&*payload, stub.tag) => {
            eprintln!(
                "collide {name}: not built yet ({}, Stage {})",
                stub.tag, stub.stage
            );
            exit::NOT_BUILT
        }
        Err(payload) => panic::resume_unwind(payload),
    }
}

/// The text a `todo!("<tag>")` panics with.
fn stub_message(tag: &str) -> String {
    format!("not yet implemented: {tag}")
}

fn message_of(payload: &(dyn Any + Send)) -> Option<&str> {
    payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
}

fn is_stub_of(payload: &(dyn Any + Send), tag: &str) -> bool {
    message_of(payload) == Some(stub_message(tag).as_str())
}

/// Keeps the default panic hook from printing the one stub panic that [`run_catching`] is about to turn into a message.
/// Every other panic still prints as before. The hook stays installed: the process exits right after.
fn silence_panic_message_of(tag: &'static str) {
    let previous = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        if is_stub_of(info.payload(), tag) {
            return;
        }
        previous(info);
    }));
}

/// `collide report span|metric`: writes a line to the file named by the hook's report variable, and needs no socket or
/// token (Stage 7).
pub fn report(args: &[String]) -> u8 {
    let _ = args;
    todo!("cli: report")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(_: &[String]) -> u8 {
        0
    }

    fn own_stub(_: &[String]) -> u8 {
        panic!("not yet implemented: cli: test")
    }

    fn another_stub(_: &[String]) -> u8 {
        panic!("not yet implemented: cli: other")
    }

    fn broken(_: &[String]) -> u8 {
        panic!("boom")
    }

    fn stub(handler: Handler) -> Stub {
        Stub {
            tag: "cli: test",
            stage: "99",
            handler,
        }
    }

    #[test]
    fn a_handler_that_returns_gives_its_own_exit_code() {
        assert_eq!(run_catching("t", &stub(ok), &[]), 0);
    }

    #[test]
    fn a_handler_that_reaches_its_own_stub_exits_with_the_not_built_code() {
        assert_eq!(run_catching("t", &stub(own_stub), &[]), exit::NOT_BUILT);
    }

    #[test]
    #[should_panic(expected = "not yet implemented: cli: other")]
    fn a_stub_with_another_tag_is_not_caught() {
        run_catching("t", &stub(another_stub), &[]);
    }

    #[test]
    #[should_panic(expected = "boom")]
    fn a_panic_that_is_no_stub_is_not_caught() {
        run_catching("t", &stub(broken), &[]);
    }

    #[test]
    fn the_tree_lists_the_commands_the_plan_names_in_order() {
        let names: Vec<&str> = COMMANDS.iter().map(|command| command.name).collect();
        assert_eq!(
            names,
            [
                "run", "stop", "tasks", "task", "schema", "report", "config", "pack", "init",
                "trust", "board", "sections", "guide"
            ]
        );
    }

    #[test]
    fn the_built_commands_are_not_run_as_stubs() {
        for name in [
            "run", "stop", "tasks", "task", "schema", "sections", "config", "pack", "init",
            "trust", "board", "guide", "nonsense",
        ] {
            assert_eq!(execute_stub(name, &[]), None, "{name}");
        }
    }
}
