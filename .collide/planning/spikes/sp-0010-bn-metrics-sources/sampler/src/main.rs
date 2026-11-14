// S8 spike sampler: process CPU, memory and disk I/O over time through libproc, std only.
//
//   s8-sampler --interval-ms 1000 (--root PID | --all) --out FILE [--duration-s N]
//
// Writes one JSON line per tick:
//   {"t":unix_ns,"m":mach_ns,"work_us":N,"p":[[pid,ppid,start_ticks,"comm",user_ns,sys_ns,rss,footprint,rd,wr,wkups,instr],..]}
// On exit (duration, root gone, SIGINT/SIGTERM) prints one SUMMARY json line on stdout with the sampler's own CPU.
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::os::raw::{c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[repr(C)]
struct ProcBsdInfo {
    flags: u32, status: u32, xstatus: u32, pid: u32, ppid: u32, uid: u32, gid: u32, ruid: u32, rgid: u32,
    svuid: u32, svgid: u32, rfu_1: u32,
    comm: [u8; 16], name: [u8; 32],
    nfiles: u32, pgid: u32, pjobc: u32, e_tdev: u32, e_tpgid: u32, nice: i32,
    start_tvsec: u64, start_tvusec: u64,
}

#[repr(C)]
#[derive(Default)]
struct RusageV4 {
    uuid: [u8; 16],
    user_time: u64, system_time: u64, pkg_idle_wkups: u64, interrupt_wkups: u64, pageins: u64, wired_size: u64,
    resident_size: u64, phys_footprint: u64, proc_start_abstime: u64, proc_exit_abstime: u64,
    child_user_time: u64, child_system_time: u64, child_pkg_idle_wkups: u64, child_interrupt_wkups: u64,
    child_pageins: u64, child_elapsed_abstime: u64, diskio_bytesread: u64, diskio_byteswritten: u64,
    qos: [u64; 7], billed_system_time: u64, serviced_system_time: u64, logical_writes: u64,
    lifetime_max_phys_footprint: u64, instructions: u64, cycles: u64, billed_energy: u64, serviced_energy: u64,
    interval_max_phys_footprint: u64, runnable_time: u64,
}

#[repr(C)]
struct Timebase { numer: u32, denom: u32 }

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Timeval { tv_sec: i64, tv_usec: i32 }

#[repr(C)]
#[derive(Default)]
struct Rusage { utime: Timeval, stime: Timeval, rest: [i64; 14] }

extern "C" {
    fn proc_listpids(kind: u32, info: u32, buf: *mut c_void, size: c_int) -> c_int;
    fn proc_pidinfo(pid: c_int, flavor: c_int, arg: u64, buf: *mut c_void, size: c_int) -> c_int;
    fn proc_pid_rusage(pid: c_int, flavor: c_int, buf: *mut c_void) -> c_int;
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut Timebase) -> c_int;
    fn getrusage(who: c_int, usage: *mut Rusage) -> c_int;
    fn signal(sig: c_int, handler: usize) -> usize;
    fn kill(pid: c_int, sig: c_int) -> c_int;
}

static STOP: AtomicBool = AtomicBool::new(false);
extern "C" fn on_signal(_: c_int) { STOP.store(true, Ordering::SeqCst); }

fn list_pids(buf: &mut Vec<c_int>) -> usize {
    let need = unsafe { proc_listpids(1, 0, std::ptr::null_mut(), 0) };
    if need <= 0 { return 0; }
    let want = need as usize / 4 + 64;
    if buf.len() < want { buf.resize(want, 0); }
    let got = unsafe { proc_listpids(1, 0, buf.as_mut_ptr() as *mut c_void, (buf.len() * 4) as c_int) };
    if got <= 0 { 0 } else { got as usize / 4 }
}

fn bsd_info(pid: c_int) -> Option<ProcBsdInfo> {
    let mut info: ProcBsdInfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<ProcBsdInfo>() as c_int;
    let n = unsafe { proc_pidinfo(pid, 3, 0, &mut info as *mut _ as *mut c_void, size) };
    if n == size { Some(info) } else { None }
}

fn rusage_of(pid: c_int) -> Option<RusageV4> {
    let mut ri = RusageV4::default();
    let r = unsafe { proc_pid_rusage(pid, 4, &mut ri as *mut _ as *mut c_void) };
    if r == 0 { Some(ri) } else { None }
}

fn json_str(s: &[u8], out: &mut String) {
    out.push('"');
    for &b in s.iter().take_while(|&&b| b != 0) {
        match b {
            b'"' | b'\\' => { out.push('\\'); out.push(b as char); }
            0x20..=0x7e => out.push(b as char),
            _ => out.push('?'),
        }
    }
    out.push('"');
}

fn self_cpu_ns() -> u64 {
    let mut ru = Rusage::default();
    unsafe { getrusage(0, &mut ru) };
    let t = |v: Timeval| v.tv_sec as u64 * 1_000_000_000 + v.tv_usec as u64 * 1000;
    t(ru.utime) + t(ru.stime)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut interval_ms: u64 = 1000;
    let mut root: Option<c_int> = None;
    let mut out_path = String::from("samples.jsonl");
    let mut duration_s: f64 = 0.0;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--interval-ms" => { interval_ms = args[i + 1].parse().unwrap(); i += 1; }
            "--root" => { root = Some(args[i + 1].parse().unwrap()); i += 1; }
            "--all" => { root = None; }
            "--out" => { out_path = args[i + 1].clone(); i += 1; }
            "--duration-s" => { duration_s = args[i + 1].parse().unwrap(); i += 1; }
            other => { eprintln!("unknown argument {other}"); std::process::exit(2); }
        }
        i += 1;
    }
    unsafe {
        signal(2, on_signal as *const () as usize);
        signal(15, on_signal as *const () as usize);
    }
    let mut tb = Timebase { numer: 0, denom: 1 };
    unsafe { mach_timebase_info(&mut tb) };
    let to_ns = |t: u64| t * tb.numer as u64 / tb.denom as u64;

    let mut out = BufWriter::new(File::create(&out_path).expect("cannot create output"));
    let interval = Duration::from_millis(interval_ms);
    let started = Instant::now();
    let cpu0 = self_cpu_ns();
    let mut next = started;
    let mut ticks = 0u64;
    let mut work_us: Vec<u64> = Vec::new();
    let mut rows = 0u64;
    let mut bytes = 0u64;
    let mut pidbuf: Vec<c_int> = Vec::new();
    let mut line = String::with_capacity(64 * 1024);
    let (mut t_list, mut t_bsd, mut t_ru, mut t_write) = (0u64, 0u64, 0u64, 0u64);
    let mut ids_seen: HashSet<(c_int, u64)> = HashSet::new();

    while !STOP.load(Ordering::SeqCst) {
        let w0 = Instant::now();
        let n = list_pids(&mut pidbuf);
        let t1 = Instant::now();
        let mut parents: HashMap<c_int, c_int> = HashMap::with_capacity(n);
        let mut comm: HashMap<c_int, [u8; 16]> = HashMap::with_capacity(n);
        for &pid in &pidbuf[..n] {
            if pid <= 0 { continue; }
            if let Some(bi) = bsd_info(pid) {
                parents.insert(pid, bi.ppid as c_int);
                comm.insert(pid, bi.comm);
            }
        }
        let t2 = Instant::now();
        let wanted: Vec<c_int> = match root {
            None => parents.keys().cloned().collect(),
            Some(r) => {
                let mut kids: HashMap<c_int, Vec<c_int>> = HashMap::new();
                for (&p, &pp) in &parents { kids.entry(pp).or_default().push(p); }
                let mut seen = HashSet::new();
                let mut todo = vec![r];
                while let Some(p) = todo.pop() {
                    if !seen.insert(p) { continue; }
                    if let Some(k) = kids.get(&p) { todo.extend(k.iter().cloned()); }
                }
                seen.into_iter().filter(|p| parents.contains_key(p)).collect()
            }
        };
        let t_now_unix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let m_now = to_ns(unsafe { mach_absolute_time() });
        line.clear();
        line.push_str(&format!("{{\"t\":{},\"m\":{},\"p\":[", t_now_unix, m_now));
        let mut first = true;
        let mut tick_rows = 0;
        for pid in wanted {
            if let Some(ri) = rusage_of(pid) {
                if !first { line.push(','); }
                first = false;
                tick_rows += 1;
                ids_seen.insert((pid, ri.proc_start_abstime));
                line.push_str(&format!("[{},{},{},", pid, parents[&pid], ri.proc_start_abstime));
                json_str(&comm[&pid], &mut line);
                line.push_str(&format!(",{},{},{},{},{},{},{},{}]",
                    to_ns(ri.user_time), to_ns(ri.system_time), ri.resident_size, ri.phys_footprint,
                    ri.diskio_bytesread, ri.diskio_byteswritten, ri.interrupt_wkups, ri.instructions));
            }
        }
        let t3 = Instant::now();
        let work = w0.elapsed().as_micros() as u64;
        line.push_str(&format!("],\"work_us\":{}}}\n", work));
        out.write_all(line.as_bytes()).unwrap();
        out.flush().unwrap();
        let t4 = Instant::now();
        rows += tick_rows;
        bytes += line.len() as u64;
        ticks += 1;
        work_us.push(work);
        t_list += (t1 - w0).as_micros() as u64;
        t_bsd += (t2 - t1).as_micros() as u64;
        t_ru += (t3 - t2).as_micros() as u64;
        t_write += (t4 - t3).as_micros() as u64;

        if let Some(r) = root {
            if unsafe { kill(r, 0) } != 0 && rusage_of(r).is_none() { break; }
        }
        if duration_s > 0.0 && started.elapsed().as_secs_f64() >= duration_s { break; }
        next += interval;
        let now = Instant::now();
        if next > now { std::thread::sleep(next - now); } else { next = now; }
    }
    let wall = started.elapsed().as_secs_f64();
    let cpu = (self_cpu_ns() - cpu0) as f64 / 1e9;
    work_us.sort();
    let q = |p: f64| if work_us.is_empty() { 0 } else { work_us[((work_us.len() as f64 * p) as usize).min(work_us.len() - 1)] };
    println!("SUMMARY {{\"interval_ms\":{},\"wall_s\":{:.3},\"cpu_s\":{:.4},\"cpu_pct_of_core\":{:.4},\"ticks\":{},\"rows\":{},\"bytes\":{},\"distinct_procs\":{},\"work_us_p50\":{},\"work_us_p99\":{},\"work_us_max\":{},\"avg_us_list\":{},\"avg_us_bsdinfo\":{},\"avg_us_rusage\":{},\"avg_us_write\":{}}}",
        interval_ms, wall, cpu, 100.0 * cpu / wall, ticks, rows, bytes, ids_seen.len(), q(0.5), q(0.99), q(1.0),
        t_list / ticks.max(1), t_bsd / ticks.max(1), t_ru / ticks.max(1), t_write / ticks.max(1));
}
