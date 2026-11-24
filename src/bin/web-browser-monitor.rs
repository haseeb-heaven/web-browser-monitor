use std::env;
use std::io::{self, stdout, BufRead, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, size, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};

use web_browser_monitor::collector::BrowserCollector;
use web_browser_monitor::launcher::{collect_open_tabs, open_in_browser, BrowserRegistry};
use web_browser_monitor::model::{self, BrowserKind, SortBy};
use web_browser_monitor::ui::{render_dashboard, DashboardState, ViewMode};

enum ActionMode {
    Dashboard,
    ListPaths,
    SetPath(BrowserKind, PathBuf),
    LaunchBrowser {
        target: BrowserKind,
        with_tabs_from: Option<BrowserKind>,
        new_tab_only: bool,
    },
}

struct CliArgs {
    action: ActionMode,
    once: bool,
    interval_secs: f64,
    json: bool,
    filter: Option<String>,
    browser_filter: Option<BrowserKind>,
    log_file: Option<PathBuf>,
    sort_by: SortBy,
    view_mode: ViewMode,
}

fn parse_args() -> Result<CliArgs, String> {
    let mut action = ActionMode::Dashboard;
    let mut once = false;
    let mut interval_secs = 1.0;
    let mut json = false;
    let mut filter = None;
    let mut browser_filter = None;
    let mut log_file = None;
    let mut sort_by = SortBy::Memory;
    let mut view_mode = ViewMode::Browsers;

    let mut target_browser: Option<BrowserKind> = None;
    let mut with_tabs_from: Option<BrowserKind> = None;
    let mut new_tab_only = false;

    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--once" | "-1" => once = true,
            "--processes" | "-p" => view_mode = ViewMode::Processes,
            "--components" | "-c" => view_mode = ViewMode::Components,
            "--logs" | "-l" => view_mode = ViewMode::Logs,
            "--browsers-paths" => action = ActionMode::ListPaths,
            "--set-path" => {
                if i + 2 < args.len() {
                    let b = BrowserKind::from_str_loose(&args[i + 1])
                        .ok_or_else(|| format!("Unknown browser: {}", args[i + 1]))?;
                    let p = PathBuf::from(&args[i + 2]);
                    action = ActionMode::SetPath(b, p);
                    i += 2;
                } else {
                    return Err("Usage: --set-path <browser> <path>".into());
                }
            }
            "--open-browser" => {
                if i + 1 < args.len() {
                    i += 1;
                    target_browser = Some(
                        BrowserKind::from_str_loose(&args[i])
                            .ok_or_else(|| format!("Unknown browser: {}", args[i]))?,
                    );
                } else {
                    return Err("Missing browser name for --open-browser".into());
                }
            }
            "--with-tabs" => {
                if i + 1 < args.len() {
                    i += 1;
                    with_tabs_from = Some(
                        BrowserKind::from_str_loose(&args[i])
                            .ok_or_else(|| format!("Unknown source browser: {}", args[i]))?,
                    );
                } else {
                    return Err("Missing source browser for --with-tabs".into());
                }
            }
            "--new-tab" => {
                new_tab_only = true;
            }
            "--transfer" => {
                if i + 2 < args.len() {
                    let src = BrowserKind::from_str_loose(&args[i + 1])
                        .ok_or_else(|| format!("Unknown source browser: {}", args[i + 1]))?;
                    let dst = BrowserKind::from_str_loose(&args[i + 2])
                        .ok_or_else(|| format!("Unknown target browser: {}", args[i + 2]))?;
                    with_tabs_from = Some(src);
                    target_browser = Some(dst);
                    i += 2;
                } else {
                    return Err("Usage: --transfer <source_browser> <target_browser>".into());
                }
            }
            "--interval" | "-i" => {
                if i + 1 < args.len() {
                    i += 1;
                    interval_secs = args[i]
                        .parse::<f64>()
                        .map_err(|_| format!("Invalid interval: {}", args[i]))?
                        .max(0.2);
                } else {
                    return Err("Missing value for --interval".into());
                }
            }
            "--browser" | "-b" => {
                if i + 1 < args.len() {
                    i += 1;
                    browser_filter = BrowserKind::from_str_loose(&args[i]);
                    if browser_filter.is_none() {
                        return Err(format!("Unknown browser: {}", args[i]));
                    }
                    view_mode = ViewMode::Processes;
                } else {
                    return Err("Missing value for --browser".into());
                }
            }
            "--log-file" => {
                if i + 1 < args.len() {
                    i += 1;
                    log_file = Some(PathBuf::from(&args[i]));
                } else {
                    return Err("Missing value for --log-file".into());
                }
            }
            "--json" => {
                json = true;
                once = true;
            }
            "--filter" | "-f" => {
                if i + 1 < args.len() {
                    i += 1;
                    filter = Some(args[i].clone());
                    view_mode = ViewMode::Processes;
                } else {
                    return Err("Missing value for --filter".into());
                }
            }
            "--sort" | "-s" => {
                if i + 1 < args.len() {
                    i += 1;
                    match args[i].to_lowercase().as_str() {
                        "ram" | "mem" | "memory" => sort_by = SortBy::Memory,
                        "cpu" => sort_by = SortBy::Cpu,
                        "pid" => sort_by = SortBy::Pid,
                        "browser" | "name" => sort_by = SortBy::Browser,
                        _ => return Err(format!("Unknown sort mode: {}", args[i])),
                    }
                } else {
                    return Err("Missing value for --sort".into());
                }
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            arg => return Err(format!("Unknown argument: {arg}. Run with --help.")),
        }
        i += 1;
    }

    if let Some(target) = target_browser {
        action = ActionMode::LaunchBrowser {
            target,
            with_tabs_from,
            new_tab_only,
        };
    }

    Ok(CliArgs {
        action,
        once,
        interval_secs,
        json,
        filter,
        browser_filter,
        log_file,
        sort_by,
        view_mode,
    })
}

fn print_help() {
    println!(
        r#"web-browser-monitor - Real-time Multi-Browser, Hidden Service & Tab Transfer Monitor

USAGE:
    web-browser-monitor [OPTIONS]

MONITORING OPTIONS:
    -1, --once               Print a single snapshot and exit
    -p, --processes          Show detailed process list
    -c, --components         Show component role breakdown
    -l, --logs               Show activity & hidden service event logs
    -b, --browser <NAME>     Filter by browser (ladybird, chrome, safari, firefox, arc, brave, edge, ...)
    -i, --interval <SECS>    Refresh interval in seconds (default: 1.0)
    -s, --sort <MODE>        Sort mode: ram (default), cpu, pid, browser
    -f, --filter <TEXT>      Filter processes by name, role, or URL
        --log-file <PATH>    Append structured activity logs to a file
        --json               Output metrics in JSON format and exit

BROWSER PATHS & TAB TRANSFER:
        --browsers-paths               List auto-detected / custom install paths for all 12 browsers
        --set-path <BROWSER> <PATH>    Save custom install path for a browser
        --transfer <SRC> <DST>         Open all tabs from <SRC> browser in <DST> browser
        --open-browser <DST>           Open <DST> browser (combine with --with-tabs <SRC> or --new-tab)
        --with-tabs <SRC>              Transfer open tabs from <SRC> browser
        --new-tab                      Open fresh tab/window without transferring current tabs

INTERACTIVE CONTROLS (Live Mode):
    q / Esc / Ctrl+C         Quit
    t                        Cycle views (Browsers -> Components -> Processes -> Logs)
    b                        Cycle browser filter
    s                        Cycle sort order (RAM -> CPU -> PID -> Browser)
    Space                    Pause / resume
    + / -                    Adjust refresh speed
    r                        Force immediate refresh
"#
    );
}

fn resolve_or_prompt_path(reg: &mut BrowserRegistry, target: BrowserKind) -> Result<PathBuf, String> {
    if let Some(p) = reg.get_path(target) {
        if p.exists() {
            return Ok(p.clone());
        }
    }
    eprint!(
        "{} install path was not auto-detected. Enter path to app/executable: ",
        target.display_name()
    );
    let _ = io::stderr().flush();
    let mut line = String::new();
    io::stdin()
        .lock()
        .read_line(&mut line)
        .map_err(|e| format!("Failed reading path: {e}"))?;
    let trimmed = line.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        return Err("No path entered.".into());
    }
    let path = PathBuf::from(trimmed);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }
    reg.set_custom_path(target, path.clone());
    println!("Saved custom path for {}: {}", target.display_name(), path.display());
    Ok(path)
}

fn print_json(
    processes: &[model::ProcessDetail],
    browsers: &[model::BrowserSummary],
    host: &model::HostStats,
) {
    let total_mem: u64 = processes.iter().map(|p| p.memory_bytes).sum();
    let total_cpu: f32 = processes.iter().map(|p| p.cpu_usage).sum();
    let hidden_n = processes.iter().filter(|p| p.is_hidden).count();

    println!("{{");
    println!("  \"summary\": {{");
    println!("    \"total_memory_bytes\": {},", total_mem);
    println!("    \"total_cpu_percent\": {:.2},", total_cpu);
    println!("    \"process_count\": {},", processes.len());
    println!("    \"hidden_service_count\": {}", hidden_n);
    println!("  }},");
    println!("  \"host\": {{");
    println!("    \"total_memory_bytes\": {},", host.total_memory_bytes);
    println!("    \"used_memory_bytes\": {},", host.used_memory_bytes);
    println!("    \"cpu_count\": {}", host.cpu_count);
    println!("  }},");
    println!("  \"browsers\": [");
    for (i, b) in browsers.iter().enumerate() {
        println!(
            "    {{\"browser\": \"{}\", \"procs\": {}, \"hidden\": {}, \"memory_bytes\": {}, \"cpu_percent\": {:.2}}}{}",
            b.browser.display_name(),
            b.count,
            b.hidden_count,
            b.total_memory_bytes,
            b.total_cpu,
            if i + 1 < browsers.len() { "," } else { "" }
        );
    }
    println!("  ],");
    println!("  \"processes\": [");
    for (i, p) in processes.iter().enumerate() {
        println!(
            "    {{\"pid\": {}, \"browser\": \"{}\", \"role\": \"{}\", \"hidden\": {}, \"name\": \"{}\", \"memory_bytes\": {}, \"cpu_percent\": {:.2}}}{}",
            p.pid,
            p.browser.short_name(),
            p.role.short_name(),
            p.is_hidden,
            p.name.replace('"', "\\\""),
            p.memory_bytes,
            p.cpu_usage,
            if i + 1 < processes.len() { "," } else { "" }
        );
    }
    println!("  ]");
    println!("}}");
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };

    match args.action {
        ActionMode::ListPaths => {
            let reg = BrowserRegistry::load();
            println!("{:<18} {:<10} {}", "Browser", "Source", "Install Path");
            println!("{}", "-".repeat(76));
            for &b in BrowserKind::all() {
                if let Some(p) = reg.get_path(b) {
                    let src = if reg.is_custom(b) { "custom" } else { "auto" };
                    println!("{:<18} {:<10} {}", b.display_name(), src, p.display());
                } else {
                    println!("{:<18} {:<10} MISSING (use --set-path)", b.display_name(), "-");
                }
            }
            return Ok(());
        }
        ActionMode::SetPath(b, p) => {
            if !p.exists() {
                eprintln!("Error: path does not exist: {}", p.display());
                std::process::exit(1);
            }
            let mut reg = BrowserRegistry::load();
            reg.set_custom_path(b, p.clone());
            println!("Saved {} path: {}", b.display_name(), p.display());
            return Ok(());
        }
        ActionMode::LaunchBrowser {
            target,
            with_tabs_from,
            new_tab_only,
        } => {
            let mut reg = BrowserRegistry::load();
            let path = resolve_or_prompt_path(&mut reg, target)?;
            let mut urls = Vec::new();
            let with_tabs = !new_tab_only && with_tabs_from.is_some();
            if let Some(src) = with_tabs_from {
                let mut collector = BrowserCollector::new();
                let (_, browsers, _, _) = collector.collect(SortBy::Memory);
                let fallback: Vec<String> = browsers
                    .iter()
                    .find(|b| b.browser == src)
                    .map(|b| b.sites.clone())
                    .unwrap_or_default();
                urls = collect_open_tabs(src, &fallback);
                println!("Found {} tab URL(s) from {}", urls.len(), src.display_name());
            }
            let msg = open_in_browser(target, &path, &urls, with_tabs)?;
            println!("{msg}");
            return Ok(());
        }
        ActionMode::Dashboard => {}
    }

    let mut collector = BrowserCollector::new();
    collector.log.file_path = args.log_file;

    std::thread::sleep(Duration::from_millis(150));
    let (processes, browsers, components, host) = collector.collect(args.sort_by);

    if args.json {
        print_json(&processes, &browsers, &host);
        return Ok(());
    }

    let mut state = DashboardState::new(args.interval_secs);
    state.sort_by = args.sort_by;
    state.view_mode = args.view_mode;
    state.filter = args.filter;
    state.browser_filter = args.browser_filter;

    let total_mem: u64 = processes.iter().map(|p| p.memory_bytes).sum();
    let total_cpu: f32 = processes.iter().map(|p| p.cpu_usage).sum();
    state
        .history
        .push(total_mem as f64 / (1024.0 * 1024.0), total_cpu);

    if args.once {
        let mut out = stdout();
        render_dashboard(
            &mut out,
            &processes,
            &browsers,
            &components,
            &host,
            &collector.log,
            &state,
            false,
            None,
        )?;
        out.flush()?;
        return Ok(());
    }

    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, Hide)?;

    struct CleanUp;
    impl Drop for CleanUp {
        fn drop(&mut self) {
            let _ = disable_raw_mode();
            let _ = execute!(stdout(), Show, LeaveAlternateScreen);
        }
    }
    let _cleanup = CleanUp;

    let mut last_tick = Instant::now();
    let mut cur_processes = processes;
    let mut cur_browsers = browsers;
    let mut cur_components = components;
    let mut cur_host = host;

    loop {
        let term_rows = size().map(|(_, r)| r as usize).ok();
        execute!(out, MoveTo(0, 0), Clear(ClearType::All))?;
        render_dashboard(
            &mut out,
            &cur_processes,
            &cur_browsers,
            &cur_components,
            &cur_host,
            &collector.log,
            &state,
            true,
            term_rows,
        )?;
        out.flush()?;

        let tick_duration = Duration::from_secs_f64(state.interval_secs);
        let elapsed = last_tick.elapsed();
        let timeout = if elapsed >= tick_duration {
            Duration::from_millis(10)
        } else {
            tick_duration - elapsed
        };

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('c')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            break
                        }
                        KeyCode::Char('s') => {
                            state.sort_by = state.sort_by.next();
                            let (p, b, c, h) = collector.collect(state.sort_by);
                            cur_processes = p;
                            cur_browsers = b;
                            cur_components = c;
                            cur_host = h;
                        }
                        KeyCode::Char('t') | KeyCode::Char('g') => {
                            state.view_mode = state.view_mode.next();
                        }
                        KeyCode::Char('b') => {
                            if cur_browsers.is_empty() {
                                state.browser_filter = None;
                            } else {
                                state.browser_filter = match state.browser_filter {
                                    None => Some(cur_browsers[0].browser),
                                    Some(cur) => {
                                        let idx = cur_browsers
                                            .iter()
                                            .position(|x| x.browser == cur)
                                            .unwrap_or(0);
                                        if idx + 1 < cur_browsers.len() {
                                            Some(cur_browsers[idx + 1].browser)
                                        } else {
                                            None
                                        }
                                    }
                                };
                            }
                        }
                        KeyCode::Char(' ') => state.paused = !state.paused,
                        KeyCode::Char('+') | KeyCode::Char('=') => {
                            state.interval_secs = (state.interval_secs - 0.2).max(0.2);
                        }
                        KeyCode::Char('-') => {
                            if state.interval_secs < 10.0 {
                                state.interval_secs += 0.5;
                            }
                        }
                        KeyCode::Char('r') => {
                            let (p, b, c, h) = collector.collect(state.sort_by);
                            cur_processes = p;
                            cur_browsers = b;
                            cur_components = c;
                            cur_host = h;
                            let ram = cur_processes.iter().map(|p| p.memory_bytes).sum::<u64>()
                                as f64
                                / (1024.0 * 1024.0);
                            let cpu = cur_processes.iter().map(|p| p.cpu_usage).sum::<f32>();
                            state.history.push(ram, cpu);
                            last_tick = Instant::now();
                        }
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_duration {
            if !state.paused {
                let (p, b, c, h) = collector.collect(state.sort_by);
                cur_processes = p;
                cur_browsers = b;
                cur_components = c;
                cur_host = h;
                let ram = cur_processes.iter().map(|p| p.memory_bytes).sum::<u64>() as f64
                    / (1024.0 * 1024.0);
                let cpu = cur_processes.iter().map(|p| p.cpu_usage).sum::<f32>();
                state.history.push(ram, cpu);
            }
            last_tick = Instant::now();
        }
    }

    Ok(())
}
