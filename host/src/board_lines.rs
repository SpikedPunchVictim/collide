//! The lines of a board with their trust, and `board.trust` and `board.trust_review` (ts-0102-vu; 0.9, Untrusted
//! input). A board has one line in 4a: the ref its checkout is on. Its snapshot is the commit that ref names, and its
//! trust is [`Trust::trust_state`] of the line the loader read at that commit.
//! While git has `HEAD` detached for a rebase or a bisect, the ref is the branch git holds ([`Checkout::held`]).
//!
//! A line that does not load (a bad `board.yaml`, a pack that needs a first fetch) is still listed: its problems are
//! in the summary and its hash is empty, because there is nothing to hash. Two things block a load until the person
//! says yes, and a yes is recorded before the hash exists: a host never fetched from ([`LoadProblem::needs_host`]) and
//! a `dev:` folder never trusted ([`LoadProblem::needs_dev_trust`]). So `board.trust` with an empty hash records the
//! approvals it is given, for what the load needs only, and trusts nothing.

use crate::board_load::{
    LoadContext, LoadProblem, LoadedLine, PackOrigin, load_defaults, load_line,
};
use crate::boards::{self, Checkout};
use crate::config_get::{BoardRef, LoadContextFactory, WorkspaceRoots};
use crate::pack_manage::review_data;
use crate::pack_update::file_changes;
use crate::settings_files::user_config_dir;
use crate::sources::source_problems;
use crate::trust::{LineSite, Trust, TrustError, TrustStore};
use crate::user_packs::UserScope;
use collide_core::owner::{
    BoardSummary, BoardTrustParams, BoardTrustResult, BoardTrustReviewResult, LineSummary,
    TrustChange, TrustChangeKind, TrustPackReview, TrustState,
};
use collide_core::trust::{ChangeKind, DiffDetail, InputDiff, LineDiff};
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

/// Why a request about a line has no answer. The daemon words each as an owner error.
#[derive(Debug, PartialEq, Eq)]
pub enum LineError {
    /// The ref is not in the repository.
    NotFound(String),
    /// The request is understood and refused: the line changed since it was shown, or it does not load.
    Refused(String),
    /// The request is malformed.
    Invalid(String),
    /// Something outside the request failed: git, a file, the loader.
    Failed(String),
}

pub struct Lines {
    home: Option<PathBuf>,
    load_context: LoadContextFactory,
    git_env: Vec<(String, String)>,
    workspaces: WorkspaceRoots,
}

/// The line a board's sources start from (ts-0113-ck): the line the checkout is on, loaded from the commit its ref
/// names, with its trust now. `problems` say why a trust state could not be read.
pub struct SourceLine {
    /// The repository's git directory, where the line's blobs are read.
    pub git_dir: PathBuf,
    pub ref_name: String,
    pub line: LoadedLine,
    pub trust: TrustState,
    pub problems: Vec<String>,
    /// The loader's context of the load: the built-in packs and the `PATH` a provider's `node` is found on.
    pub context: LoadContext,
}

/// What a request over one line starts from: the repository, the loader's context and the user's trust.
struct Opened {
    git_dir: PathBuf,
    context: LoadContext,
    trust: Trust,
    workspaces: Vec<PathBuf>,
}

impl Opened {
    fn load(&self, commit: &str) -> Result<LoadedLine, Vec<LoadProblem>> {
        load_line(&self.git_dir, commit, &self.context)
    }
}

impl Lines {
    pub fn new(
        home: Option<PathBuf>,
        load_context: LoadContextFactory,
        git_env: Vec<(String, String)>,
        workspaces: WorkspaceRoots,
    ) -> Lines {
        Lines {
            home,
            load_context,
            git_env,
            workspaces,
        }
    }

    /// The home folder the user's files are in, when the daemon knows one.
    pub fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }

    fn open(&self, board: BoardRef, tools: &OsStr) -> Result<Opened, LineError> {
        let home = self.home.as_ref().ok_or_else(|| {
            LineError::Failed(
                "the daemon has no home folder, so there is nowhere to keep what you trust"
                    .to_string(),
            )
        })?;
        let git_dir = boards::git_dir_of(board.root, &self.git_env).map_err(LineError::Failed)?;
        let context = (self.load_context)(tools)
            .map_err(|message| LineError::Failed(format!("the loader cannot start: {message}")))?;
        let workspaces = (self.workspaces)();
        let scope = UserScope::new(home.clone(), board.id.to_string(), workspaces.clone());
        let trust =
            Trust::new(TrustStore::new(user_config_dir(home))).with_git(context.git.clone());
        Ok(Opened {
            git_dir,
            context: context.with_user(scope),
            trust,
            workspaces,
        })
    }

    /// The line of the checkout `board.root` is on, read as the daemon starts sources from it: the commit the
    /// checkout's ref names now, never the working copy, and its trust. A line that does not load is
    /// [`LineError::Refused`], one problem on each line of the message.
    pub fn for_sources(&self, board: BoardRef, tools: &OsStr) -> Result<SourceLine, LineError> {
        let checkout = boards::checkout_of(board.root, &self.git_env).map_err(LineError::Failed)?;
        let commit = boards::line_commit(&checkout, &self.git_env).ok_or_else(|| {
            LineError::Failed(format!(
                "the checkout `{}` has no commit yet, so it has no line",
                checkout.root.display()
            ))
        })?;
        let opened = self.open(board, tools)?;
        let line = opened
            .load(&commit)
            .map_err(|problems| LineError::Refused(texts(&problems).join("\n")))?;
        let root = board.root.display().to_string();
        let (trust, problems) = state_of(&opened, &root, checkout.line_branch(), &line);
        Ok(SourceLine {
            git_dir: opened.git_dir.clone(),
            ref_name: checkout.line_branch().to_string(),
            line,
            trust,
            problems,
            context: opened.context,
        })
    }

    /// The line Start with defaults reads (ts-0215-gt): the built-in packs and nothing of the person's or the repo's.
    /// It never opens the user's files, the trust store or the repository, and it is untrusted with no hash, so no
    /// pack of the repo's would load from it. The git directory is empty: nothing is read from it.
    pub fn for_defaults(&self, tools: &OsStr) -> Result<SourceLine, LineError> {
        let context = (self.load_context)(tools)
            .map_err(|message| LineError::Failed(format!("the loader cannot start: {message}")))?;
        let line = load_defaults(&context)
            .map_err(|problems| LineError::Refused(texts(&problems).join("\n")))?;
        Ok(SourceLine {
            git_dir: PathBuf::new(),
            ref_name: "defaults".to_string(),
            line,
            trust: unknown_trust(),
            problems: Vec::new(),
            context,
        })
    }

    /// The commit `line` names, or why it names none.
    fn resolve(&self, opened: &Opened, line: &str) -> Result<String, LineError> {
        opened
            .context
            .git
            .resolve_commit(&opened.git_dir, line)
            .map_err(|error| {
                LineError::NotFound(format!("the repository has no line `{line}`: {error}"))
            })
    }

    /// The board `id` with a line for each of `checkouts`, the default line (the board's own checkout) first: the ref
    /// a checkout is on, the commit that names and the trust of what the commit holds. A linked worktree the board was
    /// opened from is a line of it (ts-0191-nv); its trust is the board's, kept at the board's root. A folder that is
    /// not a checkout, and a checkout with no commit, have no line.
    pub fn summary(
        &self,
        id: &str,
        root: &str,
        checkouts: &[Checkout],
        tools: &OsStr,
    ) -> BoardSummary {
        let lines = checkouts
            .iter()
            .filter_map(|checkout| self.line_of(id, root, checkout, tools))
            .collect();
        BoardSummary {
            id: id.to_string(),
            root: root.to_string(),
            lines,
            sources_unread: None,
            missing: false,
            missing_reason: None,
            last_opened_ms: None,
        }
    }

    /// [`Lines::summary`] for a defaults connection: the same lines, each untrusted with no hash and no problem. The
    /// user's files, the trust store and the line's files are not read, so nothing of them can show.
    pub fn summary_defaults(&self, id: &str, root: &str, checkouts: &[Checkout]) -> BoardSummary {
        let lines = checkouts
            .iter()
            .filter_map(|checkout| {
                Some(LineSummary {
                    ref_name: checkout.line_branch().to_string(),
                    root: checkout.root.display().to_string(),
                    snapshot: Some(boards::line_commit(checkout, &self.git_env)?),
                    trust: unknown_trust(),
                    problems: Vec::new(),
                })
            })
            .collect();
        BoardSummary {
            id: id.to_string(),
            root: root.to_string(),
            lines,
            sources_unread: None,
            missing: false,
            missing_reason: None,
            last_opened_ms: None,
        }
    }

    /// What `board.trust_review` answers a defaults connection: nothing to review, and no hash to trust.
    pub fn review_defaults(&self) -> BoardTrustReviewResult {
        BoardTrustReviewResult {
            trust: unknown_trust(),
            problems: Vec::new(),
            changes: Vec::new(),
            packs: Vec::new(),
            hosts: Vec::new(),
            dev_folders: Vec::new(),
        }
    }

    /// Whether the user's files already hold a folder for a board named `name` (its views and packs). A new board never
    /// takes such a name, so a store that was lost cannot give a board the files of an earlier one.
    pub fn board_folder_exists(&self, name: &str) -> bool {
        let Some(home) = &self.home else {
            return false;
        };
        match UserScope::new(home.clone(), name.to_string(), Vec::new()).board_dir() {
            Ok(dir) => dir.exists(),
            // A name that cannot be a folder is never used as one.
            Err(_) => false,
        }
    }

    /// Carries what was trusted for the board at `from` to the board at `to`, for a board that was located (0.2,
    /// Boards and lines). Nothing is written when there is nothing to carry.
    pub fn move_trust(&self, from: &str, to: &str) -> Result<(), LineError> {
        let home = self.home.as_ref().ok_or_else(|| {
            LineError::Failed(
                "the daemon has no home folder, so there is nowhere to keep what you trust"
                    .to_string(),
            )
        })?;
        Trust::new(TrustStore::new(user_config_dir(home)))
            .move_board(from, to)
            .map_err(|error| LineError::Failed(error.to_string()))
    }

    fn line_of(
        &self,
        id: &str,
        root: &str,
        checkout: &Checkout,
        tools: &OsStr,
    ) -> Option<LineSummary> {
        let snapshot = boards::line_commit(checkout, &self.git_env)?;
        let summary = |trust, problems| LineSummary {
            ref_name: checkout.line_branch().to_string(),
            root: checkout.root.display().to_string(),
            snapshot: Some(snapshot.clone()),
            trust,
            problems,
        };
        let unknown = TrustState::Untrusted {
            hash: String::new(),
        };
        // The git directory is the board's, whichever checkout the line is: a linked worktree's own is a folder
        // inside it.
        let board = BoardRef {
            id,
            root: Path::new(root),
        };
        let opened = match self.open(board, tools) {
            Ok(opened) => opened,
            Err(LineError::Failed(message)) => return Some(summary(unknown, vec![message])),
            Err(other) => return Some(summary(unknown, vec![format!("{other:?}")])),
        };
        match opened.load(&snapshot) {
            Err(problems) => Some(summary(unknown, texts(&problems))),
            Ok(line) => {
                let (trust, mut problems) = state_of(&opened, root, checkout.line_branch(), &line);
                problems.extend(source_problems(&line));
                Some(summary(trust, problems))
            }
        }
    }

    /// `board.trust`: trusts the line `params.line` at `params.hash`. See the module for the approvals.
    pub fn trust(
        &self,
        board: BoardRef,
        params: &BoardTrustParams,
        tools: &OsStr,
    ) -> Result<BoardTrustResult, LineError> {
        let opened = self.open(board, tools)?;
        let commit = self.resolve(&opened, &params.line)?;
        let root = board.root.display().to_string();
        if params.hash.is_empty() && params.hosts.is_empty() && params.dev_folders.is_empty() {
            return Err(LineError::Invalid(
                "there is no hash and nothing approved: take the hash from board.trust_review"
                    .to_string(),
            ));
        }
        if let Err(problems) = opened.load(&commit) {
            approve(&opened, params, &problems)?;
        }
        let line = match opened.load(&commit) {
            Ok(line) => line,
            Err(_) if params.hash.is_empty() => {
                return Ok(BoardTrustResult {
                    trust: TrustState::Untrusted {
                        hash: String::new(),
                    },
                });
            }
            Err(problems) => return Err(LineError::Refused(texts(&problems).join("\n"))),
        };
        let site = LineSite {
            repo: &opened.git_dir,
            root: &root,
            ref_name: &params.line,
        };
        if params.hash.is_empty() {
            let (trust, _) = state_of(&opened, &root, &params.line, &line);
            return Ok(BoardTrustResult { trust });
        }
        match opened.trust.trust(&site, &line, &params.hash) {
            Ok(trust) => Ok(BoardTrustResult { trust }),
            Err(TrustError::Stale { given, current }) => Err(LineError::Refused(format!(
                "the line changed since it was reviewed: it hashes to {current}, not {given}. Review it again"
            ))),
            Err(other) => Err(LineError::Failed(other.to_string())),
        }
    }

    /// Records as trusted the files `board.init` wrote, for the line the board's checkout is on (ts-0194-qp), so the
    /// person's own commit of them asks nothing. `files` are named relative to the Collide folder. It records nothing,
    /// and says so with `false`, when the line at its commit already has a Collide folder (what is there is not the
    /// person's own work) or does not load. A failure to read or write the trust file is the error.
    pub fn trust_written(
        &self,
        board: BoardRef,
        tools: &OsStr,
        files: &[(String, Vec<u8>)],
    ) -> Result<bool, LineError> {
        let checkout = boards::checkout_of(board.root, &self.git_env).map_err(LineError::Failed)?;
        let Some(commit) = boards::line_commit(&checkout, &self.git_env) else {
            return Ok(false);
        };
        let opened = self.open(board, tools)?;
        let Ok(line) = opened.load(&commit) else {
            return Ok(false);
        };
        if line.collide_dir.is_some() {
            return Ok(false);
        }
        let root = board.root.display().to_string();
        let site = LineSite {
            repo: &opened.git_dir,
            root: &root,
            ref_name: checkout.line_branch(),
        };
        opened
            .trust
            .trust_written(&site, &line, files)
            .map_err(|error| LineError::Failed(error.to_string()))?;
        Ok(true)
    }

    /// `board.trust_review`: what a person sees before they trust the line `line`. It writes nothing.
    pub fn review(
        &self,
        board: BoardRef,
        line: &str,
        tools: &OsStr,
    ) -> Result<BoardTrustReviewResult, LineError> {
        let opened = self.open(board, tools)?;
        let commit = self.resolve(&opened, line)?;
        let root = board.root.display().to_string();
        let loaded = match opened.load(&commit) {
            Ok(loaded) => loaded,
            Err(problems) => {
                return Ok(BoardTrustReviewResult {
                    trust: TrustState::Untrusted {
                        hash: String::new(),
                    },
                    hosts: sorted(problems.iter().filter_map(|p| p.needs_host.clone())),
                    dev_folders: sorted(problems.iter().filter_map(|p| p.needs_dev_trust.clone())),
                    problems: texts(&problems),
                    changes: Vec::new(),
                    packs: Vec::new(),
                });
            }
        };
        let (trust, mut problems) = state_of(&opened, &root, line, &loaded);
        problems.extend(source_problems(&loaded));
        let site = LineSite {
            repo: &opened.git_dir,
            root: &root,
            ref_name: line,
        };
        let changes = match opened.trust.changes_since_trust_with_mirrors(
            &site,
            &loaded,
            &opened.context.cache,
        ) {
            Ok(found) => found.into_iter().map(wire_change).collect(),
            Err(error) => {
                problems.push(error.to_string());
                Vec::new()
            }
        };
        let packs = loaded
            .packs
            .iter()
            .filter(|pack| matches!(pack.origin, PackOrigin::Repo | PackOrigin::Git))
            .map(|pack| {
                let source = pack.source.clone().unwrap_or_default();
                let commit = loaded
                    .lock
                    .packs
                    .iter()
                    .find(|entry| entry.source == source)
                    .map(|entry| entry.commit.as_str());
                TrustPackReview {
                    review: review_data(
                        &pack.manifest,
                        commit,
                        pack.pin.as_deref().unwrap_or_default(),
                    ),
                    source,
                }
            })
            .collect();
        Ok(BoardTrustReviewResult {
            trust,
            problems: dedupe(problems),
            changes,
            packs,
            hosts: Vec::new(),
            dev_folders: Vec::new(),
        })
    }
}

/// Untrusted at an empty hash: what a line is when there is nothing to trust yet.
fn unknown_trust() -> TrustState {
    TrustState::Untrusted {
        hash: String::new(),
    }
}

/// Records the hosts and `dev:` folders `params` approves that the failed load needs, and no others.
fn approve(
    opened: &Opened,
    params: &BoardTrustParams,
    problems: &[LoadProblem],
) -> Result<(), LineError> {
    let hosts: BTreeSet<&str> = problems
        .iter()
        .filter_map(|problem| problem.needs_host.as_deref())
        .collect();
    for host in params
        .hosts
        .iter()
        .filter(|host| hosts.contains(host.as_str()))
    {
        let setup = opened.context.fetch.as_ref().ok_or_else(|| {
            LineError::Failed(
                "this daemon cannot fetch git packs: it has no fetch setup".to_string(),
            )
        })?;
        setup
            .hosts
            .record(host)
            .map_err(|error| LineError::Failed(error.to_string()))?;
    }
    let folders: BTreeSet<&str> = problems
        .iter()
        .filter_map(|problem| problem.needs_dev_trust.as_deref())
        .collect();
    for folder in params
        .dev_folders
        .iter()
        .filter(|folder| folders.contains(folder.as_str()))
    {
        opened
            .trust
            .trust_dev(std::path::Path::new(folder), &opened.workspaces)
            .map_err(|error| LineError::Failed(error.to_string()))?;
    }
    Ok(())
}

/// The trust of a loaded line, and why it is untrusted when the store cannot say.
fn state_of(
    opened: &Opened,
    root: &str,
    ref_name: &str,
    line: &LoadedLine,
) -> (TrustState, Vec<String>) {
    let site = LineSite {
        repo: &opened.git_dir,
        root,
        ref_name,
    };
    let mut problems = Vec::new();
    let state = match opened.trust.trust_state(&site, line) {
        Ok(state) => state,
        Err(error) => {
            problems.push(error.to_string());
            TrustState::Untrusted {
                hash: String::new(),
            }
        }
    };
    if let Some(error) = opened.trust.store_problem() {
        problems.push(error.to_string());
    }
    (state, problems)
}

fn texts(problems: &[LoadProblem]) -> Vec<String> {
    problems.iter().map(ToString::to_string).collect()
}

fn sorted(items: impl Iterator<Item = String>) -> Vec<String> {
    items.collect::<BTreeSet<_>>().into_iter().collect()
}

fn dedupe(items: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    items
        .into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

fn wire_change(diff: InputDiff) -> TrustChange {
    let kind = match diff.change.kind {
        ChangeKind::Added => TrustChangeKind::Added,
        ChangeKind::Removed => TrustChangeKind::Removed,
        ChangeKind::Changed => TrustChangeKind::Changed,
    };
    let mut change = TrustChange {
        name: diff.change.name,
        kind,
        files: Vec::new(),
        lines: Vec::new(),
        note: None,
    };
    match diff.detail {
        DiffDetail::None => {}
        DiffDetail::Files(files) => change.files = file_changes(files),
        DiffDetail::Lines(LineDiff {
            lines,
            omitted,
            binary,
        }) => {
            change.lines = lines;
            let mut notes = Vec::new();
            if binary {
                notes.push("a binary file".to_string());
            }
            if omitted > 0 {
                notes.push(format!("{omitted} more changed lines are not shown"));
            }
            change.note = (!notes.is_empty()).then(|| notes.join("; "));
        }
        DiffDetail::Unavailable(why) => change.note = Some(why),
    }
    change
}
