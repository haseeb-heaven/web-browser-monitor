use std::collections::{BTreeMap, HashMap, HashSet};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::chart::format_bytes;
use crate::model::{
    classify_process, BrowserKind, BrowserSummary, ComponentRole, ComponentSummary, EventLog,
    HostStats, LogLevel, ProcessDetail, SortBy,
};

pub struct BrowserCollector {
    sys: System,
    refresh_kind: ProcessRefreshKind,
    prev_pids: HashMap<u32, (BrowserKind, ComponentRole, u64)>,
    seen_initial: bool,
    pub log: EventLog,
}

impl BrowserCollector {
    pub fn new() -> Self {
        let mut sys = System::new();
        let refresh_kind = ProcessRefreshKind::everything();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        sys.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind);

        Self {
            sys,
            refresh_kind,
            prev_pids: HashMap::new(),
            seen_initial: false,
            log: EventLog::new(250),
        }
    }

    pub fn collect(
        &mut self,
        sort_by: SortBy,
    ) -> (
        Vec<ProcessDetail>,
        Vec<BrowserSummary>,
        Vec<ComponentSummary>,
        HostStats,
    ) {
        self.sys.refresh_cpu_all();
        self.sys.refresh_memory();
        self.sys
            .refresh_processes_specifics(ProcessesToUpdate::All, true, self.refresh_kind);

        let mut processes = Vec::new();
        let mut browsers_with_ui: HashSet<BrowserKind> = HashSet::new();

        for (pid, proc_) in self.sys.processes() {
            let exe = proc_
                .exe()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let name = proc_.name().to_string_lossy().to_string();
            let cmd: Vec<String> = proc_
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().to_string())
                .collect();

            let Some((browser, role)) = classify_process(&exe, &name, &cmd) else {
                continue;
            };

            if role == ComponentRole::MainUI {
                browsers_with_ui.insert(browser);
            }

            let mut site = None;
            for i in 0..cmd.len() {
                if cmd[i] == "--site" && i + 1 < cmd.len() {
                    site = Some(cmd[i + 1].clone());
                    break;
                }
            }

            processes.push(ProcessDetail {
                pid: pid.as_u32(),
                ppid: proc_.parent().map(|p: Pid| p.as_u32()),
                browser,
                role,
                is_hidden: role == ComponentRole::HiddenDaemon,
                name,
                exe,
                cmd,
                site,
                memory_bytes: proc_.memory(),
                virtual_memory_bytes: proc_.virtual_memory(),
                cpu_usage: proc_.cpu_usage(),
                run_time_secs: proc_.run_time(),
            });
        }

        // Mark background processes belonging to a browser with NO visible MainUI window as hidden
        for p in &mut processes {
            if !browsers_with_ui.contains(&p.browser) {
                p.is_hidden = true;
            }
        }

        // Diff against previous snapshot to emit structured activity & alert logs
        let mut current_map: HashMap<u32, (BrowserKind, ComponentRole, u64)> = HashMap::new();
        for p in &processes {
            current_map.insert(p.pid, (p.browser, p.role, p.memory_bytes));
            if !self.seen_initial {
                if p.is_hidden {
                    self.log.push(
                        LogLevel::Hidden,
                        p.browser,
                        format!(
                            "Hidden service active: PID {} ({}) using {}",
                            p.pid,
                            p.name,
                            format_bytes(p.memory_bytes)
                        ),
                    );
                }
            } else if !self.prev_pids.contains_key(&p.pid) {
                let lvl = if p.is_hidden {
                    LogLevel::Hidden
                } else {
                    LogLevel::Spawn
                };
                self.log.push(
                    lvl,
                    p.browser,
                    format!(
                        "Started PID {} [{}] ({})",
                        p.pid,
                        p.role.short_name(),
                        p.site.as_deref().unwrap_or(&p.name)
                    ),
                );
            } else if let Some(&(_, _, prev_mem)) = self.prev_pids.get(&p.pid) {
                let delta_mb = (p.memory_bytes as i64 - prev_mem as i64) / (1024 * 1024);
                if delta_mb >= 50 {
                    self.log.push(
                        LogLevel::Alert,
                        p.browser,
                        format!(
                            "PID {} ({}) RAM spiked +{} MB (now {})",
                            p.pid,
                            p.role.short_name(),
                            delta_mb,
                            format_bytes(p.memory_bytes)
                        ),
                    );
                }
            }

            if self.seen_initial && p.cpu_usage > 45.0 {
                self.log.push(
                    LogLevel::Alert,
                    p.browser,
                    format!(
                        "High CPU {:.1}% on PID {} ({})",
                        p.cpu_usage,
                        p.pid,
                        p.role.short_name()
                    ),
                );
            }
        }

        if !self.seen_initial && !processes.is_empty() {
            let hidden_n = processes.iter().filter(|p| p.is_hidden).count();
            self.log.push(
                LogLevel::Info,
                processes[0].browser,
                format!(
                    "Initial scan complete: {} browser processes ({} hidden services)",
                    processes.len(),
                    hidden_n
                ),
            );
        }

        if self.seen_initial {
            for (old_pid, (old_b, old_r, _)) in &self.prev_pids {
                if !current_map.contains_key(old_pid) {
                    self.log.push(
                        LogLevel::Exit,
                        *old_b,
                        format!("Exited PID {} [{}]", old_pid, old_r.short_name()),
                    );
                }
            }
        }

        self.prev_pids = current_map;
        self.seen_initial = true;

        // Aggregate per browser & per role
        let mut browser_map: BTreeMap<BrowserKind, BrowserSummary> = BTreeMap::new();
        let mut comp_map: BTreeMap<ComponentRole, ComponentSummary> = BTreeMap::new();

        for p in &processes {
            let b_entry = browser_map.entry(p.browser).or_insert_with(|| BrowserSummary {
                browser: p.browser,
                count: 0,
                hidden_count: 0,
                has_main_ui: browsers_with_ui.contains(&p.browser),
                total_memory_bytes: 0,
                total_virtual_memory_bytes: 0,
                total_cpu: 0.0,
                sites: Vec::new(),
            });
            b_entry.count += 1;
            if p.is_hidden {
                b_entry.hidden_count += 1;
            }
            b_entry.total_memory_bytes += p.memory_bytes;
            b_entry.total_virtual_memory_bytes += p.virtual_memory_bytes;
            b_entry.total_cpu += p.cpu_usage;
            if let Some(ref s) = p.site {
                if !b_entry.sites.contains(s) {
                    b_entry.sites.push(s.clone());
                }
            }

            let c_entry = comp_map.entry(p.role).or_insert_with(|| ComponentSummary {
                role: p.role,
                count: 0,
                hidden_count: 0,
                total_memory_bytes: 0,
                total_virtual_memory_bytes: 0,
                total_cpu: 0.0,
            });
            c_entry.count += 1;
            if p.is_hidden {
                c_entry.hidden_count += 1;
            }
            c_entry.total_memory_bytes += p.memory_bytes;
            c_entry.total_virtual_memory_bytes += p.virtual_memory_bytes;
            c_entry.total_cpu += p.cpu_usage;
        }

        match sort_by {
            SortBy::Memory => processes.sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes)),
            SortBy::Cpu => processes.sort_by(|a, b| {
                b.cpu_usage
                    .partial_cmp(&a.cpu_usage)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortBy::Pid => processes.sort_by_key(|p| p.pid),
            SortBy::Browser => processes.sort_by_key(|p| p.browser),
        }

        let mut browser_summaries: Vec<BrowserSummary> = browser_map.into_values().collect();
        browser_summaries.sort_by(|a, b| b.total_memory_bytes.cmp(&a.total_memory_bytes));

        let mut comp_summaries: Vec<ComponentSummary> = comp_map.into_values().collect();
        comp_summaries.sort_by(|a, b| b.total_memory_bytes.cmp(&a.total_memory_bytes));

        let host = HostStats {
            total_memory_bytes: self.sys.total_memory(),
            used_memory_bytes: self.sys.used_memory(),
            available_memory_bytes: self.sys.available_memory(),
            cpu_count: self.sys.cpus().len(),
        };

        (processes, browser_summaries, comp_summaries, host)
    }
}
