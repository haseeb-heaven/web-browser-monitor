use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BrowserKind {
    Ladybird,
    Chrome,
    Safari,
    Firefox,
    Arc,
    Brave,
    Edge,
    Opera,
    Vivaldi,
    Zen,
    Orion,
    Chromium,
}

impl BrowserKind {
    pub fn all() -> &'static [BrowserKind] {
        &[
            BrowserKind::Ladybird,
            BrowserKind::Chrome,
            BrowserKind::Safari,
            BrowserKind::Firefox,
            BrowserKind::Arc,
            BrowserKind::Brave,
            BrowserKind::Edge,
            BrowserKind::Opera,
            BrowserKind::Vivaldi,
            BrowserKind::Zen,
            BrowserKind::Orion,
            BrowserKind::Chromium,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            BrowserKind::Ladybird => "Ladybird",
            BrowserKind::Chrome => "Google Chrome",
            BrowserKind::Safari => "Safari / WebKit",
            BrowserKind::Firefox => "Firefox",
            BrowserKind::Arc => "Arc",
            BrowserKind::Brave => "Brave",
            BrowserKind::Edge => "Microsoft Edge",
            BrowserKind::Opera => "Opera",
            BrowserKind::Vivaldi => "Vivaldi",
            BrowserKind::Zen => "Zen Browser",
            BrowserKind::Orion => "Orion",
            BrowserKind::Chromium => "Chromium",
        }
    }

    pub fn short_name(&self) -> &'static str {
        match self {
            BrowserKind::Ladybird => "Ladybird",
            BrowserKind::Chrome => "Chrome",
            BrowserKind::Safari => "Safari",
            BrowserKind::Firefox => "Firefox",
            BrowserKind::Arc => "Arc",
            BrowserKind::Brave => "Brave",
            BrowserKind::Edge => "Edge",
            BrowserKind::Opera => "Opera",
            BrowserKind::Vivaldi => "Vivaldi",
            BrowserKind::Zen => "Zen",
            BrowserKind::Orion => "Orion",
            BrowserKind::Chromium => "Chromium",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "ladybird" | "lb" => Some(BrowserKind::Ladybird),
            "chrome" | "google-chrome" => Some(BrowserKind::Chrome),
            "safari" | "webkit" => Some(BrowserKind::Safari),
            "firefox" | "ff" => Some(BrowserKind::Firefox),
            "arc" => Some(BrowserKind::Arc),
            "brave" => Some(BrowserKind::Brave),
            "edge" | "msedge" => Some(BrowserKind::Edge),
            "opera" => Some(BrowserKind::Opera),
            "vivaldi" => Some(BrowserKind::Vivaldi),
            "zen" => Some(BrowserKind::Zen),
            "orion" => Some(BrowserKind::Orion),
            "chromium" => Some(BrowserKind::Chromium),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ComponentRole {
    MainUI,
    WebContent,
    Network,
    GpuCompositor,
    MediaAudio,
    ImageDecoder,
    PluginExtension,
    BrokerIpc,
    HiddenDaemon,
    Utility,
}

impl ComponentRole {
    pub fn short_name(&self) -> &'static str {
        match self {
            ComponentRole::MainUI => "Main UI",
            ComponentRole::WebContent => "WebContent",
            ComponentRole::Network => "Network",
            ComponentRole::GpuCompositor => "GPU/Paint",
            ComponentRole::MediaAudio => "Media/Audio",
            ComponentRole::ImageDecoder => "ImageDecoder",
            ComponentRole::PluginExtension => "Plugin/Ext",
            ComponentRole::BrokerIpc => "Hub/IPC",
            ComponentRole::HiddenDaemon => "HiddenDaemon",
            ComponentRole::Utility => "Utility",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ComponentRole::MainUI => "Main UI (Browser)",
            ComponentRole::WebContent => "WebContent (Tabs/JS)",
            ComponentRole::Network => "Network / Requests",
            ComponentRole::GpuCompositor => "GPU / Compositor",
            ComponentRole::MediaAudio => "Media & Audio",
            ComponentRole::ImageDecoder => "Image Decoder",
            ComponentRole::PluginExtension => "Plugin / Extension",
            ComponentRole::BrokerIpc => "Hub / IPC Broker",
            ComponentRole::HiddenDaemon => "Hidden Service / Daemon",
            ComponentRole::Utility => "Utility / Helper",
        }
    }
}

/// Strict, false-positive-free browser & component classifier.
pub fn classify_process(exe: &str, name: &str, cmd: &[String]) -> Option<(BrowserKind, ComponentRole)> {
    let exe_l = exe.to_lowercase();
    let name_l = name.to_lowercase();
    let cmd_joined = cmd.join(" ").to_lowercase();

    // Never match our own monitors or non-browser tools
    if name_l.starts_with("web-browser-monitor")
        || name_l.starts_with("ladybird-track")
        || exe_l.ends_with("/web-browser-monitor")
        || exe_l.ends_with("/web-browser-monitor-gui")
        || exe_l.ends_with("/ladybird-track")
        || exe_l.ends_with("/ladybird-track-gui")
    {
        return None;
    }

    // 1. Ladybird
    let is_ladybird = exe_l.contains("ladybird.app/")
        || exe_l.contains("/ladybird/")
        || matches!(
            name_l.as_str(),
            "ladybird"
                | "webcontent"
                | "requestserver"
                | "compositor"
                | "mediaserver"
                | "videodecoderserver"
                | "audioserver"
                | "imagedecoder"
                | "webrtcclient"
                | "processreaper"
                | "sqlserver"
                | "webworker"
        )
        || (name_l == "hub" && cmd_joined.contains("ladybird"));

    if is_ladybird {
        let role = if exe_l.ends_with("/ladybird") || name_l == "ladybird" {
            ComponentRole::MainUI
        } else if exe_l.ends_with("/webcontent") || name_l == "webcontent" || name_l == "webworker" {
            ComponentRole::WebContent
        } else if exe_l.ends_with("/requestserver") || name_l == "requestserver" {
            ComponentRole::Network
        } else if exe_l.ends_with("/compositor") || name_l == "compositor" {
            ComponentRole::GpuCompositor
        } else if exe_l.ends_with("/audioserver")
            || exe_l.ends_with("/mediaserver")
            || exe_l.ends_with("/videodecoderserver")
            || matches!(
                name_l.as_str(),
                "audioserver" | "mediaserver" | "videodecoderserver" | "webrtcclient"
            )
        {
            ComponentRole::MediaAudio
        } else if exe_l.ends_with("/imagedecoder") || name_l == "imagedecoder" {
            ComponentRole::ImageDecoder
        } else if exe_l.ends_with("/hub") || name_l == "hub" {
            ComponentRole::BrokerIpc
        } else if exe_l.ends_with("/processreaper") || name_l == "processreaper" {
            ComponentRole::HiddenDaemon
        } else {
            ComponentRole::Utility
        };
        return Some((BrowserKind::Ladybird, role));
    }

    // 2. Safari & Apple WebKit hidden XPC services
    if exe_l.contains("safari.app/")
        || exe_l.contains("safariplatformsupport")
        || exe_l.contains("com.apple.webkit.")
        || name_l == "safari"
        || name_l.starts_with("com.apple.webkit")
        || name_l.starts_with("safaribookmarks")
        || name_l.starts_with("safaricloudhistory")
        || name_l.starts_with("safarihistory")
        || name_l.starts_with("safarilaunchagent")
        || name_l.starts_with("safarinotification")
        || name_l.starts_with("safarisafebrowsing")
        || name_l.contains("safariplatformsupport")
    {
        let role = if exe_l.ends_with("/safari") || name_l == "safari" {
            ComponentRole::MainUI
        } else if name_l.contains("webcontent") || exe_l.contains("webkit.webcontent") {
            ComponentRole::WebContent
        } else if name_l.contains("networking") || exe_l.contains("webkit.networking") {
            ComponentRole::Network
        } else if name_l.contains("gpu") || exe_l.contains("webkit.gpu") {
            ComponentRole::GpuCompositor
        } else if exe_l.contains("cachedeleteextension")
            || name_l.contains("platformsupport")
            || name_l.contains("agent")
            || name_l.contains("sync")
            || name_l.contains("safebrowsing")
        {
            ComponentRole::HiddenDaemon
        } else {
            ComponentRole::PluginExtension
        };
        return Some((BrowserKind::Safari, role));
    }

    // 3. Google Chrome & updaters/crashpad
    if exe_l.contains("google chrome.app/")
        || exe_l.contains("/google/chrome/")
        || name_l == "google chrome"
        || name_l == "chrome"
        || name_l == "google-chrome"
        || name_l == "chrome.exe"
        || name_l == "chrome_crashpad_handler"
        || exe_l.contains("googleupdater")
        || exe_l.contains("googlesoftwareupdate")
    {
        return Some((BrowserKind::Chrome, classify_chromium_role(&exe_l, &name_l, &cmd_joined)));
    }

    // 4. Arc Browser (strict: avoid TrialArchivingService / searchpartyd)
    if exe_l.contains("/arc.app/") || name_l == "arc" || name_l.starts_with("arc helper") {
        return Some((BrowserKind::Arc, classify_chromium_role(&exe_l, &name_l, &cmd_joined)));
    }

    // 5. Brave Browser
    if exe_l.contains("brave browser.app/")
        || exe_l.contains("/brave-browser/")
        || name_l == "brave browser"
        || name_l == "brave"
        || name_l == "brave.exe"
        || name_l == "brave_crashpad_handler"
        || name_l.starts_with("brave browser helper")
    {
        return Some((BrowserKind::Brave, classify_chromium_role(&exe_l, &name_l, &cmd_joined)));
    }

    // 6. Microsoft Edge
    if exe_l.contains("microsoft edge.app/")
        || exe_l.contains("/msedge")
        || name_l == "microsoft edge"
        || name_l == "msedge"
        || name_l == "msedge.exe"
        || name_l.starts_with("microsoft edge helper")
        || exe_l.contains("microsoft edge webview")
    {
        return Some((BrowserKind::Edge, classify_chromium_role(&exe_l, &name_l, &cmd_joined)));
    }

    // 7. Firefox
    if exe_l.contains("firefox.app/")
        || exe_l.contains("/firefox/")
        || name_l == "firefox"
        || name_l == "firefox-bin"
        || name_l == "firefox.exe"
        || (name_l == "plugin-container" && exe_l.contains("firefox"))
    {
        let role = if exe_l.ends_with("/firefox") || name_l == "firefox" || name_l == "firefox-bin" {
            if cmd_joined.contains("-contentproc") {
                if cmd_joined.contains(" tab") {
                    ComponentRole::WebContent
                } else if cmd_joined.contains(" gpu") {
                    ComponentRole::GpuCompositor
                } else if cmd_joined.contains(" socket") {
                    ComponentRole::Network
                } else if cmd_joined.contains(" rdd") {
                    ComponentRole::MediaAudio
                } else {
                    ComponentRole::Utility
                }
            } else {
                ComponentRole::MainUI
            }
        } else if name_l.contains("plugin-container") {
            ComponentRole::PluginExtension
        } else if name_l.contains("crashreporter") || exe_l.contains("updater") {
            ComponentRole::HiddenDaemon
        } else {
            ComponentRole::Utility
        };
        return Some((BrowserKind::Firefox, role));
    }

    // 8. Zen Browser (Gecko-based)
    if exe_l.contains("zen browser.app/") || exe_l.contains("zen.app/") || name_l == "zen-bin" {
        let role = if cmd_joined.contains("-contentproc") {
            ComponentRole::WebContent
        } else {
            ComponentRole::MainUI
        };
        return Some((BrowserKind::Zen, role));
    }

    // 9. Opera / Opera GX
    if exe_l.contains("opera.app/")
        || exe_l.contains("opera gx.app/")
        || name_l == "opera"
        || name_l.starts_with("opera helper")
    {
        return Some((BrowserKind::Opera, classify_chromium_role(&exe_l, &name_l, &cmd_joined)));
    }

    // 10. Vivaldi
    if exe_l.contains("vivaldi.app/") || name_l == "vivaldi" || name_l.starts_with("vivaldi helper") {
        return Some((BrowserKind::Vivaldi, classify_chromium_role(&exe_l, &name_l, &cmd_joined)));
    }

    // 11. Orion
    if exe_l.contains("orion.app/") || name_l == "orion" {
        let role = if exe_l.ends_with("/orion") || name_l == "orion" {
            ComponentRole::MainUI
        } else if name_l.contains("webcontent") {
            ComponentRole::WebContent
        } else if name_l.contains("networking") {
            ComponentRole::Network
        } else {
            ComponentRole::Utility
        };
        return Some((BrowserKind::Orion, role));
    }

    // 12. Generic Chromium
    if exe_l.contains("chromium.app/") || name_l == "chromium" || name_l == "chromium-browser" {
        return Some((BrowserKind::Chromium, classify_chromium_role(&exe_l, &name_l, &cmd_joined)));
    }

    None
}

fn classify_chromium_role(exe_l: &str, name_l: &str, cmd_l: &str) -> ComponentRole {
    if cmd_l.contains("crashpad")
        || name_l.contains("crashpad")
        || exe_l.contains("updater")
        || exe_l.contains("softwareupdate")
    {
        ComponentRole::HiddenDaemon
    } else if cmd_l.contains("--type=renderer") || name_l.contains("helper (renderer)") {
        ComponentRole::WebContent
    } else if cmd_l.contains("--type=gpu-process") || name_l.contains("helper (gpu)") {
        ComponentRole::GpuCompositor
    } else if cmd_l.contains("network.mojom.networkservice") {
        ComponentRole::Network
    } else if cmd_l.contains("audio.mojom.audioservice") {
        ComponentRole::MediaAudio
    } else if cmd_l.contains("--type=ppapi")
        || cmd_l.contains("--type=plugin")
        || name_l.contains("helper (plugin)")
    {
        ComponentRole::PluginExtension
    } else if cmd_l.contains("--type=utility") || name_l.contains("helper") {
        ComponentRole::Utility
    } else {
        ComponentRole::MainUI
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProcessDetail {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub browser: BrowserKind,
    pub role: ComponentRole,
    pub is_hidden: bool,
    pub name: String,
    pub exe: String,
    pub cmd: Vec<String>,
    pub site: Option<String>,
    pub memory_bytes: u64,
    pub virtual_memory_bytes: u64,
    pub cpu_usage: f32,
    pub run_time_secs: u64,
}

#[derive(Debug, Clone)]
pub struct BrowserSummary {
    pub browser: BrowserKind,
    pub count: usize,
    pub hidden_count: usize,
    pub has_main_ui: bool,
    pub total_memory_bytes: u64,
    pub total_virtual_memory_bytes: u64,
    pub total_cpu: f32,
    pub sites: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ComponentSummary {
    pub role: ComponentRole,
    pub count: usize,
    pub hidden_count: usize,
    pub total_memory_bytes: u64,
    pub total_virtual_memory_bytes: u64,
    pub total_cpu: f32,
}

#[derive(Debug, Clone)]
pub struct HostStats {
    pub total_memory_bytes: u64,
    pub used_memory_bytes: u64,
    pub available_memory_bytes: u64,
    pub cpu_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct HistoryBuffer {
    pub ram_history: VecDeque<f64>,
    pub cpu_history: VecDeque<f32>,
    pub max_samples: usize,
}

impl HistoryBuffer {
    pub fn new(max_samples: usize) -> Self {
        Self {
            ram_history: VecDeque::with_capacity(max_samples),
            cpu_history: VecDeque::with_capacity(max_samples),
            max_samples,
        }
    }

    pub fn push(&mut self, ram_mb: f64, cpu_percent: f32) {
        if self.ram_history.len() >= self.max_samples {
            self.ram_history.pop_front();
        }
        self.ram_history.push_back(ram_mb);

        if self.cpu_history.len() >= self.max_samples {
            self.cpu_history.pop_front();
        }
        self.cpu_history.push_back(cpu_percent);
    }

    pub fn set_max_samples(&mut self, max_samples: usize) {
        self.max_samples = max_samples.max(5);
        while self.ram_history.len() > self.max_samples {
            self.ram_history.pop_front();
        }
        while self.cpu_history.len() > self.max_samples {
            self.cpu_history.pop_front();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Spawn,
    Exit,
    Hidden,
    Alert,
}

impl LogLevel {
    pub fn tag(&self) -> &'static str {
        match self {
            LogLevel::Info => "INFO",
            LogLevel::Spawn => "SPAWN",
            LogLevel::Exit => "EXIT",
            LogLevel::Hidden => "HIDDEN",
            LogLevel::Alert => "ALERT",
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: LogLevel,
    pub browser: BrowserKind,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct EventLog {
    pub entries: VecDeque<LogEntry>,
    pub max_entries: usize,
    pub file_path: Option<std::path::PathBuf>,
}

impl EventLog {
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: VecDeque::with_capacity(max_entries),
            max_entries,
            file_path: None,
        }
    }

    pub fn push(&mut self, level: LogLevel, browser: BrowserKind, message: impl Into<String>) {
        let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();
        let entry = LogEntry {
            timestamp,
            level,
            browser,
            message: message.into(),
        };
        if let Some(ref path) = self.file_path {
            let _ = append_log_line(path, &entry);
        }
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    pub fn export_to_file(&self, path: &Path) -> std::io::Result<usize> {
        let mut f = OpenOptions::new().create(true).write(true).truncate(true).open(path)?;
        for e in &self.entries {
            writeln!(
                f,
                "[{}] {:<6} [{}] {}",
                e.timestamp,
                e.level.tag(),
                e.browser.short_name(),
                e.message
            )?;
        }
        Ok(self.entries.len())
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

fn append_log_line(path: &Path, entry: &LogEntry) -> std::io::Result<()> {
    let mut f = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(
        f,
        "[{}] {:<6} [{}] {}",
        entry.timestamp,
        entry.level.tag(),
        entry.browser.short_name(),
        entry.message
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Memory,
    Cpu,
    Pid,
    Browser,
}

impl SortBy {
    pub fn next(&self) -> Self {
        match self {
            SortBy::Memory => SortBy::Cpu,
            SortBy::Cpu => SortBy::Pid,
            SortBy::Pid => SortBy::Browser,
            SortBy::Browser => SortBy::Memory,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            SortBy::Memory => "RAM (High -> Low)",
            SortBy::Cpu => "CPU (High -> Low)",
            SortBy::Pid => "PID (Low -> High)",
            SortBy::Browser => "Browser",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_all_major_browsers_and_hidden_services() {
        assert_eq!(
            classify_process(
                "/Applications/Ladybird.app/Contents/MacOS/WebContent",
                "WebContent",
                &[]
            ),
            Some((BrowserKind::Ladybird, ComponentRole::WebContent))
        );
        assert_eq!(
            classify_process(
                "/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Helpers/Google Chrome Helper (Renderer).app/Contents/MacOS/Google Chrome Helper (Renderer)",
                "Google Chrome Helper (Renderer)",
                &["--type=renderer".into()]
            ),
            Some((BrowserKind::Chrome, ComponentRole::WebContent))
        );
        assert_eq!(
            classify_process(
                "/System/Library/PrivateFrameworks/SafariPlatformSupport.framework/Versions/A/XPCServices/com.apple.SafariPlatformSupport.Helper.xpc/Contents/MacOS/com.apple.SafariPlatformSupport.Helper",
                "com.apple.SafariPlatformSupport.Helper",
                &[]
            ),
            Some((BrowserKind::Safari, ComponentRole::HiddenDaemon))
        );
        assert_eq!(
            classify_process(
                "/Applications/Firefox.app/Contents/MacOS/plugin-container.app/Contents/MacOS/plugin-container",
                "plugin-container",
                &["-contentproc".into()]
            ),
            Some((BrowserKind::Firefox, ComponentRole::PluginExtension))
        );
        assert_eq!(
            classify_process("/Applications/Arc.app/Contents/MacOS/Arc", "Arc", &[]),
            Some((BrowserKind::Arc, ComponentRole::MainUI))
        );
        assert_eq!(
            classify_process("/Applications/Brave Browser.app/Contents/MacOS/Brave Browser", "Brave Browser", &[]),
            Some((BrowserKind::Brave, ComponentRole::MainUI))
        );
        assert_eq!(
            classify_process("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge", "Microsoft Edge", &[]),
            Some((BrowserKind::Edge, ComponentRole::MainUI))
        );
    }

    #[test]
    fn avoids_false_positives_on_system_daemons_and_self() {
        assert_eq!(
            classify_process(
                "/System/Library/PrivateFrameworks/TrialServer.framework/Versions/A/XPCServices/TrialArchivingService.xpc/Contents/MacOS/TrialArchivingService",
                "TrialArchivingService",
                &[]
            ),
            None
        );
        assert_eq!(
            classify_process("/usr/libexec/searchpartyd", "searchpartyd", &[]),
            None
        );
        assert_eq!(
            classify_process("/Users/me/.local/bin/web-browser-monitor-gui", "web-browser-monitor-gui", &[]),
            None
        );
    }

    #[test]
    fn event_log_evicts_oldest_entries() {
        let mut log = EventLog::new(2);
        log.push(LogLevel::Info, BrowserKind::Chrome, "first");
        log.push(LogLevel::Spawn, BrowserKind::Ladybird, "second");
        log.push(LogLevel::Hidden, BrowserKind::Safari, "third");
        assert_eq!(log.entries.len(), 2);
        assert_eq!(log.entries[0].message, "second");
        assert_eq!(log.entries[1].message, "third");
    }

    #[test]
    fn history_buffer_evicts_oldest_samples() {
        let mut buf = HistoryBuffer::new(3);
        buf.push(10.0, 1.0);
        buf.push(20.0, 2.0);
        buf.push(30.0, 3.0);
        buf.push(40.0, 4.0);
        assert_eq!(buf.ram_history.len(), 3);
        assert_eq!(buf.ram_history.front(), Some(&20.0));
    }
}
