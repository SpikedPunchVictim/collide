//! The free-space floor (0.2, Durability; R5). Free space is checked **before a workspace is made and before each
//! phase**. Under the floor nothing is started: the run goes to `paused: disk` with a Queue item. A phase that ends
//! with no `result` on a full volume fails with cause `disk`, not `lost` ([`DiskGuard::volume_full`]
//! feeds [`collide_core::transition::PhaseEnding::volume_full`]).
//!
//! Free space is read through [`FreeSpace`], so a test sets it ([`SystemFreeSpace`] is the real one, `statvfs`).
//!
//! ## The floor
//!
//! The plan gives no number, so [`DEFAULT_FLOOR_BYTES`] is a choice: 5 GiB of space available to an ordinary user
//! on the volume that holds the path. It is an absolute size and not a fraction of the volume, because the floor
//! stands for what a workspace and a phase need to write, and that does not grow with the disk. A provisioned
//! worktree of the reference pack measured 4.1 GB (R3, 0.13), so 5 GiB is enough to provision one and still leave
//! room to write. The owner may change it; it is one named constant and one field ([`Floor`]).
//!
//! ## What this does not do
//!
//! It decides and describes. It does not pause a run or write a Queue item: nothing drives runs until the daemon
//! (ts-0056-ag), which turns a [`DiskPause`] into the run's state with [`DiskPause::run_state`] and into a Queue item
//! with [`DiskPause::queue_item`].

use collide_core::owner::AnswerKind;
use collide_core::state::{PauseCause, RunState};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

/// The floor, in bytes of free space (see the module notes).
pub const DEFAULT_FLOOR_BYTES: u64 = 5 * GIB;

/// The least free space a workspace or a phase may start with. A volume with exactly this much is over the floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Floor {
    pub min_free_bytes: u64,
}

impl Default for Floor {
    fn default() -> Floor {
        Floor {
            min_free_bytes: DEFAULT_FLOOR_BYTES,
        }
    }
}

/// Where free space is read from. The daemon and the workspace take it as a value (0.15, Dependency injection).
pub trait FreeSpace {
    /// Bytes available to an ordinary user on the volume that holds `path`. `path` need not exist yet.
    fn free_bytes(&self, path: &Path) -> io::Result<u64>;
}

/// The real reader: `statvfs` on `path`, or on its nearest ancestor that exists, because a workspace's folder is
/// read before it is made. It counts `f_bavail`, the space an ordinary user can use, and not `f_bfree`, which
/// includes the part the system keeps back.
pub struct SystemFreeSpace;

impl FreeSpace for SystemFreeSpace {
    fn free_bytes(&self, path: &Path) -> io::Result<u64> {
        use std::os::unix::ffi::OsStrExt;
        let existing = path
            .ancestors()
            .find(|candidate| candidate.exists())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("no part of {} exists", path.display()),
                )
            })?;
        let c_path = std::ffi::CString::new(existing.as_os_str().as_bytes()).map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "the path holds a NUL byte")
        })?;
        let mut stats = std::mem::MaybeUninit::<libc::statvfs>::zeroed();
        // SAFETY: `c_path` is a NUL-terminated string that outlives the call, and `stats` is a valid, writable
        // `statvfs` that the call fills in when it returns 0.
        let rc = unsafe { libc::statvfs(c_path.as_ptr(), stats.as_mut_ptr()) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: `statvfs` returned 0, so it filled in the struct.
        let stats = unsafe { stats.assume_init() };
        // The fields are `u32` on some platforms and `u64` on others, so the conversion is not always a no-op.
        #[allow(clippy::useless_conversion)]
        let free = u64::from(stats.f_bavail).saturating_mul(u64::from(stats.f_frsize));
        Ok(free)
    }
}

/// What was about to start when the floor stopped it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Before {
    Workspace,
    Phase,
}

/// Free space was under the floor, so nothing was started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskPause {
    pub before: Before,
    /// The path whose volume was read.
    pub path: PathBuf,
    pub free_bytes: u64,
    pub floor_bytes: u64,
}

/// What a Queue item for a disk pause says. `core`'s `QueueKind` has no kind for it yet (see the item's Notes), so
/// this is the content and the daemon chooses the item's kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskQueueItem {
    pub title: String,
    pub detail: String,
    /// What a person can do: free space and resume, or stop the run.
    pub answers: Vec<AnswerKind>,
}

impl DiskPause {
    pub fn cause(&self) -> PauseCause {
        PauseCause::Disk
    }

    /// The state the run goes to. A phase that has not started stays `pending`, because core does not let a phase no
    /// shim owns pause, so only the run carries the pause.
    pub fn run_state(&self) -> RunState {
        RunState::Paused {
            cause: self.cause(),
        }
    }

    pub fn queue_item(&self) -> DiskQueueItem {
        let wanted = match self.before {
            Before::Workspace => "make a workspace",
            Before::Phase => "start a phase",
        };
        DiskQueueItem {
            title: format!("Paused: not enough disk space to {wanted}"),
            detail: format!(
                "{} free on the volume that holds {}; Collide starts nothing below {}. Free some space, then resume.",
                format_bytes(self.free_bytes),
                self.path.display(),
                format_bytes(self.floor_bytes)
            ),
            answers: vec![AnswerKind::Resume, AnswerKind::Stop],
        }
    }
}

/// `3.0 MiB`, `5.0 GiB`: one decimal, binary units.
fn format_bytes(bytes: u64) -> String {
    let (unit, name) = if bytes >= GIB {
        (GIB, "GiB")
    } else if bytes >= MIB {
        (MIB, "MiB")
    } else if bytes >= KIB {
        (KIB, "KiB")
    } else {
        return format!("{bytes} B");
    };
    // Tenths in whole numbers, so the text does not depend on float rounding.
    let tenths = bytes.saturating_mul(10) / unit;
    format!("{}.{} {name}", tenths / 10, tenths % 10)
}

#[derive(Debug)]
pub enum DiskError {
    Paused(DiskPause),
    /// The volume could not be read. This is not a pause: with no reading nothing says space is short, and a guess
    /// either way would hide the fault.
    Unreadable {
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for DiskError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiskError::Paused(pause) => write!(f, "{}", pause.queue_item().title),
            DiskError::Unreadable { path, source } => write!(
                f,
                "could not read the free space of the volume that holds {}: {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for DiskError {}

/// A reader and a floor, asked before a workspace is made and before each phase.
pub struct DiskGuard<'a> {
    free: &'a dyn FreeSpace,
    floor: Floor,
}

impl<'a> DiskGuard<'a> {
    pub fn new(free: &'a dyn FreeSpace, floor: Floor) -> DiskGuard<'a> {
        DiskGuard { free, floor }
    }

    /// Ok when the volume of `path` has at least the floor free; otherwise the pause that says what was about to
    /// start.
    pub fn check(&self, path: &Path, before: Before) -> Result<(), DiskError> {
        let free_bytes = self.read(path)?;
        if free_bytes >= self.floor.min_free_bytes {
            return Ok(());
        }
        Err(DiskError::Paused(DiskPause {
            before,
            path: path.to_path_buf(),
            free_bytes,
            floor_bytes: self.floor.min_free_bytes,
        }))
    }

    /// Whether the volume of `path` is under the floor now: `PhaseEnding::volume_full` for a phase that ended with
    /// no `result` (0.2).
    pub fn volume_full(&self, path: &Path) -> Result<bool, DiskError> {
        Ok(self.read(path)? < self.floor.min_free_bytes)
    }

    fn read(&self, path: &Path) -> Result<u64, DiskError> {
        self.free
            .free_bytes(path)
            .map_err(|source| DiskError::Unreadable {
                path: path.to_path_buf(),
                source,
            })
    }
}
