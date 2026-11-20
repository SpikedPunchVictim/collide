//! `collide tasks`, `collide task <id>`, `collide task <id> <op> ...` (ts-0020), `collide task create`, `collide task
//! reid`, `collide task new` and `collide sections` (ts-0049): the command line (what each command takes, its help and
//! its exit codes) and the text the commands print. Every command goes through the daemon ([`crate::task_daemon`]),
//! which holds the corpus, the provider supervisor and the write guard; nothing here reads the planning files.

use crate::checkout::{CheckoutError, resolve_checkout};
use crate::commit::commit_files;
use crate::task_view::heading;
use collide_core::provider::Task;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Exit codes. An op's outcome has its own: `ok` is 0, `conflict` 4 and `refused` 5.
pub mod exit {
    pub const OK: u8 = 0;
    /// Something failed: no repository, the daemon did not start or answered badly, or the commit that `--commit`
    /// asked for failed (the write itself stands).
    pub const FAILED: u8 = 1;
    /// The command line is wrong: an option, an op, an argument, a `--checkout` that is no checkout of the repository.
    pub const USAGE: u8 = 2;
    /// An id names no item, or several.
    pub const ID: u8 = 3;
    /// The item changed since it was read, and nothing was written.
    pub const CONFLICT: u8 = 4;
    /// The provider refused the op, and said why.
    pub const REFUSED: u8 = 5;
    /// The command is in the tree but not built yet; the message names its tag and stage (`crate::commands`).
    pub const NOT_BUILT: u8 = 6;
    /// `collide run`: the run's phase failed (`crate::run_cli`).
    pub const RUN_FAILED: u8 = 7;
    /// `collide run`: the run was stopped.
    pub const RUN_STOPPED: u8 = 8;
    /// `collide run`: the run waits for a person and the command left it (a permission request with no answer, a full
    /// disk, a budget that ran out).
    pub const WAITING: u8 = 9;
    /// `collide tasks --view` and `--ready`: the view has an error of its own, so it lists no task
    /// (`crate::task_daemon`).
    pub const VIEW_FAILED: u8 = 10;
    /// A write (an op, `create`, `reid`): it is in the daemon's Queue and was not applied yet, because its checkout is
    /// busy (`--no-wait`, or the wait ran out).
    pub const QUEUED: u8 = 11;
    /// `collide task <id> <op>`: the op is a person's alone (accept, defer, reject, resolve, reopen), so nothing was
    /// written and a confirmation waits in the app (`crate::task_daemon`).
    pub const CONFIRMATION: u8 = 12;
}

pub const USAGE: &str = "\
usage:
  collide tasks [--ready] [--kind <kind>] [--view <name>] [--checkout <path>] [--json]
  collide task <id> [--json]
  collide task new <prompt> [--json]
  collide task <id> <op> [<value>] [name=value ...] [--checkout <path>] [--commit] [--json]
  collide task create <title> [epic=<epic>] [kind=task|epic|proposal|spike|question] [seed=<number>]
    [root=shared|private] [name=value ...] [--checkout <path>] [--commit] [--json]
  collide task reid <id> [seed=<number>] [--checkout <path>] [--commit] [--json]
  a write (an op, create, reid) also takes [--wait <seconds> | --no-wait]
  collide task renameSection <from> <to> [--checkout <path>] [--commit] [--json]
  collide sections [--json]";

/// What `collide help` prints.
pub const HELP: &str = "\
Every one of these goes through the daemon, which is started when none runs (and says so, once). If it cannot start,
the command exits 1 and reads nothing itself.

collide tasks: list the board's items (epics, tasks, proposals, spikes and questions), one line each: key, status,
  kind, title, in the order the daemon lists them. The folders it reads are the board's: the layout setting's, and the
  private root's. --kind keeps only items of that kind. --json prints the daemon's answer.
  A source of the board that could not start is named on stderr with the reason; the others are still listed. When
  none started, the command exits 1.
collide tasks --ready: the members of the built-in `ready` view: the items that can be started now. A blocked-by cycle
  and a blocked-by target that is no item keep their items from being ready and are reported on stderr.
collide tasks --view <name>: the members of the board's view of that name (the person's, the repository's or a
  pack's), in the order the daemon lists them. --ready and --kind apply on top. A view with an error of its own lists
  no task, prints the error and exits 10. Tasks a view could not check, and problems in the parts of its items, are
  counted on stderr, and the rest are listed. An unknown view exits 3 and names the board's views.
collide tasks --checkout <path>: only the tasks that have a copy in that checkout, which must be an entry of `git
  worktree list` (anything else exits 2). It takes --view too.
collide task <id>: show one item: its title, kind, status, why it is or is not ready, its epic, the fields its source
  sets (needs, resolved_by, ...), the number of its claims, a proposal's coverage, the number of problems in its
  parts, its links (an incoming link is named by its inverse, such as `evidence for`, and a link to a feature names
  it), a proposal's features by section with their states and the tasks that implement them, then the problems of its
  Features part and the body.
collide task <id> --json: the daemon's answer for one item: its fields, state (status and category), links one hop out
  in both directions (each with the other end's kind, status and category), version, latest run, runs and copies.
collide task new <prompt>: a scratch task for this repository's board (in Collide's store, in no file) and its key,
  such as scratch:3. Give the prompt as one argument.
collide task <id> <op> ...: change one item with an op its source declares (an unknown op lists them). The write goes
  through the daemon. If its checkout is busy (a rebase, a merge, a cherry-pick, a revert, a bisect or a live phase)
  the daemon queues it: the command says why, prints the Queue item's id, and waits for the write to apply
  (10 minutes; --wait <seconds> sets that). --no-wait, and a wait that ran out, leave it queued and exit 11. --commit
  does not apply to a write that was queued: commit it when it has landed.
  Accepting, deferring, rejecting, resolving or reopening a proposal or a question (and returning a deferred proposal
  to proposed) is a person's: the command writes nothing, says a confirmation waits in the app, prints the Queue
  item's id and exits 12. The person answers Accept or Dismiss in the app.
collide task renameSection <from> <to>: rename a section in every proposal that uses it (a name already in use merges).
  It names no item and is written like any op. addFeature and moveFeature are ops of a proposal: collide task
  <proposal> addFeature section=<name> title=<text> summary=<text>, and collide task <proposal> moveFeature
  feature=F2 section=<name>.
collide sections: list the sections in use in the features of the proposals that are not dropped (not rejected or
  superseded), each with its feature count and its proposal count, most features first, then by name, the features
  with no section last as Unsorted, then the near-duplicate groups (--json: machine-readable, with the ids of the
  proposals under each section)

<id> is the key (plan:ts-0012-ua, scratch:3), the full id (ts-0012-ua) or a short form: 12, 0012, 12-ua, ts-12. A
short form must match exactly one item; if it matches several they are listed with their keys and epics, and you
add the suffix.

collide task create <title>: make an item in an epic (epic=ep-002-va, or a short form such as ep-2; kind=epic makes an
  epic, which takes no epic=; kind=proposal makes a draft proposal in the epic's proposals/ folder; kind=spike and
  kind=question make those records; root=private makes it in the private root). The id is the next free number for
  the kind across every local branch, every fetched remote branch and the working tree, with a two-letter suffix
  drawn from a seeded generator (seed=, random when left out). The new file is written; without --commit nothing is
  committed. Any other name=value goes to the source's create if it declares that name (a source whose plan is not
  Collide's has its own), read as an op's value is (a list or an object as JSON); one it does not declare is refused
  with the names it does. A source with no epic items takes epic=<id> of one of its own collections.
collide task reid <id>: give an item a new suffix and rewrite every link that names the old id, in every item file.
  Where two files share one id, the second (the one `collide tasks` reports as skipped) is the one renamed; links meant
  for the other file are rewritten too. It prints the old and the new id and each file touched. Nothing is committed
  without --commit: review with git diff.

Where a write goes: the main checkout's .collide/planning/, also when you run collide inside a linked worktree (which
is left untouched). --checkout <path> names another checkout instead: <path> must be an entry of `git worktree list`
for this repository (relative paths are taken from the working directory), or nothing is written and the exit code is
2. The item is then read from that checkout too, so a version check is against the file that is written. A write
leaves the file modified (or untracked, for a new one) and commits nothing.
--commit commits the write, in the checkout it went to: exactly the files the write reported (for reid, the new and
the removed paths too), with a message naming the item and the op, such as: task ts-0012-ua setStatus: <title>. Another
modified, staged or untracked file is not touched, and what you have staged stays staged. Hooks run (there is no
--no-verify). An op that changes nothing, and a conflict or a refusal, commit nothing. If the commit fails (a hook,
git's own refusal) git's message is printed, the files stay written and uncommitted, and the exit code is 1.

Sections are the `###` headings of a proposal's Features part, matched by name across proposals, with no registry.
Two names are one section when they match after trimming, in any case: Packs and packs.
Two names are near-duplicates when they match once
  spaces, hyphens and underscores are taken out and one final s is dropped: Pack and packs, Task view and task-view.
The rule is plain and has false matches (Bus and Bu are grouped); s alone keeps its s. A group is a suggestion for a
person: collide sections never merges anything.

A task links to the feature it builds with: collide task <task> link pr-0003-km#F2 type=implements. The target is a
proposal and a feature it has (F<n>), or the link is refused and the message names the features the proposal has.

An op's words are name=value, or one bare value that sets the op's main property (setStatus active sets status).
Lengths and patterns are checked by the provider.

An item is ready when its status is ready and each blocked-by target is done or dropped.

exit codes: 0 ok, 1 failed (also: the write was made but --commit's commit failed; no daemon), 2 wrong command line
(also: a --checkout that is not a checkout of this repository), 3 id not found or ambiguous, 4 conflict (nothing was
written), 5 refused by the provider, 10 the view has an error of its own, 11 the write is queued in the daemon and not
applied yet, 12 a person-only op: a confirmation waits in the app";

/// A failure, with the exit code it ends with.
#[derive(Debug)]
pub struct CliError {
    pub code: u8,
    pub message: String,
}

impl CliError {
    pub(crate) fn new(code: u8, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub(crate) fn failed(error: impl std::fmt::Display) -> Self {
        Self::new(exit::FAILED, error.to_string())
    }

    pub(crate) fn usage(message: impl Into<String>) -> Self {
        Self::new(exit::USAGE, message)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct TasksOptions {
    pub ready: bool,
    pub kind: Option<String>,
    pub json: bool,
    /// `--view <name>`: the members of a named view, through the daemon.
    pub view: Option<String>,
    /// `--checkout <path>`: only the tasks that have a copy in this checkout, through the daemon.
    pub checkout: Option<String>,
}

/// `collide task new <prompt>`: a scratch task.
#[derive(Debug, PartialEq, Eq)]
pub struct NewOptions {
    pub prompt: String,
    pub json: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SectionsOptions {
    pub json: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TaskOptions {
    pub id: String,
    pub op: Option<String>,
    pub words: Vec<String>,
    pub write: WriteOptions,
    pub json: bool,
}

/// Where a write goes and whether it is committed (ts-0031).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct WriteOptions {
    /// `--checkout <path>`: write in this checkout instead of the main one.
    pub checkout: Option<String>,
    /// `--commit`: commit the files the write reported.
    pub commit: bool,
    /// `--wait <seconds>` and `--no-wait`: how long to wait for a checkout in the middle of a rebase or the like.
    pub wait: Wait,
}

/// How long a write waits for a busy checkout (0.4, Root busy): the daemon queues it, and the command waits for the
/// Queue item to leave.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Wait {
    /// [`DEFAULT_WAIT`].
    #[default]
    Default,
    /// `--wait <seconds>`.
    Seconds(u64),
    /// `--no-wait`: refuse at once.
    Never,
}

impl Wait {
    pub(crate) fn limit(self) -> Duration {
        match self {
            Wait::Default => DEFAULT_WAIT,
            Wait::Seconds(seconds) => Duration::from_secs(seconds),
            Wait::Never => Duration::ZERO,
        }
    }
}

/// How long a write waits for a busy checkout when `--wait` does not say.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(10 * 60);

/// `collide task create <title> [epic=] [kind=] [seed=] [root=]`.
#[derive(Debug, PartialEq, Eq)]
pub struct CreateOptions {
    pub title: String,
    pub epic: Option<String>,
    pub kind: Option<String>,
    pub seed: Option<u64>,
    pub root: Option<String>,
    /// The other `name=value` words, as typed: the planning source's `create` says which it declares.
    pub args: BTreeMap<String, String>,
    pub write: WriteOptions,
    pub json: bool,
}

/// `collide task renameSection <from> <to>`.
#[derive(Debug, PartialEq, Eq)]
pub struct RenameSectionOptions {
    pub from: String,
    pub to: String,
    pub write: WriteOptions,
    pub json: bool,
}

/// `collide task reid <id> [seed=]`.
#[derive(Debug, PartialEq, Eq)]
pub struct ReidOptions {
    pub id: String,
    pub seed: Option<u64>,
    pub write: WriteOptions,
    pub json: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Tasks(TasksOptions),
    Sections(SectionsOptions),
    Task(TaskOptions),
    Create(CreateOptions),
    Reid(ReidOptions),
    RenameSection(RenameSectionOptions),
    New(NewOptions),
}

/// Reads the words after `collide tasks`.
pub fn parse_tasks(args: &[String]) -> Result<Command, CliError> {
    let mut options = TasksOptions {
        ready: false,
        kind: None,
        json: false,
        view: None,
        checkout: None,
    };
    let mut words = args.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--ready" => options.ready = true,
            "--json" => options.json = true,
            "--kind" => match words.next() {
                Some(kind) => options.kind = Some(kind.clone()),
                None => return Err(CliError::usage("--kind needs a kind, such as --kind task")),
            },
            "--view" => set_once(&mut options.view, words.next(), "--view", "a view's name")?,
            "--checkout" => set_once(
                &mut options.checkout,
                words.next(),
                "--checkout",
                "a path, such as --checkout ../other-worktree",
            )?,
            other => {
                if let Some(kind) = other
                    .strip_prefix("--kind=")
                    .filter(|kind| !kind.is_empty())
                {
                    options.kind = Some(kind.to_string());
                } else if let Some(name) = other.strip_prefix("--view=") {
                    set_once(
                        &mut options.view,
                        Some(&name.to_string()),
                        "--view",
                        "a view's name",
                    )?;
                } else if let Some(path) = other.strip_prefix("--checkout=") {
                    set_once(
                        &mut options.checkout,
                        Some(&path.to_string()),
                        "--checkout",
                        "a path, such as --checkout ../other-worktree",
                    )?;
                } else if other.starts_with("--") {
                    return Err(CliError::usage(format!("unknown option `{other}`")));
                } else {
                    return Err(CliError::usage(format!(
                        "`collide tasks` takes no argument; got `{other}`"
                    )));
                }
            }
        }
    }
    Ok(Command::Tasks(options))
}

/// Sets an option that takes one value, once: a value that is missing or empty, and an option given twice, are usage
/// errors that name the option.
fn set_once(
    slot: &mut Option<String>,
    given: Option<&String>,
    option: &str,
    needs: &str,
) -> Result<(), CliError> {
    let Some(value) = given.filter(|value| !value.is_empty()) else {
        return Err(CliError::usage(format!("{option} needs {needs}")));
    };
    if slot.replace(value.clone()).is_some() {
        return Err(CliError::usage(format!("{option} is given twice")));
    }
    Ok(())
}

/// Reads the words after `collide sections`.
pub fn parse_sections(args: &[String]) -> Result<Command, CliError> {
    let mut options = SectionsOptions { json: false };
    for word in args {
        match word.as_str() {
            "--json" => options.json = true,
            other if other.starts_with("--") => {
                return Err(CliError::usage(format!("unknown option `{other}`")));
            }
            other => {
                return Err(CliError::usage(format!(
                    "`collide sections` takes no argument; got `{other}`"
                )));
            }
        }
    }
    Ok(Command::Sections(options))
}

/// Reads the words after `collide task`: an id, then an op and its words.
pub fn parse_task(args: &[String]) -> Result<Command, CliError> {
    let mut json = false;
    let mut write = WriteOptions::default();
    let mut plain = Vec::new();
    let mut words = args.iter();
    while let Some(word) = words.next() {
        let given = match word.as_str() {
            "--json" => {
                json = true;
                continue;
            }
            "--commit" => {
                write.commit = true;
                continue;
            }
            "--no-wait" => {
                set_wait(&mut write, Wait::Never, "--no-wait")?;
                continue;
            }
            "--wait" => {
                let seconds = parse_seconds(words.next().map(String::as_str))?;
                set_wait(&mut write, Wait::Seconds(seconds), "--wait")?;
                continue;
            }
            "--checkout" => match words.next() {
                Some(path) => path.clone(),
                None => {
                    return Err(CliError::usage(
                        "--checkout needs a path, such as --checkout ../other-worktree",
                    ));
                }
            },
            other => match other.strip_prefix("--checkout=") {
                Some(path) => path.to_string(),
                None if other.starts_with("--wait=") => {
                    let seconds = parse_seconds(other.strip_prefix("--wait="))?;
                    set_wait(&mut write, Wait::Seconds(seconds), "--wait")?;
                    continue;
                }
                None if other.starts_with("--") => {
                    return Err(CliError::usage(format!("unknown option `{other}`")));
                }
                None => {
                    plain.push(word.clone());
                    continue;
                }
            },
        };
        if given.is_empty() {
            return Err(CliError::usage(
                "--checkout needs a path, such as --checkout ../other-worktree",
            ));
        }
        if write.checkout.replace(given).is_some() {
            return Err(CliError::usage("--checkout is given twice"));
        }
    }
    let mut plain = plain.into_iter();
    let Some(id) = plain.next() else {
        return Err(CliError::usage(
            "`collide task` needs an id, such as 12 or ts-0012-ua",
        ));
    };
    // `create` and `reid` are words of their own: neither is an id form, so they cannot be taken for one.
    if id == "create" {
        return parse_create(plain.collect(), write, json);
    }
    if id == "renameSection" {
        return parse_rename_section(plain.collect(), write, json);
    }
    if id == "new" {
        return parse_new(plain.collect(), &write, json);
    }
    if id == "reid" {
        let Some(id) = plain.next() else {
            return Err(CliError::usage(
                "`collide task reid` needs an id, such as 12 or ts-0012-ua",
            ));
        };
        return parse_reid(id, plain.collect(), write, json);
    }
    let op = plain.next();
    // `collide task <id> reid` is `collide task reid <id>`: it is not a write against one version of one file.
    if op.as_deref() == Some("reid") {
        return parse_reid(id, plain.collect(), write, json);
    }
    if op.is_none() && write != WriteOptions::default() {
        return Err(CliError::usage(
            "--checkout, --commit, --wait and --no-wait are for a write: give an op after the id, \
             such as setStatus active",
        ));
    }
    Ok(Command::Task(TaskOptions {
        id,
        op,
        words: plain.collect(),
        write,
        json,
    }))
}

/// The words of `collide task create`: one bare value, the title, and `name=value` for `title`, `epic`, `kind`, `seed`
/// and `root`. The provider checks what a value may be (the kinds, the epic, the lengths).
fn parse_create(words: Vec<String>, write: WriteOptions, json: bool) -> Result<Command, CliError> {
    let mut given: BTreeMap<&str, String> = BTreeMap::new();
    for word in &words {
        let (name, value) = match word.split_once('=') {
            Some((name, value)) if !name.is_empty() => (name, value),
            _ => ("title", word.as_str()),
        };
        if given.insert(name, value.to_string()).is_some() {
            return Err(CliError::usage(
                if name == "title" && !word.starts_with("title=") {
                    format!("`{word}` is a second bare value; name each value as name=value")
                } else {
                    format!("`{name}` is given twice")
                },
            ));
        }
    }
    let Some(title) = given.remove("title") else {
        return Err(CliError::usage(
            "op `create` needs title: collide task create <title> [epic=<epic>] [kind=<kind>]",
        ));
    };
    let seed = given
        .remove("seed")
        .map(|text| parse_seed(&text))
        .transpose()?;
    let epic = given.remove("epic");
    let kind = given.remove("kind");
    let root = given.remove("root");
    // What is left is for the source: the daemon knows which names its `create` declares.
    let args = given
        .into_iter()
        .map(|(name, value)| (name.to_string(), value))
        .collect();
    Ok(Command::Create(CreateOptions {
        title,
        epic,
        kind,
        seed,
        root,
        args,
        write,
        json,
    }))
}

/// The words of `collide task reid <id>`: `seed=<number>` only.
fn parse_reid(
    id: String,
    words: Vec<String>,
    write: WriteOptions,
    json: bool,
) -> Result<Command, CliError> {
    let mut seed = None;
    for word in &words {
        let Some(text) = word.strip_prefix("seed=") else {
            return Err(CliError::usage(format!(
                "`{word}` is not seed=<number>: `reid` takes no other argument"
            )));
        };
        if seed.replace(parse_seed(text)?).is_some() {
            return Err(CliError::usage("`seed` is given twice"));
        }
    }
    Ok(Command::Reid(ReidOptions {
        id,
        seed,
        write,
        json,
    }))
}

/// The words of `collide task renameSection`: the old name and the new, bare or as `from=` and `to=`.
fn parse_rename_section(
    words: Vec<String>,
    write: WriteOptions,
    json: bool,
) -> Result<Command, CliError> {
    let (mut from, mut to) = (None, None);
    for word in words {
        let (slot, value) = match word.split_once('=') {
            Some(("from", value)) => (&mut from, value.to_string()),
            Some(("to", value)) => (&mut to, value.to_string()),
            _ if from.is_none() => (&mut from, word),
            _ => (&mut to, word),
        };
        if slot.replace(value).is_some() {
            return Err(CliError::usage(
                "renameSection takes two values: collide task renameSection <from> <to>",
            ));
        }
    }
    let (Some(from), Some(to)) = (from, to) else {
        return Err(CliError::usage(
            "renameSection needs both: collide task renameSection <from> <to>",
        ));
    };
    Ok(Command::RenameSection(RenameSectionOptions {
        from,
        to,
        write,
        json,
    }))
}

fn parse_seed(text: &str) -> Result<u64, CliError> {
    text.parse()
        .map_err(|_| CliError::usage(format!("`seed` must be a whole number, not `{text}`")))
}

/// Reads the words after `collide task new`: one prompt. A scratch task is made in Collide's store and not in a
/// checkout, so the options of a write to a checkout are refused, by name.
fn parse_new(words: Vec<String>, write: &WriteOptions, json: bool) -> Result<Command, CliError> {
    let given = [
        write.checkout.as_ref().map(|_| "--checkout"),
        write.commit.then_some("--commit"),
        match write.wait {
            Wait::Default => None,
            Wait::Seconds(_) => Some("--wait"),
            Wait::Never => Some("--no-wait"),
        },
    ];
    if let Some(option) = given.into_iter().flatten().next() {
        return Err(CliError::usage(format!(
            "`collide task new` takes no `{option}`: a scratch task is made in Collide's store, not in a checkout"
        )));
    }
    let mut words = words.into_iter();
    let Some(prompt) = words.next() else {
        return Err(CliError::usage(
            "`collide task new` needs a prompt, such as: collide task new \"try the cache\"",
        ));
    };
    if words.next().is_some() {
        return Err(CliError::usage(
            "`collide task new` takes the prompt as one argument: put it in quotes",
        ));
    }
    if prompt.trim().is_empty() {
        return Err(CliError::usage("the prompt of `collide task new` is empty"));
    }
    Ok(Command::New(NewOptions { prompt, json }))
}

/// Sets how long to wait, once: `--wait` and `--no-wait` say opposite things, and either given twice is a mistake.
fn set_wait(write: &mut WriteOptions, wait: Wait, option: &str) -> Result<(), CliError> {
    if write.wait != Wait::Default {
        return Err(CliError::usage(format!(
            "give one of --wait <seconds> and --no-wait, once; `{option}` came after another"
        )));
    }
    write.wait = wait;
    Ok(())
}

/// The seconds `--wait` takes: a whole number.
fn parse_seconds(given: Option<&str>) -> Result<u64, CliError> {
    let message = "--wait needs a number of seconds, such as --wait 30";
    given
        .filter(|text| !text.is_empty())
        .and_then(|text| text.parse::<u64>().ok())
        .ok_or_else(|| CliError::usage(message))
}

/// The checkout that `--checkout` names: an entry of `git worktree list` of the repository whose main checkout is
/// `root`, or a usage error.
pub(crate) fn checkout_named(root: &Path, cwd: &Path, given: &str) -> Result<PathBuf, CliError> {
    resolve_checkout(root, cwd, given).map_err(|error| match error {
        CheckoutError::NotACheckout { .. } | CheckoutError::UnreadableCheckout { .. } => {
            CliError::usage(format!("--checkout: {error}"))
        }
        other => CliError::failed(other),
    })
}

pub(crate) fn emit(out: &mut dyn Write, text: &str) -> Result<(), CliError> {
    out.write_all(text.as_bytes())
        .map_err(|error| CliError::failed(format!("could not write the output: {error}")))
}

/// The text for an id that matched several items: each with its epic, and what to add.
pub(crate) fn ambiguous_message(query: &str, matches: &[&Task]) -> String {
    let mut text = format!("`{query}` matches {} items:\n", matches.len());
    for task in matches {
        // The key, so each match names its source.
        let name = match task.epic() {
            Some(epic) => format!("{epic} / {}", task.key),
            None => task.key.clone(),
        };
        text.push_str(&format!(
            "  {name}  {}  {}\n",
            task.kind.as_deref().unwrap_or(""),
            task.title
        ));
    }
    let example = matches
        .first()
        .and_then(|task| {
            let (prefix, rest) = task.id.split_once('-')?;
            let (number, suffix) = rest.split_once('-')?;
            Some(format!(", such as {number}-{suffix} or {prefix}-{number}"))
        })
        .unwrap_or_default();
    text.push_str(&format!(
        "add the suffix or the prefix{example}, or use the full id"
    ));
    text
}

/// What an `ok` changed, by comparing the item as it was read with what the write returned.
pub(crate) fn ok_text(before: &Task, after: &Task, files: &[String]) -> String {
    let mut changes = Vec::new();
    if before.state.status != after.state.status {
        changes.push(format!(
            "  status  {} -> {}\n",
            before.state.status, after.state.status
        ));
    }
    if before.owners != after.owners {
        let shown = |owners: &[String]| {
            if owners.is_empty() {
                "(none)".to_string()
            } else {
                owners.join(", ")
            }
        };
        changes.push(format!(
            "  owners  {} -> {}\n",
            shown(&before.owners),
            shown(&after.owners)
        ));
    }
    let link_names = |task: &Task| -> Vec<String> {
        task.links
            .iter()
            .filter(|link| link.link_type != "part-of")
            .map(|link| format!("{} {}", link.link_type, link.to))
            .collect()
    };
    let (was, now) = (link_names(before), link_names(after));
    for added in now.iter().filter(|link| !was.contains(link)) {
        changes.push(format!("  links   +{added}\n"));
    }
    for removed in was.iter().filter(|link| !now.contains(link)) {
        changes.push(format!("  links   -{removed}\n"));
    }
    let heading = heading(after);
    if changes.is_empty() && files.is_empty() {
        return format!("ok  {heading}: nothing to change\n");
    }
    let mut text = format!("ok  {heading}\n{}", changes.concat());
    for file in files {
        text.push_str(&format!("  file    {file}\n"));
    }
    text
}

/// Commits what an `ok` reported, when `--commit` was given (`commit` is the checkout to commit in), and returns the
/// exit code: 0, or 1 when the commit failed (the files are then left written) or the item is a private one, which
/// is in no repository (ts-0125-uh). Under `--json` the one document on
/// stdout stays the provider's result, so the new commit is not printed.
pub(crate) fn commit_reported(
    commit: Option<&Path>,
    task: &Task,
    files: &[String],
    subject: &str,
    json: bool,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> u8 {
    // A private item is a file of the pack's data folder, not of the repository: there is nothing to commit.
    if commit.is_some()
        && task
            .path
            .as_deref()
            .is_some_and(|path| path.starts_with("private:"))
    {
        let _ = writeln!(
            err,
            "collide: --commit refused: {} is in the private root, which is in no repository; the write was \
             made and nothing was committed",
            task.id
        );
        return exit::FAILED;
    }
    let Some(dir) = commit.filter(|_| !files.is_empty()) else {
        return exit::OK;
    };
    match commit_files(dir, files, subject) {
        Ok(short) => {
            if !json {
                let _ = writeln!(out, "  commit  {short}  {subject}");
            }
            exit::OK
        }
        Err(message) => {
            let _ = writeln!(
                err,
                "collide: {} was written, but git commit failed; the files are left written and uncommitted:\n{message}",
                task.id
            );
            exit::FAILED
        }
    }
}
