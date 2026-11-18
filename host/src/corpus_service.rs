//! The corpus of a board in the daemon (ts-0116-wh; 0.10, Identity, Protocol and Ref-scoped sources). One service
//! for each board holds a [`Corpus`] ([`collide_core::corpus`]), fills it from the board's sources, keeps it in step
//! with their notifications, and keeps a copy of it in the store ([`crate::corpus_store`]) for the next start.
//!
//! **Start.** A board's service starts when the daemon does (for each board the store holds), and at the first
//! `task.list` or `task.get` of a board that was opened since. Its own thread does the work, so no start waits for a
//! provider. It declares each source of the board's line that may start (an untrusted line starts none: the sources
//! refuse), builds the corpus the cache held and serves it with `refreshing` set, lists every source for every
//! checkout, and records what differs as `task.changed` with the cause `startup`, `task.removed` and `task.invalid`
//! ([`reconcile_by_source`]). Then it replaces the cache, serves the fresh corpus, and tells each source that has
//! `watch` which checkouts to watch.
//!
//! **Checkouts.** The entries of `git worktree list` that pass the board's `checkout_filter`: the board's root, the
//! worktrees below the daemon's workspaces folder, and any other. The set is read again every `recheck` (10 s). A
//! change to it relists each ref-scoped source and tells the watching ones the new set.
//!
//! **Notifications.** One pump thread for each provider that has `watch` applies each notification to the corpus and
//! to the cache and records its event, with the source. A write that went through `task.apply` is noted first
//! ([`BoardCorpus::note_write`]) with its (key, checkout, version), and the notification of the same triple is
//! dropped, so a write is one event, with the actor of the connection that made it. `task.collision` is a hint to
//! list the source again, since collisions come from the lists. `source.stale`, a provider that cannot be reached and
//! a list that fails mark the source stale: its last tasks stay listed, and the next recheck lists it again.
//!
//! **Every `recheck`.** A source with no `watch` is listed again. So is one whose provider was started again (the
//! supervisor restarted it, or a setting changed and the daemon stopped it), after the watch is told once more, and
//! one that is stale. A source that did not start is tried again after a longer wait each time.
//!
//! **One lock order.** The gate is taken before the state, never the other way: a write holds the gate across its
//! `apply`, its event and [`BoardCorpus::note_write`], and a listing holds it across the lists and what it applies,
//! so a list that began before a write cannot undo it. `task.list` and `task.get` read the state alone.
//!
//! **What is not here.** Views, `task.sections`, experiment branches and the adopted-worktree reading are other
//! items'. The scratch source is listed live and never cached: only the daemon writes it.

use crate::boards;
use crate::config_get::BoardRef;
use crate::corpus_store::CorpusCache;
use crate::scratch;
use crate::sources::{Source, SourceError, Sources};
use crate::store::Store;
use crate::supervisor::SupervisedProvider;
use collide_core::change::Effect;
use collide_core::corpus::{
    CheckoutInfo, CheckoutKind, Corpus, ListFilter, Page, reconcile_by_source,
};
use collide_core::events::EventBody;
use collide_core::owner::task::TaskListResult;
use collide_core::provider::{
    ChangeCause, GetParams, InitializeResult, ListParams, ListResult, ProviderEvent, Scope,
    SourceStale, Task, TaskChanged, WatchParams,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How often a thread looks at the stop flag while it waits.
const SLICE: Duration = Duration::from_millis(50);

/// How many (key, checkout, version) of writes are kept to drop the notifications of the same writes.
const WRITTEN_KEPT: usize = 256;

/// The most a source that did not start waits before it is tried again, in `recheck` intervals.
const BACKOFF_CAP: u32 = 64;

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

fn short_of(key: &str) -> &str {
    key.split_once(':').map_or(key, |(_, short)| short)
}

fn without_body(task: Task) -> Task {
    Task { body: None, ..task }
}

/// What the services of all boards are made of.
pub struct CorporaConfig {
    pub sources: Arc<Sources>,
    pub store: Arc<Store>,
    /// Added to the environment of every git command, as the daemon's other git is ([`crate::daemon::DaemonOptions`]).
    pub git_env: Vec<(String, String)>,
    /// The folder the daemon's workspaces are made in: a worktree below it is a workspace of the board.
    pub workspaces_dir: PathBuf,
    /// The `PATH` the tools a pack needs are found on, read at each use.
    pub tools: Arc<dyn Fn() -> OsString + Send + Sync>,
    /// Records an event with the daemon as its actor.
    pub record: Arc<dyn Fn(EventBody) + Send + Sync>,
    pub log: Arc<dyn Fn(&str) + Send + Sync>,
    /// Whether the daemon is stopping.
    pub stopping: Arc<dyn Fn() -> bool + Send + Sync>,
    /// How often the checkout set and the sources that need it are looked at again (10 s).
    pub recheck: Duration,
}

/// The services of every board the daemon has been asked about.
pub struct Corpora {
    config: Arc<CorporaConfig>,
    boards: Mutex<BTreeMap<String, Arc<BoardCorpus>>>,
    closed: AtomicBool,
}

impl Corpora {
    pub fn new(config: CorporaConfig) -> Corpora {
        Corpora {
            config: Arc::new(config),
            boards: Mutex::new(BTreeMap::new()),
            closed: AtomicBool::new(false),
        }
    }

    /// The service of `board`, started if it is not. `None` once the daemon is stopping.
    pub fn ensure(&self, board: &str, root: &Path) -> Option<Arc<BoardCorpus>> {
        if self.closed.load(Ordering::SeqCst) {
            return None;
        }
        let mut boards = locked(&self.boards);
        if let Some(found) = boards.get(board) {
            return Some(found.clone());
        }
        let service = BoardCorpus::start(self.config.clone(), board, root);
        boards.insert(board.to_string(), service.clone());
        Some(service)
    }

    /// The service of `board` if one has started.
    pub fn get(&self, board: &str) -> Option<Arc<BoardCorpus>> {
        locked(&self.boards).get(board).cloned()
    }

    /// Starts the service of every board the store holds. Each does its work on its own thread.
    pub fn start_stored(&self) {
        match self.config.store.boards() {
            Ok(boards) => {
                for board in boards {
                    self.ensure(&board.id, Path::new(&board.root));
                }
            }
            Err(error) => (self.config.log)(&format!(
                "the boards could not be read to start their corpus: {error}"
            )),
        }
    }

    /// Ends every service and joins its threads. The daemon stops its sources first, so a call that is waiting on a
    /// provider returns.
    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::SeqCst);
        let boards = std::mem::take(&mut *locked(&self.boards));
        for service in boards.into_values() {
            service.join();
        }
    }
}

// ---- one board ----------------------------------------------------------------------------------------------------

/// What the service knows of one source it brought up.
struct Run {
    /// Which start of the provider this was ([`Source::instance`]).
    instance: u64,
    watch: bool,
    /// The checkouts the source was last told to watch.
    told: Option<Vec<String>>,
    /// The source's last list or notification said it is stale: the whole source when `whole`, else these checkouts.
    stale: Option<StaleAt>,
}

#[derive(Default)]
struct StaleAt {
    whole: bool,
    checkouts: BTreeSet<String>,
}

#[derive(Default)]
struct State {
    corpus: Corpus,
    /// The first listing has not ended: the corpus is the cached one, or empty.
    refreshing: bool,
    checkouts: Vec<CheckoutInfo>,
    runs: BTreeMap<String, Run>,
    /// (key, checkout, version) of the writes made through `task.apply`, oldest first.
    written: VecDeque<(String, Option<String>, String)>,
    /// Sources that did not start: how many times in a row, and when they are tried again.
    failing: BTreeMap<String, (u32, Instant)>,
    /// Sources to list again at the next look, for a `task.collision` hint.
    wanted: BTreeSet<String>,
}

struct Inner {
    config: Arc<CorporaConfig>,
    board: String,
    root: PathBuf,
    state: Mutex<State>,
    gate: Mutex<()>,
    nudged: AtomicBool,
    handles: Mutex<Vec<JoinHandle<()>>>,
    /// What the cache held when the service started, for the first listing's comparison.
    cache: Mutex<Option<CorpusCache>>,
}

/// The service of one board. See the module doc.
pub struct BoardCorpus {
    inner: Arc<Inner>,
}

impl BoardCorpus {
    fn start(config: Arc<CorporaConfig>, board: &str, root: &Path) -> Arc<BoardCorpus> {
        let inner = Arc::new(Inner {
            config,
            board: board.to_string(),
            root: root.to_path_buf(),
            state: Mutex::new(State {
                refreshing: true,
                ..State::default()
            }),
            gate: Mutex::new(()),
            nudged: AtomicBool::new(false),
            handles: Mutex::new(Vec::new()),
            cache: Mutex::new(None),
        });
        inner.serve_cache();
        let running = inner.clone();
        let spawned = std::thread::Builder::new()
            .name("collide-corpus".to_string())
            .spawn(move || running.run());
        match spawned {
            Ok(handle) => locked(&inner.handles).push(handle),
            Err(error) => {
                (inner.config.log)(&format!("the corpus of {board} could not start: {error}"));
                locked(&inner.state).refreshing = false;
            }
        }
        Arc::new(BoardCorpus { inner })
    }

    /// `task.list`: the corpus through `filter`, one page. `refreshing` says the first listing is still running.
    pub fn list(&self, filter: &ListFilter<'_>, page: &Page<'_>) -> TaskListResult {
        let state = locked(&self.inner.state);
        let mut result = state.corpus.list(filter, page);
        result.refreshing = state.refreshing;
        result
    }

    /// Reads the corpus.
    pub fn read<R>(&self, read: impl FnOnce(&Corpus) -> R) -> R {
        read(&locked(&self.inner.state).corpus)
    }

    /// Whether the first listing has ended.
    pub fn refreshing(&self) -> bool {
        locked(&self.inner.state).refreshing
    }

    /// What the write path holds across a source's `apply`, its event and [`BoardCorpus::note_write`].
    pub fn gate(&self) -> MutexGuard<'_, ()> {
        locked(&self.inner.gate)
    }

    /// Looks at the sources now and not at the next recheck: the line was trusted, for one.
    pub fn wake(&self) {
        self.inner.nudged.store(true, Ordering::SeqCst);
    }

    /// A write went through: puts the written task in the corpus and the cache, and remembers what the watcher will
    /// say of it so that its notification is dropped. The caller holds [`BoardCorpus::gate`]. Another item the write
    /// changed is updated from its effect when the corpus holds it; the watcher's notice of one it does not hold is
    /// not dropped.
    pub fn note_write(&self, source: &str, task: &Task, items: &[Effect]) {
        self.inner.note_write(source, task, items);
    }

    fn join(&self) {
        let handles = std::mem::take(&mut *locked(&self.inner.handles));
        for handle in handles {
            let _ = handle.join();
        }
    }
}

impl Inner {
    fn stopping(&self) -> bool {
        (self.config.stopping)()
    }

    fn log(&self, text: &str) {
        (self.config.log)(&format!("corpus of {}: {text}", self.board));
    }

    fn board_ref(&self) -> BoardRef<'_> {
        BoardRef {
            id: &self.board,
            root: &self.root,
        }
    }

    fn source(&self, name: &str) -> Result<Source, SourceError> {
        self.config
            .sources
            .source(self.board_ref(), name, &(self.config.tools)())
    }

    // ---- the thread --------------------------------------------------------------------------------------------

    fn run(self: Arc<Self>) {
        self.startup();
        while !self.stopping() {
            self.wait_for_next_look();
            if self.stopping() {
                break;
            }
            self.look();
        }
    }

    fn wait_for_next_look(&self) {
        let until = Instant::now() + self.config.recheck;
        while Instant::now() < until && !self.stopping() {
            if self.nudged.swap(false, Ordering::SeqCst) {
                return;
            }
            if !locked(&self.state).wanted.is_empty() {
                return;
            }
            std::thread::sleep(SLICE.min(self.config.recheck));
        }
    }

    // ---- the checkouts ----------------------------------------------------------------------------------------

    /// The board's checkouts now, from git, with the board's `checkout_filter` applied. `Err` says why not.
    fn discover(&self, exclude: &[String]) -> Result<Vec<CheckoutInfo>, String> {
        let listed = boards::worktrees(&self.root, &self.config.git_env)?;
        let root = std::fs::canonicalize(&self.root).unwrap_or_else(|_| self.root.clone());
        let workspaces = std::fs::canonicalize(&self.config.workspaces_dir)
            .unwrap_or_else(|_| self.config.workspaces_dir.clone());
        let mut found = Vec::new();
        for worktree in listed {
            let path = worktree.path.display().to_string();
            let kind = if worktree.path == root {
                CheckoutKind::Line { default: true }
            } else if boards::checkout_excluded(exclude, &worktree) {
                continue;
            } else if worktree.path.starts_with(&workspaces) {
                CheckoutKind::Workspace {
                    forked_from: root.display().to_string(),
                }
            } else {
                CheckoutKind::Other
            };
            found.push(CheckoutInfo { path, kind });
        }
        Ok(found)
    }

    fn targets(&self, scope: Scope, checkouts: &[CheckoutInfo]) -> Vec<Option<String>> {
        match scope {
            Scope::Global => vec![None],
            Scope::Ref => checkouts
                .iter()
                .map(|checkout| Some(checkout.path.clone()))
                .collect(),
        }
    }

    // ---- listing -----------------------------------------------------------------------------------------------

    /// Every page of one source's `list` for `checkout`, joined. Each task is without its body and names its checkout.
    fn list_all(&self, source: &Source, checkout: Option<&str>) -> Result<ListResult, SourceError> {
        let mut all = ListResult {
            tasks: Vec::new(),
            collections: Vec::new(),
            invalid: Vec::new(),
            next_cursor: None,
        };
        let mut cursor: Option<String> = None;
        let mut first = true;
        loop {
            let page = source.list(&ListParams {
                checkout: checkout.map(str::to_string),
                cursor: cursor.clone(),
            })?;
            if first {
                all.collections = page.collections;
                first = false;
            }
            all.tasks.extend(page.tasks.into_iter().map(|task| {
                let mut task = without_body(task);
                task.checkout = checkout.map(str::to_string).or(task.checkout.take());
                task
            }));
            all.invalid.extend(page.invalid.into_iter().map(|mut item| {
                item.checkout = checkout.map(str::to_string).or(item.checkout.take());
                item
            }));
            match page.next_cursor {
                Some(next) if Some(&next) == cursor.as_ref() => {
                    return Err(SourceError::Failed(format!(
                        "the source `{}` answered the same page cursor twice",
                        source.name()
                    )));
                }
                Some(next) => cursor = Some(next),
                None => return Ok(all),
            }
            if self.stopping() {
                return Err(SourceError::Unavailable(
                    "the daemon is shutting down".to_string(),
                ));
            }
        }
    }

    /// Lists `source` for each target. A target that cannot be listed gives the error.
    fn list_targets(
        &self,
        source: &Source,
        targets: &[Option<String>],
    ) -> Result<Vec<(Option<String>, ListResult)>, SourceError> {
        targets
            .iter()
            .map(|target| {
                self.list_all(source, target.as_deref())
                    .map(|list| (target.clone(), list))
            })
            .collect()
    }

    fn keep(&self, source: &str, checkout: Option<&str>, list: &ListResult) {
        if source == scratch::SOURCE {
            return;
        }
        if let Err(error) = self.config.store.corpus_replace(
            &self.board,
            source,
            checkout,
            &list.tasks,
            &list.invalid,
        ) {
            self.log(&format!("the cache of {source} was not kept: {error}"));
        }
    }

    fn record(&self, source: &str, event: ProviderEvent, cause: Option<ChangeCause>) {
        let body = match event {
            ProviderEvent::Changed(mut task) => {
                if let Some(cause) = cause {
                    task.cause = cause;
                }
                EventBody::TaskChanged {
                    source: source.to_string(),
                    task,
                }
            }
            ProviderEvent::Renamed(rename) => EventBody::TaskRenamed {
                source: source.to_string(),
                rename,
            },
            ProviderEvent::Removed(task) => EventBody::TaskRemoved {
                source: source.to_string(),
                task,
            },
            ProviderEvent::Invalid(invalid) => EventBody::TaskInvalid {
                source: source.to_string(),
                invalid,
            },
            _ => return,
        };
        (self.config.record)(body);
    }

    // ---- the start ---------------------------------------------------------------------------------------------

    fn startup(self: &Arc<Self>) {
        let _gate = locked(&self.gate);
        let plan = self
            .config
            .sources
            .board_plan(self.board_ref(), &(self.config.tools)());
        let (names, exclude) = match &plan {
            Ok(plan) => (plan.sources.clone(), plan.checkout_filter.exclude.clone()),
            Err(error) => {
                self.log(&format!("its sources are not read: {error:?}"));
                (Vec::new(), Vec::new())
            }
        };
        let checkouts = match self.discover(&exclude) {
            Ok(found) => found,
            Err(error) => {
                self.log(&error);
                Vec::new()
            }
        };
        let cache = locked(&self.cache).take().unwrap_or_default();
        let mut inits: BTreeMap<String, InitializeResult> = cache.sources.iter().cloned().collect();
        locked(&self.state).checkouts = checkouts.clone();

        let mut up: Vec<(String, Source, InitializeResult)> = Vec::new();
        let mut not_up: Vec<(String, String)> = Vec::new();
        for name in std::iter::once(scratch::SOURCE.to_string()).chain(names) {
            match self.bring(&name) {
                Ok((source, init)) => {
                    if name != scratch::SOURCE
                        && let Err(error) =
                            self.config
                                .store
                                .corpus_put_source(&self.board, &name, &init)
                    {
                        self.log(&format!("the declaration of {name} was not kept: {error}"));
                    }
                    inits.insert(name.clone(), init.clone());
                    up.push((name, source, init));
                }
                Err(error) => {
                    self.failed_to_start(&name, &error);
                    not_up.push((name, source_text(&error)));
                }
            }
        }

        let cached = self.cached_corpus(&inits, &cache, &checkouts);
        let mut fresh = Corpus::default();
        fresh.set_checkouts(checkouts.clone());
        for (name, init) in &inits {
            fresh.declare_source(name, init);
        }
        // A source that did not start keeps the tasks it last listed, marked stale: the next look starts it again.
        for (name, why) in &not_up {
            if let Some(init) = inits.get(name) {
                for target in self.targets(init.scope, &checkouts) {
                    let _ = fresh.load(
                        name,
                        target.as_deref(),
                        list_from(&cache, name, target.as_deref()),
                    );
                }
                let _ = fresh.mark_stale(name, why);
            }
        }

        let mut runs = BTreeMap::new();
        let mut cached = cached;
        for (name, source, init) in &up {
            let targets = self.targets(init.scope, &checkouts);
            let mut run = Run {
                instance: source.instance(),
                watch: init.capabilities.watch,
                told: None,
                stale: None,
            };
            match self.list_targets(source, &targets) {
                Ok(lists) => {
                    for (target, list) in lists {
                        self.keep(name, target.as_deref(), &list);
                        if name == scratch::SOURCE {
                            let _ = cached.load(name, target.as_deref(), list.clone());
                        }
                        let _ = fresh.load(name, target.as_deref(), list);
                    }
                    self.forget_gone_checkouts(name, init.scope, &checkouts, &cache.tasks);
                }
                Err(error) => {
                    let why = source_text(&error);
                    self.log(&format!("{name} could not be listed: {why}"));
                    for target in &targets {
                        let _ = fresh.load(
                            name,
                            target.as_deref(),
                            list_from(&cache, name, target.as_deref()),
                        );
                    }
                    let _ = fresh.mark_stale(name, &why);
                    run.stale = Some(StaleAt {
                        whole: true,
                        checkouts: BTreeSet::new(),
                    });
                }
            }
            runs.insert(name.clone(), run);
        }

        for (source, event) in reconcile_by_source(&cached, &fresh) {
            self.record(&source, event, None);
        }
        {
            let mut state = locked(&self.state);
            state.corpus = fresh;
            state.runs = runs;
            state.refreshing = false;
        }
        for (name, source, init) in &up {
            self.watch_and_pump(name, source, init, &checkouts);
        }
    }

    /// Reads the cache and serves it, before the thread starts and so before any provider or git is asked: what the
    /// last start declared is enough to show the cached tasks. Their checkouts are taken as they were listed, as any
    /// other worktree, until the first listing knows the board's. The first listing then replaces them.
    fn serve_cache(&self) {
        let cache = match self.config.store.corpus_cache(&self.board) {
            Ok(cache) => cache,
            Err(error) => {
                self.log(&format!("the cache is not read: {error}"));
                return;
            }
        };
        for line in &cache.dropped {
            self.log(line);
        }
        let paths: BTreeSet<&str> = cache
            .tasks
            .iter()
            .filter_map(|held| held.checkout.as_deref())
            .chain(
                cache
                    .invalid
                    .iter()
                    .filter_map(|held| held.item.checkout.as_deref()),
            )
            .collect();
        let checkouts: Vec<CheckoutInfo> = paths
            .into_iter()
            .map(|path| CheckoutInfo {
                path: path.to_string(),
                kind: CheckoutKind::Other,
            })
            .collect();
        let inits: BTreeMap<String, InitializeResult> = cache.sources.iter().cloned().collect();
        let serving = self.cached_corpus(&inits, &cache, &checkouts);
        locked(&self.state).corpus = serving;
        *locked(&self.cache) = Some(cache);
    }

    /// The corpus of what the cache holds: each source declared as it last was, with the tasks and invalid items it
    /// listed in the checkouts the board has now.
    fn cached_corpus(
        &self,
        inits: &BTreeMap<String, InitializeResult>,
        cache: &CorpusCache,
        checkouts: &[CheckoutInfo],
    ) -> Corpus {
        let mut corpus = Corpus::default();
        corpus.set_checkouts(checkouts.to_vec());
        for (name, init) in inits {
            if name == scratch::SOURCE {
                continue;
            }
            corpus.declare_source(name, init);
            for target in self.targets(init.scope, checkouts) {
                let _ = corpus.load(
                    name,
                    target.as_deref(),
                    list_from(cache, name, target.as_deref()),
                );
            }
        }
        corpus
    }

    /// Drops the cache of a checkout the set no longer has.
    fn forget_gone_checkouts(
        &self,
        source: &str,
        scope: Scope,
        checkouts: &[CheckoutInfo],
        cached: &[crate::corpus_store::CachedTask],
    ) {
        if scope == Scope::Global || source == scratch::SOURCE {
            return;
        }
        let held: BTreeSet<&str> = checkouts.iter().map(|info| info.path.as_str()).collect();
        let gone: BTreeSet<&str> = cached
            .iter()
            .filter(|row| row.source == source)
            .filter_map(|row| row.checkout.as_deref())
            .filter(|checkout| !held.contains(checkout))
            .collect();
        for checkout in gone {
            let empty = ListResult {
                tasks: Vec::new(),
                collections: Vec::new(),
                invalid: Vec::new(),
                next_cursor: None,
            };
            self.keep(source, Some(checkout), &empty);
        }
    }

    /// The source `name` and what it declared. A source of a pack that is not trusted is refused and not started.
    fn bring(&self, name: &str) -> Result<(Source, InitializeResult), SourceError> {
        let source = self.source(name)?;
        let init = source.declared()?;
        Ok((source, init))
    }

    fn failed_to_start(&self, name: &str, error: &SourceError) {
        let mut state = locked(&self.state);
        let (times, _) = state
            .failing
            .get(name)
            .copied()
            .unwrap_or((0, Instant::now()));
        let times = times.saturating_add(1);
        let wait = self.config.recheck * 2u32.saturating_pow(times).min(BACKOFF_CAP);
        let first = !state.failing.contains_key(name);
        state
            .failing
            .insert(name.to_string(), (times, Instant::now() + wait));
        let declared = state.runs.contains_key(name);
        if declared {
            let _ = state.corpus.mark_stale(name, &source_text(error));
        }
        drop(state);
        if first {
            self.log(&format!(
                "the source {name} is not started: {}",
                source_text(error)
            ));
        }
    }

    /// Tells a source with `watch` which checkouts to watch, and starts the thread that reads its notifications.
    fn watch_and_pump(
        self: &Arc<Self>,
        name: &str,
        source: &Source,
        init: &InitializeResult,
        checkouts: &[CheckoutInfo],
    ) {
        if !init.capabilities.watch {
            return;
        }
        let told: Vec<String> = match init.scope {
            Scope::Ref => checkouts.iter().map(|info| info.path.clone()).collect(),
            Scope::Global => Vec::new(),
        };
        if let Err(error) = source.watch(&WatchParams {
            checkouts: told.clone(),
        }) {
            self.mark_source_stale(name, &source_text(&error));
            return;
        }
        if let Some(run) = locked(&self.state).runs.get_mut(name) {
            run.told = Some(told);
        }
        if let Some(provider) = source.supervised()
            && let Some(events) = provider.take_events()
        {
            let inner = self.clone();
            let name = name.to_string();
            let spawned = std::thread::Builder::new()
                .name("collide-corpus-pump".to_string())
                .spawn(move || inner.pump(&name, &provider, &events));
            match spawned {
                Ok(handle) => locked(&self.handles).push(handle),
                Err(error) => self.log(&format!("a notification thread did not start: {error}")),
            }
        }
    }

    fn mark_source_stale(&self, name: &str, reason: &str) {
        let mut state = locked(&self.state);
        let _ = state.corpus.mark_stale(name, reason);
        if let Some(run) = state.runs.get_mut(name) {
            run.stale.get_or_insert_with(StaleAt::default).whole = true;
        }
    }

    // ---- the looks ---------------------------------------------------------------------------------------------

    /// One recheck: the checkout set, then each source of the line.
    fn look(self: &Arc<Self>) {
        let wanted = std::mem::take(&mut locked(&self.state).wanted);
        let plan = match self
            .config
            .sources
            .board_plan(self.board_ref(), &(self.config.tools)())
        {
            Ok(plan) => plan,
            Err(error) => {
                self.log(&format!("its sources are not read: {error:?}"));
                return;
            }
        };
        let checkouts = match self.discover(&plan.checkout_filter.exclude) {
            Ok(found) => found,
            Err(error) => {
                self.log(&error);
                return;
            }
        };
        let changed = {
            let state = locked(&self.state);
            state.checkouts != checkouts
        };
        let mut names = vec![scratch::SOURCE.to_string()];
        names.extend(plan.sources);
        for name in names {
            self.look_at(&name, &checkouts, changed, wanted.contains(&name));
        }
        if changed {
            let mut state = locked(&self.state);
            state.corpus.set_checkouts(checkouts.clone());
            state.checkouts = checkouts;
        }
    }

    fn look_at(
        self: &Arc<Self>,
        name: &str,
        checkouts: &[CheckoutInfo],
        checkouts_changed: bool,
        hinted: bool,
    ) {
        if let Some((_, next)) = locked(&self.state).failing.get(name)
            && Instant::now() < *next
        {
            return;
        }
        let (source, init) = match self.bring(name) {
            Ok(brought) => {
                locked(&self.state).failing.remove(name);
                brought
            }
            Err(error) => {
                self.failed_to_start(name, &error);
                return;
            }
        };
        let known = {
            let state = locked(&self.state);
            state.runs.get(name).map(|run| {
                (
                    run.instance,
                    run.watch,
                    run.told.clone(),
                    run.stale.is_some(),
                )
            })
        };
        let targets = self.targets(init.scope, checkouts);
        match known {
            None => {
                // New to the corpus: its pack was trusted since the start, or it did not start before.
                self.declare(name, &source, &init);
                self.relist(name, &source, &targets, &[], ChangeCause::Worktree);
                self.watch_and_pump(name, &source, &init, checkouts);
            }
            Some((instance, _, _, _)) if instance != source.instance() => {
                // A new provider: told nothing of the old one's watch, with a stream of its own.
                self.declare(name, &source, &init);
                self.relist(name, &source, &targets, &[], ChangeCause::Worktree);
                self.watch_and_pump(name, &source, &init, checkouts);
            }
            Some((_, watching, told, stale)) => {
                let gone = self.gone_checkouts(checkouts);
                let moved = checkouts_changed && init.scope == Scope::Ref;
                let provider = source.supervised();
                let ended = watching && provider.as_ref().is_some_and(|p| !p.is_running());
                if watching && (moved || ended) {
                    let told_now: Vec<String> =
                        checkouts.iter().map(|info| info.path.clone()).collect();
                    let same = told.as_ref() == Some(&told_now);
                    if ended || !same {
                        match source.watch(&WatchParams {
                            checkouts: if init.scope == Scope::Ref {
                                told_now.clone()
                            } else {
                                Vec::new()
                            },
                        }) {
                            Ok(()) => {
                                if let Some(run) = locked(&self.state).runs.get_mut(name) {
                                    run.told = Some(told_now);
                                }
                            }
                            Err(error) => {
                                self.mark_source_stale(name, &source_text(&error));
                                return;
                            }
                        }
                    }
                }
                if moved || ended || !watching || stale || hinted {
                    let which = if stale && !moved && !ended && watching && !hinted {
                        self.stale_targets(name, &targets)
                    } else {
                        targets.clone()
                    };
                    let empty: Vec<String> = if moved { gone } else { Vec::new() };
                    self.relist(name, &source, &which, &empty, ChangeCause::Worktree);
                }
            }
        }
    }

    /// The checkouts the corpus lists that the set no longer has.
    fn gone_checkouts(&self, now: &[CheckoutInfo]) -> Vec<String> {
        let state = locked(&self.state);
        state
            .checkouts
            .iter()
            .filter(|old| !now.iter().any(|info| info.path == old.path))
            .map(|old| old.path.clone())
            .collect()
    }

    /// The targets a stale source lists again: the checkouts it said it could not read, else all.
    fn stale_targets(&self, name: &str, all: &[Option<String>]) -> Vec<Option<String>> {
        let state = locked(&self.state);
        match state.runs.get(name).and_then(|run| run.stale.as_ref()) {
            Some(stale) if !stale.whole && !stale.checkouts.is_empty() => all
                .iter()
                .filter(|target| {
                    target
                        .as_ref()
                        .is_some_and(|checkout| stale.checkouts.contains(checkout))
                })
                .cloned()
                .collect(),
            _ => all.to_vec(),
        }
    }

    fn declare(&self, name: &str, source: &Source, init: &InitializeResult) {
        if name != scratch::SOURCE
            && let Err(error) = self.config.store.corpus_put_source(&self.board, name, init)
        {
            self.log(&format!("the declaration of {name} was not kept: {error}"));
        }
        let mut state = locked(&self.state);
        state.corpus.declare_source(name, init);
        let stale = state.runs.remove(name).and_then(|run| run.stale);
        state.runs.insert(
            name.to_string(),
            Run {
                instance: source.instance(),
                watch: init.capabilities.watch,
                told: None,
                stale,
            },
        );
    }

    /// Lists `source` again for `targets`, empties `gone` checkouts, and records what differs from what the corpus
    /// held, with `cause`. A list that fails marks the source stale and changes nothing else.
    fn relist(
        &self,
        name: &str,
        source: &Source,
        targets: &[Option<String>],
        gone: &[String],
        cause: ChangeCause,
    ) {
        let _gate = locked(&self.gate);
        let lists = match self.list_targets(source, targets) {
            Ok(lists) => lists,
            Err(error) => {
                let why = source_text(&error);
                self.log(&format!("{name} could not be listed: {why}"));
                self.mark_source_stale(name, &why);
                return;
            }
        };
        let mut state = locked(&self.state);
        let before = state.corpus.clone();
        for (target, list) in lists {
            self.keep(name, target.as_deref(), &list);
            let _ = state.corpus.load(name, target.as_deref(), list);
        }
        for checkout in gone {
            let empty = ListResult {
                tasks: Vec::new(),
                collections: Vec::new(),
                invalid: Vec::new(),
                next_cursor: None,
            };
            self.keep(name, Some(checkout), &empty);
            let _ = state.corpus.load(name, Some(checkout), empty);
        }
        let _ = state.corpus.mark_fresh(name);
        if let Some(run) = state.runs.get_mut(name) {
            run.stale = None;
        }
        let events = reconcile_by_source(&before, &state.corpus);
        drop(state);
        for (event_source, event) in events {
            let cause = match &event {
                ProviderEvent::Changed(changed) if changed.checkout.is_none() => {
                    ChangeCause::Remote
                }
                _ => cause,
            };
            self.record(&event_source, event, Some(cause));
        }
    }

    // ---- the notifications -------------------------------------------------------------------------------------

    /// Reads one provider's notifications until it is stopped or the daemon is.
    fn pump(
        self: &Arc<Self>,
        name: &str,
        provider: &SupervisedProvider,
        events: &Receiver<ProviderEvent>,
    ) {
        loop {
            match events.recv_timeout(SLICE) {
                Ok(event) => self.handle(name, event),
                Err(RecvTimeoutError::Timeout) => {
                    if self.stopping() || provider.is_stopped() {
                        return;
                    }
                }
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    fn handle(&self, name: &str, event: ProviderEvent) {
        let _gate = locked(&self.gate);
        match event {
            ProviderEvent::Changed(changed) => self.changed(name, changed),
            ProviderEvent::Renamed(rename) => self.renamed(name, rename),
            ProviderEvent::Removed(removed) => {
                let checkouts = self.copies_of(&removed.key, removed.checkout.as_deref());
                {
                    let mut state = locked(&self.state);
                    let _ = state.corpus.apply_removed(name, &removed);
                }
                for checkout in &checkouts {
                    self.drop_row(name, checkout.as_deref(), &removed.key);
                }
                self.record(name, ProviderEvent::Removed(removed), None);
            }
            ProviderEvent::Invalid(item) => {
                {
                    let mut state = locked(&self.state);
                    let _ = state.corpus.apply_invalid(name, &item);
                }
                if let Err(error) = self
                    .config
                    .store
                    .corpus_put_invalid(&self.board, name, &item)
                {
                    self.log(&format!("the cache of {name} was not kept: {error}"));
                }
                self.record(name, ProviderEvent::Invalid(item), None);
            }
            ProviderEvent::Collision(_) => {
                locked(&self.state).wanted.insert(name.to_string());
            }
            ProviderEvent::Stale(SourceStale { reason, checkout }) => {
                let mut state = locked(&self.state);
                let _ = state.corpus.mark_stale(name, &reason);
                if let Some(run) = state.runs.get_mut(name) {
                    let stale = run.stale.get_or_insert_with(StaleAt::default);
                    match checkout {
                        Some(checkout) => {
                            stale.checkouts.insert(checkout);
                        }
                        None => stale.whole = true,
                    }
                }
            }
            ProviderEvent::Unreadable { method, detail } => {
                self.log(&format!(
                    "{name} sent {method} that is not readable: {detail}"
                ));
            }
        }
    }

    fn changed(&self, name: &str, changed: TaskChanged) {
        let triple = (
            changed.key.clone(),
            changed.checkout.clone(),
            changed.version.clone(),
        );
        {
            let mut state = locked(&self.state);
            if let Some(at) = state.written.iter().position(|held| *held == triple) {
                state.written.remove(at);
                return;
            }
        }
        let applied = locked(&self.state)
            .corpus
            .apply_changed(name, &changed, None);
        match applied {
            Ok(collide_core::corpus::Applied::Done) => {}
            Ok(collide_core::corpus::Applied::NeedsTask) => {
                let Some(task) = self.fetch(name, &changed.key, changed.checkout.as_deref()) else {
                    self.log(&format!(
                        "{} changed but the source does not have it",
                        changed.key
                    ));
                    return;
                };
                let mut state = locked(&self.state);
                let _ = state.corpus.apply_changed(name, &changed, Some(task));
            }
            Err(error) => {
                self.log(&format!(
                    "a change of {} was not applied: {error}",
                    changed.key
                ));
                return;
            }
        }
        if changed.to.is_none() {
            self.drop_row(name, changed.checkout.as_deref(), &changed.key);
        } else {
            self.keep_copy(name, &changed.key, changed.checkout.as_deref());
        }
        self.record(name, ProviderEvent::Changed(changed), None);
    }

    fn renamed(&self, name: &str, rename: collide_core::change::Rename) {
        let moved = self.copies_of(&rename.from, rename.checkout.as_deref());
        // The renamed copy keeps its old id until it is listed again, so the one task is fetched first, and the
        // rename and the fetched task go in together: a list never shows the new key with the old id.
        let fetched = self.fetch(name, &rename.to, rename.checkout.as_deref());
        {
            let mut state = locked(&self.state);
            let _ = state.corpus.apply_renamed(name, &rename);
            if let Some(task) = fetched {
                let checkout = task.checkout.clone().or(rename.checkout.clone());
                let note = TaskChanged {
                    key: rename.to.clone(),
                    version: task.version.clone(),
                    from: None,
                    to: Some(task.state.clone()),
                    diff: Default::default(),
                    cause: ChangeCause::Worktree,
                    actor: None,
                    automated: None,
                    checkout,
                };
                let _ = state.corpus.apply_changed(name, &note, Some(task));
            }
        }
        for checkout in &moved {
            self.drop_row(name, checkout.as_deref(), &rename.from);
        }
        for checkout in &moved {
            self.keep_copy(name, &rename.to, checkout.as_deref());
        }
        self.record(name, ProviderEvent::Renamed(rename), None);
    }

    /// The checkouts that hold a copy of `key`, or the one named.
    fn copies_of(&self, key: &str, checkout: Option<&str>) -> Vec<Option<String>> {
        if let Some(checkout) = checkout {
            return vec![Some(checkout.to_string())];
        }
        let state = locked(&self.state);
        let copies = state.corpus.get(key).copies;
        if copies.is_empty() {
            vec![None]
        } else {
            copies.into_iter().map(|copy| Some(copy.checkout)).collect()
        }
    }

    fn fetch(&self, name: &str, key: &str, checkout: Option<&str>) -> Option<Task> {
        let source = self.source(name).ok()?;
        match source.get(&GetParams {
            id: short_of(key).to_string(),
            checkout: checkout.map(str::to_string),
        }) {
            Ok(got) => got.task.map(without_body),
            Err(error) => {
                self.log(&format!("{key} was not fetched: {}", source_text(&error)));
                None
            }
        }
    }

    fn drop_row(&self, source: &str, checkout: Option<&str>, key: &str) {
        if let Err(error) = self
            .config
            .store
            .corpus_drop_task(&self.board, source, checkout, key)
        {
            self.log(&format!("the cache of {source} was not kept: {error}"));
        }
    }

    /// Puts the corpus's copy of `key` in `checkout` into the cache.
    fn keep_copy(&self, source: &str, key: &str, checkout: Option<&str>) {
        let copy = locked(&self.state).corpus.copy(key, checkout).cloned();
        if let Some(task) = copy
            && let Err(error) =
                self.config
                    .store
                    .corpus_put_task(&self.board, source, checkout, &task)
        {
            self.log(&format!("the cache of {source} was not kept: {error}"));
        }
    }

    // ---- a write -----------------------------------------------------------------------------------------------

    fn note_write(&self, source: &str, task: &Task, items: &[Effect]) {
        let kept = source != scratch::SOURCE;
        let cause = if kept {
            ChangeCause::Worktree
        } else {
            ChangeCause::Store
        };
        for effect in items {
            let note = TaskChanged {
                key: effect.key.clone(),
                version: effect.version.clone(),
                from: effect.from.clone(),
                to: effect.to.clone(),
                diff: Default::default(),
                cause,
                actor: None,
                automated: None,
                checkout: task.checkout.clone(),
            };
            let primary = effect.key == task.key;
            let applied = {
                let mut state = locked(&self.state);
                let given = primary.then(|| without_body(task.clone()));
                match state.corpus.apply_changed(source, &note, given) {
                    Ok(collide_core::corpus::Applied::Done) => true,
                    Ok(collide_core::corpus::Applied::NeedsTask) | Err(_) => false,
                }
            };
            if !applied || !kept {
                continue;
            }
            if effect.to.is_none() {
                self.drop_row(source, task.checkout.as_deref(), &effect.key);
            } else {
                self.keep_copy(source, &effect.key, task.checkout.as_deref());
            }
            let mut state = locked(&self.state);
            state.written.push_back((
                effect.key.clone(),
                task.checkout.clone(),
                effect.version.clone(),
            ));
            while state.written.len() > WRITTEN_KEPT {
                state.written.pop_front();
            }
        }
    }
}

/// What the cache holds of one source in one checkout, as a complete `list`.
fn list_from(cache: &CorpusCache, source: &str, checkout: Option<&str>) -> ListResult {
    ListResult {
        tasks: cache
            .tasks
            .iter()
            .filter(|held| held.source == source && held.checkout.as_deref() == checkout)
            .map(|held| held.task.clone())
            .collect(),
        collections: Vec::new(),
        invalid: cache
            .invalid
            .iter()
            .filter(|held| held.source == source && held.item.checkout.as_deref() == checkout)
            .map(|held| held.item.clone())
            .collect(),
        next_cursor: None,
    }
}

fn source_text(error: &SourceError) -> String {
    match error {
        SourceError::Refused(text)
        | SourceError::NoSource(text)
        | SourceError::Unavailable(text)
        | SourceError::Unsupported(text)
        | SourceError::Failed(text) => text.clone(),
    }
}
