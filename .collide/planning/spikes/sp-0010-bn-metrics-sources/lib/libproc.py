"""ctypes access to libproc and the Mach clocks. Python stdlib only (macOS arm64)."""
import ctypes
import ctypes.util
import time

_lib = ctypes.CDLL("/usr/lib/libSystem.B.dylib", use_errno=True)

PROC_ALL_PIDS = 1
PROC_PIDTBSDINFO = 3
RUSAGE_INFO_V4 = 4


class ProcBsdInfo(ctypes.Structure):
    _fields_ = [
        ("pbi_flags", ctypes.c_uint32), ("pbi_status", ctypes.c_uint32), ("pbi_xstatus", ctypes.c_uint32),
        ("pbi_pid", ctypes.c_uint32), ("pbi_ppid", ctypes.c_uint32), ("pbi_uid", ctypes.c_uint32),
        ("pbi_gid", ctypes.c_uint32), ("pbi_ruid", ctypes.c_uint32), ("pbi_rgid", ctypes.c_uint32),
        ("pbi_svuid", ctypes.c_uint32), ("pbi_svgid", ctypes.c_uint32), ("rfu_1", ctypes.c_uint32),
        ("pbi_comm", ctypes.c_char * 16), ("pbi_name", ctypes.c_char * 32),
        ("pbi_nfiles", ctypes.c_uint32), ("pbi_pgid", ctypes.c_uint32), ("pbi_pjobc", ctypes.c_uint32),
        ("e_tdev", ctypes.c_uint32), ("e_tpgid", ctypes.c_uint32), ("pbi_nice", ctypes.c_int32),
        ("pbi_start_tvsec", ctypes.c_uint64), ("pbi_start_tvusec", ctypes.c_uint64),
    ]


class RusageInfoV4(ctypes.Structure):
    _fields_ = [("ri_uuid", ctypes.c_uint8 * 16)] + [(n, ctypes.c_uint64) for n in (
        "ri_user_time", "ri_system_time", "ri_pkg_idle_wkups", "ri_interrupt_wkups", "ri_pageins",
        "ri_wired_size", "ri_resident_size", "ri_phys_footprint", "ri_proc_start_abstime",
        "ri_proc_exit_abstime", "ri_child_user_time", "ri_child_system_time", "ri_child_pkg_idle_wkups",
        "ri_child_interrupt_wkups", "ri_child_pageins", "ri_child_elapsed_abstime",
        "ri_diskio_bytesread", "ri_diskio_byteswritten",
        "ri_cpu_time_qos_default", "ri_cpu_time_qos_maintenance", "ri_cpu_time_qos_background",
        "ri_cpu_time_qos_utility", "ri_cpu_time_qos_legacy", "ri_cpu_time_qos_user_initiated",
        "ri_cpu_time_qos_user_interactive", "ri_billed_system_time", "ri_serviced_system_time",
        "ri_logical_writes", "ri_lifetime_max_phys_footprint", "ri_instructions", "ri_cycles",
        "ri_billed_energy", "ri_serviced_energy", "ri_interval_max_phys_footprint", "ri_runnable_time")]


assert ctypes.sizeof(ProcBsdInfo) == 136, ctypes.sizeof(ProcBsdInfo)
assert ctypes.sizeof(RusageInfoV4) == 296, ctypes.sizeof(RusageInfoV4)


class _Timebase(ctypes.Structure):
    _fields_ = [("numer", ctypes.c_uint32), ("denom", ctypes.c_uint32)]


_lib.mach_absolute_time.restype = ctypes.c_uint64
_lib.mach_continuous_time.restype = ctypes.c_uint64
_lib.proc_listpids.argtypes = [ctypes.c_uint32, ctypes.c_uint32, ctypes.c_void_p, ctypes.c_int]
_lib.proc_pidinfo.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_int]
_lib.proc_pid_rusage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_void_p]

_tb = _Timebase()
_lib.mach_timebase_info(ctypes.byref(_tb))
TIMEBASE = (_tb.numer, _tb.denom)  # 125/3 on Apple M-series: 1 mach tick = 41.67 ns


def mach_to_ns(ticks):
    return ticks * _tb.numer // _tb.denom


def mach_absolute_ns():
    """Uptime clock: stops while the machine sleeps."""
    return mach_to_ns(_lib.mach_absolute_time())


def mach_continuous_ns():
    """Continuous clock: keeps counting through sleep."""
    return mach_to_ns(_lib.mach_continuous_time())


def list_pids():
    need = _lib.proc_listpids(PROC_ALL_PIDS, 0, None, 0)
    if need <= 0:
        return []
    buf = (ctypes.c_int * (need // 4 + 64))()
    got = _lib.proc_listpids(PROC_ALL_PIDS, 0, buf, ctypes.sizeof(buf))
    return [p for p in buf[: got // 4] if p > 0]


def bsd_info(pid):
    """(ppid, start_usec, comm) or None when the process is gone or not readable."""
    info = ProcBsdInfo()
    n = _lib.proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, ctypes.byref(info), ctypes.sizeof(info))
    if n != ctypes.sizeof(info):
        return None
    return info.pbi_ppid, info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec, info.pbi_comm.decode("utf8", "replace")


def rusage(pid):
    """RusageInfoV4 or None. CPU fields are in mach ticks; use mach_to_ns."""
    ri = RusageInfoV4()
    if _lib.proc_pid_rusage(pid, RUSAGE_INFO_V4, ctypes.byref(ri)) != 0:
        return None
    return ri


def cpu_ns(ri):
    return mach_to_ns(ri.ri_user_time), mach_to_ns(ri.ri_system_time)


def descendants(root, parents):
    """parents: {pid: ppid}. Returns the set of pids under root, root included."""
    kids = {}
    for pid, ppid in parents.items():
        kids.setdefault(ppid, []).append(pid)
    out, todo = set(), [root]
    while todo:
        p = todo.pop()
        if p in out:
            continue
        out.add(p)
        todo.extend(kids.get(p, []))
    return out


if __name__ == "__main__":
    import os
    print("timebase", TIMEBASE, "tick ns", TIMEBASE[0] / TIMEBASE[1])
    print("pids", len(list_pids()), "self", os.getpid(), bsd_info(os.getpid()))
    ri = rusage(os.getpid())
    print("self cpu ns (user, sys)", cpu_ns(ri), "footprint", ri.ri_phys_footprint)
    a, c = mach_absolute_ns(), mach_continuous_ns()
    print("absolute ns", a, "continuous ns", c, "sleep since boot s", (c - a) / 1e9)
