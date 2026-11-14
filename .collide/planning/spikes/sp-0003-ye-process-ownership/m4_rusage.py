"""Measurement 4: which exit-time resource totals survive.

  python3 m4_rusage.py

E1 direct child: wait4 rusage vs the child's own getrusage.
E2 child that waits for a heavy grandchild: does the child's wait4 include it?
E3 child that double-forks a heavy orphan: is the orphan counted anywhere?
E4 reattach: the parent dies, a new owner tries wait4, kqueue NOTE_EXITSTATUS,
   and proc_pid_rusage sampling on the (non-child) process.
E5 shim: exit.json written by fixtures/shim.py after the owner died.
"""
import ctypes
import json
import os
import select
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "lib"))
import s3  # noqa: E402

SPAWNER = os.path.join(s3.FIXTURES, "spawner.py")
AGENT = os.path.join(s3.FIXTURES, "fake_agent.py")
NOTE_EXITSTATUS = 0x04000000

libproc = ctypes.CDLL("/usr/lib/libproc.dylib")


class RusageInfoV4(ctypes.Structure):
    _fields_ = [("ri_uuid", ctypes.c_uint8 * 16)] + [(n, ctypes.c_uint64) for n in (
        "ri_user_time", "ri_system_time", "ri_pkg_idle_wkups", "ri_interrupt_wkups", "ri_pageins",
        "ri_wired_size", "ri_resident_size", "ri_phys_footprint", "ri_proc_start_abstime",
        "ri_proc_exit_abstime", "ri_child_user_time", "ri_child_system_time", "ri_child_pkg_idle_wkups",
        "ri_child_interrupt_wkups", "ri_child_pageins", "ri_child_elapsed_abstime", "ri_diskio_bytesread",
        "ri_diskio_byteswritten", "ri_cpu_time_qos_default", "ri_cpu_time_qos_maintenance",
        "ri_cpu_time_qos_background", "ri_cpu_time_qos_utility", "ri_cpu_time_qos_legacy",
        "ri_cpu_time_qos_user_initiated", "ri_cpu_time_qos_user_interactive", "ri_billed_system_time",
        "ri_serviced_system_time", "ri_logical_writes", "ri_lifetime_max_phys_footprint", "ri_instructions",
        "ri_cycles", "ri_billed_energy", "ri_serviced_energy", "ri_interval_max_phys_footprint",
        "ri_runnable_time")]


class Timebase(ctypes.Structure):
    _fields_ = [("numer", ctypes.c_uint32), ("denom", ctypes.c_uint32)]


_tb = Timebase()
ctypes.CDLL(None).mach_timebase_info(ctypes.byref(_tb))


def pid_rusage(pid):
    ri = RusageInfoV4()
    if libproc.proc_pid_rusage(pid, 4, ctypes.byref(ri)) != 0:
        return None
    ticks = ri.ri_user_time + ri.ri_system_time
    return {"cpu_s_if_ns": round(ticks / 1e9, 3),
            "cpu_s_if_mach_ticks": round(ticks * _tb.numer / _tb.denom / 1e9, 3),
            "lifetime_max_footprint_mb": round(ri.ri_lifetime_max_phys_footprint / 1048576, 1)}


def truth(path):
    for _ in range(50):
        v = s3.read_json(path)
        if v:
            return v
        time.sleep(0.1)
    return None


def e1_e2_e3(sd):
    rows = []
    # E1
    t = os.path.join(sd, "e1.truth")
    p = subprocess.Popen([s3.PY, AGENT, "--seconds", "0.3", "--burn", "1.0", "--alloc-mb", "200"],
                         stdout=open(os.path.join(sd, "e1.log"), "w"))
    _, status, ru = os.wait4(p.pid, 0)
    p.returncode = 0
    own = open(os.path.join(sd, "e1.log")).read().strip().splitlines()[-1]
    rows.append(("E1 direct child", s3.rusage_dict(ru), own))
    # E2, E3
    for mode in ("waited", "orphan"):
        t = os.path.join(sd, "%s.truth" % mode)
        p = subprocess.Popen([s3.PY, SPAWNER, "--mode", mode, "--burn", "1.0", "--alloc-mb", "300", "--truth", t])
        _, status, ru = os.wait4(p.pid, 0)
        p.returncode = 0
        rows.append(("E2 child waited for grandchild" if mode == "waited" else "E3 child left an orphan",
                     s3.rusage_dict(ru), "grandchild's own: %s" % truth(t)))
    return rows


def e4(sd):
    """The parent exits; a new 'owner' (this process, not the parent) watches the child."""
    log = os.path.join(sd, "e4.log")
    pidfile = os.path.join(sd, "e4.pid")
    # A short-lived parent starts a session-leader child, records its pid, and exits.
    code = ("import os,subprocess,sys\n"
            "p=subprocess.Popen([%r,%r,'--seconds','2','--burn','1.0','--alloc-mb','150','--exit','7'],"
            "stdout=open(%r,'w'),start_new_session=True)\n"
            "open(%r,'w').write(str(p.pid))\n" % (s3.PY, AGENT, log, pidfile))
    subprocess.run([s3.PY, "-c", code], check=True)
    pid = int(open(pidfile).read())
    out = {"pid": pid, "ppid_now": s3.ps_table().get(pid, {}).get("ppid")}
    try:
        os.wait4(pid, os.WNOHANG)
        out["wait4"] = "worked"
    except ChildProcessError as e:
        out["wait4"] = "ChildProcessError: " + e.strerror
    kq = select.kqueue()
    ev = select.kevent(pid, filter=select.KQ_FILTER_PROC, flags=select.KQ_EV_ADD | select.KQ_EV_ONESHOT,
                       fflags=select.KQ_NOTE_EXIT | NOTE_EXITSTATUS)
    samples = []
    try:
        kq.control([ev], 0, 0)
        out["kqueue_register"] = "ok"
    except OSError as e:
        out["kqueue_register"] = "error %s" % e
    # Sample the child with proc_pid_rusage until kqueue reports its exit.
    deadline = time.time() + 20
    exit_event = None
    while time.time() < deadline:
        s = pid_rusage(pid)
        if s:
            samples.append(s)
        got = kq.control(None, 1, 0.1)
        if got:
            exit_event = got[0]
            break
    out["kqueue_exit_seen"] = exit_event is not None
    if exit_event is not None:
        out["kqueue_fflags_has_exitstatus"] = bool(exit_event.fflags & NOTE_EXITSTATUS)
        out["kqueue_data"] = exit_event.data
        out["kqueue_decoded"] = s3.decode_status(exit_event.data)
    out["proc_pid_rusage_samples"] = len(samples)
    out["proc_pid_rusage_last"] = samples[-1] if samples else None
    out["proc_pid_rusage_after_exit"] = pid_rusage(pid)
    time.sleep(0.2)
    out["truth_child_self_report"] = open(log).read().strip().splitlines()[-1]
    return out


def main():
    run_id = s3.new_run_id("m4")
    sd = s3.run_dir(run_id)
    s3.say("timebase %d/%d" % (_tb.numer, _tb.denom))
    for name, got, own in e1_e2_e3(sd):
        s3.say("%-32s wait4: %s | %s" % (name, json.dumps(got), own))
    r = e4(sd)
    s3.say("E4 reattach (not the parent): " + json.dumps(r))
    shim_runs = s3.read_json(os.path.join(s3.WORK, "m1_m2_owner_ab.json"))
    if shim_runs:
        for run in shim_runs:
            if run["mode"] == "shim":
                for ph in run["phases"]:
                    if ph["phase"] == "p2" and run["option"] == "a":
                        s3.say("E5 shim, option a, phase ended after an owner SIGKILL+restart: %s"
                               % json.dumps(ph["exit"]))
    else:
        s3.say("E5: run m1_m2_owner_ab.py first for the shim case")


if __name__ == "__main__":
    main()
