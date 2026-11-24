//! The corpus (0.10, Identity, Status categories, Ref-scoped sources): what a board holds, merged from what each
//! source's `list` returned for each checkout. It is a pure structure. It reads no file, starts no process and
//! keeps nothing on disk: the daemon (`ts-0116-wh`) and the tests feed it data.
//!
//! What it answers:
//!
//! - **One task for each key.** A key is `source:shortId`. Its copies are held by checkout, and the board shows
//!   one: the copy at the default line's root, else the first checkout in path order, marked `branch_only`.
//! - **Divergence.** A workspace's copy whose state differs from the copy at the root it was forked from. The state
//!   of another line's root is a copy, never a divergence.
//! - **Collisions.** One key at different paths in different checkouts. It is never a divergence, and a lookup of
//!   the key is [`Found::Collided`].
//! - **Categories.** Each status maps to its category through the source's declared statuses. A status the source
//!   did not declare makes the item an invalid entry, with the status named, and the task is not listed.
//! - **Caps.** A task whose lists or strings pass the caps of 0.9 (`crate::view_expr::check_task_caps`) is an invalid
//!   item, with the reason, and is not listed: a view never has to bind it.
//! - **Links**, one hop out, both directions, across sources ([`Corpus::links_of`]). A link to a key the corpus does
//!   not hold is reported with `missing` set, never dropped. A link whose type targets a feature (`pr-0003-km#F2`)
//!   goes to the proposal and names the feature; it is `missing` when the proposal is not held or has no such feature.
//! - **Updates.** Each notification of a source is applied by a method, and [`reconcile`] is the difference of a
//!   cached corpus and a fresh one, as the events core emits at start.
//!
//! **Scale.** A board has about 1,600 tasks, and a notification costs one task, not a pass over them. Every
//! notification therefore touches one entry and its indexes: `entries` by key, `names` (id, aliases and short id) by
//! name, `inbound` (the targets a task's links name) by target, and `locations` by (source, checkout, path). An entry
//! keeps what it put into the indexes, so [`Corpus::reindex`] removes exactly that and adds it again. Only a whole
//! `list` ([`Corpus::load`]), a view of every row ([`Corpus::rows`]) and [`reconcile`] go over all entries.
//!
//! **What it does not hold.** It holds a task as its source listed it, and no text beyond that. `latest_run`, the
//! runs and the store cache of the daemon are not here.

use crate::change::{FieldDiff, ItemState, Rename};
use crate::feature::parse_feature_target;
use crate::hook::HookLink;
use crate::item::{Category, KindDecl, Link, LinkDir, LinkTarget, LinkTypeDecl, StatusDecl};
use crate::owner::task::{
    BoardSource, DivergentCopy, StaleSource, TaskCopy, TaskDivergence, TaskGetResult,
    TaskListResult,
};
use crate::provider::{
    ChangeCause, Collection, CollectionsChanged, FieldDecl, InitializeResult, ListResult,
    ProviderEvent, RemovedCause, Scope, Task, TaskChanged, TaskCollision, TaskInvalid, TaskRemoved,
};
use crate::ready::UnknownTarget;
use crate::short_id;
use crate::view_expr::check_task_caps;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// How far a chain of renames is followed when an old key is resolved.
const MAX_REDIRECTS: usize = 16;

/// What a checkout is, for the corpus (0.10, Ref-scoped sources).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckoutKind {
    /// A line's root. `default` is the default line's root, which is the board root: the board shows its copy.
    Line { default: bool },
    /// A workspace forked from the root at `forked_from`: its copy is compared with that root's.
    Workspace { forked_from: String },
    /// Any other `git worktree list` entry that passes the board's checkout filter.
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckoutInfo {
    pub path: String,
    pub kind: CheckoutKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorpusError {
    /// A list or notification arrived for a source that was never declared with [`Corpus::declare_source`].
    UnknownSource(String),
}

impl fmt::Display for CorpusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CorpusError::UnknownSource(name) => {
                write!(f, "source `{name}` is not declared to the corpus")
            }
        }
    }
}

impl std::error::Error for CorpusError {}

/// A workspace's copy that differs from the copy at the root it was forked from (the stage's first criterion).
#[derive(Debug, Clone, PartialEq)]
pub struct Divergence {
    pub key: String,
    pub path: Option<String>,
    /// The workspace.
    pub checkout: String,
    pub state: ItemState,
    /// The root the workspace was forked from, and its state.
    pub root: String,
    pub root_state: ItemState,
}

/// One task as the board shows it. A key at two paths is a collision and has one row for each path, since it is no
/// one task.
#[derive(Debug, Clone, PartialEq)]
pub struct Row<'a> {
    /// The shown copy, with its category from the source's declared statuses.
    pub task: &'a Task,
    /// The shown copy is not the default root's: the task is in other checkouts only.
    pub branch_only: bool,
    pub divergence: Vec<Divergence>,
    pub collided: bool,
    /// The task's source is stale.
    pub stale: bool,
}

/// An item that could not be read as a task, with its source.
#[derive(Debug, Clone, PartialEq)]
pub struct Invalid<'a> {
    pub source: &'a str,
    pub item: &'a TaskInvalid,
}

/// What a query named.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    One(String),
    /// The key is at several paths: it is no one task.
    Collided(String, TaskCollision),
    /// The query fits several keys, in key order.
    Ambiguous(Vec<String>),
    None,
}

/// What `task.list` keeps of the corpus: with `ready`, the tasks the ready rule lists; with `kind`, the tasks of that
/// kind; with `checkout`, the tasks that have a copy there, each as that copy; with `only`, the tasks whose key it
/// holds (a view's members). The others apply on top of `only`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ListFilter<'a> {
    pub ready: bool,
    pub kind: Option<&'a str>,
    pub checkout: Option<&'a str>,
    pub only: Option<&'a BTreeSet<String>>,
}

/// One page of a list: at most `limit` tasks (all of them when `None`), after the task `cursor` names.
#[derive(Debug, Clone, Copy, Default)]
pub struct Page<'a> {
    pub limit: Option<usize>,
    pub cursor: Option<&'a str>,
}

/// What [`Corpus::apply_changed`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    Done,
    /// The task is new to the corpus and the notification carries only its state: the caller fetches that one task
    /// and applies the notification again with it.
    NeedsTask,
}

#[derive(Debug, Clone)]
struct SourceState {
    scope: Scope,
    id_pattern: String,
    short_id_pattern: String,
    statuses: Vec<StatusDecl>,
    category_status: BTreeMap<Category, String>,
    fields: Vec<FieldDecl>,
    kinds: Vec<KindDecl>,
    /// The link types the source declared, in its order.
    links: Vec<LinkTypeDecl>,
    /// The names of those that target a feature of a proposal.
    feature_links: BTreeSet<String>,
    stale: Option<String>,
    /// By (checkout, path): the source's own invalid items, and the tasks whose status was not declared.
    invalid: BTreeMap<(Option<String>, String), TaskInvalid>,
    collections: BTreeMap<Option<String>, Vec<Collection>>,
    /// The checkouts a `list` was loaded for.
    listed: BTreeSet<Option<String>>,
}

impl SourceState {
    fn new(init: &InitializeResult) -> SourceState {
        let mut state = SourceState {
            scope: init.scope,
            id_pattern: String::new(),
            short_id_pattern: String::new(),
            statuses: Vec::new(),
            category_status: BTreeMap::new(),
            fields: Vec::new(),
            kinds: Vec::new(),
            links: Vec::new(),
            feature_links: BTreeSet::new(),
            stale: None,
            invalid: BTreeMap::new(),
            collections: BTreeMap::new(),
            listed: BTreeSet::new(),
        };
        state.declare(init);
        state
    }

    fn declare(&mut self, init: &InitializeResult) {
        self.scope = init.scope;
        self.id_pattern = init.id_pattern.clone();
        self.short_id_pattern = init.short_id_pattern.clone();
        self.statuses = init.statuses.clone();
        self.category_status = init.category_status.clone();
        self.fields = init.fields.clone();
        self.kinds = init.kinds.clone();
        self.links = init.links.clone();
        self.feature_links = init
            .links
            .iter()
            .filter(|decl| decl.target == LinkTarget::Feature)
            .map(|decl| decl.name.clone())
            .collect();
    }

    /// The category the source declared for the task's status: from its kind's statuses when it declared that kind,
    /// else from its own. `None` when the source did not declare the status.
    fn category_of(&self, task: &Task) -> Option<Category> {
        let named = |statuses: &[StatusDecl]| {
            statuses
                .iter()
                .find(|decl| decl.name == task.state.status)
                .map(|decl| decl.category)
        };
        let kind = task
            .kind
            .as_deref()
            .and_then(|name| self.kinds.iter().find(|kind| kind.name == name));
        kind.and_then(|kind| named(&kind.statuses))
            .or_else(|| named(&self.statuses))
    }

    /// The kind of a task that names none: the source's one kind. Empty when it has several or none.
    fn kind_of(&self, task: &Task) -> String {
        match (&task.kind, self.kinds.as_slice()) {
            (Some(kind), _) => kind.clone(),
            (None, [only]) => only.name.clone(),
            (None, _) => String::new(),
        }
    }
}

/// One key: its copies by checkout (`None` for a source that is not ref-scoped), and what it put into the corpus's
/// indexes, so that a change removes exactly that.
#[derive(Debug, Clone)]
struct Entry {
    source: String,
    copies: BTreeMap<Option<String>, Task>,
    names: BTreeSet<String>,
    targets: BTreeSet<String>,
}

type Location = (String, Option<String>, String);

/// What a link points at, read from its `to`: the task's name, and for a feature link the feature (`F2`). A feature
/// link whose `to` is not a feature target has no feature and names `to` whole.
struct Reach<'a> {
    item: &'a str,
    feature: Option<String>,
}

impl<'a> Reach<'a> {
    fn of(link: &'a Link, feature_link: bool) -> Reach<'a> {
        if feature_link && let Ok(target) = parse_feature_target(&link.to) {
            let item = link
                .to
                .split_once('#')
                .map_or(link.to.as_str(), |(item, _)| item);
            return Reach {
                item,
                feature: Some(target.feature_id()),
            };
        }
        Reach {
            item: &link.to,
            feature: None,
        }
    }
}

/// A link a task holds, resolved: the key of its target when the corpus holds one, the key to show when it does not,
/// and the feature a feature link names.
struct OutLink<'a> {
    link: &'a Link,
    target: Option<String>,
    shown: String,
    feature: Option<String>,
}

/// What the ready rule found over the whole corpus ([`Corpus::readiness`]), as keys.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CorpusReadiness {
    pub ready: BTreeSet<String>,
    /// Each `blocked-by` cycle, its keys in order, one source after another.
    pub cycles: Vec<Vec<String>>,
    /// Each `blocked-by` that names no item, in the order of the items.
    pub unknown_blockers: Vec<UnknownTarget>,
}

/// What a board holds. See the module comment.
#[derive(Debug, Clone, Default)]
pub struct Corpus {
    checkouts: BTreeMap<String, CheckoutKind>,
    sources: BTreeMap<String, SourceState>,
    entries: BTreeMap<String, Entry>,
    /// An old key to the key it was renamed to. An entry under the old key wins.
    redirects: HashMap<String, String>,
    /// A name (id, alias, short id, bare and as `source:name`) to the keys that have it.
    names: HashMap<String, BTreeSet<String>>,
    /// The `source:target` that links name, to the keys of the tasks that hold such a link.
    inbound: HashMap<String, BTreeSet<String>>,
    /// (source, checkout, path) to the key of the copy there.
    locations: HashMap<Location, String>,
    /// What the corpus held last changed to, from one counter for the process. A copy has its original's stamp, so
    /// what was worked out from a stamp is still true of the copy. See [`Corpus::stamp`].
    stamp: u64,
}

/// The last stamp given: a stamp is never given twice, so two corpora with one stamp hold the same.
static LAST_STAMP: AtomicU64 = AtomicU64::new(0);

fn empty_list() -> TaskListResult {
    TaskListResult {
        tasks: Vec::new(),
        collections: Vec::new(),
        invalid: Vec::new(),
        next_cursor: None,
        divergent: Vec::new(),
        divergence: Vec::new(),
        sources: Vec::new(),
        collisions: Vec::new(),
        stale_sources: Vec::new(),
        not_started: Vec::new(),
        sources_started: 0,
        sources_unread: None,
        refreshing: false,
        view: None,
    }
}

/// A row's place in a list: its key, then its path, so that one key at two paths has two places.
fn cursor_of(row: &Row<'_>) -> String {
    format!(
        "{}\n{}",
        row.task.key,
        row.task.path.as_deref().unwrap_or("")
    )
}

fn after_cursor(row: &Row<'_>, cursor: &str) -> bool {
    cursor_of(row).as_str() > cursor
}

fn short_of(key: &str) -> &str {
    key.split_once(':').map_or(key, |(_, short)| short)
}

fn source_of_key(key: &str) -> Option<&str> {
    key.split_once(':').map(|(source, _)| source)
}

/// A link target as a key: a `to` with a `:` is one already, and any other is a name in the holder's source.
fn qualified(source: &str, to: &str) -> String {
    if to.contains(':') {
        to.to_string()
    } else {
        format!("{source}:{to}")
    }
}

fn names_of(source: &str, task: &Task) -> BTreeSet<String> {
    let bare = std::iter::once(task.id.as_str())
        .chain(task.aliases.iter().map(String::as_str))
        .chain(std::iter::once(short_of(&task.key)));
    let mut names = BTreeSet::new();
    for name in bare {
        names.insert(name.to_string());
        names.insert(format!("{source}:{name}"));
    }
    names
}

impl Corpus {
    // --- what the corpus is told ---

    /// A number that is new each time the corpus is told something, and the same for a copy. What was worked out
    /// from a corpus with this stamp (a view's members) holds for any corpus that has it.
    pub fn stamp(&self) -> u64 {
        self.stamp
    }

    fn touch(&mut self) {
        self.stamp = LAST_STAMP.fetch_add(1, Ordering::Relaxed) + 1;
    }

    /// The board's checkouts and what each is. They decide the shown copy and the divergences, and nothing is
    /// indexed by them, so this may be called again at any time.
    pub fn set_checkouts(&mut self, checkouts: Vec<CheckoutInfo>) {
        self.touch();
        self.checkouts = checkouts
            .into_iter()
            .map(|info| (info.path, info.kind))
            .collect();
    }

    /// Declares a source from its `initialize` result, or replaces its declaration. A new declaration applies to
    /// what is loaded or notified next; what is held keeps the category it was given.
    pub fn declare_source(&mut self, name: &str, init: &InitializeResult) {
        self.touch();
        match self.sources.get_mut(name) {
            Some(source) => source.declare(init),
            None => {
                self.sources
                    .insert(name.to_string(), SourceState::new(init));
            }
        }
    }

    /// The source's declared statuses, in its order. Empty for a source that is not declared.
    pub fn statuses(&self, source: &str) -> &[StatusDecl] {
        self.sources
            .get(source)
            .map_or(&[], |state| state.statuses.as_slice())
    }

    fn source(&self, name: &str) -> Result<&SourceState, CorpusError> {
        self.sources
            .get(name)
            .ok_or_else(|| CorpusError::UnknownSource(name.to_string()))
    }

    fn source_mut(&mut self, name: &str) -> Result<&mut SourceState, CorpusError> {
        self.sources
            .get_mut(name)
            .ok_or_else(|| CorpusError::UnknownSource(name.to_string()))
    }

    /// Replaces what the source held for `checkout` with one complete `list` (the caller joins the pages first). The
    /// other checkouts are left alone. `checkout` is `None` for a source that is not ref-scoped. A task listed twice
    /// in one checkout is the first one and an invalid item for the later one, as a source reports it.
    pub fn load(
        &mut self,
        source: &str,
        checkout: Option<&str>,
        list: ListResult,
    ) -> Result<(), CorpusError> {
        self.source(source)?;
        self.touch();
        let checkout = checkout.map(str::to_string);
        self.forget(source, &checkout);
        let state = self.source_mut(source)?;
        state.listed.insert(checkout.clone());
        state.collections.insert(checkout.clone(), list.collections);
        for mut item in list.invalid {
            item.checkout = item.checkout.or_else(|| checkout.clone());
            self.add_invalid(source, item);
        }
        let mut held: HashMap<String, Option<String>> = HashMap::new();
        for mut task in list.tasks {
            task.checkout = checkout.clone().or(task.checkout.take());
            if let Some(first) = held.get(&task.key) {
                let reason = match first {
                    Some(path) => format!(
                        "`{}` is already held at `{path}` in this checkout",
                        task.key
                    ),
                    None => format!("`{}` is already held in this checkout", task.key),
                };
                let path = task.path.clone().unwrap_or_else(|| task.key.clone());
                let item = TaskInvalid {
                    path,
                    checkout: task.checkout.clone(),
                    reason,
                };
                self.add_invalid(source, item);
                continue;
            }
            held.insert(task.key.clone(), task.path.clone());
            self.insert(source, task);
        }
        Ok(())
    }

    /// Drops what the source held for `checkout`: its copies, its invalid items and its collections.
    fn forget(&mut self, source: &str, checkout: &Option<String>) {
        let keys: Vec<String> = self
            .entries
            .iter()
            .filter(|(_, entry)| entry.source == source && entry.copies.contains_key(checkout))
            .map(|(key, _)| key.clone())
            .collect();
        for key in keys {
            self.take_copy(&key, checkout);
        }
        if let Some(state) = self.sources.get_mut(source) {
            state.invalid.retain(|(held, _), _| held != checkout);
            state.collections.remove(checkout);
            state.listed.remove(checkout);
        }
    }

    /// `task.changed`. The notification carries the new state, the version and a `diff`, and not the task. A copy the
    /// corpus holds is updated from it when the `diff` names no field but the status (see
    /// [`names_only_the_status`]); a `diff` that names any other field, and a task the corpus does not hold, need the
    /// task itself, and `task` is it (the caller asks for [`Applied::NeedsTask`] first, and fetches that one task). A
    /// `task` that is given replaces the copy. With no new state (`to` is null) the copy is removed. A status the
    /// source did not declare makes the item an invalid entry, and a change that is declared brings the path back.
    pub fn apply_changed(
        &mut self,
        source: &str,
        event: &TaskChanged,
        task: Option<Task>,
    ) -> Result<Applied, CorpusError> {
        self.apply(source, event, task, false)
    }

    /// `task.changed` for a held copy, taking only the new state and version from it, whatever its `diff` names. For
    /// a caller whose fetch of the task failed.
    pub fn apply_state(
        &mut self,
        source: &str,
        event: &TaskChanged,
    ) -> Result<Applied, CorpusError> {
        self.apply(source, event, None, true)
    }

    fn apply(
        &mut self,
        source: &str,
        event: &TaskChanged,
        task: Option<Task>,
        state_only: bool,
    ) -> Result<Applied, CorpusError> {
        self.source(source)?;
        self.touch();
        let key = self
            .resolve_key(&event.key)
            .unwrap_or_else(|| event.key.clone());
        let Some(to) = &event.to else {
            self.remove_copies(&key, event.checkout.as_deref());
            return Ok(Applied::Done);
        };
        let task = match task {
            Some(mut task) => {
                task.checkout = task.checkout.take().or_else(|| event.checkout.clone());
                task
            }
            None => {
                let held = self
                    .entries
                    .get(&key)
                    .and_then(|entry| entry.copies.get(&event.checkout));
                let Some(held) = held else {
                    return Ok(Applied::NeedsTask);
                };
                if !state_only && !names_only_the_status(event) {
                    return Ok(Applied::NeedsTask);
                }
                let mut task = held.clone();
                task.state = to.clone();
                task.version = event.version.clone();
                task
            }
        };
        self.insert(source, task);
        Ok(Applied::Done)
    }

    /// `collections.changed`: the checkout's collections are the ones in the notification, and each task it names is
    /// in the collections it says. Only a copy the corpus holds is changed: a key it does not hold, and a checkout it
    /// was never listed for, are left to the list that brings them. Returns the keys of the copies that changed.
    pub fn apply_collections(
        &mut self,
        source: &str,
        change: &CollectionsChanged,
    ) -> Result<Vec<String>, CorpusError> {
        self.source(source)?;
        self.touch();
        let checkout = change.checkout.clone();
        let state = self.source_mut(source)?;
        if !state.listed.contains(&checkout) {
            return Ok(Vec::new());
        }
        state
            .collections
            .insert(checkout.clone(), change.collections.clone());
        let mut moved = Vec::new();
        for member in &change.members {
            let key = self
                .resolve_key(&member.key)
                .unwrap_or_else(|| member.key.clone());
            let held = self
                .entries
                .get_mut(&key)
                .filter(|entry| entry.source == source)
                .and_then(|entry| entry.copies.get_mut(&checkout));
            if let Some(copy) = held
                && copy.collections != member.collections
            {
                copy.collections = member.collections.clone();
                moved.push(key);
            }
        }
        Ok(moved)
    }

    /// `task.renamed`. Both are keys. With a `checkout`, only that checkout's copy moves, and the old key goes on
    /// holding the others; with none, every copy moves. The old key resolves to the new one once it holds nothing.
    /// The copy keeps its `id` until its source lists it again: only the key is in the notification.
    pub fn apply_renamed(&mut self, source: &str, rename: &Rename) -> Result<(), CorpusError> {
        self.source(source)?;
        self.touch();
        let (from, to) = (&rename.from, &rename.to);
        let Some(entry) = self.entries.get(from) else {
            return Ok(());
        };
        if from == to {
            return Ok(());
        }
        let moving: Vec<Option<String>> = match &rename.checkout {
            None => entry.copies.keys().cloned().collect(),
            Some(checkout) => vec![Some(checkout.clone())],
        };
        for checkout in moving {
            if let Some(mut task) = self.take_copy(from, &checkout) {
                task.key = to.clone();
                self.place(source, task);
            }
        }
        if !self.entries.contains_key(from) && self.entries.contains_key(to) {
            self.redirects.insert(from.clone(), to.clone());
        }
        Ok(())
    }

    /// `task.removed`: the copy in the notification's checkout, or every copy when it names none.
    pub fn apply_removed(
        &mut self,
        source: &str,
        removed: &TaskRemoved,
    ) -> Result<(), CorpusError> {
        self.source(source)?;
        self.touch();
        if let Some(key) = self.resolve_key(&removed.key) {
            self.remove_copies(&key, removed.checkout.as_deref());
        }
        Ok(())
    }

    /// Drops the source's invalid items of `checkout` at `paths`, the files a write names. A write leaves each file it
    /// names valid, or gone (a file that was renamed), so an invalid item at one of them is of the file as it was, and
    /// the next list would drop it. A task that the write puts in is then held after this, and may add its own.
    pub fn clear_invalid(&mut self, source: &str, checkout: Option<&str>, paths: &[String]) {
        let Some(state) = self.sources.get_mut(source) else {
            return;
        };
        state
            .invalid
            .retain(|(held, path), _| held.as_deref() != checkout || !paths.contains(path));
        self.touch();
    }

    /// `task.invalid`: the item joins the source's invalid items, and the task at its path leaves the listing.
    pub fn apply_invalid(&mut self, source: &str, item: &TaskInvalid) -> Result<(), CorpusError> {
        self.source(source)?;
        self.touch();
        let at = (source.to_string(), item.checkout.clone(), item.path.clone());
        if let Some(key) = self.locations.get(&at).cloned() {
            self.take_copy(&key, &item.checkout);
        }
        self.add_invalid(source, item.clone());
        Ok(())
    }

    /// `source.stale`: what the source gave last may be out of date. Links to its tasks say so.
    pub fn mark_stale(&mut self, source: &str, reason: &str) -> Result<(), CorpusError> {
        self.source_mut(source)?.stale = Some(reason.to_string());
        self.touch();
        Ok(())
    }

    /// The source read again: it is no longer stale.
    pub fn mark_fresh(&mut self, source: &str) -> Result<(), CorpusError> {
        self.source_mut(source)?.stale = None;
        self.touch();
        Ok(())
    }

    // --- what the corpus holds ---

    /// The stale sources and why, in name order.
    pub fn stale_sources(&self) -> Vec<(&str, &str)> {
        self.sources
            .iter()
            .filter_map(|(name, state)| Some((name.as_str(), state.stale.as_deref()?)))
            .collect()
    }

    fn is_stale(&self, source: &str) -> bool {
        self.sources
            .get(source)
            .is_some_and(|state| state.stale.is_some())
    }

    /// Every source's invalid items, in source, checkout and path order.
    pub fn invalid(&self) -> Vec<Invalid<'_>> {
        self.sources
            .iter()
            .flat_map(|(source, state)| {
                state.invalid.values().map(move |item| Invalid {
                    source: source.as_str(),
                    item,
                })
            })
            .collect()
    }

    fn default_root(&self) -> Option<&str> {
        self.checkouts
            .iter()
            .find(|(_, kind)| matches!(kind, CheckoutKind::Line { default: true }))
            .map(|(path, _)| path.as_str())
    }

    /// The preferred of some things held by checkout: the one at the default root, else the first in path order.
    fn preferred<'a, T>(&self, items: impl Iterator<Item = (&'a Option<String>, T)>) -> Option<T> {
        let root = self.default_root();
        let mut first = None;
        for (checkout, item) in items {
            if checkout.is_some() && checkout.as_deref() == root {
                return Some(item);
            }
            first.get_or_insert(item);
        }
        first
    }

    fn shown<'a>(&self, entry: &'a Entry) -> &'a Task {
        self.preferred(entry.copies.iter())
            .expect("an entry always holds a copy")
    }

    /// The task rows of the board, in key order. A collided key has a row for each of its paths.
    pub fn rows(&self) -> Vec<Row<'_>> {
        self.rows_in(None)
    }

    /// The rows of one checkout, or of the board with `None`. With a checkout, a task is the copy that is there, and
    /// a task that has none there has no row.
    fn rows_in(&self, only: Option<&str>) -> Vec<Row<'_>> {
        let root = self.default_root();
        let mut rows = Vec::new();
        for (key, entry) in &self.entries {
            let collided = self.collision_of(key).is_some();
            let divergences = self.divergences_of(key);
            let stale = self.is_stale(&entry.source);
            let mut by_path: BTreeMap<Option<&str>, Vec<(&Option<String>, &Task)>> =
                BTreeMap::new();
            for (checkout, task) in &entry.copies {
                if only.is_some() && checkout.as_deref() != only {
                    continue;
                }
                by_path
                    .entry(task.path.as_deref())
                    .or_default()
                    .push((checkout, task));
            }
            for (path, group) in by_path {
                let task = self
                    .preferred(group.into_iter())
                    .expect("a group holds a copy");
                rows.push(Row {
                    task,
                    branch_only: task.checkout.is_some() && task.checkout.as_deref() != root,
                    divergence: divergences
                        .iter()
                        .filter(|divergence| divergence.path.as_deref() == path)
                        .cloned()
                        .collect(),
                    collided,
                    stale,
                });
            }
        }
        rows
    }

    /// The copies of a key that differ from the copy at the root they were forked from.
    fn divergences_of(&self, key: &str) -> Vec<Divergence> {
        let Some(entry) = self.entries.get(key) else {
            return Vec::new();
        };
        let mut found = Vec::new();
        for (checkout, task) in &entry.copies {
            let Some(checkout) = checkout else { continue };
            let Some(CheckoutKind::Workspace { forked_from }) = self.checkouts.get(checkout) else {
                continue;
            };
            let Some(root) = entry.copies.get(&Some(forked_from.clone())) else {
                continue;
            };
            if root.path == task.path && root.state != task.state {
                found.push(Divergence {
                    key: key.to_string(),
                    path: task.path.clone(),
                    checkout: checkout.clone(),
                    state: task.state.clone(),
                    root: forked_from.clone(),
                    root_state: root.state.clone(),
                });
            }
        }
        found
    }

    /// Every divergence of the board, in key and checkout order.
    pub fn divergences(&self) -> Vec<Divergence> {
        self.entries
            .keys()
            .flat_map(|key| self.divergences_of(key))
            .collect()
    }

    /// The collision of a key: its short id at different paths in different checkouts.
    fn collision_of(&self, key: &str) -> Option<TaskCollision> {
        let entry = self.entries.get(key)?;
        let places: Vec<_> = entry
            .copies
            .iter()
            .filter_map(|(checkout, task)| {
                Some(crate::provider::Location {
                    checkout: checkout.clone()?,
                    path: task.path.clone()?,
                })
            })
            .collect();
        let paths: BTreeSet<&str> = places.iter().map(|place| place.path.as_str()).collect();
        (paths.len() > 1).then(|| TaskCollision {
            short_id: short_of(key).to_string(),
            locations: places,
        })
    }

    /// Every collision of the board, in key order.
    pub fn collisions(&self) -> Vec<TaskCollision> {
        self.entries
            .keys()
            .filter_map(|key| self.collision_of(key))
            .collect()
    }

    // --- lookup ---

    /// The key an entry is held under: `key` itself, else what it was renamed to. No id, alias or short id is read as
    /// a key: for those, use [`Corpus::lookup`].
    pub fn resolve_key(&self, key: &str) -> Option<String> {
        let mut at = key;
        for _ in 0..MAX_REDIRECTS {
            if self.entries.contains_key(at) {
                return Some(at.to_string());
            }
            at = self.redirects.get(at)?;
        }
        None
    }

    /// Finds what `query` names: a key (an old one too), then an id, an alias or a short id, then the short form of
    /// a number (`12`, `12-ua`, as `short_id::resolve` reads it). One name that fits several keys is `Ambiguous`,
    /// never a guess.
    pub fn lookup(&self, query: &str) -> Found {
        if let Some(key) = self.resolve_key(query) {
            return self.found_one(key);
        }
        let keys = match self.names.get(query) {
            Some(keys) => keys.clone(),
            None => {
                let matching = short_id::matching_ids(self.names.keys().map(String::as_str), query);
                matching
                    .into_iter()
                    .flat_map(|name| self.names[name].iter().cloned())
                    .collect()
            }
        };
        let mut keys = keys.into_iter();
        match (keys.next(), keys.next()) {
            (None, _) => Found::None,
            (Some(key), None) => self.found_one(key),
            (Some(first), Some(second)) => {
                let mut all: BTreeSet<String> = BTreeSet::from([first, second]);
                all.extend(keys);
                Found::Ambiguous(all.into_iter().collect())
            }
        }
    }

    fn found_one(&self, key: String) -> Found {
        match self.collision_of(&key) {
            Some(collision) => Found::Collided(key, collision),
            None => Found::One(key),
        }
    }

    fn copies_of(&self, key: &str) -> Vec<TaskCopy> {
        let Some(entry) = self.entries.get(key) else {
            return Vec::new();
        };
        entry
            .copies
            .iter()
            .filter_map(|(checkout, task)| {
                Some(TaskCopy {
                    checkout: checkout.clone()?,
                    path: task.path.clone(),
                    state: task.state.clone(),
                    version: task.version.clone(),
                })
            })
            .collect()
    }

    /// `task.get` for one query, without the runs: the shown copy, every copy, and the collision. A query that
    /// fits several keys, or a key that is collided, has no `task` and names the keys in `ambiguous`. A query that
    /// fits nothing is an answer with nothing in it.
    pub fn get(&self, query: &str) -> TaskGetResult {
        let nothing = TaskGetResult {
            task: None,
            ambiguous: Vec::new(),
            copies: Vec::new(),
            collisions: Vec::new(),
            runs: Vec::new(),
            latest_run: None,
            links: Vec::new(),
            ops: Vec::new(),
            link_types: Vec::new(),
            views: Vec::new(),
        };
        match self.lookup(query) {
            Found::None => nothing,
            Found::Ambiguous(keys) => TaskGetResult {
                ambiguous: keys,
                ..nothing
            },
            Found::Collided(key, collision) => TaskGetResult {
                ambiguous: vec![key.clone()],
                copies: self.copies_of(&key),
                collisions: vec![collision],
                ..nothing
            },
            Found::One(key) => TaskGetResult {
                task: Some(self.shown(&self.entries[&key]).clone()),
                copies: self.copies_of(&key),
                links: self.titled(self.links_of(&key)),
                link_types: self.link_types_of(&key),
                ..nothing
            },
        }
    }

    /// `task.list` for the whole corpus: the shown tasks with no body, the collections of each source as its shown
    /// checkout listed them, and every invalid item.
    pub fn list_result(&self) -> TaskListResult {
        let tasks = self
            .rows()
            .into_iter()
            .map(|row| Task {
                body: None,
                ..row.task.clone()
            })
            .collect();
        let collections = self
            .sources
            .values()
            .filter_map(|state| self.preferred(state.collections.iter()))
            .flatten()
            .cloned()
            .collect();
        TaskListResult {
            tasks,
            collections,
            invalid: self
                .invalid()
                .into_iter()
                .map(|entry| entry.item.clone())
                .collect(),
            next_cursor: None,
            divergent: Vec::new(),
            divergence: Vec::new(),
            sources: Vec::new(),
            collisions: Vec::new(),
            stale_sources: Vec::new(),
            not_started: Vec::new(),
            sources_started: 0,
            sources_unread: None,
            refreshing: false,
            view: None,
        }
    }

    /// `task.list`: the rows that pass `filter`, in key then path order, one page of them. The cursor names the last
    /// row of the page before, so a change to that row, or its removal, moves no other row. The board-wide lists
    /// (collections, invalid items, divergent keys, collisions, stale sources) come with the first page only.
    /// `refreshing`, `not_started`, `sources_started` and `sources_unread` are the caller's to set.
    pub fn list(&self, filter: &ListFilter<'_>, page: &Page<'_>) -> TaskListResult {
        let ready = filter.ready.then(|| self.ready_ids());
        let mut rows: Vec<Row<'_>> = self
            .rows_in(filter.checkout)
            .into_iter()
            .filter(|row| {
                let source = source_of_key(&row.task.key).unwrap_or_default();
                filter
                    .kind
                    .is_none_or(|kind| self.kind_of(source, row.task) == kind)
                    && filter.only.is_none_or(|keys| keys.contains(&row.task.key))
                    && ready.as_ref().is_none_or(|ids| ids.contains(&row.task.key))
            })
            .collect();
        if let Some(cursor) = page.cursor {
            rows.retain(|row| after_cursor(row, cursor));
        }
        let more = page.limit.is_some_and(|limit| rows.len() > limit);
        if let Some(limit) = page.limit {
            rows.truncate(limit);
        }
        let next_cursor = more
            .then(|| rows.last().map(|row| cursor_of(row)))
            .flatten();
        let first = page.cursor.is_none();
        let mut result = if first {
            self.list_result()
        } else {
            TaskListResult { ..empty_list() }
        };
        result.tasks = rows
            .into_iter()
            .map(|row| Task {
                body: None,
                ..row.task.clone()
            })
            .collect();
        result.next_cursor = next_cursor;
        if first {
            result.divergent = self.divergent_keys();
            result.divergence = self.divergence_entries();
            result.sources = self.declarations();
            result.collisions = self.collisions();
            result.stale_sources = self
                .stale_sources()
                .into_iter()
                .map(|(source, reason)| StaleSource {
                    source: source.to_string(),
                    reason: reason.to_string(),
                })
                .collect();
        }
        result
    }

    /// The names of the kinds the sources declare, in each source's order, a name once. A source that declares none
    /// (the scratch source) adds none.
    pub fn declared_kinds(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        for state in self.sources.values() {
            for kind in &state.kinds {
                if !names.contains(&kind.name) {
                    names.push(kind.name.clone());
                }
            }
        }
        names
    }

    /// The kind of a task: its own, else its source's one kind, else empty (a view's `kind` variable).
    pub fn kind_of(&self, source: &str, task: &Task) -> String {
        self.sources
            .get(source)
            .map_or_else(String::new, |state| state.kind_of(task))
    }

    /// The ready rule over the corpus, each source on its own: a `blocked-by` target is resolved as `follow` resolves
    /// a link ([`Corpus::resolve_target`]), and a target that lands in another source is no item of this one. Keys
    /// are qualified (`plan:ts-0001-aa`). The `to` of an unknown blocker stays as it was written.
    pub fn readiness(&self) -> CorpusReadiness {
        let mut found = CorpusReadiness::default();
        for source in self.sources.keys() {
            let tasks: Vec<Task> = self
                .entries
                .values()
                .filter(|entry| &entry.source == source)
                .map(|entry| self.shown(entry).clone())
                .collect();
            let result = crate::ready::readiness(&tasks, |to| self.resolve_target(source, to));
            found.ready.extend(result.ready);
            found.cycles.extend(result.cycles);
            found.unknown_blockers.extend(result.unknown_targets);
        }
        found
    }

    /// The keys the ready rule lists.
    fn ready_ids(&self) -> BTreeSet<String> {
        self.readiness().ready
    }

    /// What each declared source gave at `initialize`, for the Field: the statuses in the source's order, its fields
    /// and its id patterns, in source-name order. `pack` is the caller's to set, since the corpus does not know packs.
    pub fn declarations(&self) -> Vec<BoardSource> {
        self.sources
            .iter()
            .map(|(name, state)| BoardSource {
                name: name.clone(),
                pack: None,
                scope: state.scope,
                statuses: state.statuses.clone(),
                category_status: state.category_status.clone(),
                fields: state.fields.clone(),
                blocking_links: state.links.iter().any(|decl| decl.blocking),
                id_pattern: state.id_pattern.clone(),
                short_id_pattern: state.short_id_pattern.clone(),
            })
            .collect()
    }

    /// The divergent keys with the copies that differ, in key order and then checkout order. `run` is the caller's to
    /// set, since the corpus does not know which run made a workspace.
    pub fn divergence_entries(&self) -> Vec<TaskDivergence> {
        self.entries
            .keys()
            .filter_map(|key| {
                let copies: Vec<DivergentCopy> = self
                    .divergences_of(key)
                    .into_iter()
                    .map(|found| DivergentCopy {
                        checkout: found.checkout,
                        status: found.state.status,
                        category: found.state.category,
                        run: None,
                    })
                    .collect();
                (!copies.is_empty()).then(|| TaskDivergence {
                    key: key.clone(),
                    copies,
                })
            })
            .collect()
    }

    fn divergent_keys(&self) -> Vec<String> {
        self.entries
            .keys()
            .filter(|key| !self.divergences_of(key).is_empty())
            .cloned()
            .collect()
    }

    /// The keys a key was renamed from and that are not held on their own, in key order.
    pub fn former_keys(&self, key: &str) -> Vec<String> {
        let Some(key) = self.resolve_key(key) else {
            return Vec::new();
        };
        let mut old: Vec<String> = self
            .names_for(&key)
            .into_iter()
            .filter(|name| self.redirects.contains_key(name) && !self.entries.contains_key(name))
            .collect();
        old.sort();
        old
    }

    /// The copy of `key` in `checkout` (`None` for a source that is not ref-scoped), with no body.
    pub fn copy(&self, key: &str, checkout: Option<&str>) -> Option<&Task> {
        self.entries
            .get(key)?
            .copies
            .iter()
            .find(|(held, _)| held.as_deref() == checkout)
            .map(|(_, task)| task)
    }

    // --- links ---

    /// The key a link's target names: a `to` with a `:` is a key (an old one too), and any other is a name in
    /// `source`. A name that fits several tasks is not resolved, so the link is reported as missing.
    pub fn resolve_target(&self, source: &str, to: &str) -> Option<String> {
        let target = qualified(source, to);
        if let Some(key) = self.resolve_key(&target) {
            return Some(key);
        }
        let mut keys = self.names.get(&target)?.iter();
        match (keys.next(), keys.next()) {
            (Some(key), None) => Some(key.clone()),
            _ => None,
        }
    }

    /// The link types the source of `key` declared, so a reader can name a link from its other end.
    fn link_types_of(&self, key: &str) -> Vec<LinkTypeDecl> {
        let source = &self.entries[key].source;
        self.sources
            .get(source)
            .map_or_else(Vec::new, |state| state.links.clone())
    }

    /// The links a task holds, each with the key of its target when the corpus holds one. A link of a type that
    /// targets a feature goes to the proposal, and names the feature.
    fn out_links<'a>(&self, source: &str, task: &'a Task) -> Vec<OutLink<'a>> {
        let features = self.sources.get(source).map(|state| &state.feature_links);
        task.links
            .iter()
            .map(|link| {
                let feature_link = features.is_some_and(|set| set.contains(&link.link_type));
                let reach = Reach::of(link, feature_link);
                // A feature link whose target does not read points at nothing, whatever `to` names.
                let reads = !feature_link || reach.feature.is_some();
                OutLink {
                    target: reads
                        .then(|| self.resolve_target(source, reach.item))
                        .flatten(),
                    shown: qualified(source, reach.item),
                    feature: reach.feature,
                    link,
                }
            })
            .collect()
    }

    /// Whether the shown copy of `key` has the feature `feature` (`F2`).
    fn has_feature(&self, key: &str, feature: &str) -> bool {
        self.shown(&self.entries[key])
            .features
            .iter()
            .any(|held| held.id == feature)
    }

    fn link_to(
        &self,
        link_type: &str,
        dir: LinkDir,
        key: &str,
        feature: Option<String>,
    ) -> HookLink {
        let entry = &self.entries[key];
        let task = self.shown(entry);
        // The other end of an incoming link is the task that holds it, so only an outgoing one can name a feature
        // the proposal lacks.
        let missing = dir == LinkDir::Out
            && feature
                .as_deref()
                .is_some_and(|feature| !self.has_feature(key, feature));
        HookLink {
            link_type: link_type.to_string(),
            dir,
            key: key.to_string(),
            kind: self.sources[&entry.source].kind_of(task),
            status: task.state.status.clone(),
            category: task.state.category,
            stale: self.is_stale(&entry.source),
            missing,
            feature,
            title: None,
        }
    }

    /// A task's links one hop out, as a hook reads them (0.5): its own first, in its order (`out`), then the ones
    /// other tasks hold to it (`in`), in key order. They are read from the shown copy, and resolved from the corpus
    /// alone, across sources. A link to a key the corpus does not hold is `missing`, with no kind or status and the
    /// category `backlog`. A link of a type that targets a feature (`ts-0133-kg`) is to the proposal, with `feature`
    /// set: it is `missing` when the proposal has no such feature, with the proposal's own state. A target that does
    /// not read as a feature target is `missing`, names no feature and is never an incoming link.
    pub fn links_of(&self, key: &str) -> Vec<HookLink> {
        let Some(key) = self.resolve_key(key) else {
            return Vec::new();
        };
        let entry = &self.entries[&key];
        let mut links = Vec::new();
        for out in self.out_links(&entry.source, self.shown(entry)) {
            links.push(match out.target {
                Some(target) => {
                    self.link_to(&out.link.link_type, LinkDir::Out, &target, out.feature)
                }
                None => HookLink {
                    link_type: out.link.link_type.clone(),
                    dir: LinkDir::Out,
                    stale: source_of_key(&out.shown).is_some_and(|source| self.is_stale(source)),
                    key: out.shown,
                    kind: String::new(),
                    status: String::new(),
                    category: Category::Backlog,
                    missing: true,
                    feature: out.feature,
                    title: None,
                },
            });
        }
        let mut holders = BTreeSet::new();
        for name in self.names_for(&key) {
            holders.extend(self.inbound.get(&name).into_iter().flatten().cloned());
        }
        for holder in holders {
            let held = &self.entries[&holder];
            for out in self.out_links(&held.source, self.shown(held)) {
                if out.target.as_deref() == Some(key.as_str()) {
                    links.push(self.link_to(
                        &out.link.link_type,
                        LinkDir::In,
                        &holder,
                        out.feature,
                    ));
                }
            }
        }
        links
    }

    /// `links` with the title of each other task that the corpus holds. A link that is `missing` has none. Only
    /// `task.get` asks: a title is task text, which a hook is not given (`HookLink.title`).
    fn titled(&self, mut links: Vec<HookLink>) -> Vec<HookLink> {
        for link in &mut links {
            if !link.missing
                && let Some(entry) = self.entries.get(&link.key)
            {
                link.title = Some(self.shown(entry).title.clone());
            }
        }
        links
    }

    /// Every name a link may use for `key`: its key and names, and the keys that were renamed to it.
    fn names_for(&self, key: &str) -> BTreeSet<String> {
        let mut names = self.entries[key].names.clone();
        names.insert(key.to_string());
        for old in self.redirects.keys() {
            if !self.entries.contains_key(old) && self.resolve_key(old).as_deref() == Some(key) {
                names.insert(old.clone());
            }
        }
        names
    }

    // --- one entry changes ---

    /// Puts a task in, from a list or a notification: a task over a cap of 0.9 (`check_task_caps`) or with a status
    /// that is not declared makes the item an invalid entry; its category comes from the source's declared
    /// statuses; and a task at a path clears the invalid item the path had.
    fn insert(&mut self, source: &str, mut task: Task) {
        if let Err(cap) = check_task_caps(&task, None)
            && self.sources.contains_key(source)
        {
            let item = TaskInvalid {
                path: task.path.clone().unwrap_or_else(|| task.key.clone()),
                checkout: task.checkout.clone(),
                reason: cap.reason,
            };
            self.take_copy(&task.key, &task.checkout);
            self.add_invalid(source, item);
            return;
        }
        let Some(state) = self.sources.get_mut(source) else {
            return;
        };
        let Some(category) = state.category_of(&task) else {
            let item = TaskInvalid {
                path: task.path.clone().unwrap_or_else(|| task.key.clone()),
                checkout: task.checkout.clone(),
                reason: format!(
                    "status `{}` is not declared by source `{source}`",
                    task.state.status
                ),
            };
            self.take_copy(&task.key, &task.checkout);
            self.add_invalid(source, item);
            return;
        };
        task.state.category = category;
        if let Some(path) = &task.path {
            state.invalid.remove(&(task.checkout.clone(), path.clone()));
        }
        self.place(source, task);
    }

    fn add_invalid(&mut self, source: &str, item: TaskInvalid) {
        if let Some(state) = self.sources.get_mut(source) {
            state
                .invalid
                .insert((item.checkout.clone(), item.path.clone()), item);
        }
    }

    /// Holds `task` as the copy of its key in its checkout, in place of the one there.
    fn place(&mut self, source: &str, task: Task) {
        let key = task.key.clone();
        let checkout = task.checkout.clone();
        let path = task.path.clone();
        let entry = self.entries.entry(key.clone()).or_insert_with(|| Entry {
            source: source.to_string(),
            copies: BTreeMap::new(),
            names: BTreeSet::new(),
            targets: BTreeSet::new(),
        });
        let old = entry.copies.insert(checkout.clone(), task);
        if let Some(old) = old
            && let Some(old_path) = old.path
        {
            self.locations
                .remove(&(source.to_string(), checkout.clone(), old_path));
        }
        if let Some(path) = path {
            self.locations
                .insert((source.to_string(), checkout, path), key.clone());
        }
        self.reindex(&key);
    }

    /// Takes the copy of `key` in `checkout` out, and the entry with it when it was the last.
    fn take_copy(&mut self, key: &str, checkout: &Option<String>) -> Option<Task> {
        let entry = self.entries.get_mut(key)?;
        let task = entry.copies.remove(checkout)?;
        let source = entry.source.clone();
        let empty = entry.copies.is_empty();
        if let Some(path) = &task.path {
            self.locations
                .remove(&(source, checkout.clone(), path.clone()));
        }
        if empty {
            self.unindex(key);
            self.entries.remove(key);
        } else {
            self.reindex(key);
        }
        Some(task)
    }

    /// Removes the copy in `checkout`, or every copy when there is none named.
    fn remove_copies(&mut self, key: &str, checkout: Option<&str>) {
        match checkout {
            Some(checkout) => {
                self.take_copy(key, &Some(checkout.to_string()));
            }
            None => {
                let checkouts: Vec<Option<String>> = self
                    .entries
                    .get(key)
                    .map(|entry| entry.copies.keys().cloned().collect())
                    .unwrap_or_default();
                for checkout in checkouts {
                    self.take_copy(key, &checkout);
                }
            }
        }
    }

    /// Takes what the entry put into `names` and `inbound` out again.
    fn unindex(&mut self, key: &str) {
        let Some(entry) = self.entries.get_mut(key) else {
            return;
        };
        let names = std::mem::take(&mut entry.names);
        let targets = std::mem::take(&mut entry.targets);
        for (index, held) in [(&mut self.names, names), (&mut self.inbound, targets)] {
            for name in held {
                if let Some(keys) = index.get_mut(&name) {
                    keys.remove(key);
                    if keys.is_empty() {
                        index.remove(&name);
                    }
                }
            }
        }
    }

    /// Puts the entry's names and link targets into the indexes, from every copy it holds.
    fn reindex(&mut self, key: &str) {
        self.unindex(key);
        let Some(entry) = self.entries.get(key) else {
            return;
        };
        let source = entry.source.clone();
        let features = self.sources.get(&source).map(|state| &state.feature_links);
        let mut names = BTreeSet::new();
        let mut targets = BTreeSet::new();
        for task in entry.copies.values() {
            names.extend(names_of(&source, task));
            for link in &task.links {
                let feature_link = features.is_some_and(|set| set.contains(&link.link_type));
                // A feature target that does not read points at nothing, so it is nobody's incoming link.
                if !feature_link || Reach::of(link, true).feature.is_some() {
                    targets.insert(qualified(&source, Reach::of(link, feature_link).item));
                }
            }
        }
        for name in &names {
            self.names
                .entry(name.clone())
                .or_default()
                .insert(key.to_string());
        }
        for target in &targets {
            self.inbound
                .entry(target.clone())
                .or_default()
                .insert(key.to_string());
        }
        if let Some(entry) = self.entries.get_mut(key) {
            entry.names = names;
            entry.targets = targets;
        }
    }
}

// --- reconcile ---

fn json_text(text: &str) -> Value {
    Value::String(text.to_string())
}

fn json_option(text: &Option<String>) -> Value {
    text.as_deref().map_or(Value::Null, json_text)
}

/// Whether a notice's `diff` names no field but the state: the status, and the category that moves with it (the
/// notice's `to` carries both). A provider names every field a copy holds that changed, so a notice that names none
/// changed only what the notice itself carries: the version.
fn names_only_the_status(event: &TaskChanged) -> bool {
    event
        .diff
        .keys()
        .all(|name| name == "status" || name == "category")
}

fn set_diff(old: &[String], new: &[String]) -> Option<FieldDiff> {
    let added: Vec<Value> = new
        .iter()
        .filter(|item| !old.contains(item))
        .map(|item| json_text(item))
        .collect();
    let removed: Vec<Value> = old
        .iter()
        .filter(|item| !new.contains(item))
        .map(|item| json_text(item))
        .collect();
    (!added.is_empty() || !removed.is_empty()).then_some(FieldDiff::Set { added, removed })
}

/// What changed between two copies of one task: its status, title, path, kind, the sets of labels, owners and
/// collections, and each of its `fields`.
fn diff_of(old: &Task, new: &Task) -> BTreeMap<String, FieldDiff> {
    let mut diff = BTreeMap::new();
    let mut changed = |name: &str, from: Value, to: Value| {
        if from != to {
            diff.insert(name.to_string(), FieldDiff::Changed { from, to });
        }
    };
    changed(
        "status",
        json_text(&old.state.status),
        json_text(&new.state.status),
    );
    changed("title", json_text(&old.title), json_text(&new.title));
    changed("path", json_option(&old.path), json_option(&new.path));
    changed("kind", json_option(&old.kind), json_option(&new.kind));
    let names: BTreeSet<&String> = old.fields.keys().chain(new.fields.keys()).collect();
    for name in names {
        let value = |task: &Task| task.fields.get(name).cloned().unwrap_or(Value::Null);
        changed(&format!("fields.{name}"), value(old), value(new));
    }
    let sets = [
        ("labels", &old.labels, &new.labels),
        ("owners", &old.owners, &new.owners),
        ("collections", &old.collections, &new.collections),
    ];
    for (name, from, to) in sets {
        if let Some(set) = set_diff(from, to) {
            diff.insert(name.to_string(), set);
        }
    }
    diff
}

/// The startup reconcile (0.10; S11): what differs between the corpus the store cached and the one a fresh `list` of
/// each source gave, as the events core emits at start.
///
/// - A copy that is new gives a `task.changed` with `from` null, and one that differs (in anything its source
///   listed, its version included) gives one with the old and new state. The cause is `startup`.
/// - A copy that is gone gives a `task.removed` with the cause `unknown`, since a list does not say why.
/// - An invalid item that is new or whose reason changed gives a `task.invalid`. A task that became invalid gives
///   that and no removal, as the live notification does.
/// - An identical copy gives nothing, so no chip lights for every task at every start.
///
/// It says nothing of what the fresh corpus was not given a list for: a checkout it did not load, and a source that
/// is stale, whose lists are not the truth. Renames are not guessed: a key that is gone and a key that is new are
/// two events.
pub fn reconcile(cached: &Corpus, fresh: &Corpus) -> Vec<ProviderEvent> {
    reconcile_by_source(cached, fresh)
        .into_iter()
        .map(|(_, event)| event)
        .collect()
}

/// [`reconcile`] with the name of the source each event is of: an invalid item does not carry it.
pub fn reconcile_by_source(cached: &Corpus, fresh: &Corpus) -> Vec<(String, ProviderEvent)> {
    let mut events: Vec<(String, ProviderEvent)> = Vec::new();
    for (name, source) in &fresh.sources {
        if source.stale.is_some() {
            continue;
        }
        let before = cached.sources.get(name);
        for (key, entry) in fresh
            .entries
            .iter()
            .filter(|(_, entry)| &entry.source == name)
        {
            for (checkout, task) in &entry.copies {
                let old = cached
                    .entries
                    .get(key)
                    .and_then(|entry| entry.copies.get(checkout));
                if old == Some(task) {
                    continue;
                }
                events.push((
                    name.clone(),
                    ProviderEvent::Changed(TaskChanged {
                        key: key.clone(),
                        version: task.version.clone(),
                        from: old.map(|old| old.state.clone()),
                        to: Some(task.state.clone()),
                        diff: old.map_or_else(BTreeMap::new, |old| diff_of(old, task)),
                        cause: ChangeCause::Startup,
                        actor: None,
                        automated: None,
                        checkout: checkout.clone(),
                    }),
                ));
            }
        }
        for (key, entry) in cached
            .entries
            .iter()
            .filter(|(_, entry)| &entry.source == name)
        {
            for (checkout, task) in &entry.copies {
                let relisted = source.listed.contains(checkout);
                let held = fresh
                    .entries
                    .get(key)
                    .is_some_and(|entry| entry.copies.contains_key(checkout));
                let invalid = task.path.as_ref().is_some_and(|path| {
                    source
                        .invalid
                        .contains_key(&(checkout.clone(), path.clone()))
                });
                if relisted && !held && !invalid {
                    events.push((
                        name.clone(),
                        ProviderEvent::Removed(TaskRemoved {
                            key: key.clone(),
                            cause: RemovedCause::Unknown,
                            checkout: checkout.clone(),
                        }),
                    ));
                }
            }
        }
        for (at, item) in &source.invalid {
            if before.and_then(|before| before.invalid.get(at)) != Some(item) {
                events.push((name.clone(), ProviderEvent::Invalid(item.clone())));
            }
        }
    }
    events
}
