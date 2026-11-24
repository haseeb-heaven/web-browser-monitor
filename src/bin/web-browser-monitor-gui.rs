use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use gpui_kit::{
    div, prelude::*, px, relative, rgb, size, App, Bounds, ClipboardItem, Context, ElementId,
    FontWeight, IntoElement, MouseButton, MouseDownEvent, PathPromptOptions, Render, SharedString,
    TitlebarOptions, Window, WindowBounds, WindowOptions,
};

use web_browser_monitor::chart::{format_bytes, format_duration};
use web_browser_monitor::collector::BrowserCollector;
use web_browser_monitor::launcher::{collect_open_tabs, open_in_browser, BrowserRegistry};
use web_browser_monitor::model::{
    BrowserKind, BrowserSummary, ComponentRole, ComponentSummary, HistoryBuffer, HostStats,
    LogLevel, ProcessDetail, SortBy,
};
use web_browser_monitor::theme::{ThemeMode, ThemePalette};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Overview,
    Browsers,
    Processes,
    Transfer,
    Logs,
    Settings,
}

impl ActiveTab {
    fn label(&self) -> &'static str {
        match self {
            ActiveTab::Overview => "Overview",
            ActiveTab::Browsers => "Browsers",
            ActiveTab::Processes => "Processes",
            ActiveTab::Transfer => "Tab Transfer",
            ActiveTab::Logs => "Logs",
            ActiveTab::Settings => "Settings",
        }
    }

    fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "overview" | "home" => Some(ActiveTab::Overview),
            "browsers" | "browser" => Some(ActiveTab::Browsers),
            "processes" | "procs" | "process" => Some(ActiveTab::Processes),
            "transfer" | "tabs" => Some(ActiveTab::Transfer),
            "logs" | "log" | "events" => Some(ActiveTab::Logs),
            "settings" | "prefs" => Some(ActiveTab::Settings),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GuiSortCol {
    Pid,
    Browser,
    Role,
    Hidden,
    Procs,
    Cpu,
    Ram,
    Vsz,
    Uptime,
}

impl GuiSortCol {
    fn label(&self) -> &'static str {
        match self {
            GuiSortCol::Pid => "PID",
            GuiSortCol::Browser => "Browser",
            GuiSortCol::Role => "Role",
            GuiSortCol::Hidden => "Hidden",
            GuiSortCol::Procs => "Procs",
            GuiSortCol::Cpu => "CPU%",
            GuiSortCol::Ram => "RAM",
            GuiSortCol::Vsz => "VSZ",
            GuiSortCol::Uptime => "Uptime",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RoleFilter {
    All,
    HiddenOnly,
    Role(ComponentRole),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LogFilter {
    All,
    SpawnsExits,
    Hidden,
    Alerts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpenDropdown {
    None,
    ToolbarBrowser,
    ToolbarSort,
    TransferSource,
    TransferTarget,
    SettingsBrowser,
}

impl OpenDropdown {
    fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "toolbar-browser" | "browser" => Some(OpenDropdown::ToolbarBrowser),
            "toolbar-sort" | "sort" => Some(OpenDropdown::ToolbarSort),
            "transfer-source" | "source" => Some(OpenDropdown::TransferSource),
            "transfer-target" | "target" => Some(OpenDropdown::TransferTarget),
            "settings-browser" | "settings" => Some(OpenDropdown::SettingsBrowser),
            _ => None,
        }
    }
}

fn browser_color(b: BrowserKind, p: &ThemePalette) -> u32 {
    match b {
        BrowserKind::Ladybird => p.accent_cyan,
        BrowserKind::Chrome => p.accent_green,
        BrowserKind::Safari => p.accent_blue,
        BrowserKind::Firefox => p.accent_amber,
        BrowserKind::Arc => p.accent_purple,
        BrowserKind::Brave => p.accent_amber,
        BrowserKind::Edge => p.accent_cyan,
        _ => p.accent_purple,
    }
}

fn role_color(r: ComponentRole, p: &ThemePalette) -> u32 {
    match r {
        ComponentRole::WebContent => p.accent_cyan,
        ComponentRole::MainUI => p.accent_green,
        ComponentRole::Network => p.accent_amber,
        ComponentRole::GpuCompositor => p.accent_purple,
        ComponentRole::MediaAudio => p.accent_blue,
        ComponentRole::HiddenDaemon => p.accent_red,
        _ => p.text_muted,
    }
}

fn active_text_color(theme: ThemeMode, active: bool, p: &ThemePalette) -> u32 {
    if active && theme == ThemeMode::Classic {
        0xffffff
    } else {
        p.text_primary
    }
}

struct GuiLaunchArgs {
    theme: ThemeMode,
    tab: ActiveTab,
    browser: Option<BrowserKind>,
    open_dropdown: OpenDropdown,
}

fn parse_gui_args() -> GuiLaunchArgs {
    let mut theme = ThemeMode::Dark;
    let mut tab = ActiveTab::Overview;
    let mut browser = None;
    let mut open_dropdown = OpenDropdown::None;
    let args: Vec<String> = env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--theme" | "-T" => {
                if i + 1 < args.len() {
                    i += 1;
                    if let Some(t) = ThemeMode::from_str_loose(&args[i]) {
                        theme = t;
                    }
                }
            }
            "--tab" | "-t" => {
                if i + 1 < args.len() {
                    i += 1;
                    if let Some(tb) = ActiveTab::from_str_loose(&args[i]) {
                        tab = tb;
                    }
                }
            }
            "--browser" | "-b" => {
                if i + 1 < args.len() {
                    i += 1;
                    browser = BrowserKind::from_str_loose(&args[i]);
                }
            }
            "--open-dropdown" => {
                if i + 1 < args.len() {
                    i += 1;
                    if let Some(dd) = OpenDropdown::from_str_loose(&args[i]) {
                        open_dropdown = dd;
                    }
                }
            }
            "--settings" => tab = ActiveTab::Settings,
            "--help" | "-h" => {
                println!(
                    "web-browser-monitor-gui — GPUI Multi-Browser, Hidden Service & Tab Transfer Monitor\n\n\
                     USAGE:\n  web-browser-monitor-gui [--theme dark|light|classic] [--tab overview|browsers|processes|transfer|logs|settings] [--browser <name>] [--open-dropdown toolbar-browser|toolbar-sort|transfer-source|transfer-target|settings-browser]"
                );
                std::process::exit(0);
            }
            _ => {}
        }
        i += 1;
    }
    GuiLaunchArgs {
        theme,
        tab,
        browser,
        open_dropdown,
    }
}

struct BrowserMonitorGui {
    collector: BrowserCollector,
    registry: BrowserRegistry,
    processes: Vec<ProcessDetail>,
    browsers: Vec<BrowserSummary>,
    components: Vec<ComponentSummary>,
    host: HostStats,
    history: HistoryBuffer,
    active_tab: ActiveTab,
    sort_col: GuiSortCol,
    sort_desc: bool,
    paused: bool,
    theme: ThemeMode,
    selected_browser: Option<BrowserKind>,
    role_filter: RoleFilter,
    log_filter: LogFilter,
    selected_pid: Option<u32>,
    confirm_kill_pid: Option<u32>,
    transfer_source: BrowserKind,
    transfer_target: BrowserKind,
    settings_browser: BrowserKind,
    open_dropdown: OpenDropdown,
    scanned_tabs: Vec<String>,
    disabled_tabs: HashSet<String>,
    refresh_interval_ms: u64,
    history_capacity: usize,
    show_host_gauge: bool,
    show_charts: bool,
    show_breakdown: bool,
    show_vsz: bool,
    status_notice: Option<String>,
}

impl BrowserMonitorGui {
    fn new(launch: GuiLaunchArgs, cx: &mut Context<Self>) -> Self {
        let mut collector = BrowserCollector::new();
        let registry = BrowserRegistry::load();
        std::thread::sleep(Duration::from_millis(120));
        let (processes, browsers, components, host) = collector.collect(SortBy::Memory);

        let transfer_source = browsers
            .first()
            .map(|b| b.browser)
            .unwrap_or(BrowserKind::Ladybird);
        let transfer_target = BrowserKind::all()
            .iter()
            .copied()
            .find(|&b| b != transfer_source && registry.get_path(b).is_some())
            .unwrap_or(BrowserKind::Chrome);
        let settings_browser = BrowserKind::all()
            .iter()
            .copied()
            .find(|&b| registry.get_path(b).is_some())
            .unwrap_or(BrowserKind::Chrome);

        let fallback_sites: Vec<String> = browsers
            .iter()
            .find(|b| b.browser == transfer_source)
            .map(|b| b.sites.clone())
            .unwrap_or_default();
        let scanned_tabs = collect_open_tabs(transfer_source, &fallback_sites);

        let history_capacity = 30;
        let mut history = HistoryBuffer::new(history_capacity);
        let ram_mb = processes
            .iter()
            .filter(|p| launch.browser.map_or(true, |sb| p.browser == sb))
            .map(|p| p.memory_bytes)
            .sum::<u64>() as f64
            / (1024.0 * 1024.0);
        let cpu_pct = processes
            .iter()
            .filter(|p| launch.browser.map_or(true, |sb| p.browser == sb))
            .map(|p| p.cpu_usage)
            .sum::<f32>();
        history.push(ram_mb, cpu_pct);

        cx.spawn(async move |this, cx| {
            let mut last_tick = Instant::now();
            loop {
                cx.background_executor().timer(Duration::from_millis(250)).await;
                let alive = this.update(cx, |app, cx| {
                    if !app.paused
                        && last_tick.elapsed() >= Duration::from_millis(app.refresh_interval_ms)
                    {
                        app.refresh_now(cx);
                        last_tick = Instant::now();
                    }
                });
                if alive.is_err() {
                    break;
                }
            }
        })
        .detach();

        Self {
            collector,
            registry,
            processes,
            browsers,
            components,
            host,
            history,
            active_tab: launch.tab,
            sort_col: GuiSortCol::Ram,
            sort_desc: true,
            paused: false,
            theme: launch.theme,
            selected_browser: launch.browser,
            role_filter: RoleFilter::All,
            log_filter: LogFilter::All,
            selected_pid: None,
            confirm_kill_pid: None,
            transfer_source,
            transfer_target,
            settings_browser,
            open_dropdown: launch.open_dropdown,
            scanned_tabs,
            disabled_tabs: HashSet::new(),
            refresh_interval_ms: 1000,
            history_capacity,
            show_host_gauge: true,
            show_charts: true,
            show_breakdown: true,
            show_vsz: true,
            status_notice: None,
        }
    }

    fn refresh_now(&mut self, cx: &mut Context<Self>) {
        let (processes, browsers, components, host) = self.collector.collect(SortBy::Memory);
        self.processes = processes;
        self.browsers = browsers;
        self.components = components;
        self.host = host;

        let filtered = self.filtered_processes();
        let ram_mb = filtered.iter().map(|p| p.memory_bytes).sum::<u64>() as f64 / (1024.0 * 1024.0);
        let cpu_pct = filtered.iter().map(|p| p.cpu_usage).sum::<f32>();
        self.history.push(ram_mb, cpu_pct);

        if let Some(pid) = self.selected_pid {
            if !self.processes.iter().any(|p| p.pid == pid) {
                self.selected_pid = None;
                self.confirm_kill_pid = None;
            }
        }
        cx.notify();
    }

    fn scan_tabs_for(&mut self, src: BrowserKind, cx: &mut Context<Self>) {
        self.transfer_source = src;
        let fallback_sites: Vec<String> = self
            .browsers
            .iter()
            .find(|b| b.browser == src)
            .map(|b| b.sites.clone())
            .unwrap_or_default();
        self.scanned_tabs = collect_open_tabs(src, &fallback_sites);
        self.disabled_tabs.clear();
        self.status_notice = Some(format!(
            "Scanned {} open tab(s) in {}",
            self.scanned_tabs.len(),
            src.display_name()
        ));
        cx.notify();
    }

    fn prompt_and_set_browser_path(
        &mut self,
        browser: BrowserKind,
        pending_launch: Option<(Vec<String>, bool)>,
        cx: &mut Context<Self>,
    ) {
        let rx = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: true,
            multiple: false,
            prompt: Some(format!("Locate {} app or executable", browser.display_name()).into()),
        });
        self.status_notice = Some(format!(
            "Select install location for {}...",
            browser.display_name()
        ));
        cx.notify();

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await {
                if let Some(chosen) = paths.into_iter().next() {
                    let _ = this.update(cx, |app, cx| {
                        app.registry.set_custom_path(browser, chosen.clone());
                        app.collector.log.push(
                            LogLevel::Info,
                            browser,
                            format!("Saved custom install path: {}", chosen.display()),
                        );
                        if let Some((urls, with_tabs)) = pending_launch {
                            match open_in_browser(browser, &chosen, &urls, with_tabs) {
                                Ok(msg) => {
                                    app.collector.log.push(LogLevel::Info, browser, msg.clone());
                                    app.status_notice = Some(msg);
                                }
                                Err(err) => {
                                    app.collector.log.push(LogLevel::Alert, browser, err.clone());
                                    app.status_notice = Some(err);
                                }
                            }
                        } else {
                            app.status_notice = Some(format!(
                                "Saved {} path: {}",
                                browser.display_name(),
                                chosen.display()
                            ));
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn launch_target_browser(&mut self, with_tabs: bool, cx: &mut Context<Self>) {
        let target = self.transfer_target;
        let urls: Vec<String> = if with_tabs {
            self.scanned_tabs
                .iter()
                .filter(|u| !self.disabled_tabs.contains(*u))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        if let Some(path) = self.registry.get_path(target).cloned() {
            if path.exists() {
                match open_in_browser(target, &path, &urls, with_tabs) {
                    Ok(msg) => {
                        self.collector.log.push(LogLevel::Info, target, msg.clone());
                        self.status_notice = Some(msg);
                    }
                    Err(err) => {
                        self.collector.log.push(LogLevel::Alert, target, err.clone());
                        self.status_notice = Some(err);
                    }
                }
                cx.notify();
                return;
            }
        }

        // Missing install location -> ask user via native file picker then launch
        self.prompt_and_set_browser_path(target, Some((urls, with_tabs)), cx);
    }

    fn set_sort(&mut self, col: GuiSortCol, cx: &mut Context<Self>) {
        if self.sort_col == col {
            self.sort_desc = !self.sort_desc;
        } else {
            self.sort_col = col;
            self.sort_desc = matches!(
                col,
                GuiSortCol::Ram
                    | GuiSortCol::Cpu
                    | GuiSortCol::Vsz
                    | GuiSortCol::Procs
                    | GuiSortCol::Uptime
                    | GuiSortCol::Hidden
            );
        }
        cx.notify();
    }

    fn sorted_processes(&self) -> Vec<&ProcessDetail> {
        let mut list: Vec<&ProcessDetail> = self
            .processes
            .iter()
            .filter(|p| match self.selected_browser {
                Some(b) => p.browser == b,
                None => true,
            })
            .filter(|p| match self.role_filter {
                RoleFilter::All => true,
                RoleFilter::HiddenOnly => p.is_hidden,
                RoleFilter::Role(r) => p.role == r,
            })
            .collect();

        list.sort_by(|a, b| {
            let ord = match self.sort_col {
                GuiSortCol::Pid => a.pid.cmp(&b.pid),
                GuiSortCol::Browser => a.browser.short_name().cmp(b.browser.short_name()),
                GuiSortCol::Role => a.role.short_name().cmp(b.role.short_name()),
                GuiSortCol::Hidden => a.is_hidden.cmp(&b.is_hidden),
                GuiSortCol::Cpu => a
                    .cpu_usage
                    .partial_cmp(&b.cpu_usage)
                    .unwrap_or(Ordering::Equal),
                GuiSortCol::Ram | GuiSortCol::Procs => a.memory_bytes.cmp(&b.memory_bytes),
                GuiSortCol::Vsz => a.virtual_memory_bytes.cmp(&b.virtual_memory_bytes),
                GuiSortCol::Uptime => a.run_time_secs.cmp(&b.run_time_secs),
            };
            if self.sort_desc {
                ord.reverse()
            } else {
                ord
            }
        });
        list
    }

    fn filtered_processes(&self) -> Vec<&ProcessDetail> {
        self.processes
            .iter()
            .filter(|p| self.selected_browser.map_or(true, |b| p.browser == b))
            .collect()
    }

    fn filtered_components(&self) -> Vec<ComponentSummary> {
        let mut by_role: BTreeMap<ComponentRole, ComponentSummary> = BTreeMap::new();
        for p in self.filtered_processes() {
            let entry = by_role.entry(p.role).or_insert(ComponentSummary {
                role: p.role,
                count: 0,
                hidden_count: 0,
                total_memory_bytes: 0,
                total_virtual_memory_bytes: 0,
                total_cpu: 0.0,
            });
            entry.count += 1;
            if p.is_hidden {
                entry.hidden_count += 1;
            }
            entry.total_memory_bytes += p.memory_bytes;
            entry.total_virtual_memory_bytes += p.virtual_memory_bytes;
            entry.total_cpu += p.cpu_usage;
        }
        let mut list: Vec<ComponentSummary> = by_role.into_values().collect();
        list.sort_by(|a, b| b.total_memory_bytes.cmp(&a.total_memory_bytes));
        list
    }

    fn sorted_browsers(&self) -> Vec<&BrowserSummary> {
        let mut list: Vec<&BrowserSummary> = self
            .browsers
            .iter()
            .filter(|b| self.selected_browser.map_or(true, |sb| b.browser == sb))
            .collect();
        list.sort_by(|a, b| {
            let ord = match self.sort_col {
                GuiSortCol::Browser | GuiSortCol::Pid | GuiSortCol::Role | GuiSortCol::Uptime => {
                    a.browser.short_name().cmp(b.browser.short_name())
                }
                GuiSortCol::Hidden => a.hidden_count.cmp(&b.hidden_count),
                GuiSortCol::Procs => a.count.cmp(&b.count),
                GuiSortCol::Cpu => a
                    .total_cpu
                    .partial_cmp(&b.total_cpu)
                    .unwrap_or(Ordering::Equal),
                GuiSortCol::Ram => a.total_memory_bytes.cmp(&b.total_memory_bytes),
                GuiSortCol::Vsz => a
                    .total_virtual_memory_bytes
                    .cmp(&b.total_virtual_memory_bytes),
            };
            if self.sort_desc {
                ord.reverse()
            } else {
                ord
            }
        });
        list
    }

    fn toggle_dropdown(&mut self, dd: OpenDropdown, cx: &mut Context<Self>) {
        self.open_dropdown = if self.open_dropdown == dd {
            OpenDropdown::None
        } else {
            dd
        };
        cx.notify();
    }

    fn set_selected_browser(&mut self, b: Option<BrowserKind>, cx: &mut Context<Self>) {
        self.selected_browser = b;
        self.selected_pid = None;
        self.confirm_kill_pid = None;
        self.open_dropdown = OpenDropdown::None;
        self.status_notice = None;
        self.history = HistoryBuffer::new(self.history_capacity);
        let filtered = self.filtered_processes();
        let ram_mb = filtered.iter().map(|p| p.memory_bytes).sum::<u64>() as f64 / (1024.0 * 1024.0);
        let cpu_pct = filtered.iter().map(|p| p.cpu_usage).sum::<f32>();
        self.history.push(ram_mb, cpu_pct);
        cx.notify();
    }

    fn select_browser_and_open(&mut self, b: Option<BrowserKind>, cx: &mut Context<Self>) {
        self.set_selected_browser(b, cx);
        if b.is_some() {
            self.active_tab = ActiveTab::Processes;
        }
        cx.notify();
    }

    fn reset_defaults(&mut self, _: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.theme = ThemeMode::Dark;
        self.refresh_interval_ms = 1000;
        self.history_capacity = 30;
        self.history.set_max_samples(30);
        self.show_host_gauge = true;
        self.show_charts = true;
        self.show_breakdown = true;
        self.show_vsz = true;
        self.selected_browser = None;
        self.role_filter = RoleFilter::All;
        self.log_filter = LogFilter::All;
        self.selected_pid = None;
        self.confirm_kill_pid = None;
        self.open_dropdown = OpenDropdown::None;
        self.status_notice = Some("Settings reset to defaults".into());
        cx.notify();
    }

    fn render_button(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        active: bool,
        theme: ThemeMode,
        p: &ThemePalette,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let bg = if active { p.bg_button_active } else { p.bg_button };
        let border = if active { p.accent_cyan } else { p.border_strong };
        let hover_bg = p.border;
        let text_col = active_text_color(theme, active, p);
        div()
            .id(id)
            .px_2p5()
            .py_1()
            .rounded_md()
            .bg(rgb(bg))
            .border_1()
            .border_color(rgb(border))
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(text_col))
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(hover_bg)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(label.into())
    }

    fn render_dropdown_trigger(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        is_open: bool,
        min_width_px: f32,
        theme: ThemeMode,
        p: &ThemePalette,
        on_click: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    ) -> impl IntoElement {
        let bg = if is_open { p.bg_button_active } else { p.bg_button };
        let border = if is_open { p.accent_cyan } else { p.border_strong };
        let hover_bg = p.border;
        let text_col = active_text_color(theme, is_open, p);
        let arrow = if is_open { "▲" } else { "▾" };
        div()
            .id(id)
            .min_w(px(min_width_px))
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .gap_2()
            .px_2p5()
            .py_1p5()
            .rounded_md()
            .bg(rgb(bg))
            .border_1()
            .border_color(rgb(border))
            .text_xs()
            .font_weight(FontWeight::MEDIUM)
            .text_color(rgb(text_col))
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(hover_bg)))
            .on_mouse_down(MouseButton::Left, on_click)
            .child(label.into())
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(if is_open { p.accent_cyan } else { p.text_muted }))
                    .child(arrow),
            )
    }

    fn render_browser_dropdown_menu(
        &self,
        menu_id: &'static str,
        include_all: bool,
        selected: Option<BrowserKind>,
        show_path_status: bool,
        width_px: f32,
        p: &ThemePalette,
        on_pick: impl Fn(&mut Self, Option<BrowserKind>, &mut Context<Self>) + Clone + 'static,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let hover_bg = p.bg_row_alt;
        let mut menu = div()
            .id(menu_id)
            .w(px(width_px))
            .h(px(250.0))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_0p5()
            .p_1p5()
            .rounded_md()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.accent_cyan));

        if include_all {
            let active = selected.is_none();
            let bg = if active { p.bg_button_active } else { p.bg_panel };
            let text_col = active_text_color(self.theme, active, p);
            let picker = on_pick.clone();
            menu = menu.child(
                div()
                    .id((menu_id, 999usize))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(bg))
                    .text_xs()
                    .font_weight(if active { FontWeight::BOLD } else { FontWeight::NORMAL })
                    .text_color(rgb(text_col))
                    .cursor_pointer()
                    .hover(move |s| s.bg(rgb(hover_bg)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |app, _, _, cx| {
                            picker(app, None, cx);
                        }),
                    )
                    .child("All Browsers")
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(p.text_muted))
                            .child(format!("{} procs", self.processes.len())),
                    ),
            );
        }

        for (idx, &bk) in BrowserKind::all().iter().enumerate() {
            let active = selected == Some(bk);
            let bg = if active { p.bg_button_active } else { p.bg_panel };
            let text_col = active_text_color(self.theme, active, p);
            let dot_col = browser_color(bk, p);
            let picker = on_pick.clone();

            let right_badge = if show_path_status {
                if self.registry.get_path(bk).is_some() {
                    if self.registry.is_custom(bk) {
                        ("Custom", p.accent_cyan)
                    } else {
                        ("Auto", p.accent_green)
                    }
                } else {
                    ("Missing", p.accent_amber)
                }
            } else {
                let cnt = self
                    .browsers
                    .iter()
                    .find(|b| b.browser == bk)
                    .map(|b| b.count)
                    .unwrap_or(0);
                if cnt > 0 {
                    ("Running", p.accent_green)
                } else {
                    ("Idle", p.text_muted)
                }
            };

            let right_label = if !show_path_status {
                let cnt = self
                    .browsers
                    .iter()
                    .find(|b| b.browser == bk)
                    .map(|b| b.count)
                    .unwrap_or(0);
                if cnt > 0 {
                    format!("● {cnt} procs")
                } else {
                    "Idle".to_string()
                }
            } else {
                right_badge.0.to_string()
            };

            menu = menu.child(
                div()
                    .id((menu_id, idx))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(bg))
                    .text_xs()
                    .font_weight(if active { FontWeight::BOLD } else { FontWeight::NORMAL })
                    .text_color(rgb(text_col))
                    .cursor_pointer()
                    .hover(move |s| s.bg(rgb(hover_bg)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |app, _, _, cx| {
                            picker(app, Some(bk), cx);
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .child(div().size(px(8.0)).rounded_full().bg(rgb(dot_col)))
                            .child(bk.display_name()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(if active && self.theme == ThemeMode::Classic {
                                0xffffff
                            } else {
                                right_badge.1
                            }))
                            .child(right_label),
                    ),
            );
        }
        menu
    }

    fn render_sort_header(
        &self,
        id: &'static str,
        label: &'static str,
        col: GuiSortCol,
        width_px: Option<f32>,
        p: &ThemePalette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.sort_col == col;
        let arrow = if is_active {
            if self.sort_desc { " ▼" } else { " ▲" }
        } else {
            ""
        };
        let text_col = if is_active { p.accent_cyan } else { p.text_muted };
        let hover_bg = p.bg_row_alt;

        let mut el = div()
            .id(id)
            .py_1()
            .px_1()
            .rounded_sm()
            .text_xs()
            .font_weight(FontWeight::BOLD)
            .text_color(rgb(text_col))
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(hover_bg)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |app, _, _, cx| {
                    app.set_sort(col, cx);
                }),
            )
            .child(format!("{label}{arrow}"));

        if let Some(w) = width_px {
            el = el.w(px(w));
        } else {
            el = el.flex_1();
        }
        el
    }

    fn render_kpi_card(
        title: impl Into<SharedString>,
        value: String,
        subtitle: String,
        accent: u32,
        p: &ThemePalette,
    ) -> impl IntoElement {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap_1()
            .p_3()
            .rounded_lg()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.border))
            .child(div().text_xs().text_color(rgb(p.text_muted)).child(title.into()))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(accent))
                    .child(value),
            )
            .child(div().text_xs().text_color(rgb(p.text_muted)).child(subtitle))
    }

    fn render_spark_chart(
        title: impl Into<SharedString>,
        samples: &[f64],
        max_bars: usize,
        unit: &'static str,
        color: u32,
        p: &ThemePalette,
    ) -> impl IntoElement {
        let min_v = samples.iter().copied().fold(f64::INFINITY, f64::min);
        let max_v = samples.iter().copied().fold(0.0_f64, f64::max);
        let cur_v = samples.last().copied().unwrap_or(0.0);
        let range = (max_v - min_v).max(0.1);
        let pad_count = max_bars.saturating_sub(samples.len());

        let mut bars_row = div()
            .flex()
            .flex_row()
            .items_end()
            .gap_1()
            .h(px(46.0))
            .pt_1();

        for _ in 0..pad_count {
            bars_row = bars_row.child(div().flex_1().h(px(3.0)).rounded_sm().bg(rgb(p.border)));
        }

        for &v in samples.iter().rev().take(max_bars).collect::<Vec<_>>().into_iter().rev() {
            let norm = if (max_v - min_v).abs() < f64::EPSILON {
                0.5
            } else {
                ((v - min_v) / range).clamp(0.08, 1.0)
            };
            let h_px = (norm * 42.0).max(4.0) as f32;
            bars_row = bars_row.child(div().flex_1().h(px(h_px)).rounded_sm().bg(rgb(color)));
        }

        let summary = if samples.is_empty() {
            "No samples yet".to_string()
        } else {
            format!(
                "Cur: {:.1}{}  |  Min: {:.1}{}  |  Max: {:.1}{}",
                cur_v, unit, min_v, unit, max_v, unit
            )
        };

        div()
            .flex_1()
            .flex()
            .flex_col()
            .gap_1()
            .p_3()
            .rounded_lg()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(p.text_primary))
                            .child(title.into()),
                    )
                    .child(div().text_xs().text_color(rgb(p.text_muted)).child(summary)),
            )
            .child(bars_row)
    }

    fn render_sidebar(&self, p: &ThemePalette, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered_proc_count = self.filtered_processes().len();
        let status_label = if self.paused {
            "PAUSED".to_string()
        } else {
            format!("LIVE {:.1}s", self.refresh_interval_ms as f64 / 1000.0)
        };
        let status_color = if self.paused { p.accent_amber } else { p.accent_green };
        let hover_bg = p.bg_row_alt;

        let mut nav_group = div().flex().flex_col().gap_1().px_2();
        for tab in [
            ActiveTab::Overview,
            ActiveTab::Browsers,
            ActiveTab::Processes,
            ActiveTab::Transfer,
            ActiveTab::Logs,
            ActiveTab::Settings,
        ] {
            let active = self.active_tab == tab;
            let bg = if active { p.bg_button_active } else { p.bg_panel };
            let border = if active { p.accent_cyan } else { p.bg_panel };
            let text_col = active_text_color(self.theme, active, p);
            let badge_text = match tab {
                ActiveTab::Browsers => format!("{}", self.browsers.len()),
                ActiveTab::Processes => format!("{filtered_proc_count}"),
                ActiveTab::Transfer => format!("{}", self.scanned_tabs.len()),
                ActiveTab::Logs => format!("{}", self.collector.log.entries.len()),
                _ => String::new(),
            };

            nav_group = nav_group.child(
                div()
                    .id(SharedString::from(format!("nav-{}", tab.label())))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .px_2p5()
                    .py_1p5()
                    .rounded_md()
                    .bg(rgb(bg))
                    .border_1()
                    .border_color(rgb(border))
                    .text_xs()
                    .font_weight(if active { FontWeight::BOLD } else { FontWeight::MEDIUM })
                    .text_color(rgb(text_col))
                    .cursor_pointer()
                    .hover(move |s| s.bg(rgb(hover_bg)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |app, _, _, cx| {
                            app.active_tab = tab;
                            app.open_dropdown = OpenDropdown::None;
                            cx.notify();
                        }),
                    )
                    .child(tab.label())
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(if active && self.theme == ThemeMode::Classic {
                                0xffffff
                            } else {
                                p.text_muted
                            }))
                            .child(badge_text),
                    ),
            );
        }

        div()
            .w(px(230.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_2p5()
            .bg(rgb(p.bg_panel))
            .border_r_1()
            .border_color(rgb(p.border_strong))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .p_3()
                    .bg(rgb(p.bg_header))
                    .border_b_1()
                    .border_color(rgb(p.border_strong))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text_header))
                            .child("Web Browser Monitor"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .px_2()
                                    .py_0p5()
                                    .rounded_md()
                                    .bg(rgb(p.bg_panel))
                                    .border_1()
                                    .border_color(rgb(status_color))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(status_color))
                                    .child(status_label),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(p.text_header))
                                    .child("v0.1.0"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .px_3()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text_muted))
                            .child("VIEWS"),
                    )
                    .child(nav_group),
            )
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .p_2p5()
                    .border_t_1()
                    .border_color(rgb(p.border))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text_muted))
                            .child("THEME"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_1()
                            .child(Self::render_button(
                                "sb-theme-dark",
                                "Dark",
                                self.theme == ThemeMode::Dark,
                                self.theme,
                                p,
                                cx.listener(|app, _, _, cx| {
                                    app.theme = ThemeMode::Dark;
                                    cx.notify();
                                }),
                            ))
                            .child(Self::render_button(
                                "sb-theme-light",
                                "Light",
                                self.theme == ThemeMode::Light,
                                self.theme,
                                p,
                                cx.listener(|app, _, _, cx| {
                                    app.theme = ThemeMode::Light;
                                    cx.notify();
                                }),
                            ))
                            .child(Self::render_button(
                                "sb-theme-classic",
                                "Classic",
                                self.theme == ThemeMode::Classic,
                                self.theme,
                                p,
                                cx.listener(|app, _, _, cx| {
                                    app.theme = ThemeMode::Classic;
                                    cx.notify();
                                }),
                            )),
                    ),
            )
    }

    fn render_process_row(
        &self,
        id_prefix: &'static str,
        proc_: &ProcessDetail,
        idx: usize,
        show_vsz: bool,
        p: &ThemePalette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let hover_bg = p.border;
        let pid_val = proc_.pid;
        let detail = proc_
            .site
            .as_deref()
            .map(|s| s.trim_start_matches("https://").trim_start_matches("http://"))
            .unwrap_or(&proc_.name)
            .to_string();

        let is_sel = self.selected_pid == Some(pid_val);
        let row_bg = if is_sel {
            p.bg_button_active
        } else if idx % 2 == 1 {
            p.bg_row_alt
        } else {
            p.bg_panel
        };
        let text_col = active_text_color(self.theme, is_sel, p);
        let muted_col = if is_sel && self.theme == ThemeMode::Classic {
            0xffffff
        } else {
            p.text_muted
        };

        let mut row = div()
            .id((id_prefix, pid_val as usize))
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .py_1p5()
            .rounded_sm()
            .bg(rgb(row_bg))
            .text_xs()
            .text_color(rgb(text_col))
            .cursor_pointer()
            .hover(move |s| s.bg(rgb(hover_bg)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |app, _, _, cx| {
                    app.selected_pid = if app.selected_pid == Some(pid_val) {
                        None
                    } else {
                        Some(pid_val)
                    };
                    app.confirm_kill_pid = None;
                    app.status_notice = None;
                    cx.notify();
                }),
            )
            .child(div().w(px(64.0)).px_1().child(pid_val.to_string()))
            .child(
                div()
                    .w(px(98.0))
                    .px_1()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(rgb(if is_sel && self.theme == ThemeMode::Classic {
                        0xffffff
                    } else {
                        browser_color(proc_.browser, p)
                    }))
                    .child(proc_.browser.short_name()),
            )
            .child(
                div()
                    .w(px(115.0))
                    .px_1()
                    .text_color(rgb(if is_sel && self.theme == ThemeMode::Classic {
                        0xffffff
                    } else {
                        role_color(proc_.role, p)
                    }))
                    .child(proc_.role.short_name()),
            )
            .child(
                div()
                    .w(px(64.0))
                    .px_1()
                    .text_color(rgb(if proc_.is_hidden { p.accent_red } else { muted_col }))
                    .child(if proc_.is_hidden { "Hidden" } else { "-" }),
            )
            .child(
                div()
                    .flex_1()
                    .px_1()
                    .text_color(rgb(muted_col))
                    .overflow_hidden()
                    .child(detail),
            )
            .child(div().w(px(68.0)).px_1().child(format!("{:.1}%", proc_.cpu_usage)))
            .child(
                div()
                    .w(px(90.0))
                    .px_1()
                    .font_weight(FontWeight::MEDIUM)
                    .child(format_bytes(proc_.memory_bytes)),
            );
        if show_vsz {
            row = row.child(
                div()
                    .w(px(88.0))
                    .px_1()
                    .text_color(rgb(muted_col))
                    .child(format_bytes(proc_.virtual_memory_bytes)),
            );
        }
        row.child(
            div()
                .w(px(76.0))
                .px_1()
                .text_color(rgb(muted_col))
                .child(format_duration(proc_.run_time_secs)),
        )
    }

    fn render_overview(&self, p: &ThemePalette, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered_procs = self.filtered_processes();
        let filtered_comps = self.filtered_components();
        let total_mem: u64 = filtered_procs.iter().map(|pr| pr.memory_bytes).sum();
        let total_vsz: u64 = filtered_procs.iter().map(|pr| pr.virtual_memory_bytes).sum();
        let total_cpu: f32 = filtered_procs.iter().map(|pr| pr.cpu_usage).sum();
        let hidden_n = filtered_procs.iter().filter(|pr| pr.is_hidden).count();
        let hidden_mem: u64 = filtered_procs
            .iter()
            .filter(|pr| pr.is_hidden)
            .map(|pr| pr.memory_bytes)
            .sum();

        let host_total = self.host.total_memory_bytes.max(1);
        let pct_host = (total_mem as f64 / host_total as f64) * 100.0;
        let host_used_pct = (self.host.used_memory_bytes as f64 / host_total as f64) * 100.0;
        let other_used = self.host.used_memory_bytes.saturating_sub(total_mem);

        let b_frac = (total_mem as f64 / host_total as f64).clamp(0.0, 1.0) as f32;
        let other_frac =
            (other_used as f64 / host_total as f64).clamp(0.0, 1.0 - b_frac as f64) as f32;
        let free_frac = (1.0 - b_frac - other_frac).max(0.0);

        let ram_samples: Vec<f64> = self.history.ram_history.iter().copied().collect();
        let cpu_samples: Vec<f64> = self.history.cpu_history.iter().map(|&v| v as f64).collect();
        let hover_bg = p.bg_row_alt;

        let (ram_title, cpu_title, scope_label) = match self.selected_browser {
            Some(bk) => (
                format!("{} RAM", bk.short_name().to_uppercase()),
                format!("{} CPU", bk.short_name().to_uppercase()),
                bk.display_name().to_string(),
            ),
            None => (
                "ALL BROWSERS RAM".to_string(),
                "TOTAL BROWSER CPU".to_string(),
                "All Browsers".to_string(),
            ),
        };
        let hidden_sub = match self.selected_browser {
            Some(bk) => format!(
                "{} hidden in {} {} procs",
                format_bytes(hidden_mem),
                filtered_procs.len(),
                bk.short_name()
            ),
            None => format!(
                "{} hidden across {} procs",
                format_bytes(hidden_mem),
                filtered_procs.len()
            ),
        };

        let mut col = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2p5()
                    .child(Self::render_kpi_card(
                        ram_title,
                        format_bytes(total_mem),
                        format!("{:.1}% of host  |  VSZ {}", pct_host, format_bytes(total_vsz)),
                        self.selected_browser
                            .map(|bk| browser_color(bk, p))
                            .unwrap_or(p.accent_cyan),
                        p,
                    ))
                    .child(Self::render_kpi_card(
                        cpu_title,
                        format!("{:.1}%", total_cpu),
                        format!("{} host cores", self.host.cpu_count),
                        if total_cpu > 30.0 {
                            p.accent_red
                        } else {
                            p.accent_amber
                        },
                        p,
                    ))
                    .child(Self::render_kpi_card(
                        "HIDDEN SERVICES",
                        format!("{hidden_n} bg"),
                        hidden_sub,
                        p.accent_red,
                        p,
                    ))
                    .child(Self::render_kpi_card(
                        "HOST SYSTEM RAM",
                        format_bytes(self.host.used_memory_bytes),
                        format!("{:.1}% of {} total", host_used_pct, format_bytes(host_total)),
                        p.accent_blue,
                        p,
                    )),
            );

        if self.show_host_gauge {
            col = col.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1p5()
                    .p_3()
                    .rounded_lg()
                    .bg(rgb(p.bg_panel))
                    .border_1()
                    .border_color(rgb(p.border))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .h(px(12.0))
                            .w_full()
                            .rounded_sm()
                            .overflow_hidden()
                            .bg(rgb(p.bg_app))
                            .child(div().w(relative(b_frac)).h_full().bg(rgb(p.accent_cyan)))
                            .child(div().w(relative(other_frac)).h_full().bg(rgb(p.accent_blue)))
                            .child(div().w(relative(free_frac)).h_full().bg(rgb(p.border))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_4()
                            .text_xs()
                            .text_color(rgb(p.text_muted))
                            .child(format!("{scope_label}: {}", format_bytes(total_mem)))
                            .child(format!("Other Apps: {}", format_bytes(other_used)))
                            .child(format!(
                                "Free: {}",
                                format_bytes(host_total.saturating_sub(self.host.used_memory_bytes))
                            )),
                    ),
            );
        }

        if self.show_charts {
            col = col.child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2p5()
                    .child(Self::render_spark_chart(
                        if let Some(bk) = self.selected_browser {
                            format!("{} RAM TREND", bk.short_name().to_uppercase())
                        } else {
                            "RAM TREND".to_string()
                        },
                        &ram_samples,
                        self.history_capacity,
                        " MB",
                        p.accent_cyan,
                        p,
                    ))
                    .child(Self::render_spark_chart(
                        if let Some(bk) = self.selected_browser {
                            format!("{} CPU TREND", bk.short_name().to_uppercase())
                        } else {
                            "CPU TREND".to_string()
                        },
                        &cpu_samples,
                        self.history_capacity,
                        "%",
                        p.accent_amber,
                        p,
                    )),
            );
        }

        if self.show_breakdown {
            let mut browser_bars = div()
                .flex()
                .flex_col()
                .gap_1p5()
                .p_3()
                .rounded_lg()
                .bg(rgb(p.bg_panel))
                .border_1()
                .border_color(rgb(p.border))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .items_center()
                        .pb_1()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(p.text_primary))
                                .child(match self.selected_browser {
                                    Some(bk) => format!(
                                        "{} SUMMARY ({} procs, {} hidden)",
                                        bk.display_name().to_uppercase(),
                                        filtered_procs.len(),
                                        hidden_n
                                    ),
                                    None => {
                                        "BROWSER RAM BREAKDOWN (Click any browser or use top dropdown to filter)"
                                            .to_string()
                                    }
                                }),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_1p5()
                                .when(self.selected_browser.is_some(), |el| {
                                    el.child(Self::render_button(
                                        "ov-show-all-browsers",
                                        "Show All Browsers",
                                        false,
                                        self.theme,
                                        p,
                                        cx.listener(|app, _, _, cx| {
                                            app.set_selected_browser(None, cx);
                                        }),
                                    ))
                                })
                                .child(Self::render_button(
                                    "ov-transfer",
                                    "Transfer Tabs →",
                                    false,
                                    self.theme,
                                    p,
                                    cx.listener(|app, _, _, cx| {
                                        app.active_tab = ActiveTab::Transfer;
                                        cx.notify();
                                    }),
                                ))
                                .child(Self::render_button(
                                    "ov-hidden-procs",
                                    format!("Hidden ({hidden_n}) →"),
                                    false,
                                    self.theme,
                                    p,
                                    cx.listener(|app, _, _, cx| {
                                        app.role_filter = RoleFilter::HiddenOnly;
                                        app.active_tab = ActiveTab::Processes;
                                        cx.notify();
                                    }),
                                )),
                        ),
                );

            let shown_browsers: Vec<&BrowserSummary> = self.sorted_browsers();
            if shown_browsers.is_empty() {
                browser_bars = browser_bars.child(
                    div()
                        .p_2()
                        .text_xs()
                        .text_color(rgb(p.accent_amber))
                        .child(format!("No running processes detected for {scope_label}.")),
                );
            } else {
                for (idx, b) in shown_browsers.into_iter().enumerate() {
                    let bk = b.browser;
                    let pct = if total_mem > 0 {
                        (b.total_memory_bytes as f64 / total_mem as f64) * 100.0
                    } else {
                        0.0
                    };
                    let bar_frac = (pct / 100.0).clamp(0.01, 1.0) as f32;
                    let col_b = browser_color(bk, p);
                    let meta = if !b.has_main_ui {
                        format!("{} hidden bg · {:.1}% CPU", b.count, b.total_cpu)
                    } else if b.hidden_count > 0 {
                        format!("{} procs ({} hidden) · {:.1}% CPU", b.count, b.hidden_count, b.total_cpu)
                    } else {
                        format!("{} procs · {:.1}% CPU", b.count, b.total_cpu)
                    };

                    browser_bars = browser_bars.child(
                        div()
                            .id(("ov-bbar", idx))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_3()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(move |s| s.bg(rgb(hover_bg)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |app, _, _, cx| {
                                    app.set_selected_browser(Some(bk), cx);
                                }),
                            )
                            .child(
                                div()
                                    .w(px(165.0))
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(rgb(col_b))
                                    .child(bk.display_name()),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(11.0))
                                    .rounded_sm()
                                    .bg(rgb(p.bg_app))
                                    .overflow_hidden()
                                    .flex()
                                    .child(div().w(relative(bar_frac)).h_full().bg(rgb(col_b))),
                            )
                            .child(
                                div()
                                    .w(px(115.0))
                                    .text_xs()
                                    .text_color(rgb(p.text_primary))
                                    .child(format!("{} ({:.1}%)", format_bytes(b.total_memory_bytes), pct)),
                            )
                            .child(
                                div()
                                    .w(px(210.0))
                                    .text_xs()
                                    .text_color(rgb(p.text_muted))
                                    .overflow_hidden()
                                    .child(meta),
                            ),
                    );
                }
            }
            col = col.child(browser_bars);
        }

        // 1. Expanded ALL COMPONENTS BREAKDOWN (visible by default)
        let mut comp_panel = div()
            .flex()
            .flex_col()
            .gap_1p5()
            .p_3()
            .rounded_lg()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .pb_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text_primary))
                            .child(format!(
                                "ALL COMPONENTS BREAKDOWN — {} ({} roles across {} procs)",
                                scope_label.to_uppercase(),
                                filtered_comps.len(),
                                filtered_procs.len()
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(p.text_muted))
                            .child("Click any component row to filter Processes"),
                    ),
            );

        if filtered_comps.is_empty() {
            comp_panel = comp_panel.child(
                div()
                    .p_2()
                    .text_xs()
                    .text_color(rgb(p.accent_amber))
                    .child("No active components for this selection."),
            );
        } else {
            for (idx, comp) in filtered_comps.iter().enumerate() {
                let role = comp.role;
                let rc = role_color(role, p);
                let pct = if total_mem > 0 {
                    (comp.total_memory_bytes as f64 / total_mem as f64) * 100.0
                } else {
                    0.0
                };
                let bar_frac = (pct / 100.0).clamp(0.01, 1.0) as f32;
                let proc_info = if comp.hidden_count > 0 {
                    format!(
                        "{} procs ({} hidden) · {:.1}% CPU · VSZ {}",
                        comp.count,
                        comp.hidden_count,
                        comp.total_cpu,
                        format_bytes(comp.total_virtual_memory_bytes)
                    )
                } else {
                    format!(
                        "{} procs · {:.1}% CPU · VSZ {}",
                        comp.count,
                        comp.total_cpu,
                        format_bytes(comp.total_virtual_memory_bytes)
                    )
                };

                comp_panel = comp_panel.child(
                    div()
                        .id(("ov-comp-row", idx))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2p5()
                        .px_2()
                        .py_1()
                        .rounded_md()
                        .cursor_pointer()
                        .hover(move |s| s.bg(rgb(hover_bg)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |app, _, _, cx| {
                                app.role_filter = RoleFilter::Role(role);
                                app.active_tab = ActiveTab::Processes;
                                cx.notify();
                            }),
                        )
                        .child(div().size(px(8.0)).rounded_full().bg(rgb(rc)))
                        .child(
                            div()
                                .w(px(165.0))
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(rc))
                                .child(role.display_name()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .h(px(10.0))
                                .rounded_sm()
                                .bg(rgb(p.bg_app))
                                .overflow_hidden()
                                .flex()
                                .child(div().w(relative(bar_frac)).h_full().bg(rgb(rc))),
                        )
                        .child(
                            div()
                                .w(px(115.0))
                                .text_xs()
                                .text_color(rgb(p.text_primary))
                                .child(format!("{} ({:.1}%)", format_bytes(comp.total_memory_bytes), pct)),
                        )
                        .child(
                            div()
                                .w(px(235.0))
                                .text_xs()
                                .text_color(rgb(p.text_muted))
                                .overflow_hidden()
                                .child(proc_info),
                        ),
                );
            }
        }
        col = col.child(comp_panel);

        // 2. Expanded ACTIVE & HIDDEN COMPONENT PROCESSES directly on Overview (visible by default)
        let mut sorted_ov_procs = filtered_procs;
        sorted_ov_procs.sort_by(|a, b| {
            let ord = match self.sort_col {
                GuiSortCol::Pid => a.pid.cmp(&b.pid),
                GuiSortCol::Browser => a.browser.short_name().cmp(b.browser.short_name()),
                GuiSortCol::Role => a.role.short_name().cmp(b.role.short_name()),
                GuiSortCol::Hidden => a.is_hidden.cmp(&b.is_hidden),
                GuiSortCol::Cpu => a
                    .cpu_usage
                    .partial_cmp(&b.cpu_usage)
                    .unwrap_or(Ordering::Equal),
                GuiSortCol::Ram | GuiSortCol::Procs => a.memory_bytes.cmp(&b.memory_bytes),
                GuiSortCol::Vsz => a.virtual_memory_bytes.cmp(&b.virtual_memory_bytes),
                GuiSortCol::Uptime => a.run_time_secs.cmp(&b.run_time_secs),
            };
            if self.sort_desc {
                ord.reverse()
            } else {
                ord
            }
        });

        let proc_hdr = div()
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(rgb(p.border_strong))
            .child(self.render_sort_header("ovhdr-pid", "PID", GuiSortCol::Pid, Some(64.0), p, cx))
            .child(self.render_sort_header("ovhdr-br", "Browser", GuiSortCol::Browser, Some(98.0), p, cx))
            .child(self.render_sort_header("ovhdr-role", "Role", GuiSortCol::Role, Some(115.0), p, cx))
            .child(self.render_sort_header("ovhdr-hid", "Hidden", GuiSortCol::Hidden, Some(64.0), p, cx))
            .child(div().flex_1().px_1().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(p.text_muted)).child("Site / Process Name"))
            .child(self.render_sort_header("ovhdr-cpu", "CPU%", GuiSortCol::Cpu, Some(68.0), p, cx))
            .child(self.render_sort_header("ovhdr-ram", "RAM", GuiSortCol::Ram, Some(90.0), p, cx))
            .child(self.render_sort_header("ovhdr-up", "Uptime", GuiSortCol::Uptime, Some(76.0), p, cx));

        let mut proc_panel = div()
            .flex()
            .flex_col()
            .p_3()
            .rounded_lg()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .pb_1p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text_primary))
                            .child(format!(
                                "ACTIVE & HIDDEN COMPONENT PROCESSES — {} ({} shown)",
                                scope_label.to_uppercase(),
                                sorted_ov_procs.len()
                            )),
                    )
                    .child(Self::render_button(
                        "ov-open-procs-tab",
                        "Full Filter View →",
                        false,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.active_tab = ActiveTab::Processes;
                            cx.notify();
                        }),
                    )),
            )
            .child(proc_hdr);

        if sorted_ov_procs.is_empty() {
            proc_panel = proc_panel.child(
                div()
                    .p_3()
                    .text_xs()
                    .text_color(rgb(p.accent_amber))
                    .child("No active browser processes matching selection."),
            );
        } else {
            for (idx, proc_) in sorted_ov_procs.into_iter().enumerate() {
                proc_panel = proc_panel.child(self.render_process_row("ov-proc", proc_, idx, false, p, cx));
            }
        }
        col = col.child(proc_panel);

        col
    }

    fn render_browsers_tab(&self, p: &ThemePalette, cx: &mut Context<Self>) -> impl IntoElement {
        let total_mem: u64 = self.processes.iter().map(|pr| pr.memory_bytes).sum();
        let browsers = self.sorted_browsers();
        let hover_bg = p.border;

        let mut hdr = div()
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(rgb(p.border_strong))
            .child(self.render_sort_header("bhdr-b", "Browser", GuiSortCol::Browser, Some(165.0), p, cx))
            .child(self.render_sort_header("bhdr-p", "Procs", GuiSortCol::Procs, Some(68.0), p, cx))
            .child(self.render_sort_header("bhdr-h", "Hidden", GuiSortCol::Hidden, Some(72.0), p, cx))
            .child(self.render_sort_header("bhdr-r", "RAM (RSS)", GuiSortCol::Ram, Some(100.0), p, cx))
            .child(div().w(px(72.0)).px_1().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(p.text_muted)).child("% All"));
        if self.show_vsz {
            hdr = hdr.child(self.render_sort_header("bhdr-v", "VSZ", GuiSortCol::Vsz, Some(98.0), p, cx));
        }
        hdr = hdr
            .child(self.render_sort_header("bhdr-c", "CPU%", GuiSortCol::Cpu, Some(74.0), p, cx))
            .child(div().flex_1().px_1().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(p.text_muted)).child("Window State / Sites"));

        let mut table = div()
            .flex()
            .flex_col()
            .p_3()
            .rounded_lg()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.border))
            .child(hdr);

        for (idx, b) in browsers.into_iter().enumerate() {
            let bk = b.browser;
            let pct = if total_mem > 0 {
                (b.total_memory_bytes as f64 / total_mem as f64) * 100.0
            } else {
                0.0
            };
            let state_info = if !b.has_main_ui {
                "Background / Hidden Only (no main window)".to_string()
            } else if b.sites.is_empty() {
                "Active Browser UI".to_string()
            } else {
                b.sites
                    .iter()
                    .map(|s| s.trim_start_matches("https://").trim_start_matches("http://"))
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let row_bg = if idx % 2 == 1 { p.bg_row_alt } else { p.bg_panel };

            let mut row = div()
                .id(("brw-row", idx))
                .flex()
                .flex_row()
                .items_center()
                .px_2()
                .py_1p5()
                .rounded_sm()
                .bg(rgb(row_bg))
                .text_xs()
                .text_color(rgb(p.text_primary))
                .cursor_pointer()
                .hover(move |s| s.bg(rgb(hover_bg)))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |app, _, _, cx| {
                        app.role_filter = RoleFilter::All;
                        app.select_browser_and_open(Some(bk), cx);
                    }),
                )
                .child(
                    div()
                        .w(px(165.0))
                        .px_1()
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(browser_color(bk, p)))
                        .child(bk.display_name()),
                )
                .child(div().w(px(68.0)).px_1().child(b.count.to_string()))
                .child(
                    div()
                        .w(px(72.0))
                        .px_1()
                        .text_color(rgb(if b.hidden_count > 0 { p.accent_red } else { p.text_muted }))
                        .child(b.hidden_count.to_string()),
                )
                .child(div().w(px(100.0)).px_1().font_weight(FontWeight::MEDIUM).child(format_bytes(b.total_memory_bytes)))
                .child(div().w(px(72.0)).px_1().child(format!("{:.1}%", pct)));
            if self.show_vsz {
                row = row.child(
                    div()
                        .w(px(98.0))
                        .px_1()
                        .text_color(rgb(p.text_muted))
                        .child(format_bytes(b.total_virtual_memory_bytes)),
                );
            }
            row = row
                .child(div().w(px(74.0)).px_1().child(format!("{:.1}%", b.total_cpu)))
                .child(
                    div()
                        .flex_1()
                        .px_1()
                        .text_color(rgb(p.text_muted))
                        .overflow_hidden()
                        .child(state_info),
                );
            table = table.child(row);
        }
        table
    }

    fn render_processes_tab(&self, p: &ThemePalette, cx: &mut Context<Self>) -> impl IntoElement {
        let procs = self.sorted_processes();

        let role_bar = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_1p5()
            .pb_2()
            .child(div().text_xs().text_color(rgb(p.text_muted)).child("Role:"))
            .child(Self::render_button(
                "rf-all",
                "All",
                self.role_filter == RoleFilter::All,
                self.theme,
                p,
                cx.listener(|app, _, _, cx| {
                    app.role_filter = RoleFilter::All;
                    cx.notify();
                }),
            ))
            .child(Self::render_button(
                "rf-hidden",
                "Hidden Services",
                self.role_filter == RoleFilter::HiddenOnly,
                self.theme,
                p,
                cx.listener(|app, _, _, cx| {
                    app.role_filter = RoleFilter::HiddenOnly;
                    cx.notify();
                }),
            ))
            .children(
                [
                    ComponentRole::WebContent,
                    ComponentRole::Network,
                    ComponentRole::GpuCompositor,
                    ComponentRole::MediaAudio,
                    ComponentRole::MainUI,
                    ComponentRole::BrokerIpc,
                ]
                .into_iter()
                .map(|r| {
                    Self::render_button(
                        SharedString::from(format!("rf-{}", r.short_name())),
                        r.short_name(),
                        self.role_filter == RoleFilter::Role(r),
                        self.theme,
                        p,
                        cx.listener(move |app, _, _, cx| {
                            app.role_filter = RoleFilter::Role(r);
                            cx.notify();
                        }),
                    )
                }),
            );

        let mut hdr = div()
            .flex()
            .flex_row()
            .items_center()
            .px_2()
            .py_1()
            .border_b_1()
            .border_color(rgb(p.border_strong))
            .child(self.render_sort_header("hdr-pid", "PID", GuiSortCol::Pid, Some(64.0), p, cx))
            .child(self.render_sort_header("hdr-brw", "Browser", GuiSortCol::Browser, Some(98.0), p, cx))
            .child(self.render_sort_header("hdr-role", "Role", GuiSortCol::Role, Some(115.0), p, cx))
            .child(self.render_sort_header("hdr-hd", "Hidden", GuiSortCol::Hidden, Some(64.0), p, cx))
            .child(div().flex_1().px_1().text_xs().font_weight(FontWeight::BOLD).text_color(rgb(p.text_muted)).child("Site / Process Name"))
            .child(self.render_sort_header("hdr-cpu", "CPU%", GuiSortCol::Cpu, Some(68.0), p, cx))
            .child(self.render_sort_header("hdr-ram", "RAM", GuiSortCol::Ram, Some(90.0), p, cx));
        if self.show_vsz {
            hdr = hdr.child(self.render_sort_header("hdr-vsz", "VSZ", GuiSortCol::Vsz, Some(88.0), p, cx));
        }
        hdr = hdr.child(self.render_sort_header("hdr-up", "Uptime", GuiSortCol::Uptime, Some(76.0), p, cx));

        let mut table = div()
            .flex()
            .flex_col()
            .p_3()
            .rounded_lg()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.border))
            .child(role_bar)
            .child(hdr);

        if procs.is_empty() {
            table = table.child(
                div()
                    .p_4()
                    .text_sm()
                    .text_color(rgb(p.accent_amber))
                    .child("No matching browser processes."),
            );
        } else {
            for (idx, proc_) in procs.into_iter().enumerate() {
                table = table.child(self.render_process_row("proc-row", proc_, idx, self.show_vsz, p, cx));
            }
        }
        table
    }

    fn render_transfer_tab(&self, p: &ThemePalette, cx: &mut Context<Self>) -> impl IntoElement {
        let hover_bg = p.border;
        let selected_count = self
            .scanned_tabs
            .iter()
            .filter(|u| !self.disabled_tabs.contains(*u))
            .count();

        let src_running = self
            .browsers
            .iter()
            .find(|b| b.browser == self.transfer_source)
            .map(|b| b.count)
            .unwrap_or(0);
        let src_label = if src_running > 0 {
            format!("● {}", self.transfer_source.display_name())
        } else {
            self.transfer_source.display_name().to_string()
        };

        let mut source_card = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_lg()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text_primary))
                            .child("1. Source Browser & Open Tabs"),
                    )
                    .child(Self::render_button(
                        "tr-scan",
                        format!("Scan {} Tabs", self.transfer_source.short_name()),
                        false,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            let src = app.transfer_source;
                            app.open_dropdown = OpenDropdown::None;
                            app.scan_tabs_for(src, cx);
                        }),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(p.text_muted))
                            .child("Source Browser:"),
                    )
                    .child(Self::render_dropdown_trigger(
                        "tr-src-dropdown",
                        src_label,
                        self.open_dropdown == OpenDropdown::TransferSource,
                        250.0,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.toggle_dropdown(OpenDropdown::TransferSource, cx);
                        }),
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(if src_running > 0 {
                                p.accent_green
                            } else {
                                p.text_muted
                            }))
                            .child(if src_running > 0 {
                                format!("{src_running} active process(es)")
                            } else {
                                "Not currently running".to_string()
                            }),
                    ),
            );

        if self.open_dropdown == OpenDropdown::TransferSource {
            source_card = source_card.child(self.render_browser_dropdown_menu(
                "tr-src-menu",
                false,
                Some(self.transfer_source),
                false,
                320.0,
                p,
                |app, opt_bk, cx| {
                    app.open_dropdown = OpenDropdown::None;
                    if let Some(bk) = opt_bk {
                        app.scan_tabs_for(bk, cx);
                    }
                },
                cx,
            ));
        }

        let mut tabs_card = div()
            .flex()
            .flex_col()
            .gap_1p5()
            .p_3()
            .rounded_lg()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text_primary))
                            .child(format!(
                                "DETECTED OPEN TABS FROM {} ({} selected of {})",
                                self.transfer_source.display_name().to_uppercase(),
                                selected_count,
                                self.scanned_tabs.len()
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1p5()
                            .child(Self::render_button(
                                "tr-sel-all",
                                "Select All",
                                false,
                                self.theme,
                                p,
                                cx.listener(|app, _, _, cx| {
                                    app.disabled_tabs.clear();
                                    cx.notify();
                                }),
                            ))
                            .child(Self::render_button(
                                "tr-sel-none",
                                "Clear",
                                false,
                                self.theme,
                                p,
                                cx.listener(|app, _, _, cx| {
                                    for u in &app.scanned_tabs {
                                        app.disabled_tabs.insert(u.clone());
                                    }
                                    cx.notify();
                                }),
                            )),
                    ),
            );

        if self.scanned_tabs.is_empty() {
            tabs_card = tabs_card.child(
                div()
                    .p_2()
                    .text_xs()
                    .text_color(rgb(p.text_muted))
                    .child("No active http/https tab URLs detected for this browser (you can still use 'Open New Tab WITHOUT Current Tabs' below)."),
            );
        } else {
            for (idx, url) in self.scanned_tabs.iter().enumerate() {
                let enabled = !self.disabled_tabs.contains(url);
                let url_owned = url.clone();
                let row_bg = if idx % 2 == 1 { p.bg_row_alt } else { p.bg_panel };
                tabs_card = tabs_card.child(
                    div()
                        .id(("tr-url", idx))
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .bg(rgb(row_bg))
                        .text_xs()
                        .cursor_pointer()
                        .hover(move |s| s.bg(rgb(hover_bg)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |app, _, _, cx| {
                                if app.disabled_tabs.contains(&url_owned) {
                                    app.disabled_tabs.remove(&url_owned);
                                } else {
                                    app.disabled_tabs.insert(url_owned.clone());
                                }
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(if enabled { p.accent_green } else { p.text_muted }))
                                .child(if enabled { "✓" } else { "○" }),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_color(rgb(if enabled { p.text_primary } else { p.text_muted }))
                                .overflow_hidden()
                                .child(url.clone()),
                        ),
                );
            }
        }

        let target_installed = self.registry.get_path(self.transfer_target).is_some();
        let target_dd_label = if target_installed {
            format!("✓ {}", self.transfer_target.display_name())
        } else {
            format!("⚠ {}", self.transfer_target.display_name())
        };
        let target_path_info = self
            .registry
            .get_path(self.transfer_target)
            .map(|pt| pt.display().to_string())
            .unwrap_or_else(|| "Not auto-detected — clicking Open will prompt you to locate it".into());

        let mut target_card = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_lg()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text_primary))
                            .child("2. Target Browser & Install Location"),
                    )
                    .child(Self::render_button(
                        "tr-locate-target",
                        "Locate / Change Path...",
                        false,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            let t = app.transfer_target;
                            app.prompt_and_set_browser_path(t, None, cx);
                        }),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(rgb(p.text_muted))
                            .child("Target Browser:"),
                    )
                    .child(Self::render_dropdown_trigger(
                        "tr-dst-dropdown",
                        target_dd_label,
                        self.open_dropdown == OpenDropdown::TransferTarget,
                        250.0,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.toggle_dropdown(OpenDropdown::TransferTarget, cx);
                        }),
                    ))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(if target_installed {
                                p.accent_green
                            } else {
                                p.accent_amber
                            }))
                            .child(if target_installed {
                                "Installed"
                            } else {
                                "Missing — will prompt for path"
                            }),
                    ),
            );

        if self.open_dropdown == OpenDropdown::TransferTarget {
            target_card = target_card.child(self.render_browser_dropdown_menu(
                "tr-dst-menu",
                false,
                Some(self.transfer_target),
                true,
                320.0,
                p,
                |app, opt_bk, cx| {
                    if let Some(bk) = opt_bk {
                        app.transfer_target = bk;
                    }
                    app.open_dropdown = OpenDropdown::None;
                    cx.notify();
                },
                cx,
            ));
        }

        target_card = target_card
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(if target_installed { p.text_muted } else { p.accent_amber }))
                    .child(format!(
                        "{} path: {}",
                        self.transfer_target.display_name(),
                        target_path_info
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2p5()
                    .pt_1()
                    .child(Self::render_button(
                        "tr-open-with-tabs",
                        format!(
                            "Open {} WITH Current Tabs ({selected_count})",
                            self.transfer_target.short_name()
                        ),
                        true,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.launch_target_browser(true, cx);
                        }),
                    ))
                    .child(Self::render_button(
                        "tr-open-new-tab",
                        format!(
                            "Open New Tab in {} (WITHOUT Current Tabs)",
                            self.transfer_target.short_name()
                        ),
                        false,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.launch_target_browser(false, cx);
                        }),
                    )),
            );

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(source_card)
            .child(tabs_card)
            .child(target_card)
    }

    fn render_logs_tab(&self, p: &ThemePalette, cx: &mut Context<Self>) -> impl IntoElement {
        let entries: Vec<_> = self
            .collector
            .log
            .entries
            .iter()
            .rev()
            .filter(|e| self.selected_browser.map_or(true, |sb| e.browser == sb))
            .filter(|e| match self.log_filter {
                LogFilter::All => true,
                LogFilter::SpawnsExits => matches!(e.level, LogLevel::Spawn | LogLevel::Exit),
                LogFilter::Hidden => e.level == LogLevel::Hidden,
                LogFilter::Alerts => e.level == LogLevel::Alert,
            })
            .collect();

        let controls = div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .pb_2()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1p5()
                    .child(Self::render_button(
                        "lf-all",
                        "All Events",
                        self.log_filter == LogFilter::All,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.log_filter = LogFilter::All;
                            cx.notify();
                        }),
                    ))
                    .child(Self::render_button(
                        "lf-hidden",
                        "Hidden Services",
                        self.log_filter == LogFilter::Hidden,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.log_filter = LogFilter::Hidden;
                            cx.notify();
                        }),
                    ))
                    .child(Self::render_button(
                        "lf-spawn",
                        "Spawns & Exits",
                        self.log_filter == LogFilter::SpawnsExits,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.log_filter = LogFilter::SpawnsExits;
                            cx.notify();
                        }),
                    ))
                    .child(Self::render_button(
                        "lf-alert",
                        "Alerts (CPU/RAM)",
                        self.log_filter == LogFilter::Alerts,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.log_filter = LogFilter::Alerts;
                            cx.notify();
                        }),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_1p5()
                    .child(Self::render_button(
                        "log-export",
                        "Export Log",
                        false,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            let home = env::var("HOME").unwrap_or_else(|_| ".".into());
                            let out_path = PathBuf::from(home).join("Downloads/web-browser-monitor.log");
                            match app.collector.log.export_to_file(&out_path) {
                                Ok(n) => {
                                    app.status_notice = Some(format!(
                                        "Exported {n} log entries to ~/Downloads/web-browser-monitor.log"
                                    ));
                                }
                                Err(err) => {
                                    app.status_notice = Some(format!("Export failed: {err}"));
                                }
                            }
                            cx.notify();
                        }),
                    ))
                    .child(Self::render_button(
                        "log-clear",
                        "Clear",
                        false,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.collector.log.clear();
                            app.status_notice = Some("Cleared activity log".into());
                            cx.notify();
                        }),
                    )),
            );

        let mut panel = div()
            .flex()
            .flex_col()
            .p_3()
            .rounded_lg()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.border))
            .child(controls);

        if entries.is_empty() {
            panel = panel.child(
                div()
                    .p_4()
                    .text_sm()
                    .text_color(rgb(p.text_muted))
                    .child("No log events match this filter yet."),
            );
        } else {
            for (idx, e) in entries.into_iter().enumerate() {
                let lvl_col = match e.level {
                    LogLevel::Alert => p.accent_red,
                    LogLevel::Hidden => p.accent_purple,
                    LogLevel::Spawn => p.accent_green,
                    LogLevel::Exit => p.accent_amber,
                    LogLevel::Info => p.accent_cyan,
                };
                let row_bg = if idx % 2 == 1 { p.bg_row_alt } else { p.bg_panel };
                panel = panel.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .px_2()
                        .py_1()
                        .bg(rgb(row_bg))
                        .text_xs()
                        .child(div().w(px(64.0)).text_color(rgb(p.text_muted)).child(e.timestamp.clone()))
                        .child(
                            div()
                                .w(px(58.0))
                                .font_weight(FontWeight::BOLD)
                                .text_color(rgb(lvl_col))
                                .child(e.level.tag()),
                        )
                        .child(
                            div()
                                .w(px(90.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(browser_color(e.browser, p)))
                                .child(e.browser.short_name()),
                        )
                        .child(div().flex_1().text_color(rgb(p.text_primary)).child(e.message.clone())),
                );
            }
        }
        panel
    }

    fn render_settings_tab(&self, p: &ThemePalette, cx: &mut Context<Self>) -> impl IntoElement {
        let toggle_chip = |id: &'static str,
                           label: &'static str,
                           enabled: bool,
                           on_click: Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App) + 'static>| {
            let prefix = if enabled { "✓ " } else { "○ " };
            Self::render_button(
                id,
                format!("{prefix}{label}"),
                enabled,
                self.theme,
                p,
                move |e, w, app| on_click(e, w, app),
            )
        };

        let sel_bk = self.settings_browser;
        let sel_path_opt = self.registry.get_path(sel_bk);
        let sel_is_custom = self.registry.is_custom(sel_bk);
        let sel_trigger_label = if sel_path_opt.is_some() {
            format!("✓ {}", sel_bk.display_name())
        } else {
            format!("⚠ {}", sel_bk.display_name())
        };
        let sel_status_text = if sel_path_opt.is_some() {
            if sel_is_custom {
                "Custom Path"
            } else {
                "Auto-Detected"
            }
        } else {
            "Missing — Browse to locate"
        };
        let sel_status_col = if sel_path_opt.is_some() {
            if sel_is_custom {
                p.accent_cyan
            } else {
                p.accent_green
            }
        } else {
            p.accent_amber
        };
        let sel_path_display = sel_path_opt
            .map(|pt| pt.display().to_string())
            .unwrap_or_else(|| "Not auto-detected — click Browse / Change Path... to locate".into());

        let mut paths_card = div()
            .flex()
            .flex_col()
            .gap_2p5()
            .p_4()
            .rounded_lg()
            .bg(rgb(p.bg_panel))
            .border_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(p.text_primary))
                                    .child("Browser Install Locations (Auto-Detected + Custom)"),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(p.text_muted))
                                    .child("Select any browser from the dropdown to inspect, browse, or reset its install path."),
                            ),
                    )
                    .child(Self::render_button(
                        "set-redetect",
                        "Auto-Detect All",
                        false,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.registry.redetect_all();
                            app.status_notice = Some("Re-scanned browser install locations".into());
                            cx.notify();
                        }),
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2p5()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text_muted))
                            .child("Select Browser:"),
                    )
                    .child(Self::render_dropdown_trigger(
                        "set-browser-dropdown",
                        sel_trigger_label,
                        self.open_dropdown == OpenDropdown::SettingsBrowser,
                        270.0,
                        self.theme,
                        p,
                        cx.listener(|app, _, _, cx| {
                            app.toggle_dropdown(OpenDropdown::SettingsBrowser, cx);
                        }),
                    )),
            );

        if self.open_dropdown == OpenDropdown::SettingsBrowser {
            paths_card = paths_card.child(self.render_browser_dropdown_menu(
                "set-browser-menu",
                false,
                Some(sel_bk),
                true,
                340.0,
                p,
                |app, opt_bk, cx| {
                    if let Some(bk) = opt_bk {
                        app.settings_browser = bk;
                    }
                    app.open_dropdown = OpenDropdown::None;
                    cx.notify();
                },
                cx,
            ));
        }

        paths_card = paths_card.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .rounded_md()
                .bg(rgb(p.bg_row_alt))
                .border_1()
                .border_color(rgb(sel_status_col))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .child(div().size(px(10.0)).rounded_full().bg(rgb(browser_color(sel_bk, p))))
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(p.text_primary))
                                        .child(sel_bk.display_name()),
                                )
                                .child(
                                    div()
                                        .px_2()
                                        .py_0p5()
                                        .rounded_sm()
                                        .bg(rgb(p.bg_panel))
                                        .border_1()
                                        .border_color(rgb(sel_status_col))
                                        .text_xs()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(rgb(sel_status_col))
                                        .child(sel_status_text),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_1p5()
                                .child(Self::render_button(
                                    "set-sel-browse",
                                    "Browse / Change Path...",
                                    true,
                                    self.theme,
                                    p,
                                    cx.listener(move |app, _, _, cx| {
                                        app.prompt_and_set_browser_path(sel_bk, None, cx);
                                    }),
                                ))
                                .when(sel_is_custom, |el| {
                                    el.child(Self::render_button(
                                        "set-sel-reset",
                                        "Reset to Auto",
                                        false,
                                        self.theme,
                                        p,
                                        cx.listener(move |app, _, _, cx| {
                                            app.registry.clear_custom_path(sel_bk);
                                            app.status_notice = Some(format!(
                                                "Reset {} path to auto-detect",
                                                sel_bk.short_name()
                                            ));
                                            cx.notify();
                                        }),
                                    ))
                                }),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(if sel_path_opt.is_some() {
                            p.text_primary
                        } else {
                            p.accent_amber
                        }))
                        .child(format!("Install Path: {sel_path_display}")),
                ),
        );

        let hover_bg = p.border;
        for (idx, &bk) in BrowserKind::all().iter().enumerate() {
            let path_opt = self.registry.get_path(bk);
            let is_custom = self.registry.is_custom(bk);
            let is_selected = sel_bk == bk;
            let path_str = path_opt
                .map(|pt| pt.display().to_string())
                .unwrap_or_else(|| "Missing — click Browse to locate".to_string());
            let row_bg = if is_selected {
                p.bg_button_active
            } else if idx % 2 == 1 {
                p.bg_row_alt
            } else {
                p.bg_panel
            };
            let name_col = active_text_color(self.theme, is_selected, p);
            let muted_col = if is_selected && self.theme == ThemeMode::Classic {
                0xffffff
            } else if path_opt.is_some() {
                p.text_muted
            } else {
                p.accent_amber
            };

            paths_card = paths_card.child(
                div()
                    .id(("set-path-row", idx))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_sm()
                    .bg(rgb(row_bg))
                    .text_xs()
                    .cursor_pointer()
                    .hover(move |s| s.bg(rgb(hover_bg)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |app, _, _, cx| {
                            app.settings_browser = bk;
                            app.open_dropdown = OpenDropdown::None;
                            cx.notify();
                        }),
                    )
                    .child(div().size(px(8.0)).rounded_full().bg(rgb(browser_color(bk, p))))
                    .child(
                        div()
                            .w(px(126.0))
                            .font_weight(if is_selected { FontWeight::BOLD } else { FontWeight::MEDIUM })
                            .text_color(rgb(name_col))
                            .child(bk.display_name()),
                    )
                    .child(
                        div()
                            .w(px(58.0))
                            .text_color(rgb(if is_selected && self.theme == ThemeMode::Classic {
                                0xffffff
                            } else if path_opt.is_some() {
                                if is_custom { p.accent_cyan } else { p.accent_green }
                            } else {
                                p.accent_amber
                            }))
                            .child(if path_opt.is_some() {
                                if is_custom { "Custom" } else { "Auto" }
                            } else {
                                "Missing"
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .text_color(rgb(muted_col))
                            .overflow_hidden()
                            .child(path_str),
                    )
                    .child(Self::render_button(
                        SharedString::from(format!("browse-{}", bk.short_name())),
                        "Browse...",
                        false,
                        self.theme,
                        p,
                        cx.listener(move |app, _, _, cx| {
                            app.settings_browser = bk;
                            app.prompt_and_set_browser_path(bk, None, cx);
                        }),
                    )),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(paths_card)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(p.bg_panel))
                    .border_1()
                    .border_color(rgb(p.border))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text_primary))
                            .child("Appearance & Theme"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(Self::render_button(
                                "set-dark",
                                "Dark Theme",
                                self.theme == ThemeMode::Dark,
                                self.theme,
                                p,
                                cx.listener(|app, _, _, cx| {
                                    app.theme = ThemeMode::Dark;
                                    app.status_notice = Some("Theme switched to Dark".into());
                                    cx.notify();
                                }),
                            ))
                            .child(Self::render_button(
                                "set-light",
                                "Light Theme",
                                self.theme == ThemeMode::Light,
                                self.theme,
                                p,
                                cx.listener(|app, _, _, cx| {
                                    app.theme = ThemeMode::Light;
                                    app.status_notice = Some("Theme switched to Light".into());
                                    cx.notify();
                                }),
                            ))
                            .child(Self::render_button(
                                "set-classic",
                                "Classic Theme",
                                self.theme == ThemeMode::Classic,
                                self.theme,
                                p,
                                cx.listener(|app, _, _, cx| {
                                    app.theme = ThemeMode::Classic;
                                    app.status_notice = Some("Theme switched to Classic".into());
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(p.bg_panel))
                    .border_1()
                    .border_color(rgb(p.border))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(p.text_primary))
                                    .child("Sampling, History & Visible Panels"),
                            )
                            .child(Self::render_button(
                                "set-reset",
                                "Reset All Defaults",
                                false,
                                self.theme,
                                p,
                                cx.listener(Self::reset_defaults),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(120.0)).text_xs().text_color(rgb(p.text_muted)).child("Refresh Interval:"))
                            .children([500u64, 1000, 2000, 5000].into_iter().map(|ms| {
                                Self::render_button(
                                    SharedString::from(format!("set-rate-{ms}")),
                                    format!("{:.1}s", ms as f64 / 1000.0),
                                    self.refresh_interval_ms == ms,
                                    self.theme,
                                    p,
                                    cx.listener(move |app, _, _, cx| {
                                        app.refresh_interval_ms = ms;
                                        cx.notify();
                                    }),
                                )
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(120.0)).text_xs().text_color(rgb(p.text_muted)).child("History Samples:"))
                            .children([15usize, 30, 60].into_iter().map(|cap| {
                                Self::render_button(
                                    SharedString::from(format!("set-hist-{cap}")),
                                    format!("{cap} samples"),
                                    self.history_capacity == cap,
                                    self.theme,
                                    p,
                                    cx.listener(move |app, _, _, cx| {
                                        app.history_capacity = cap;
                                        app.history.set_max_samples(cap);
                                        cx.notify();
                                    }),
                                )
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .pt_1()
                            .child(toggle_chip(
                                "set-gauge",
                                "Host RAM Gauge",
                                self.show_host_gauge,
                                Box::new(cx.listener(|app, _, _, cx| {
                                    app.show_host_gauge = !app.show_host_gauge;
                                    cx.notify();
                                })),
                            ))
                            .child(toggle_chip(
                                "set-charts",
                                "Trend Charts",
                                self.show_charts,
                                Box::new(cx.listener(|app, _, _, cx| {
                                    app.show_charts = !app.show_charts;
                                    cx.notify();
                                })),
                            ))
                            .child(toggle_chip(
                                "set-breakdown",
                                "Browser Breakdown",
                                self.show_breakdown,
                                Box::new(cx.listener(|app, _, _, cx| {
                                    app.show_breakdown = !app.show_breakdown;
                                    cx.notify();
                                })),
                            ))
                            .child(toggle_chip(
                                "set-vsz",
                                "VSZ Column",
                                self.show_vsz,
                                Box::new(cx.listener(|app, _, _, cx| {
                                    app.show_vsz = !app.show_vsz;
                                    cx.notify();
                                })),
                            )),
                    ),
            )
    }
}

impl Render for BrowserMonitorGui {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = ThemePalette::for_mode(self.theme);

        let page_title = match self.active_tab {
            ActiveTab::Overview => match self.selected_browser {
                Some(bk) => format!("{} Overview", bk.display_name()),
                None => "All Browsers Overview".to_string(),
            },
            ActiveTab::Browsers => "Browsers Summary".to_string(),
            ActiveTab::Processes => "Process & Hidden Service Manager".to_string(),
            ActiveTab::Transfer => "Cross-Browser Tab Transfer".to_string(),
            ActiveTab::Logs => "Activity & Event Logs".to_string(),
            ActiveTab::Settings => "Settings & Browser Install Paths".to_string(),
        };

        let mut toolbar = div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .p_3()
            .bg(rgb(p.bg_panel))
            .border_b_1()
            .border_color(rgb(p.border))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2p5()
                    .child(
                        div()
                            .text_base()
                            .font_weight(FontWeight::BOLD)
                            .text_color(rgb(p.text_primary))
                            .child(page_title),
                    )
                    .when_some(self.selected_browser, |el, bk| {
                        el.child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1p5()
                                .px_2()
                                .py_0p5()
                                .rounded_md()
                                .bg(rgb(p.bg_button_active))
                                .border_1()
                                .border_color(rgb(browser_color(bk, &p)))
                                .text_xs()
                                .text_color(rgb(active_text_color(self.theme, true, &p)))
                                .child(format!("Browser: {}", bk.short_name()))
                                .child(
                                    div()
                                        .id("tb-clear-browser")
                                        .pl_1()
                                        .font_weight(FontWeight::BOLD)
                                        .cursor_pointer()
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(|app, _, _, cx| {
                                                app.set_selected_browser(None, cx);
                                            }),
                                        )
                                        .child("✕"),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1p5()
                    .child(Self::render_dropdown_trigger(
                        "tb-browser-dropdown",
                        match self.selected_browser {
                            Some(b) => format!("Browser: {}", b.short_name()),
                            None => "Browser: All Browsers".to_string(),
                        },
                        self.open_dropdown == OpenDropdown::ToolbarBrowser,
                        180.0,
                        self.theme,
                        &p,
                        cx.listener(|app, _, _, cx| {
                            app.toggle_dropdown(OpenDropdown::ToolbarBrowser, cx);
                        }),
                    ))
                    .child(Self::render_dropdown_trigger(
                        "tb-sort-dropdown",
                        format!("Sort: {}", self.sort_col.label()),
                        self.open_dropdown == OpenDropdown::ToolbarSort,
                        120.0,
                        self.theme,
                        &p,
                        cx.listener(|app, _, _, cx| {
                            app.toggle_dropdown(OpenDropdown::ToolbarSort, cx);
                        }),
                    ))
                    .child(Self::render_button(
                        "tb-sort-dir",
                        if self.sort_desc { "▼ Desc" } else { "▲ Asc" },
                        false,
                        self.theme,
                        &p,
                        cx.listener(|app, _, _, cx| {
                            app.sort_desc = !app.sort_desc;
                            cx.notify();
                        }),
                    ))
                    .child(Self::render_button(
                        "tb-pause",
                        if self.paused { "Resume" } else { "Pause" },
                        self.paused,
                        self.theme,
                        &p,
                        cx.listener(|app, _, _, cx| {
                            app.paused = !app.paused;
                            cx.notify();
                        }),
                    ))
                    .child(Self::render_button(
                        "tb-refresh",
                        "Refresh",
                        false,
                        self.theme,
                        &p,
                        cx.listener(|app, _, _, cx| {
                            app.status_notice = Some("Refreshed metrics".into());
                            app.refresh_now(cx);
                        }),
                    )),
            );

        if let Some(ref notice) = self.status_notice {
            toolbar = toolbar.child(
                div()
                    .text_xs()
                    .text_color(rgb(p.accent_green))
                    .child(notice.clone()),
            );
        }

        let main_body = div()
            .id("main-body")
            .flex_1()
            .min_h(px(0.0))
            .overflow_y_scroll()
            .p_3()
            .child(match self.active_tab {
                ActiveTab::Overview => self.render_overview(&p, cx).into_any_element(),
                ActiveTab::Browsers => self.render_browsers_tab(&p, cx).into_any_element(),
                ActiveTab::Processes => self.render_processes_tab(&p, cx).into_any_element(),
                ActiveTab::Transfer => self.render_transfer_tab(&p, cx).into_any_element(),
                ActiveTab::Logs => self.render_logs_tab(&p, cx).into_any_element(),
                ActiveTab::Settings => self.render_settings_tab(&p, cx).into_any_element(),
            });

        let selected_proc = self
            .selected_pid
            .and_then(|pid| self.processes.iter().find(|pr| pr.pid == pid));

        let mut right_pane = div()
            .flex_1()
            .min_w(px(0.0))
            .h_full()
            .flex()
            .flex_col()
            .child(toolbar);

        if self.open_dropdown == OpenDropdown::ToolbarBrowser {
            let menu = self.render_browser_dropdown_menu(
                "tb-browser-menu",
                true,
                self.selected_browser,
                false,
                280.0,
                &p,
                |app, opt_bk, cx| {
                    app.set_selected_browser(opt_bk, cx);
                },
                cx,
            );
            right_pane = right_pane.child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .px_3()
                    .py_2()
                    .bg(rgb(p.bg_header))
                    .border_b_1()
                    .border_color(rgb(p.accent_cyan))
                    .child(menu),
            );
        } else if self.open_dropdown == OpenDropdown::ToolbarSort {
            let hover_bg = p.bg_row_alt;
            let sort_cols = [
                GuiSortCol::Ram,
                GuiSortCol::Cpu,
                GuiSortCol::Pid,
                GuiSortCol::Browser,
                GuiSortCol::Role,
                GuiSortCol::Hidden,
                GuiSortCol::Procs,
                GuiSortCol::Vsz,
                GuiSortCol::Uptime,
            ];
            let mut sort_menu = div()
                .w(px(200.0))
                .flex()
                .flex_col()
                .gap_0p5()
                .p_1p5()
                .rounded_md()
                .bg(rgb(p.bg_panel))
                .border_1()
                .border_color(rgb(p.accent_cyan));
            for (idx, &col) in sort_cols.iter().enumerate() {
                let active = self.sort_col == col;
                let bg = if active { p.bg_button_active } else { p.bg_panel };
                let text_col = active_text_color(self.theme, active, &p);
                sort_menu = sort_menu.child(
                    div()
                        .id(("tb-sort-item", idx))
                        .flex()
                        .flex_row()
                        .justify_between()
                        .items_center()
                        .px_2()
                        .py_1()
                        .rounded_sm()
                        .bg(rgb(bg))
                        .text_xs()
                        .font_weight(if active { FontWeight::BOLD } else { FontWeight::NORMAL })
                        .text_color(rgb(text_col))
                        .cursor_pointer()
                        .hover(move |s| s.bg(rgb(hover_bg)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |app, _, _, cx| {
                                app.open_dropdown = OpenDropdown::None;
                                app.set_sort(col, cx);
                            }),
                        )
                        .child(col.label())
                        .child(if active { "✓" } else { "" }),
                );
            }
            right_pane = right_pane.child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .px_3()
                    .py_2()
                    .bg(rgb(p.bg_header))
                    .border_b_1()
                    .border_color(rgb(p.accent_cyan))
                    .child(sort_menu),
            );
        }

        right_pane = right_pane.child(main_body);

        if let Some(proc_) = selected_proc {
            let pid = proc_.pid;
            let cmd_str = if proc_.cmd.is_empty() {
                proc_.exe.clone()
            } else {
                proc_.cmd.join(" ")
            };
            let armed = self.confirm_kill_pid == Some(pid);

            right_pane = right_pane.child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .p_2p5()
                    .px_3()
                    .bg(rgb(p.bg_panel))
                    .border_t_1()
                    .border_color(rgb(p.accent_cyan))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap_3()
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(p.text_primary))
                                    .child(format!(
                                        "PID {pid} ({} · {})",
                                        proc_.browser.short_name(),
                                        proc_.role.short_name()
                                    ))
                                    .child(format!("RAM: {}", format_bytes(proc_.memory_bytes)))
                                    .child(format!("CPU: {:.1}%", proc_.cpu_usage))
                                    .child(format!("Uptime: {}", format_duration(proc_.run_time_secs))),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(p.text_muted))
                                    .overflow_hidden()
                                    .child(proc_.site.clone().unwrap_or_else(|| proc_.exe.clone())),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_1p5()
                            .child(Self::render_button(
                                "insp-copy-pid",
                                "Copy PID",
                                false,
                                self.theme,
                                &p,
                                cx.listener(move |app, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(pid.to_string()));
                                    app.status_notice = Some(format!("Copied PID {pid}"));
                                    cx.notify();
                                }),
                            ))
                            .child(Self::render_button(
                                "insp-copy-cmd",
                                "Copy Cmd",
                                false,
                                self.theme,
                                &p,
                                cx.listener(move |app, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(cmd_str.clone()));
                                    app.status_notice = Some(format!("Copied command for PID {pid}"));
                                    cx.notify();
                                }),
                            ))
                            .child(
                                div()
                                    .id("insp-kill")
                                    .px_2p5()
                                    .py_1()
                                    .rounded_md()
                                    .bg(rgb(if armed { p.accent_red } else { p.bg_button }))
                                    .border_1()
                                    .border_color(rgb(p.accent_red))
                                    .text_xs()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(if armed { 0xffffff } else { p.accent_red }))
                                    .cursor_pointer()
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |app, _, _, cx| {
                                            if app.confirm_kill_pid == Some(pid) {
                                                let ok = Command::new("kill")
                                                    .args(["-TERM", &pid.to_string()])
                                                    .status()
                                                    .map(|s| s.success())
                                                    .unwrap_or(false);
                                                app.confirm_kill_pid = None;
                                                app.selected_pid = None;
                                                app.status_notice = Some(if ok {
                                                    format!("Sent SIGTERM to PID {pid}")
                                                } else {
                                                    format!("Failed to terminate PID {pid}")
                                                });
                                                app.refresh_now(cx);
                                            } else {
                                                app.confirm_kill_pid = Some(pid);
                                                cx.notify();
                                            }
                                        }),
                                    )
                                    .child(if armed { "Confirm Kill?" } else { "Kill Process" }),
                            )
                            .child(Self::render_button(
                                "insp-close",
                                "Close",
                                false,
                                self.theme,
                                &p,
                                cx.listener(|app, _, _, cx| {
                                    app.selected_pid = None;
                                    app.confirm_kill_pid = None;
                                    cx.notify();
                                }),
                            )),
                    ),
            );
        }

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(p.bg_app))
            .text_color(rgb(p.text_primary))
            .child(self.render_sidebar(&p, cx))
            .child(right_pane)
    }
}

fn main() {
    let launch = parse_gui_args();
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            let bounds = Bounds::centered(None, size(px(1240.0), px(820.0)), cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(960.0), px(640.0))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Web Browser Monitor — All Browsers, Hidden Services & Tab Transfer".into()),
                    appears_transparent: false,
                    ..Default::default()
                }),
                ..Default::default()
            };

            cx.on_window_closed(|cx, _id| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            gpui_kit::open_window(options, cx, |_, cx| {
                cx.new(|cx| BrowserMonitorGui::new(launch, cx))
            })
            .expect("Failed to open GPUI window");
            cx.activate(true);
        });
}
