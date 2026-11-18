//! Views in the daemon (ts-0119-tj; 0.10, Views; 0.9, Trust). Where a view comes from, how one is read, and when its
//! members are worked out again. [`crate::daemon`] turns these into `task.views`, `task.list` with a view and
//! `task.save_view`.
//!
//! **Three places, in this order.** The person's own files, `~/.config/collide/boards/<board-id>/views/*.yaml`: they
//! are global to the board, read from the file system at each call, and not hashed. The repo's, `.collide/views/*.yaml`
//! as the board's line has them: read from the blobs of the commit the line names, never from the working tree, and
//! only while the line is trusted, because the trust hash covers each view's blob id. A pack's, from its manifest's
//! `views:`, named `pack/name`: trusted with the pack, so a repo or a `git:` pack's view waits for the line, and a
//! built-in, added or linked pack's does not.
//!
//! **One name, one view.** The first view of a name in that order is the view; each later one is listed with an
//! error that says so. A file that does not parse, is not trusted yet or is too big is listed too, under its file name
//! without `.yaml`, with an empty view and the reason: one bad file never hides the others.
//!
//! **Cost.** A view is compiled, and its cost estimated, each time the views are listed ([`compile_view_with`]), so a
//! view that is too costly has its error before anything is evaluated. What a view's members are is worked out
//! again only when the board's corpus has changed ([`ViewCache`]), not at each call.
//!
//! **Writing.** [`save_view`] writes one file, atomically, and never commits it. The repo's file lands in the
//! working tree, and the line has it once the person commits it.

use crate::atomic_write::write_atomically;
use crate::board_lines::SourceLine;
use crate::board_load::{PackOrigin, ViewFile};
use crate::user_packs::UserScope;
use collide_core::corpus::Corpus;
use collide_core::owner::{TrustState, ViewSource};
use collide_core::resolve::is_safe_token;
use collide_core::view::View;
use collide_core::view_eval::{
    CompiledView, ViewFailure, ViewResult, compile_view_with, evaluate_view,
};
use collide_core::view_expr::Budgets;
use collide_core::yaml::{MAX_YAML_BYTES, load_yaml, load_yaml_file};
use collide_core::yaml_out::to_yaml;
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// Called with a view's `where` text just before it is parsed: a test makes it panic ([`compile_view_with`]).
pub type BeforeParse = Arc<dyn Fn(&str) + Send + Sync>;

/// The most evaluations kept for one board at one corpus: a filter bar that keeps making new ad-hoc views would
/// otherwise add to them until the corpus changes.
const KEPT: usize = 64;

const NOT_TRUSTED: &str =
    "the line is not trusted, so this view is not read: review it and trust it (`collide trust`)";

/// A view as it was found, before it is compiled.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub view: View,
    pub source: ViewSource,
    /// Why it cannot be used. Then `view` is empty but for its name.
    pub error: Option<String>,
}

impl Candidate {
    fn found(view: View, source: ViewSource) -> Candidate {
        Candidate {
            view,
            source,
            error: None,
        }
    }

    fn failed(name: &str, source: ViewSource, error: impl Into<String>) -> Candidate {
        Candidate {
            view: stand_in(name),
            source,
            error: Some(error.into()),
        }
    }
}

/// What a view that could not be read is listed as: its name, and nothing else.
fn stand_in(name: &str) -> View {
    View {
        name: name.to_string(),
        where_: None,
        include: Vec::new(),
        exclude: Vec::new(),
        closure: None,
        workflow: None,
        profile: None,
        layout: None,
        queue: false,
    }
}

fn stem(file: &str) -> &str {
    file.strip_suffix(".yaml").unwrap_or(file)
}

/// A candidate, compiled when it could be. `compiled` is `Some` exactly when `error` is `None`.
#[derive(Debug)]
pub struct Loaded {
    pub view: View,
    pub source: ViewSource,
    pub error: Option<String>,
    compiled: Option<CompiledView>,
}

impl Loaded {
    pub fn compiled(&self) -> Option<&CompiledView> {
        self.compiled.as_ref()
    }
}

fn parse(bytes: &[u8], origin: &str, name: &str, source: ViewSource) -> Candidate {
    match load_yaml::<View>(bytes, origin) {
        Ok(view) => Candidate::found(view, source),
        Err(error) => Candidate::failed(name, source, error.to_string()),
    }
}

// ---- where the views are --------------------------------------------------------------------------------------------

/// The folder of the person's views for `board`, or why the board's id cannot name one: a board id that is not a safe
/// name is never made into a path.
pub fn personal_dir(home: &Path, board: &str) -> Result<PathBuf, String> {
    let scope = UserScope::new(home.to_path_buf(), board.to_string(), Vec::new());
    scope.board_dir().map(|dir| dir.join("views"))
}

/// The person's views of `board`, in file name order. A folder that is not there has none. A file that is not a
/// regular file, or does not parse, is a candidate with its error.
pub fn personal_candidates(home: &Path, board: &str) -> Result<Vec<Candidate>, String> {
    let dir = personal_dir(home, board)?;
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("{}: {error}", dir.display())),
    };
    let mut names: Vec<String> = entries
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter(|name| name.ends_with(".yaml"))
        .collect();
    names.sort();
    let source = ViewSource::Personal {};
    Ok(names
        .into_iter()
        .map(|name| {
            let path = dir.join(&name);
            let regular = fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file());
            if !regular {
                let reason = format!("{} is not a regular file", path.display());
                return Candidate::failed(stem(&name), source.clone(), reason);
            }
            match load_yaml_file::<View>(&path) {
                Ok(view) => Candidate::found(view, source.clone()),
                Err(error) => Candidate::failed(stem(&name), source.clone(), error.to_string()),
            }
        })
        .collect())
}

/// The repo's views as the line has them, in file name order: read from the blobs of the line's commit while the
/// line is trusted, and listed with the reason and not read while it is not.
pub fn repo_candidates(line: &SourceLine) -> Vec<Candidate> {
    let files = &line.line.views;
    let source = ViewSource::Repo {};
    if !is_trusted(line) {
        return files
            .iter()
            .map(|file| Candidate::failed(stem(&file.name), source.clone(), NOT_TRUSTED))
            .collect();
    }
    let readable: Vec<&ViewFile> = files
        .iter()
        .filter(|file| file.size <= MAX_YAML_BYTES as u64)
        .collect();
    let ids: Vec<String> = readable.iter().map(|file| file.id.clone()).collect();
    let mut bodies: Result<VecDeque<Vec<u8>>, String> = line
        .context
        .git
        .read_blobs(&line.git_dir, &ids)
        .map(VecDeque::from)
        .map_err(|error| error.to_string());
    let dir = line.line.collide_dir.as_deref().unwrap_or(".collide");
    files
        .iter()
        .map(|file| {
            let name = stem(&file.name);
            if file.size > MAX_YAML_BYTES as u64 {
                let reason = format!(
                    "{dir}/views/{} is {} bytes, and a YAML file is capped at {MAX_YAML_BYTES}",
                    file.name, file.size
                );
                return Candidate::failed(name, source.clone(), reason);
            }
            match &mut bodies {
                Ok(bodies) => match bodies.pop_front() {
                    Some(bytes) => parse(
                        &bytes,
                        &format!("{dir}/views/{}", file.name),
                        name,
                        source.clone(),
                    ),
                    None => {
                        Candidate::failed(name, source.clone(), "git gave no content for this file")
                    }
                },
                Err(error) => Candidate::failed(name, source.clone(), error.clone()),
            }
        })
        .collect()
}

/// The views of the line's packs, named `pack/name`, in pack order. A pack whose code the line's trust covers gives
/// none to a line that is not trusted.
pub fn pack_candidates(line: &SourceLine) -> Vec<Candidate> {
    let trusted = is_trusted(line);
    let mut found = Vec::new();
    for pack in &line.line.packs {
        let covered = matches!(pack.origin, PackOrigin::Repo | PackOrigin::Git);
        for (key, view) in &pack.manifest.views {
            let name = format!("{}/{key}", pack.name);
            let source = ViewSource::Pack {
                pack: pack.name.clone(),
            };
            found.push(if covered && !trusted {
                Candidate::failed(&name, source, NOT_TRUSTED)
            } else {
                let view = View {
                    name,
                    ..view.clone()
                };
                Candidate::found(view, source)
            });
        }
    }
    found
}

fn is_trusted(line: &SourceLine) -> bool {
    matches!(line.trust, TrustState::Trusted { .. })
}

/// Where the repo's views are written: `views/` in the line's Collide folder, below the board's root. A Collide
/// folder that is not a plain relative path is refused.
pub fn repo_dir(root: &Path, collide_dir: &str) -> Result<PathBuf, String> {
    let plain = Path::new(collide_dir)
        .components()
        .all(|part| matches!(part, Component::Normal(_)));
    if collide_dir.is_empty() || !plain {
        return Err(format!(
            "the Collide folder {collide_dir:?} is not a folder inside the repository"
        ));
    }
    Ok(root.join(collide_dir).join("views"))
}

// ---- what is compiled, and what is worked out ----------------------------------------------------------------------

struct Slot {
    stamp: u64,
    results: HashMap<String, Result<Arc<ViewResult>, ViewFailure>>,
}

/// Compiles views, and keeps what each one's members were for the corpus they were worked out on. One cache serves
/// every board of the daemon.
pub struct ViewCache {
    before_parse: Option<BeforeParse>,
    slots: Mutex<BTreeMap<String, Slot>>,
    evaluations: AtomicU64,
}

impl ViewCache {
    pub fn new(before_parse: Option<BeforeParse>) -> ViewCache {
        ViewCache {
            before_parse,
            slots: Mutex::new(BTreeMap::new()),
            evaluations: AtomicU64::new(0),
        }
    }

    /// How many times a view has been worked out, hits not counted.
    pub fn evaluations(&self) -> u64 {
        self.evaluations.load(Ordering::SeqCst)
    }

    /// [`compile_view_with`] for a board of `tasks` tasks: the checks, the compile and the cost estimate.
    pub fn compile(&self, view: &View, tasks: usize) -> Result<CompiledView, ViewFailure> {
        let hook = |text: &str| {
            if let Some(before) = &self.before_parse {
                before(text);
            }
        };
        compile_view_with(view, tasks, &Budgets::default(), &hook)
    }

    /// Compiles each candidate that can be. `candidates` are in the order of precedence: the first of a name is the
    /// view, and a later one is listed with an error that says so.
    pub fn assemble(&self, candidates: Vec<Candidate>, tasks: usize) -> Vec<Loaded> {
        let mut taken: BTreeMap<String, ViewSource> = BTreeMap::new();
        let mut loaded = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let Candidate {
                view,
                source,
                mut error,
            } = candidate;
            if let Some(first) = taken.get(&view.name) {
                error.get_or_insert_with(|| shadowed(&view.name, first));
            }
            taken
                .entry(view.name.clone())
                .or_insert_with(|| source.clone());
            let mut compiled = None;
            if error.is_none() {
                match self.compile(&view, tasks) {
                    Ok(done) => compiled = Some(done),
                    Err(failure) => error = Some(failure.to_string()),
                }
            }
            loaded.push(Loaded {
                view,
                source,
                error,
                compiled,
            });
        }
        loaded
    }

    /// The members of `compiled` on the corpus of `board` that has `stamp` ([`Corpus::stamp`]). A view already worked
    /// out on that stamp is not worked out again, and `corpus` is not asked for. When it is, the copy it gives is the
    /// one evaluated and the one that names the stamp the answer is kept for.
    pub fn evaluate(
        &self,
        board: &str,
        compiled: &CompiledView,
        stamp: u64,
        corpus: impl FnOnce() -> Corpus,
    ) -> Result<Arc<ViewResult>, ViewFailure> {
        let key = serde_json::to_string(compiled.view()).expect("a view is JSON");
        if let Some(found) = self.kept(board, stamp, &key) {
            return found;
        }
        let corpus = corpus();
        let result = evaluate_view(compiled, &corpus, &Budgets::default()).map(Arc::new);
        self.evaluations.fetch_add(1, Ordering::SeqCst);
        self.keep(board, corpus.stamp(), key, result.clone());
        result
    }

    fn kept(
        &self,
        board: &str,
        stamp: u64,
        key: &str,
    ) -> Option<Result<Arc<ViewResult>, ViewFailure>> {
        let slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        let slot = slots.get(board).filter(|slot| slot.stamp == stamp)?;
        slot.results.get(key).cloned()
    }

    fn keep(
        &self,
        board: &str,
        stamp: u64,
        key: String,
        result: Result<Arc<ViewResult>, ViewFailure>,
    ) {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        let slot = slots.entry(board.to_string()).or_insert_with(|| Slot {
            stamp,
            results: HashMap::new(),
        });
        if slot.stamp != stamp || slot.results.len() >= KEPT {
            slot.stamp = stamp;
            slot.results.clear();
        }
        slot.results.insert(key, result);
    }
}

fn shadowed(name: &str, first: &ViewSource) -> String {
    let place = match first {
        ViewSource::Personal {} => "personal".to_string(),
        ViewSource::Repo {} => "repo".to_string(),
        ViewSource::Pack { pack } => format!("`{pack}` pack"),
    };
    format!(
        "`{name}` is also a {place} view, and that one is used: personal views come first, then the repo's"
    )
}

// ---- writing ----------------------------------------------------------------------------------------------------

/// Why a view was not saved.
#[derive(Debug, PartialEq, Eq)]
pub enum SaveError {
    /// The view's name cannot be a file name.
    Invalid(String),
    /// A view of that name is there, and the call did not say to replace it.
    Exists(String),
    /// A folder or the file on the way is a link or not the kind it must be: nothing is written through it.
    Blocked(String),
    /// The folder or the file could not be written.
    Failed(String),
}

/// Writes `view` as `<folder>/<name>.yaml`, atomically, with `mode`, and returns the path. The name is a safe token
/// (0.5), so it cannot leave `folder`. An existing file is replaced only with `overwrite`. Nothing is committed.
/// Each folder from `base` (not itself checked) down to `folder` that exists must be a real folder, and the file, if
/// there, a regular one: a link would make the write land outside `base`, and a repository can commit one.
pub fn save_view(
    view: &View,
    base: &Path,
    folder: &Path,
    overwrite: bool,
    mode: u32,
) -> Result<PathBuf, SaveError> {
    if !is_safe_token(&view.name) {
        return Err(SaveError::Invalid(format!(
            "the view name {:?} cannot be a file name: use 1 to 64 letters, digits, `.`, `_` or `-`, \
             not starting with `-` or `.`",
            view.name
        )));
    }
    let text = to_yaml(view, "")
        .map_err(|error| SaveError::Failed(format!("the view cannot be written: {error}")))?;
    let file = format!("{}.yaml", view.name);
    let path = folder.join(&file);
    check_not_links(base, folder, &path)?;
    if !overwrite && fs::symlink_metadata(&path).is_ok() {
        return Err(SaveError::Exists(format!(
            "{} is already there: save with overwrite to replace it",
            path.display()
        )));
    }
    fs::create_dir_all(folder)
        .map_err(|error| SaveError::Failed(format!("{}: {error}", folder.display())))?;
    write_atomically(folder, &file, text.as_bytes(), mode, |file| file)
        .map_err(|error| SaveError::Failed(error.to_string()))?;
    Ok(path)
}

/// Refuses a folder between `base` and `folder`, or the file `path`, that is there and is a link or the wrong kind.
fn check_not_links(base: &Path, folder: &Path, path: &Path) -> Result<(), SaveError> {
    let below = folder.strip_prefix(base).map_err(|_| {
        SaveError::Blocked(format!(
            "{} is not inside {}",
            folder.display(),
            base.display()
        ))
    })?;
    let mut here = base.to_path_buf();
    for part in below.components() {
        here.push(part);
        match fs::symlink_metadata(&here) {
            Ok(found) if found.is_dir() => {}
            Ok(found) => {
                let what = if found.file_type().is_symlink() {
                    "a link"
                } else {
                    "not a folder"
                };
                return Err(SaveError::Blocked(format!(
                    "{} is {what}, so no view is written through it: make it a real folder inside the repository",
                    here.display()
                )));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(SaveError::Failed(format!("{}: {error}", here.display()))),
        }
    }
    match fs::symlink_metadata(path) {
        Ok(found) if !found.is_file() => Err(SaveError::Blocked(format!(
            "{} is not a regular file (a link, for one), so it is not replaced",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(SaveError::Failed(format!("{}: {error}", path.display()))),
    }
}
