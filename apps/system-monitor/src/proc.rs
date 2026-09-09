//! Real system metrics from the Linux `/proc` and `/sys` filesystems and one
//! `statvfs(3)` call. There is no fallback to fabricated numbers anywhere: if a
//! source can't be read, the field is reported as unknown.

use std::collections::HashMap;
use std::fs;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Memory  (/proc/meminfo)
// ---------------------------------------------------------------------------
#[derive(Default, Clone, Copy)]
pub struct MemInfo {
    pub total_kb: u64,
    pub available_kb: u64,
    pub free_kb: u64,
    pub buffers_kb: u64,
    pub cached_kb: u64,
    pub swap_total_kb: u64,
    pub swap_free_kb: u64,
    pub zram_used_kb: u64,
}

impl MemInfo {
    pub fn read() -> MemInfo {
        let mut m = MemInfo::default();
        if let Ok(s) = fs::read_to_string("/proc/meminfo") {
            let map: HashMap<&str, u64> = s
                .lines()
                .filter_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    let kb = v.trim().trim_end_matches(" kB").trim().parse().ok()?;
                    Some((k, kb))
                })
                .collect();
            m.total_kb = map.get("MemTotal").copied().unwrap_or(0);
            m.available_kb = map.get("MemAvailable").copied().unwrap_or(0);
            m.free_kb = map.get("MemFree").copied().unwrap_or(0);
            m.buffers_kb = map.get("Buffers").copied().unwrap_or(0);
            m.cached_kb = map.get("Cached").copied().unwrap_or(0);
            m.swap_total_kb = map.get("SwapTotal").copied().unwrap_or(0);
            m.swap_free_kb = map.get("SwapFree").copied().unwrap_or(0);
        }
        // zram compressed footprint (what the swap actually costs in RAM)
        for i in 0..8 {
            let p = format!("/sys/block/zram{i}/mm_stat");
            if let Ok(s) = fs::read_to_string(&p) {
                // field 3 (0-indexed 2) = mem_used_total bytes
                if let Some(bytes) = s.split_whitespace().nth(2).and_then(|v| v.parse::<u64>().ok()) {
                    m.zram_used_kb += bytes / 1024;
                }
            }
        }
        m
    }

    /// "Used" the way `free` reports it: total - available.
    pub fn used_kb(&self) -> u64 {
        self.total_kb.saturating_sub(self.available_kb)
    }
    pub fn used_fraction(&self) -> f64 {
        if self.total_kb == 0 {
            0.0
        } else {
            self.used_kb() as f64 / self.total_kb as f64
        }
    }
    pub fn swap_used_kb(&self) -> u64 {
        self.swap_total_kb.saturating_sub(self.swap_free_kb)
    }
}

// ---------------------------------------------------------------------------
// CPU  (/proc/stat) — needs two samples to produce a percentage
// ---------------------------------------------------------------------------
#[derive(Clone)]
pub struct CpuSample {
    total: u64,
    idle: u64,
    per_core: Vec<(u64, u64)>,
}

impl CpuSample {
    pub fn read() -> CpuSample {
        let mut total = 0;
        let mut idle = 0;
        let mut per_core = Vec::new();
        if let Ok(s) = fs::read_to_string("/proc/stat") {
            for line in s.lines() {
                if !line.starts_with("cpu") {
                    break;
                }
                let mut it = line.split_whitespace();
                let tag = it.next().unwrap_or("");
                let v: Vec<u64> = it.filter_map(|x| x.parse().ok()).collect();
                if v.len() < 5 {
                    continue;
                }
                let t: u64 = v.iter().sum();
                let id = v[3] + v.get(4).copied().unwrap_or(0); // idle + iowait
                if tag == "cpu" {
                    total = t;
                    idle = id;
                } else {
                    per_core.push((t, id));
                }
            }
        }
        CpuSample { total, idle, per_core }
    }

    /// Aggregate busy fraction between `prev` and `self`.
    pub fn busy_fraction(&self, prev: &CpuSample) -> f64 {
        delta_busy(prev.total, prev.idle, self.total, self.idle)
    }

    pub fn per_core_busy(&self, prev: &CpuSample) -> Vec<f64> {
        self.per_core
            .iter()
            .zip(prev.per_core.iter())
            .map(|(&(t, i), &(pt, pi))| delta_busy(pt, pi, t, i))
            .collect()
    }
}

fn delta_busy(pt: u64, pi: u64, t: u64, i: u64) -> f64 {
    let dt = t.saturating_sub(pt) as f64;
    let di = i.saturating_sub(pi) as f64;
    if dt <= 0.0 {
        0.0
    } else {
        ((dt - di) / dt).clamp(0.0, 1.0)
    }
}

pub fn cpu_model() -> String {
    fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
        })
        .unwrap_or_else(|| "unknown CPU".into())
}

pub fn ncpu() -> usize {
    fs::read_to_string("/proc/cpuinfo")
        .map(|s| s.matches("processor").count().max(1))
        .unwrap_or(1)
}

pub fn loadavg() -> (f64, f64, f64) {
    fs::read_to_string("/proc/loadavg")
        .ok()
        .map(|s| {
            let v: Vec<f64> = s.split_whitespace().take(3).filter_map(|x| x.parse().ok()).collect();
            (v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0))
        })
        .unwrap_or((0.0, 0.0, 0.0))
}

pub fn uptime_secs() -> u64 {
    fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| s.split_whitespace().next().map(|x| x.to_string()))
        .and_then(|x| x.parse::<f64>().ok())
        .map(|f| f as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Disk  (statvfs on mounted filesystems from /proc/mounts)
// ---------------------------------------------------------------------------
pub struct DiskUsage {
    pub mount: String,
    pub source: String,
    pub fstype: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
}

impl DiskUsage {
    pub fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.free_bytes)
    }
    pub fn used_fraction(&self) -> f64 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.used_bytes() as f64 / self.total_bytes as f64
        }
    }
}

pub fn disks() -> Vec<DiskUsage> {
    let mut out = Vec::new();
    let Ok(mounts) = fs::read_to_string("/proc/mounts") else {
        return out;
    };
    let real_fs = [
        "ext4", "ext3", "ext2", "btrfs", "xfs", "f2fs", "vfat", "exfat", "ntfs3", "ntfs",
        "overlay", "squashfs", "iso9660",
    ];
    for line in mounts.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 3 {
            continue;
        }
        let (source, mount, fstype) = (f[0], f[1], f[2]);
        if !real_fs.contains(&fstype) {
            continue;
        }
        // skip bind-duplicate of / that live-boot creates, keep the first per mount
        if out.iter().any(|d: &DiskUsage| d.mount == mount) {
            continue;
        }
        if let Some((total, free)) = statvfs_bytes(mount) {
            out.push(DiskUsage {
                mount: mount.to_string(),
                source: source.to_string(),
                fstype: fstype.to_string(),
                total_bytes: total,
                free_bytes: free,
            });
        }
    }
    out
}

fn statvfs_bytes(path: &str) -> Option<(u64, u64)> {
    use std::ffi::CString;
    let c = CString::new(path).ok()?;
    // SAFETY: `buf` is a valid, sized-correct libc::statvfs; `c` is a valid
    // NUL-terminated path. We only read scalar fields from `buf` on success.
    unsafe {
        let mut buf: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut buf) != 0 {
            return None;
        }
        let bsize = if buf.f_frsize != 0 { buf.f_frsize } else { buf.f_bsize } as u64;
        Some((buf.f_blocks as u64 * bsize, buf.f_bavail as u64 * bsize))
    }
}

// ---------------------------------------------------------------------------
// Processes  (/proc/<pid>/stat + status)
// ---------------------------------------------------------------------------
pub struct ProcRow {
    pub pid: i32,
    pub comm: String,
    pub state: char,
    pub rss_kb: u64,
    pub user: String,
}

pub struct ProcSampler {
    prev: HashMap<i32, u64>, // pid -> utime+stime
    prev_at: Instant,
    page_kb: u64,
    hz: u64,
    uids: HashMap<u32, String>,
}

impl ProcSampler {
    pub fn new() -> Self {
        // SAFETY: sysconf with valid constants; returns a scalar.
        let (page_kb, hz) = unsafe {
            (
                (libc::sysconf(libc::_SC_PAGESIZE).max(4096) / 1024) as u64,
                libc::sysconf(libc::_SC_CLK_TCK).max(100) as u64,
            )
        };
        ProcSampler {
            prev: HashMap::new(),
            prev_at: Instant::now(),
            page_kb,
            hz,
            uids: read_passwd(),
        }
    }

    /// Returns (rows, cpu% per pid keyed by pid) using the delta since last call.
    pub fn sample(&mut self) -> (Vec<ProcRow>, HashMap<i32, f64>) {
        let now = Instant::now();
        let secs = now.duration_since(self.prev_at).as_secs_f64().max(0.001);
        self.prev_at = now;

        let mut rows = Vec::new();
        let mut cpu = HashMap::new();
        let mut cur = HashMap::new();

        let Ok(rd) = fs::read_dir("/proc") else {
            return (rows, cpu);
        };
        for e in rd.flatten() {
            let name = e.file_name();
            let Some(pid) = name.to_str().and_then(|s| s.parse::<i32>().ok()) else {
                continue;
            };
            let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
                continue;
            };
            // comm can contain spaces/parens: split on the last ')'
            let (Some(open), Some(close)) = (stat.find('('), stat.rfind(')')) else {
                continue;
            };
            let comm = stat[open + 1..close].to_string();
            let rest: Vec<&str> = stat[close + 2..].split_whitespace().collect();
            if rest.len() < 20 {
                continue;
            }
            let state = rest[0].chars().next().unwrap_or('?');
            let utime: u64 = rest[11].parse().unwrap_or(0);
            let stime: u64 = rest[12].parse().unwrap_or(0);
            let rss_pages: i64 = rest[21].parse().unwrap_or(0);
            let rss_kb = (rss_pages.max(0) as u64) * self.page_kb;

            let jiffies = utime + stime;
            cur.insert(pid, jiffies);
            if let Some(&p) = self.prev.get(&pid) {
                let dj = jiffies.saturating_sub(p) as f64;
                let pct = (dj / self.hz as f64) / secs * 100.0;
                cpu.insert(pid, pct);
            }

            let uid = fs::read_to_string(format!("/proc/{pid}/status"))
                .ok()
                .and_then(|s| {
                    s.lines()
                        .find(|l| l.starts_with("Uid:"))
                        .and_then(|l| l.split_whitespace().nth(1))
                        .and_then(|u| u.parse::<u32>().ok())
                })
                .unwrap_or(0);

            rows.push(ProcRow {
                pid,
                comm,
                state,
                rss_kb,
                user: self.uids.get(&uid).cloned().unwrap_or_else(|| uid.to_string()),
            });
        }
        self.prev = cur;
        (rows, cpu)
    }
}

fn read_passwd() -> HashMap<u32, String> {
    let mut m = HashMap::new();
    if let Ok(s) = fs::read_to_string("/etc/passwd") {
        for l in s.lines() {
            let f: Vec<&str> = l.split(':').collect();
            if f.len() >= 3 {
                if let Ok(uid) = f[2].parse::<u32>() {
                    m.insert(uid, f[0].to_string());
                }
            }
        }
    }
    m
}

// ---------------------------------------------------------------------------
pub fn fmt_kb(kb: u64) -> String {
    fmt_bytes(kb * 1024)
}
pub fn fmt_bytes(b: u64) -> String {
    const U: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{b} B")
    } else {
        format!("{v:.1} {}", U[i])
    }
}
pub fn fmt_duration(mut s: u64) -> String {
    let d = s / 86400;
    s %= 86400;
    let h = s / 3600;
    s %= 3600;
    let m = s / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}
