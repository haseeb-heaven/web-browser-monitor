use std::collections::{BTreeMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::BrowserKind;

#[derive(Debug, Clone)]
pub struct BrowserRegistry {
    pub paths: BTreeMap<BrowserKind, PathBuf>,
    pub custom_overrides: BTreeMap<BrowserKind, PathBuf>,
    config_path: PathBuf,
}

impl BrowserRegistry {
    pub fn default_config_path() -> PathBuf {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".config")
            .join("web-browser-monitor")
            .join("browsers.conf")
    }

    pub fn load() -> Self {
        Self::load_from(Self::default_config_path())
    }

    pub fn load_from(config_path: PathBuf) -> Self {
        let mut reg = Self {
            paths: BTreeMap::new(),
            custom_overrides: BTreeMap::new(),
            config_path,
        };
        if let Ok(text) = fs::read_to_string(&reg.config_path) {
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = trimmed.split_once('=') {
                    if let Some(browser) = BrowserKind::from_str_loose(k) {
                        let p = PathBuf::from(v.trim());
                        if !v.trim().is_empty() {
                            reg.custom_overrides.insert(browser, p);
                        }
                    }
                }
            }
        }
        reg.redetect_all();
        reg
    }

    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut out = String::from("# Custom browser install paths for web-browser-monitor\n");
        for (b, path) in &self.custom_overrides {
            out.push_str(&format!("{}={}\n", b.short_name(), path.display()));
        }
        fs::write(&self.config_path, out)
    }

    pub fn redetect_all(&mut self) {
        self.paths.clear();
        for &b in BrowserKind::all() {
            if let Some(custom) = self.custom_overrides.get(&b) {
                if custom.exists() {
                    self.paths.insert(b, custom.clone());
                    continue;
                }
            }
            if let Some(detected) = auto_detect_path(b) {
                self.paths.insert(b, detected);
            }
        }
    }

    pub fn set_custom_path(&mut self, browser: BrowserKind, path: PathBuf) {
        self.custom_overrides.insert(browser, path.clone());
        self.paths.insert(browser, path);
        let _ = self.save();
    }

    pub fn clear_custom_path(&mut self, browser: BrowserKind) {
        self.custom_overrides.remove(&browser);
        self.paths.remove(&browser);
        if let Some(detected) = auto_detect_path(browser) {
            self.paths.insert(browser, detected);
        }
        let _ = self.save();
    }

    pub fn get_path(&self, browser: BrowserKind) -> Option<&PathBuf> {
        self.paths.get(&browser)
    }

    pub fn is_custom(&self, browser: BrowserKind) -> bool {
        self.custom_overrides.contains_key(&browser)
    }
}

pub fn auto_detect_path(browser: BrowserKind) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = env::var("HOME").unwrap_or_default();
        let candidates: &[&str] = match browser {
            BrowserKind::Ladybird => &["/Applications/Ladybird.app"],
            BrowserKind::Chrome => &["/Applications/Google Chrome.app"],
            BrowserKind::Safari => &["/System/Applications/Safari.app", "/Applications/Safari.app"],
            BrowserKind::Firefox => &["/Applications/Firefox.app"],
            BrowserKind::Arc => &["/Applications/Arc.app"],
            BrowserKind::Brave => &["/Applications/Brave Browser.app"],
            BrowserKind::Edge => &["/Applications/Microsoft Edge.app"],
            BrowserKind::Opera => &["/Applications/Opera.app", "/Applications/Opera GX.app"],
            BrowserKind::Vivaldi => &["/Applications/Vivaldi.app"],
            BrowserKind::Zen => &["/Applications/Zen Browser.app", "/Applications/Zen.app"],
            BrowserKind::Orion => &["/Applications/Orion.app"],
            BrowserKind::Chromium => &["/Applications/Chromium.app"],
        };
        for c in candidates {
            let p = PathBuf::from(c);
            if p.exists() {
                return Some(p);
            }
            if !home.is_empty() {
                let user_p = PathBuf::from(&home).join(c.trim_start_matches('/'));
                if user_p.exists() {
                    return Some(user_p);
                }
            }
        }
        None
    }

    #[cfg(target_os = "linux")]
    {
        let bins: &[&str] = match browser {
            BrowserKind::Ladybird => &["ladybird", "Ladybird"],
            BrowserKind::Chrome => &["google-chrome", "google-chrome-stable"],
            BrowserKind::Safari => &[],
            BrowserKind::Firefox => &["firefox"],
            BrowserKind::Arc => &[],
            BrowserKind::Brave => &["brave-browser", "brave"],
            BrowserKind::Edge => &["microsoft-edge", "microsoft-edge-stable"],
            BrowserKind::Opera => &["opera"],
            BrowserKind::Vivaldi => &["vivaldi"],
            BrowserKind::Zen => &["zen-browser", "zen"],
            BrowserKind::Orion => &[],
            BrowserKind::Chromium => &["chromium", "chromium-browser"],
        };
        for bin in bins {
            if let Ok(out) = Command::new("which").arg(bin).output() {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !s.is_empty() {
                        return Some(PathBuf::from(s));
                    }
                }
            }
        }
        None
    }

    #[cfg(target_os = "windows")]
    {
        let pf = env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
        let pf86 = env::var("ProgramFiles(x86)").unwrap_or_else(|_| "C:\\Program Files (x86)".into());
        let local = env::var("LOCALAPPDATA").unwrap_or_default();
        let rels: &[&str] = match browser {
            BrowserKind::Chrome => &["Google\\Chrome\\Application\\chrome.exe"],
            BrowserKind::Firefox => &["Mozilla Firefox\\firefox.exe"],
            BrowserKind::Edge => &["Microsoft\\Edge\\Application\\msedge.exe"],
            BrowserKind::Brave => &["BraveSoftware\\Brave-Browser\\Application\\brave.exe"],
            BrowserKind::Vivaldi => &["Vivaldi\\Application\\vivaldi.exe"],
            BrowserKind::Opera => &["Opera\\launcher.exe"],
            BrowserKind::Zen => &["Zen Browser\\zen.exe"],
            BrowserKind::Arc => &["Arc\\Arc.exe"],
            BrowserKind::Chromium => &["Chromium\\Application\\chrome.exe"],
            _ => &[],
        };
        for r in rels {
            for root in [&pf, &pf86, &local] {
                if root.is_empty() {
                    continue;
                }
                let p = PathBuf::from(root).join(r);
                if p.exists() {
                    return Some(p);
                }
            }
        }
        None
    }
}

pub fn normalize_and_dedup_urls(raw: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in raw {
        let trimmed = item.trim();
        if (trimmed.starts_with("http://") || trimmed.starts_with("https://"))
            && seen.insert(trimmed.to_string())
        {
            out.push(trimmed.to_string());
        }
    }
    out
}

pub fn collect_open_tabs(browser: BrowserKind, fallback_sites: &[String]) -> Vec<String> {
    let mut collected = Vec::new();
    collected.extend_from_slice(fallback_sites);

    #[cfg(target_os = "macos")]
    {
        let script = match browser {
            BrowserKind::Safari => Some(
                "tell application \"Safari\" to if running then get URL of every tab of every window",
            ),
            BrowserKind::Chrome => Some(
                "tell application \"Google Chrome\" to if running then get URL of every tab of every window",
            ),
            BrowserKind::Arc => Some(
                "tell application \"Arc\" to if running then get URL of every tab of every window",
            ),
            BrowserKind::Brave => Some(
                "tell application \"Brave Browser\" to if running then get URL of every tab of every window",
            ),
            BrowserKind::Edge => Some(
                "tell application \"Microsoft Edge\" to if running then get URL of every tab of every window",
            ),
            BrowserKind::Opera => Some(
                "tell application \"Opera\" to if running then get URL of every tab of every window",
            ),
            BrowserKind::Vivaldi => Some(
                "tell application \"Vivaldi\" to if running then get URL of every tab of every window",
            ),
            BrowserKind::Orion => Some(
                "tell application \"Orion\" to if running then get URL of every tab of every window",
            ),
            _ => None,
        };

        if let Some(sc) = script {
            if let Ok(out) = Command::new("osascript").args(["-e", sc]).output() {
                if out.status.success() {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    for token in stdout.split(',') {
                        let clean = token.trim();
                        if !clean.is_empty() {
                            collected.push(clean.to_string());
                        }
                    }
                }
            }
        }
    }

    normalize_and_dedup_urls(&collected)
}

pub fn open_in_browser(
    browser: BrowserKind,
    install_path: &Path,
    urls: &[String],
    with_tabs: bool,
) -> Result<String, String> {
    if !install_path.exists() {
        return Err(format!(
            "{} path does not exist: {}",
            browser.display_name(),
            install_path.display()
        ));
    }

    let valid_urls = normalize_and_dedup_urls(urls);

    #[cfg(target_os = "macos")]
    {
        if with_tabs && !valid_urls.is_empty() {
            let status = Command::new("open")
                .arg("-a")
                .arg(install_path)
                .args(&valid_urls)
                .status()
                .map_err(|e| format!("Failed to launch {}: {e}", browser.display_name()))?;
            if status.success() {
                return Ok(format!(
                    "Opened {} tab(s) in {}",
                    valid_urls.len(),
                    browser.display_name()
                ));
            }
            return Err(format!("open -a failed for {}", browser.display_name()));
        } else {
            let status = if matches!(browser, BrowserKind::Safari | BrowserKind::Ladybird | BrowserKind::Orion) {
                Command::new("open")
                    .arg("-a")
                    .arg(install_path)
                    .arg("about:blank")
                    .status()
            } else {
                Command::new("open")
                    .arg("-a")
                    .arg(install_path)
                    .args(["--args", "--new-window"])
                    .status()
            }
            .map_err(|e| format!("Failed to launch {}: {e}", browser.display_name()))?;

            if status.success() {
                return Ok(format!(
                    "Opened new tab/window in {} (without transferring tabs)",
                    browser.display_name()
                ));
            }
            return Err(format!("open -a failed for {}", browser.display_name()));
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let mut cmd = Command::new(install_path);
        if with_tabs && !valid_urls.is_empty() {
            cmd.args(&valid_urls);
        }
        cmd.spawn()
            .map_err(|e| format!("Failed to launch {}: {e}", browser.display_name()))?;
        Ok(if with_tabs && !valid_urls.is_empty() {
            format!(
                "Opened {} tab(s) in {}",
                valid_urls.len(),
                browser.display_name()
            )
        } else {
            format!(
                "Opened new window in {} (without transferring tabs)",
                browser.display_name()
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_kind_all_lists_all_twelve() {
        assert_eq!(BrowserKind::all().len(), 12);
    }

    #[test]
    fn normalizes_and_deduplicates_urls() {
        let input = vec![
            " https://github.com ".to_string(),
            "chrome://settings".to_string(),
            "https://github.com".to_string(),
            "http://example.org".to_string(),
            "about:blank".to_string(),
        ];
        assert_eq!(
            normalize_and_dedup_urls(&input),
            vec!["https://github.com", "http://example.org"]
        );
    }

    #[test]
    fn registry_saves_and_loads_custom_paths() {
        let tmp = env::temp_dir().join("wbm_test_browsers.conf");
        let existing_dir = env::temp_dir();
        let _ = fs::remove_file(&tmp);

        let mut reg = BrowserRegistry::load_from(tmp.clone());
        reg.set_custom_path(BrowserKind::Firefox, existing_dir.clone());
        assert!(reg.is_custom(BrowserKind::Firefox));

        let loaded = BrowserRegistry::load_from(tmp.clone());
        assert_eq!(loaded.get_path(BrowserKind::Firefox), Some(&existing_dir));
        let _ = fs::remove_file(&tmp);
    }
}
