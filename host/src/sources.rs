//! The sources of a board (ts-0113-ck; 0.10, Planning sources; 0.9, Trust): where the daemon finds the provider
//! behind a task key and calls it. A source is named by its provider's key in a pack's `providers:`, and a board has
//! one source of each name, besides the built-in `scratch` source that lives in the store.
//!
//! **What a board's sources are.** Each request reads the line the board's checkout is on ([`Lines::for_sources`]):
//! the commit its ref names now, never the working copy. The sources are the providers of the packs that line loaded.
//! A name two packs declare is the board's own pack over the built-in one (`collide/plan` gives way to the pack a board
//! lists), and two packs that are not built in make no source of that name: the line reports it ([`source_problems`])
//! and a request for the source is refused with the same words.
//!
//! **When a provider starts.** At the first request for its source, not when the line opens, so opening a board runs no
//! repo code. A provider of a pack that is not built in starts only while the line is trusted at its current hash: a
//! committed change to what runs makes the line untrusted, and the provider that was running is stopped by the next
//! request on the board. It starts in the cache's copy of the pack's snapshot ([`provider_launch`]), with the captured
//! base environment, the pack's data folder, and `initialize` carrying the pack's settings through the layers and the
//! board's root. A secret is read from the secret store then, and goes nowhere but into that `initialize`.
//!
//! **What is kept.** Per board, one [`Slot`] for each source, kept while the pack's folder in the cache is the same one
//! and the line is trusted, so a commit that changes nothing a provider runs does not restart it. A settings change
//! reaches a provider at its next start.
//!
//! **One write.** [`Source::apply`] is what the daemon calls for every source's write, scratch included; the daemon's
//! one function around it records the event. The line lock, the busy root and the queue of `ts-0114-hr` go there.

use crate::base_env::BaseEnvironment;
use crate::board_lines::{LineError, SourceLine};
use crate::board_load::{LoadedLine, LoadedPack, PackOrigin};
use crate::config_get::{BoardRef, ConfigService};
use crate::provider_launch::provider_launch;
use crate::scratch::{self, ScratchSource};
use crate::store::Store;
use crate::supervisor::{SupervisedProvider, SupervisorPolicy};
use collide_core::adapter::EnvStrip;
use collide_core::board::CheckoutFilter;
use collide_core::change::{Effect, FieldDiff, ItemState};
use collide_core::events::EventBody;
use collide_core::owner::TrustState;
use collide_core::ports::{ProviderError, TaskProvider};
use collide_core::provider::{
    ApplyParams, ApplyResult, ChangeCause, GetParams, GetResult, InitializeParams,
    InitializeResult, ListParams, ListResult, PreviewResult, TaskChanged, WatchParams,
};
use collide_core::roles::{check_roles, read_roles};
use serde_json::json;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

/// Why a source gave no answer. The daemon words each as an owner error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// The state refuses the request: the line does not load, it is not trusted, or two packs claim the source.
    Refused(String),
    /// The line has no source of that name.
    NoSource(String),
    /// The source cannot be reached now: it is gone, or the daemon is stopping.
    Unavailable(String),
    /// The source does not do what was asked.
    Unsupported(String),
    /// Anything else: the line's home, git, the launch, the provider's own failure.
    Failed(String),
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

// ---- which packs provide which source -----------------------------------------------------------------------------

/// One source of a line: its name and the provider that gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub name: String,
    pub pack: String,
    pub provider: String,
    pub origin: PackOrigin,
    pub dir: PathBuf,
}

/// A source name that more packs declare than one source can have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clash {
    pub name: String,
    pub packs: Vec<String>,
}

impl Clash {
    pub fn message(&self) -> String {
        format!(
            "the source `{}` is declared by {}, so no pack provides it on this board: keep one of them",
            self.name,
            self.packs
                .iter()
                .map(|pack| format!("`{pack}`"))
                .collect::<Vec<_>>()
                .join(" and ")
        )
    }
}

/// The sources of a loaded line, and the names that clash.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Plan {
    pub sources: Vec<Planned>,
    pub clashes: Vec<Clash>,
}

/// The sources a line's packs give. One pack that declares a name, or one that is not built in beside the built-in
/// one that also does, provides it; two that are not built in, or a pack and the built-in `scratch`, do not.
pub fn plan(line: &LoadedLine) -> Plan {
    let mut declared: BTreeMap<&str, Vec<(&LoadedPack, &str)>> = BTreeMap::new();
    for pack in &line.packs {
        for provider in pack.manifest.providers.keys() {
            declared
                .entry(provider.as_str())
                .or_default()
                .push((pack, provider.as_str()));
        }
    }
    let mut plan = Plan::default();
    for (name, found) in declared {
        let own: Vec<_> = found
            .iter()
            .filter(|(pack, _)| pack.origin != PackOrigin::Builtin)
            .collect();
        let scratch_clash = name == scratch::SOURCE;
        if own.len() > 1 || (scratch_clash && !own.is_empty()) {
            let mut packs: Vec<String> = Vec::new();
            if scratch_clash {
                packs.push("the built-in scratch source".to_string());
            }
            packs.extend(own.iter().map(|(pack, _)| pack.name.clone()));
            plan.clashes.push(Clash {
                name: name.to_string(),
                packs,
            });
            continue;
        }
        let (pack, provider) = own.first().map(|found| **found).unwrap_or(found[0]);
        plan.sources.push(Planned {
            name: name.to_string(),
            pack: pack.name.clone(),
            provider: provider.to_string(),
            origin: pack.origin,
            dir: pack.dir.clone(),
        });
    }
    plan
}

/// The name of the planning source: the provider `collide/plan` declares, which a board's own pack may replace.
pub const PLANNING_SOURCE: &str = "plan";

/// What `roles` gets wrong against the kinds `declared` by `planned`, when it is the planning source (ts-0135-cx; 0.10,
/// Planning sources): a kind a role names that the source does not have, one line each. A value that is not a map of
/// roles is a problem too. The source works either way.
fn role_problems(
    planned: &Planned,
    roles: Option<&serde_json::Value>,
    declared: &InitializeResult,
) -> Vec<String> {
    let Some(roles) = roles.filter(|_| planned.name == PLANNING_SOURCE) else {
        return Vec::new();
    };
    match read_roles(roles) {
        Ok(roles) => check_roles(&roles, &planned.name, &declared.kinds)
            .problems
            .iter()
            .map(ToString::to_string)
            .collect(),
        Err(message) => vec![message],
    }
}

/// What a line reports about its sources, one line each: the names two packs declare.
pub fn source_problems(line: &LoadedLine) -> Vec<String> {
    plan(line).clashes.iter().map(Clash::message).collect()
}

// ---- the events of a write ----------------------------------------------------------------------------------------

/// The `task.changed` event of one effect of a write: its key, version and states, and the fields that differ between
/// them (the status always, and the category when it moved). The actor is the event's, set by the daemon from the
/// connection, and the source names none of its own. `cause` is where the change was made, and `checkout` the copy it
/// was made in, for a ref-scoped source.
pub fn changed_event(
    source: &str,
    effect: &Effect,
    cause: ChangeCause,
    checkout: Option<String>,
) -> EventBody {
    let status = |state: &Option<ItemState>| match state {
        Some(state) => json!(state.status),
        None => serde_json::Value::Null,
    };
    let category = |state: &Option<ItemState>| match state {
        Some(state) => json!(state.category),
        None => serde_json::Value::Null,
    };
    let mut diff = BTreeMap::from([(
        "status".to_string(),
        FieldDiff::Changed {
            from: status(&effect.from),
            to: status(&effect.to),
        },
    )]);
    if category(&effect.from) != category(&effect.to) {
        diff.insert(
            "category".to_string(),
            FieldDiff::Changed {
                from: category(&effect.from),
                to: category(&effect.to),
            },
        );
    }
    EventBody::TaskChanged {
        source: source.to_string(),
        task: TaskChanged {
            key: effect.key.clone(),
            version: effect.version.clone(),
            from: effect.from.clone(),
            to: effect.to.clone(),
            diff,
            cause,
            actor: None,
            automated: None,
            checkout,
        },
    }
}

// ---- one source ---------------------------------------------------------------------------------------------------

enum Backing {
    Scratch(ScratchSource),
    Pack(Arc<SupervisedProvider>),
}

/// A source ready to be called: what the daemon's three task methods go through.
pub struct Source {
    name: String,
    /// The pack that provides it, `None` for the scratch source.
    pack: Option<String>,
    backing: Backing,
    /// What the provider answered to `initialize` when it started. `None` for the scratch source, which is asked.
    init: Option<InitializeResult>,
    /// Which start of the provider this is: a new one after the provider was stopped or its pack changed. 0 for the
    /// scratch source.
    instance: u64,
}

impl Source {
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The cause of a change this source's write makes: `store` for the scratch source, which lives in Collide's own
    /// store, and `worktree` for a pack's, which writes into the checkout it is given.
    pub fn write_cause(&self) -> ChangeCause {
        match self.backing {
            Backing::Scratch(_) => ChangeCause::Store,
            Backing::Pack(_) => ChangeCause::Worktree,
        }
    }

    /// Whether this source's write goes into a checkout, so it waits for a busy one and for the line lock (ts-0114-hr).
    /// A scratch write lives in Collide's store and touches none.
    pub fn writes_checkout(&self) -> bool {
        matches!(self.backing, Backing::Pack(_))
    }

    /// What the source declared at `initialize`: its scope, statuses, kinds, links and capabilities.
    pub fn declared(&self) -> Result<InitializeResult, SourceError> {
        match &self.init {
            Some(init) => Ok(init.clone()),
            None => self
                .provider()
                .initialize(&InitializeParams {
                    config: BTreeMap::new(),
                    root: String::new(),
                })
                .map_err(|e| self.failed(e)),
        }
    }

    /// Which start of the provider this is (see the field). A different number than before is a new provider: it was
    /// told nothing of the old one's `watch`, and its notifications come on a stream of their own.
    pub fn instance(&self) -> u64 {
        self.instance
    }

    /// The pack's supervised provider, `None` for the scratch source. The corpus reads its notifications, and whether
    /// it is running, gone or stopped.
    pub fn supervised(&self) -> Option<Arc<SupervisedProvider>> {
        match &self.backing {
            Backing::Scratch(_) => None,
            Backing::Pack(provider) => Some(provider.clone()),
        }
    }

    /// Tells the source which checkouts to watch; each call replaces the set (0.10, Protocol).
    pub fn watch(&self, params: &WatchParams) -> Result<(), SourceError> {
        self.provider()
            .watch(params)
            .map(|_| ())
            .map_err(|e| self.failed(e))
    }

    fn provider(&self) -> &dyn TaskProvider {
        match &self.backing {
            Backing::Scratch(scratch) => scratch,
            Backing::Pack(provider) => provider.as_ref(),
        }
    }

    /// One page of the source's tasks. The corpus (`ts-0116-wh`) is its caller.
    pub fn list(&self, params: &ListParams) -> Result<ListResult, SourceError> {
        self.provider().list(params).map_err(|e| self.failed(e))
    }

    pub fn get(&self, params: &GetParams) -> Result<GetResult, SourceError> {
        self.provider().get(params).map_err(|e| self.failed(e))
    }

    /// The one call every source's write goes through.
    pub fn apply(&self, params: &ApplyParams) -> Result<ApplyResult, SourceError> {
        self.provider().apply(params).map_err(|e| self.failed(e))
    }

    pub fn preview(&self, params: &ApplyParams) -> Result<PreviewResult, SourceError> {
        self.provider().preview(params).map_err(|e| self.failed(e))
    }

    fn named(&self) -> String {
        match &self.pack {
            Some(pack) => format!("the source `{}` of pack `{pack}`", self.name),
            None => format!("the source `{}`", self.name),
        }
    }

    fn failed(&self, error: ProviderError) -> SourceError {
        match error {
            ProviderError::Gone => {
                let why = match &self.backing {
                    Backing::Pack(provider) => provider
                        .last_failure()
                        .map(|why| format!("; the last failure: {why}"))
                        .unwrap_or_default(),
                    Backing::Scratch(_) => String::new(),
                };
                SourceError::Unavailable(format!(
                    "{} is gone: it kept crashing, so it is not started again until its pack changes on the \
                     line or the daemon restarts{why}",
                    self.named()
                ))
            }
            ProviderError::Unsupported { method } => {
                SourceError::Unsupported(format!("{} does not support `{method}`", self.named()))
            }
            ProviderError::Failed { message } => SourceError::Failed(message),
        }
    }
}

// ---- the registry -------------------------------------------------------------------------------------------------

/// What makes a provider the same one: the same source from the same folder of the cache.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Identity {
    pack: String,
    provider: String,
    dir: PathBuf,
}

/// A provider that started, and what it answered to `initialize`.
#[derive(Clone)]
struct Started {
    provider: Arc<SupervisedProvider>,
    init: InitializeResult,
    instance: u64,
    /// What the source's `roles` setting gets wrong against the kinds it declared ([`role_problems`]).
    problems: Vec<String>,
}

/// One source of one board, and its provider once it has started.
struct Slot {
    identity: Identity,
    started: Mutex<Option<Started>>,
}

impl Slot {
    fn new(planned: &Planned) -> Slot {
        Slot {
            identity: Identity {
                pack: planned.pack.clone(),
                provider: planned.provider.clone(),
                dir: planned.dir.clone(),
            },
            started: Mutex::new(None),
        }
    }

    fn stop(&self) {
        if let Some(started) = locked(&self.started).take() {
            started.provider.stop();
        }
    }
}

/// What a [`Sources`] is made of.
pub struct SourcesConfig {
    pub config: ConfigService,
    pub store: Arc<Store>,
    /// The folder the packs' data folders are made in: `<root>/<board>/<pack>` (0.7, Pack data).
    pub data_root: PathBuf,
    pub policy: SupervisorPolicy,
    /// The captured base environment, waiting for the first capture. `None` when there is none.
    pub base_env: Arc<dyn Fn() -> Option<BaseEnvironment> + Send + Sync>,
}

/// The sources of every board the daemon has been asked about.
pub struct Sources {
    config: SourcesConfig,
    boards: Mutex<BTreeMap<String, BTreeMap<String, Arc<Slot>>>>,
    stopped: AtomicBool,
    /// The last number [`Source::instance`] gave.
    instances: AtomicU64,
}

impl Sources {
    pub fn new(config: SourcesConfig) -> Sources {
        Sources {
            config,
            boards: Mutex::new(BTreeMap::new()),
            stopped: AtomicBool::new(false),
            instances: AtomicU64::new(0),
        }
    }

    /// The source `name` of `board`. The scratch source needs no line. Any other is read from the board's line, and its
    /// provider is started if it is not running: the line must load, be trusted (a built-in pack needs no trust), and
    /// the source must be one of its own.
    pub fn source(
        &self,
        board: BoardRef,
        name: &str,
        tools: &OsStr,
    ) -> Result<Source, SourceError> {
        if self.stopped.load(Ordering::SeqCst) {
            return Err(SourceError::Unavailable(
                "the daemon is shutting down".to_string(),
            ));
        }
        if name == scratch::SOURCE {
            return Ok(Source {
                name: name.to_string(),
                pack: None,
                backing: Backing::Scratch(ScratchSource::new(self.config.store.clone(), board.id)),
                init: None,
                instance: 0,
            });
        }
        let current = self
            .config
            .config
            .lines()
            .for_sources(board, tools)
            .map_err(line_failed)?;
        let plan = plan(&current.line);
        let slot = self.refresh(board.id, &current, &plan, name);
        if let Some(clash) = plan.clashes.iter().find(|clash| clash.name == name) {
            return Err(SourceError::Refused(clash.message()));
        }
        let planned = plan
            .sources
            .iter()
            .find(|planned| planned.name == name)
            .zip(slot)
            .ok_or_else(|| {
                SourceError::NoSource(format!(
                    "the board has no source `{name}`: its line `{}` has {}",
                    current.ref_name,
                    sources_text(&plan)
                ))
            })?;
        let (planned, slot) = planned;
        if planned.origin != PackOrigin::Builtin && !trusted(&current) {
            return Err(SourceError::Refused(untrusted_text(planned, &current)));
        }
        let started = self.start(board, &current, planned, &slot)?;
        Ok(Source {
            name: name.to_string(),
            pack: Some(planned.pack.clone()),
            backing: Backing::Pack(started.provider),
            init: Some(started.init),
            instance: started.instance,
        })
    }

    /// Brings the board's slots in step with its line: a slot stays while its source is the same pack folder and the
    /// line is trusted (or the pack is built in), and the others are stopped. Returns the slot for `name`.
    fn refresh(
        &self,
        board: &str,
        current: &SourceLine,
        plan: &Plan,
        name: &str,
    ) -> Option<Arc<Slot>> {
        let trusted = trusted(current);
        let mut stale: Vec<Arc<Slot>> = Vec::new();
        let wanted = {
            let mut boards = locked(&self.boards);
            let mut old = boards.remove(board).unwrap_or_default();
            let mut kept: BTreeMap<String, Arc<Slot>> = BTreeMap::new();
            for planned in &plan.sources {
                let fresh = Slot::new(planned);
                let allowed = planned.origin == PackOrigin::Builtin || trusted;
                match old.remove(&planned.name) {
                    Some(slot) if allowed && slot.identity == fresh.identity => {
                        kept.insert(planned.name.clone(), slot);
                    }
                    Some(slot) => {
                        stale.push(slot);
                        kept.insert(planned.name.clone(), Arc::new(fresh));
                    }
                    None => {
                        kept.insert(planned.name.clone(), Arc::new(fresh));
                    }
                }
            }
            stale.extend(old.into_values());
            let wanted = kept.get(name).cloned();
            boards.insert(board.to_string(), kept);
            wanted
        };
        // Stopping waits for the child to end, so it is done outside the registry's lock.
        for slot in stale {
            slot.stop();
        }
        wanted
    }

    fn start(
        &self,
        board: BoardRef,
        current: &SourceLine,
        planned: &Planned,
        slot: &Slot,
    ) -> Result<Started, SourceError> {
        let mut started = locked(&slot.started);
        if let Some(running) = started.as_ref() {
            return Ok(running.clone());
        }
        let described = format!("the source `{}` of pack `{}`", planned.name, planned.pack);
        let failed = |why: String| SourceError::Failed(format!("{described} did not start: {why}"));
        let pack = current
            .line
            .packs
            .iter()
            .find(|pack| pack.name == planned.pack)
            .ok_or_else(|| failed("the line no longer has its pack".to_string()))?;
        let launch = provider_launch(&current.context, pack, &planned.provider)
            .map_err(|error| failed(error.to_string()))?;
        let environment = (self.config.base_env)()
            .ok_or_else(|| failed("the daemon has no base environment yet".to_string()))?;
        let launch = launch
            .with_env(environment.for_child(&EnvStrip::default(), &BTreeMap::new()))
            .with_pack_data(&self.config.data_root, board.id, &planned.pack)
            .map_err(|error| failed(error.to_string()))?;
        let config = self
            .config
            .config
            .pack_config(board, &current.line, &planned.pack)
            .map_err(failed)?;
        let provider = Arc::new(SupervisedProvider::new(
            format!("{} (source {})", planned.pack, planned.name),
            launch,
            self.config.policy,
        ));
        let init = InitializeParams {
            config,
            root: board.root.display().to_string(),
        };
        let roles = init.config.get("roles").cloned();
        let declared = match provider.initialize(&init) {
            Ok(declared) => declared,
            Err(error) => {
                provider.stop();
                return Err(match error {
                    ProviderError::Failed { message } => SourceError::Failed(message),
                    other => failed(other.to_string()),
                });
            }
        };
        if self.stopped.load(Ordering::SeqCst) {
            provider.stop();
            return Err(SourceError::Unavailable(
                "the daemon is shutting down".to_string(),
            ));
        }
        let running = Started {
            provider,
            problems: role_problems(planned, roles.as_ref(), &declared),
            init: declared,
            instance: self.instances.fetch_add(1, Ordering::SeqCst) + 1,
        };
        *started = Some(running.clone());
        Ok(running)
    }

    /// What the board's `roles` setting gets wrong against the planning source's kinds, one line each. Read from the
    /// providers that have started, so nothing is started to answer: a source that has not started has none yet.
    pub fn role_problems(&self, board: &str) -> Vec<String> {
        let slots: Vec<Arc<Slot>> = locked(&self.boards)
            .get(board)
            .map(|slots| slots.values().cloned().collect())
            .unwrap_or_default();
        let mut problems: Vec<String> = Vec::new();
        for slot in slots {
            if let Some(started) = locked(&slot.started).as_ref() {
                problems.extend(started.problems.iter().cloned());
            }
        }
        problems
    }

    /// What a board's line plans, with no provider started: the names of its sources besides `scratch`, and the
    /// board's `checkout_filter`. The corpus asks for each source by name, which starts it if the line allows.
    pub fn board_plan(&self, board: BoardRef, tools: &OsStr) -> Result<BoardPlan, SourceError> {
        let current = self
            .config
            .config
            .lines()
            .for_sources(board, tools)
            .map_err(line_failed)?;
        let planned = plan(&current.line).sources;
        Ok(BoardPlan {
            packs: planned
                .iter()
                .map(|source| (source.name.clone(), source.pack.clone()))
                .collect(),
            sources: planned.into_iter().map(|source| source.name).collect(),
            checkout_filter: current.line.board.checkout_filter.clone(),
        })
    }

    /// Stops the providers of the pack a settings key belongs to (`<pack>.<setting>`), on `board` or, with `None`, on
    /// every board: a provider reads its settings and secrets at start, so the next call starts it with the new ones
    /// (ts-0113-ck). A key of no running pack stops nothing. The stopped provider's notifications end with it.
    pub fn stop_pack(&self, board: Option<&str>, key: &str) {
        let slots: Vec<Arc<Slot>> = {
            let boards = locked(&self.boards);
            boards
                .iter()
                .filter(|(id, _)| board.is_none_or(|only| only == id.as_str()))
                .flat_map(|(_, slots)| slots.values().cloned())
                .filter(|slot| {
                    key.strip_prefix(slot.identity.pack.as_str())
                        .is_some_and(|rest| rest.starts_with('.'))
                })
                .collect()
        };
        for slot in slots {
            slot.stop();
        }
    }

    /// Stops every provider, and starts none after. The daemon's stop calls it: no child outlives the daemon.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
        let boards = std::mem::take(&mut *locked(&self.boards));
        for slot in boards.into_values().flat_map(BTreeMap::into_values) {
            slot.stop();
        }
    }
}

/// What a board's line plans for the corpus: its sources, and which checkouts are not the board's.
#[derive(Debug, Clone, PartialEq)]
pub struct BoardPlan {
    pub sources: Vec<String>,
    /// The pack that provides each of `sources`, by source name.
    pub packs: BTreeMap<String, String>,
    pub checkout_filter: CheckoutFilter,
}

fn trusted(current: &SourceLine) -> bool {
    matches!(current.trust, TrustState::Trusted { .. })
}

fn untrusted_text(planned: &Planned, current: &SourceLine) -> String {
    let hash = match &current.trust {
        TrustState::Trusted { hash } | TrustState::Untrusted { hash } => hash.as_str(),
    };
    let mut text = format!(
        "the line `{}` is not trusted, so the source `{}` of pack `{}` is not started: \
         review it and trust it (`collide trust`); its hash is {hash}",
        current.ref_name, planned.name, planned.pack
    );
    for problem in &current.problems {
        text.push_str(&format!("; {problem}"));
    }
    text
}

fn sources_text(plan: &Plan) -> String {
    let names: Vec<&str> = plan
        .sources
        .iter()
        .map(|planned| planned.name.as_str())
        .collect();
    if names.is_empty() {
        "no sources besides `scratch`".to_string()
    } else {
        format!("`scratch` and {}", names.join(", "))
    }
}

fn line_failed(error: LineError) -> SourceError {
    match error {
        LineError::Refused(message) => SourceError::Refused(format!(
            "the board's line does not load, so its sources are not started: {message}"
        )),
        LineError::NotFound(message) | LineError::Invalid(message) | LineError::Failed(message) => {
            SourceError::Failed(message)
        }
    }
}
