use std::collections::VecDeque;

pub const COLOR_RESET: &str = "\x1b[0m";
pub const COLOR_BOLD: &str = "\x1b[1m";
pub const COLOR_DIM: &str = "\x1b[2m";

pub const COLOR_RED: &str = "\x1b[38;5;196m";
pub const COLOR_ORANGE: &str = "\x1b[38;5;208m";
pub const COLOR_YELLOW: &str = "\x1b[38;5;220m";
pub const COLOR_GREEN: &str = "\x1b[38;5;46m";
pub const COLOR_CYAN: &str = "\x1b[38;5;51m";
pub const COLOR_BLUE: &str = "\x1b[38;5;39m";
pub const COLOR_PURPLE: &str = "\x1b[38;5;141m";
pub const COLOR_MAGENTA: &str = "\x1b[38;5;201m";
pub const COLOR_GRAY: &str = "\x1b[38;5;244m";
pub const COLOR_WHITE: &str = "\x1b[38;5;255m";

/// Generates an ASCII horizontal bar chart
pub fn render_bar(fraction: f64, width: usize, color: &str) -> String {
    let clamped = fraction.clamp(0.0, 1.0);
    let filled_len = (clamped * width as f64).round() as usize;
    let empty_len = width.saturating_sub(filled_len);

    let filled_char = '█';
    let empty_char = '░';

    let mut out = String::with_capacity(width + 30);
    out.push_str(color);
    for _ in 0..filled_len {
        out.push(filled_char);
    }
    out.push_str(COLOR_GRAY);
    for _ in 0..empty_len {
        out.push(empty_char);
    }
    out.push_str(COLOR_RESET);
    out
}

/// Generates an ASCII sparkline from a series of values
pub fn render_sparkline(data: &VecDeque<f64>, min_val: f64, max_val: f64, color: &str) -> String {
    if data.is_empty() {
        return format!("{COLOR_DIM}(no data yet){COLOR_RESET}");
    }

    let ticks = [' ', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let range = (max_val - min_val).max(0.0001);

    let mut out = String::with_capacity(data.len() * 4 + 20);
    out.push_str(color);
    for &val in data {
        let normalized = ((val - min_val) / range).clamp(0.0, 1.0);
        let idx = ((normalized * (ticks.len() - 1) as f64).round() as usize).min(ticks.len() - 1);
        out.push(ticks[idx]);
    }
    out.push_str(COLOR_RESET);
    out
}

/// Render a 3-part gauge for system memory: [Ladybird | Other Apps | Free]
pub fn render_host_ram_gauge(
    ladybird_bytes: u64,
    used_bytes: u64,
    total_bytes: u64,
    width: usize,
) -> String {
    if total_bytes == 0 {
        return String::new();
    }

    let total = total_bytes as f64;
    let lb = (ladybird_bytes as f64).min(total);
    let other = (used_bytes.saturating_sub(ladybird_bytes) as f64).min(total - lb);

    let lb_chars = ((lb / total) * width as f64).round() as usize;
    let other_chars = ((other / total) * width as f64).round() as usize;
    let free_chars = width.saturating_sub(lb_chars + other_chars);

    let mut s = String::with_capacity(width * 2 + 50);
    s.push_str(COLOR_CYAN);
    for _ in 0..lb_chars {
        s.push('█');
    }
    s.push_str(COLOR_BLUE);
    for _ in 0..other_chars {
        s.push('█');
    }
    s.push_str(COLOR_GRAY);
    for _ in 0..free_chars {
        s.push('░');
    }
    s.push_str(COLOR_RESET);
    s
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_duration(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let remaining_secs = secs % 60;

    if hours > 0 {
        format!("{}h {}m", hours, mins)
    } else if mins > 0 {
        format!("{}m {:02}s", mins, remaining_secs)
    } else {
        format!("{}s", remaining_secs)
    }
}

pub fn get_cpu_color(cpu: f32) -> &'static str {
    if cpu > 50.0 {
        COLOR_RED
    } else if cpu > 20.0 {
        COLOR_ORANGE
    } else if cpu > 5.0 {
        COLOR_YELLOW
    } else if cpu > 0.5 {
        COLOR_GREEN
    } else {
        COLOR_GRAY
    }
}

pub fn get_ram_color(bytes: u64) -> &'static str {
    const MB: u64 = 1024 * 1024;
    if bytes > 500 * MB {
        COLOR_RED
    } else if bytes > 200 * MB {
        COLOR_ORANGE
    } else if bytes > 50 * MB {
        COLOR_YELLOW
    } else if bytes > 10 * MB {
        COLOR_CYAN
    } else {
        COLOR_GREEN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes_across_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(400 * 1024), "400 KB");
        assert_eq!(format_bytes(15 * 1024 * 1024), "15.0 MB");
        assert_eq!(format_bytes(8 * 1024 * 1024 * 1024), "8.0 GB");
        assert_eq!(format_bytes(4630 * 1024 * 1024 * 1024), "4.52 TB");
    }

    #[test]
    fn formats_durations() {
        assert_eq!(format_duration(45), "45s");
        assert_eq!(format_duration(125), "2m 05s");
        assert_eq!(format_duration(3725), "1h 2m");
    }
}
