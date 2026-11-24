use std::fmt::Write as FmtWrite;
use std::io::Write;

use crate::chart::*;
use crate::model::{
    BrowserKind, BrowserSummary, ComponentSummary, EventLog, HistoryBuffer, HostStats, LogLevel,
    ProcessDetail, SortBy,
};

pub const BOX_W: usize = 92;
const INNER_W: usize = BOX_W - 2; // 90
const CONTENT_W: usize = INNER_W - 2; // 88

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    Browsers,
    Components,
    Processes,
    Logs,
}

impl ViewMode {
    pub fn next(&self) -> Self {
        match self {
            ViewMode::Browsers => ViewMode::Components,
            ViewMode::Components => ViewMode::Processes,
            ViewMode::Processes => ViewMode::Logs,
            ViewMode::Logs => ViewMode::Browsers,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ViewMode::Browsers => "Browsers",
            ViewMode::Components => "Components",
            ViewMode::Processes => "Processes",
            ViewMode::Logs => "Logs",
        }
    }
}

pub struct DashboardState {
    pub sort_by: SortBy,
    pub view_mode: ViewMode,
    pub paused: bool,
    pub interval_secs: f64,
    pub history: HistoryBuffer,
    pub filter: Option<String>,
    pub browser_filter: Option<BrowserKind>,
}

impl DashboardState {
    pub fn new(interval_secs: f64) -> Self {
        Self {
            sort_by: SortBy::Memory,
            view_mode: ViewMode::Browsers,
            paused: false,
            interval_secs,
            history: HistoryBuffer::new(30),
            filter: None,
            browser_filter: None,
        }
    }
}

pub fn visible_width(s: &str) -> usize {
    let mut width = 0;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            while let Some(esc_c) = chars.next() {
                if ('@'..='~').contains(&esc_c) {
                    break;
                }
            }
        } else if (c as u32) >= 0x1F300 {
            width += 2;
        } else {
            width += 1;
        }
    }
    width
}

pub fn trunc(s: &str, max_w: usize) -> String {
    if max_w == 0 {
        return String::new();
    }
    let char_count = s.chars().count();
    if char_count <= max_w {
        return s.to_string();
    }
    if max_w == 1 {
        return "…".to_string();
    }
    let mut out: String = s.chars().take(max_w - 1).collect();
    out.push('…');
    out
}

fn box_row(inner: &str, content_w: usize, left: &str, right: &str) -> String {
    let vis = visible_width(inner);
    let pad = content_w.saturating_sub(vis);
    format!("{left}{inner}{}{right}", " ".repeat(pad))
}

fn section_top(title: &str) -> String {
    let prefix = format!("┌── {title} ");
    let dashes = BOX_W.saturating_sub(visible_width(&prefix) + 1);
    format!(
        "{COLOR_BOLD}{COLOR_WHITE}{prefix}{}┐{COLOR_RESET}",
        "─".repeat(dashes)
    )
}

fn browser_color(b: BrowserKind) -> &'static str {
    match b {
        BrowserKind::Ladybird => COLOR_CYAN,
        BrowserKind::Chrome => COLOR_GREEN,
        BrowserKind::Safari => COLOR_BLUE,
        BrowserKind::Firefox => COLOR_ORANGE,
        BrowserKind::Arc => COLOR_MAGENTA,
        BrowserKind::Brave => COLOR_YELLOW,
        BrowserKind::Edge => COLOR_CYAN,
        _ => COLOR_PURPLE,
    }
}

pub fn render_dashboard<W: Write>(
    w: &mut W,
    processes: &[ProcessDetail],
    browsers: &[BrowserSummary],
    components: &[ComponentSummary],
    host: &HostStats,
    log: &EventLog,
    state: &DashboardState,
    is_live: bool,
    max_height: Option<usize>,
) -> std::io::Result<()> {
    let mut buf = String::with_capacity(4096);

    let filtered_procs: Vec<&ProcessDetail> = processes
        .iter()
        .filter(|p| match state.browser_filter {
            Some(bf) => p.browser == bf,
            None => true,
        })
        .filter(|p| {
            if let Some(ref filt) = state.filter {
                let fl = filt.to_lowercase();
                p.name.to_lowercase().contains(&fl)
                    || p.browser.short_name().to_lowercase().contains(&fl)
                    || p.role.short_name().to_lowercase().contains(&fl)
                    || p.site
                        .as_ref()
                        .map(|s| s.to_lowercase().contains(&fl))
                        .unwrap_or(false)
            } else {
                true
            }
        })
        .collect();

    let total_mem: u64 = processes.iter().map(|p| p.memory_bytes).sum();
    let total_vsz: u64 = processes.iter().map(|p| p.virtual_memory_bytes).sum();
    let total_cpu: f32 = processes.iter().map(|p| p.cpu_usage).sum();
    let hidden_count = processes.iter().filter(|p| p.is_hidden).count();
    let hidden_mem: u64 = processes
        .iter()
        .filter(|p| p.is_hidden)
        .map(|p| p.memory_bytes)
        .sum();

    // 1. Header Box
    let title = "◈ WEB BROWSER MONITOR (ALL BROWSERS + HIDDEN SERVICES)";
    let badge = if is_live {
        if state.paused {
            format!("{COLOR_YELLOW}[PAUSED]{COLOR_RESET}")
        } else {
            format!(
                "{COLOR_GREEN}[LIVE: {:.1}s]{COLOR_RESET}",
                state.interval_secs
            )
        }
    } else {
        format!("{COLOR_BLUE}[SNAPSHOT]{COLOR_RESET}")
    };
    let header_gap = CONTENT_W.saturating_sub(visible_width(title) + visible_width(&badge));
    let header_inner = format!(
        "{COLOR_BOLD}{COLOR_WHITE}{title}{COLOR_RESET}{}{badge}",
        " ".repeat(header_gap)
    );
    let _ = writeln!(
        buf,
        "{COLOR_CYAN}╔{}╗{COLOR_RESET}",
        "═".repeat(INNER_W)
    );
    let _ = writeln!(
        buf,
        "{}",
        box_row(
            &header_inner,
            CONTENT_W,
            &format!("{COLOR_CYAN}║{COLOR_RESET} "),
            &format!(" {COLOR_CYAN}║{COLOR_RESET}")
        )
    );
    let _ = writeln!(
        buf,
        "{COLOR_CYAN}╚{}╝{COLOR_RESET}",
        "═".repeat(INNER_W)
    );

    // 2. Metrics Summary
    let mem_str = format_bytes(total_mem);
    let vsz_str = format_bytes(total_vsz);
    let host_mem_str = format_bytes(host.total_memory_bytes);
    let host_used_str = format_bytes(host.used_memory_bytes);
    let pct_host = if host.total_memory_bytes > 0 {
        (total_mem as f64 / host.total_memory_bytes as f64) * 100.0
    } else {
        0.0
    };
    let host_used_pct = if host.total_memory_bytes > 0 {
        (host.used_memory_bytes as f64 / host.total_memory_bytes as f64) * 100.0
    } else {
        0.0
    };

    let cpu_c = get_cpu_color(total_cpu);
    let ram_c = get_ram_color(total_mem);

    let _ = writeln!(
        buf,
        " {COLOR_BOLD}Browsers Total:{COLOR_RESET} {ram_c}{mem_str}{COLOR_RESET} ({pct_host:.1}% Host) | {COLOR_BOLD}VSZ:{COLOR_RESET} {vsz_str} | {COLOR_BOLD}CPU:{COLOR_RESET} {cpu_c}{total_cpu:.1}%{COLOR_RESET} | {COLOR_BOLD}Procs:{COLOR_RESET} {COLOR_CYAN}{}{COLOR_RESET} | {COLOR_BOLD}Hidden:{COLOR_RESET} {COLOR_MAGENTA}{hidden_count} ({}){COLOR_RESET}",
        processes.len(),
        format_bytes(hidden_mem)
    );
    let _ = writeln!(
        buf,
        " {COLOR_BOLD}Host System:{COLOR_RESET} Total: {host_mem_str} | Used: {host_used_str} ({host_used_pct:.1}%) | Cores: {} | View: {COLOR_CYAN}{}{COLOR_RESET}",
        host.cpu_count,
        state.view_mode.label()
    );

    let gauge_bar = render_host_ram_gauge(total_mem, host.used_memory_bytes, host.total_memory_bytes, 38);
    let _ = writeln!(
        buf,
        " {COLOR_DIM}Host Gauge: [{gauge_bar}{COLOR_DIM}] {COLOR_CYAN}■ Browsers {COLOR_BLUE}■ Other {COLOR_GRAY}░ Free{COLOR_RESET}"
    );
    let _ = writeln!(buf);

    // 3. Sparklines
    if !state.history.ram_history.is_empty() {
        let min_ram = state.history.ram_history.iter().copied().fold(f64::INFINITY, f64::min);
        let max_ram = state.history.ram_history.iter().copied().fold(0.0, f64::max);
        let cur_ram = *state.history.ram_history.back().unwrap_or(&0.0);
        let ram_sparkline = render_sparkline(&state.history.ram_history, min_ram, max_ram, COLOR_CYAN);

        let min_cpu = state.history.cpu_history.iter().copied().fold(f32::INFINITY, f32::min) as f64;
        let max_cpu = state.history.cpu_history.iter().copied().fold(0.0, f32::max) as f64;
        let cur_cpu = *state.history.cpu_history.back().unwrap_or(&0.0);
        let cpu_history_f64: std::collections::VecDeque<f64> =
            state.history.cpu_history.iter().map(|&c| c as f64).collect();
        let cpu_sparkline = render_sparkline(&cpu_history_f64, min_cpu, max_cpu, COLOR_YELLOW);

        let _ = writeln!(
            buf,
            " {COLOR_BOLD}RAM Trend:{COLOR_RESET} [{ram_sparkline}] {COLOR_CYAN}{cur_ram:.1} MB{COLOR_RESET} {COLOR_DIM}(min: {min_ram:.1}M, max: {max_ram:.1}M){COLOR_RESET}"
        );
        let _ = writeln!(
            buf,
            " {COLOR_BOLD}CPU Trend:{COLOR_RESET} [{cpu_sparkline}] {COLOR_YELLOW}{cur_cpu:.1}%{COLOR_RESET} {COLOR_DIM}(peak: {max_cpu:.1}%){COLOR_RESET}"
        );
        let _ = writeln!(buf);
    }

    // 4. Browser RAM Breakdown Bars
    let _ = writeln!(buf, "{}", section_top("BROWSER RAM ALLOCATION BREAKDOWN"));
    if browsers.is_empty() {
        let _ = writeln!(
            buf,
            "{}",
            box_row(
                &format!("{COLOR_YELLOW}No browser processes currently detected.{COLOR_RESET}"),
                CONTENT_W,
                "│ ",
                " │"
            )
        );
    } else {
        for b in browsers {
            let pct = if total_mem > 0 {
                (b.total_memory_bytes as f64 / total_mem as f64) * 100.0
            } else {
                0.0
            };
            let bc = browser_color(b.browser);
            let bar = render_bar(pct / 100.0, 22, bc);
            let mem_s = format_bytes(b.total_memory_bytes);
            let b_name = trunc(b.browser.display_name(), 23);
            let tag = if !b.has_main_ui {
                format!("({} hidden bg)", b.count)
            } else if b.hidden_count > 0 {
                format!("({} procs, {} hidden)", b.count, b.hidden_count)
            } else {
                format!("({} procs)", b.count)
            };
            let extra = trunc(&tag, 21);

            let row_inner = format!(
                "{bc}{:<23}{COLOR_RESET} [{bar}] {COLOR_BOLD}{:>8}{COLOR_RESET} ({:>5.1}%) {COLOR_DIM}{:<21}{COLOR_RESET}",
                b_name, mem_s, pct, extra
            );
            let _ = writeln!(buf, "{}", box_row(&row_inner, CONTENT_W, "│ ", " │"));
        }
    }
    let _ = writeln!(buf, "└{}┘", "─".repeat(INNER_W));
    let _ = writeln!(buf);

    let used_lines = buf.lines().count();
    let footer_lines = if is_live { 2 } else { 0 };
    let table_chrome = 4;
    let max_table_rows = if is_live {
        max_height.map(|h| h.saturating_sub(used_lines + footer_lines + table_chrome).max(1))
    } else {
        None
    };

    // 5. Active Table / Log Section
    match state.view_mode {
        ViewMode::Browsers => {
            // Cols: 19 + 7 + 8 + 11 + 8 + 10 + 8 + 12 + 9 borders = 92
            let _ = writeln!(buf, "{}", section_top("DETECTED BROWSERS SUMMARY"));
            let _ = writeln!(
                buf,
                "│ {COLOR_BOLD}{:<17}{COLOR_RESET} │ {COLOR_BOLD}{:>5}{COLOR_RESET} │ {COLOR_BOLD}{:>6}{COLOR_RESET} │ {COLOR_BOLD}{:>9}{COLOR_RESET} │ {COLOR_BOLD}{:>6}{COLOR_RESET} │ {COLOR_BOLD}{:>8}{COLOR_RESET} │ {COLOR_BOLD}{:>6}{COLOR_RESET} │ {COLOR_BOLD}{:<10}{COLOR_RESET} │",
                "Browser", "Procs", "Hidden", "RAM (RSS)", "% All", "VSZ", "CPU%", "State"
            );
            let _ = writeln!(
                buf,
                "├───────────────────┼───────┼────────┼───────────┼────────┼──────────┼────────┼────────────┤"
            );

            let total_rows = browsers.len();
            let show_rows = match max_table_rows {
                Some(cap) if total_rows > cap => cap.saturating_sub(1),
                _ => total_rows,
            };

            for b in browsers.iter().take(show_rows) {
                let pct = if total_mem > 0 {
                    (b.total_memory_bytes as f64 / total_mem as f64) * 100.0
                } else {
                    0.0
                };
                let bc = browser_color(b.browser);
                let ram_col = get_ram_color(b.total_memory_bytes);
                let cpu_col = get_cpu_color(b.total_cpu);
                let state_str = if b.has_main_ui { "Active UI" } else { "Bg Hidden" };
                let _ = writeln!(
                    buf,
                    "│ {bc}{:<17}{COLOR_RESET} │ {:>5} │ {:>6} │ {ram_col}{:>9}{COLOR_RESET} │ {:>5.1}% │ {:>8} │ {cpu_col}{:>5.1}%{COLOR_RESET} │ {:<10} │",
                    trunc(b.browser.display_name(), 17),
                    b.count,
                    b.hidden_count,
                    format_bytes(b.total_memory_bytes),
                    pct,
                    format_bytes(b.total_virtual_memory_bytes),
                    b.total_cpu,
                    state_str
                );
            }
            if show_rows < total_rows {
                let msg = format!("{COLOR_DIM}... +{} more browsers{COLOR_RESET}", total_rows - show_rows);
                let _ = writeln!(buf, "{}", box_row(&msg, CONTENT_W, "│ ", " │"));
            }
            let _ = writeln!(
                buf,
                "└───────────────────┴───────┴────────┴───────────┴────────┴──────────┴────────┴────────────┘"
            );
        }
        ViewMode::Components => {
            // Cols: 26 + 8 + 8 + 12 + 10 + 11 + 9 + 8 borders = 92
            let _ = writeln!(buf, "{}", section_top("COMPONENT ROLES BREAKDOWN"));
            let _ = writeln!(
                buf,
                "│ {COLOR_BOLD}{:<24}{COLOR_RESET} │ {COLOR_BOLD}{:>6}{COLOR_RESET} │ {COLOR_BOLD}{:>6}{COLOR_RESET} │ {COLOR_BOLD}{:>10}{COLOR_RESET} │ {COLOR_BOLD}{:>8}{COLOR_RESET} │ {COLOR_BOLD}{:>9}{COLOR_RESET} │ {COLOR_BOLD}{:>7}{COLOR_RESET} │",
                "Component Role", "Procs", "Hidden", "RAM (RSS)", "% All", "VSZ", "CPU%"
            );
            let _ = writeln!(
                buf,
                "├──────────────────────────┼────────┼────────┼────────────┼──────────┼───────────┼─────────┤"
            );

            for c in components {
                let pct = if total_mem > 0 {
                    (c.total_memory_bytes as f64 / total_mem as f64) * 100.0
                } else {
                    0.0
                };
                let ram_col = get_ram_color(c.total_memory_bytes);
                let cpu_col = get_cpu_color(c.total_cpu);
                let _ = writeln!(
                    buf,
                    "│ {:<24} │ {:>6} │ {:>6} │ {ram_col}{:>10}{COLOR_RESET} │ {:>7.1}% │ {:>9} │ {cpu_col}{:>6.1}%{COLOR_RESET} │",
                    trunc(c.role.display_name(), 24),
                    c.count,
                    c.hidden_count,
                    format_bytes(c.total_memory_bytes),
                    pct,
                    format_bytes(c.total_virtual_memory_bytes),
                    c.total_cpu
                );
            }
            let _ = writeln!(
                buf,
                "└──────────────────────────┴────────┴────────┴────────────┴──────────┴───────────┴─────────┘"
            );
        }
        ViewMode::Processes => {
            // Cols: 7 + 12 + 14 + 4 + 18 + 8 + 10 + 10 + 9 borders = 92
            let title = format!("PROCESS LIST ({})", state.sort_by.label());
            let _ = writeln!(buf, "{}", section_top(&title));
            let _ = writeln!(
                buf,
                "│ {COLOR_BOLD}{:>5}{COLOR_RESET} │ {COLOR_BOLD}{:<10}{COLOR_RESET} │ {COLOR_BOLD}{:<12}{COLOR_RESET} │ {COLOR_BOLD}{:<2}{COLOR_RESET} │ {COLOR_BOLD}{:<16}{COLOR_RESET} │ {COLOR_BOLD}{:>6}{COLOR_RESET} │ {COLOR_BOLD}{:>8}{COLOR_RESET} │ {COLOR_BOLD}{:>8}{COLOR_RESET} │",
                "PID", "Browser", "Role", "Hd", "Target / Name", "CPU%", "RAM", "Uptime"
            );
            let _ = writeln!(
                buf,
                "├───────┼────────────┼──────────────┼────┼──────────────────┼────────┼──────────┼──────────┤"
            );

            let total_rows = filtered_procs.len();
            let show_rows = match max_table_rows {
                Some(cap) if total_rows > cap => cap.saturating_sub(1),
                _ => total_rows,
            };

            for p in filtered_procs.iter().take(show_rows) {
                let target = p
                    .site
                    .as_deref()
                    .map(|s| s.trim_start_matches("https://").trim_start_matches("http://"))
                    .unwrap_or(&p.name);
                let hd = if p.is_hidden { "Y" } else { "-" };
                let bc = browser_color(p.browser);
                let ram_col = get_ram_color(p.memory_bytes);
                let cpu_col = get_cpu_color(p.cpu_usage);

                let _ = writeln!(
                    buf,
                    "│ {:>5} │ {bc}{:<10}{COLOR_RESET} │ {:<12} │ {:>2} │ {:<16} │ {cpu_col}{:>5.1}%{COLOR_RESET} │ {ram_col}{:>8}{COLOR_RESET} │ {:>8} │",
                    p.pid,
                    trunc(p.browser.short_name(), 10),
                    trunc(p.role.short_name(), 12),
                    hd,
                    trunc(target, 16),
                    p.cpu_usage,
                    format_bytes(p.memory_bytes),
                    trunc(&format_duration(p.run_time_secs), 8)
                );
            }
            if show_rows < total_rows {
                let msg = format!(
                    "{COLOR_DIM}... +{} more processes (use --filter or resize terminal){COLOR_RESET}",
                    total_rows - show_rows
                );
                let _ = writeln!(buf, "{}", box_row(&msg, CONTENT_W, "│ ", " │"));
            }
            let _ = writeln!(
                buf,
                "└───────┴────────────┴──────────────┴────┴──────────────────┴────────┴──────────┴──────────┘"
            );
        }
        ViewMode::Logs => {
            let _ = writeln!(buf, "{}", section_top("ACTIVITY & HIDDEN SERVICE LOGS"));
            let total_rows = log.entries.len();
            let cap = max_table_rows.unwrap_or(16);
            if total_rows == 0 {
                let _ = writeln!(
                    buf,
                    "{}",
                    box_row("No events recorded yet.", CONTENT_W, "│ ", " │")
                );
            } else {
                for entry in log.entries.iter().rev().take(cap) {
                    let lvl_col = match entry.level {
                        LogLevel::Alert => COLOR_RED,
                        LogLevel::Hidden => COLOR_MAGENTA,
                        LogLevel::Spawn => COLOR_GREEN,
                        LogLevel::Exit => COLOR_YELLOW,
                        LogLevel::Info => COLOR_CYAN,
                    };
                    let line = format!(
                        "{COLOR_DIM}{}{COLOR_RESET} {lvl_col}{:<6}{COLOR_RESET} {:<9} {}",
                        entry.timestamp,
                        entry.level.tag(),
                        trunc(entry.browser.short_name(), 9),
                        trunc(&entry.message, 60)
                    );
                    let _ = writeln!(buf, "{}", box_row(&line, CONTENT_W, "│ ", " │"));
                }
            }
            let _ = writeln!(buf, "└{}┘", "─".repeat(INNER_W));
        }
    }

    if is_live {
        let _ = writeln!(buf);
        let _ = write!(
            buf,
            " {COLOR_DIM}Keys:{COLOR_RESET} {COLOR_BOLD}[q]{COLOR_RESET} Quit │ {COLOR_BOLD}[t]{COLOR_RESET} View: {} │ {COLOR_BOLD}[b]{COLOR_RESET} Browser │ {COLOR_BOLD}[s]{COLOR_RESET} Sort: {} │ {COLOR_BOLD}[space]{COLOR_RESET} Pause │ {COLOR_BOLD}[+/-]{COLOR_RESET} Rate",
            state.view_mode.label(),
            state.sort_by.label()
        );
    }

    if is_live {
        let crlf = buf.replace('\n', "\r\n");
        w.write_all(crlf.as_bytes())?;
    } else {
        w.write_all(buf.as_bytes())?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ComponentRole;

    fn sample_data() -> (
        Vec<ProcessDetail>,
        Vec<BrowserSummary>,
        Vec<ComponentSummary>,
        HostStats,
        EventLog,
        DashboardState,
    ) {
        let procs = vec![
            ProcessDetail {
                pid: 101,
                ppid: Some(1),
                browser: BrowserKind::Ladybird,
                role: ComponentRole::WebContent,
                is_hidden: false,
                name: "WebContent".into(),
                exe: "/Applications/Ladybird.app/Contents/MacOS/WebContent".into(),
                cmd: vec![],
                site: None,
                memory_bytes: 250 * 1024 * 1024,
                virtual_memory_bytes: 4520 * 1024 * 1024 * 1024,
                cpu_usage: 4.5,
                run_time_secs: 600,
            },
            ProcessDetail {
                pid: 202,
                ppid: Some(1),
                browser: BrowserKind::Safari,
                role: ComponentRole::HiddenDaemon,
                is_hidden: true,
                name: "com.apple.SafariPlatformSupport.Helper".into(),
                exe: "/System/Library/PrivateFrameworks/SafariPlatformSupport.framework/com.apple.SafariPlatformSupport.Helper".into(),
                cmd: vec![],
                site: None,
                memory_bytes: 12 * 1024 * 1024,
                virtual_memory_bytes: 400 * 1024 * 1024 * 1024,
                cpu_usage: 0.1,
                run_time_secs: 3600,
            },
        ];
        let browsers = vec![
            BrowserSummary {
                browser: BrowserKind::Ladybird,
                count: 1,
                hidden_count: 0,
                has_main_ui: true,
                total_memory_bytes: 250 * 1024 * 1024,
                total_virtual_memory_bytes: 4520 * 1024 * 1024 * 1024,
                total_cpu: 4.5,
                sites: vec![],
            },
            BrowserSummary {
                browser: BrowserKind::Safari,
                count: 1,
                hidden_count: 1,
                has_main_ui: false,
                total_memory_bytes: 12 * 1024 * 1024,
                total_virtual_memory_bytes: 400 * 1024 * 1024 * 1024,
                total_cpu: 0.1,
                sites: vec![],
            },
        ];
        let comps = vec![ComponentSummary {
            role: ComponentRole::WebContent,
            count: 1,
            hidden_count: 0,
            total_memory_bytes: 250 * 1024 * 1024,
            total_virtual_memory_bytes: 4520 * 1024 * 1024 * 1024,
            total_cpu: 4.5,
        }];
        let host = HostStats {
            total_memory_bytes: 8 * 1024 * 1024 * 1024,
            used_memory_bytes: 6 * 1024 * 1024 * 1024,
            available_memory_bytes: 2 * 1024 * 1024 * 1024,
            cpu_count: 8,
        };
        let mut log = EventLog::new(20);
        log.push(LogLevel::Hidden, BrowserKind::Safari, "Hidden Safari service active");
        let mut state = DashboardState::new(1.0);
        state.history.push(262.0, 4.6);
        (procs, browsers, comps, host, log, state)
    }

    fn assert_boxed_lines_aligned(rendered: &str) {
        for line in rendered.lines() {
            let trimmed = line.trim_end_matches('\r');
            if trimmed.starts_with('╔')
                || trimmed.starts_with('║')
                || trimmed.starts_with('╚')
                || trimmed.starts_with('┌')
                || trimmed.starts_with('│')
                || trimmed.starts_with('├')
                || trimmed.starts_with('└')
                || trimmed.starts_with("\x1b[1m\x1b[38;5;255m┌")
                || trimmed.starts_with("\x1b[38;5;51m╔")
                || trimmed.starts_with("\x1b[38;5;51m║")
                || trimmed.starts_with("\x1b[38;5;51m╚")
            {
                assert_eq!(
                    visible_width(trimmed),
                    BOX_W,
                    "Line width mismatch ({}): {:?}",
                    visible_width(trimmed),
                    trimmed
                );
            }
        }
    }

    #[test]
    fn all_cli_views_align_to_box_w() {
        let (procs, browsers, comps, host, log, mut state) = sample_data();
        for mode in [
            ViewMode::Browsers,
            ViewMode::Components,
            ViewMode::Processes,
            ViewMode::Logs,
        ] {
            state.view_mode = mode;
            let mut out = Vec::new();
            render_dashboard(&mut out, &procs, &browsers, &comps, &host, &log, &state, false, None)
                .unwrap();
            assert_boxed_lines_aligned(&String::from_utf8(out).unwrap());
        }
    }

    #[test]
    fn live_mode_uses_crlf_and_respects_max_height() {
        let (procs, browsers, comps, host, log, state) = sample_data();
        let mut out = Vec::new();
        render_dashboard(&mut out, &procs, &browsers, &comps, &host, &log, &state, true, Some(22))
            .unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(!s.contains("\r\r\n"));
        assert!(s.lines().count() <= 22);
    }
}
