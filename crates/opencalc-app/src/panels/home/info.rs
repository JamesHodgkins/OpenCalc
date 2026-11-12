//! What the Home page knows about a file on the recent list, and the words it
//! uses for it.

use std::path::Path;
use std::time::SystemTime;

/// What is known about a file on the recent list.
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub name: String,
    pub folder: String,
    /// When it was last changed and how big it is; `None` when the file is gone.
    pub found: Option<(SystemTime, u64)>,
    pub template: bool,
}

pub fn info(path: &Path) -> Info {
    let name = path.file_name().map_or_else(
        || path.to_string_lossy().into_owned(),
        |f| f.to_string_lossy().into_owned(),
    );
    let folder = path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let found = std::fs::metadata(path)
        .ok()
        .and_then(|m| Some((m.modified().ok()?, m.len())));
    let template = path
        .extension()
        .is_some_and(opencalc_core::persist::is_template_extension);
    Info {
        name,
        folder,
        found,
        template,
    }
}

/// "just now", "5 minutes ago", "3 days ago".
pub fn age_text(elapsed_secs: u64) -> String {
    let (n, unit) = match elapsed_secs {
        0..=59 => return "just now".to_owned(),
        60..=3599 => (elapsed_secs / 60, "minute"),
        3600..=86_399 => (elapsed_secs / 3600, "hour"),
        86_400..=2_591_999 => (elapsed_secs / 86_400, "day"),
        2_592_000..=31_535_999 => (elapsed_secs / 2_592_000, "month"),
        _ => (elapsed_secs / 31_536_000, "year"),
    };
    format!("{n} {unit}{} ago", if n == 1 { "" } else { "s" })
}

/// "812 B", "12 KB", "1.4 MB".
pub fn size_text(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{} KB", (bytes as f64 / 1024.0).round() as u64)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

pub fn meta_text(info: &Info, now: SystemTime) -> String {
    match info.found {
        Some((modified, bytes)) => {
            let secs = now.duration_since(modified).map_or(0, |d| d.as_secs());
            format!("{} \u{b7} {}", age_text(secs), size_text(bytes))
        }
        None => "Missing".to_owned(),
    }
}

/// How much of a page picture the preview shows, as a fraction of its height: the
/// page is as wide as the preview, and the preview shows its top.
pub fn visible_fraction(width: f32, preview_height: f32, picture: [usize; 2]) -> f32 {
    let page_height = width * picture[1] as f32 / picture[0].max(1) as f32;
    if page_height <= 0.0 {
        1.0
    } else {
        (preview_height / page_height).clamp(0.05, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages_read_naturally() {
        assert_eq!(age_text(5), "just now");
        assert_eq!(age_text(60), "1 minute ago");
        assert_eq!(age_text(150), "2 minutes ago");
        assert_eq!(age_text(3600), "1 hour ago");
        assert_eq!(age_text(86_400 * 3), "3 days ago");
        assert_eq!(age_text(86_400 * 60), "2 months ago");
        assert_eq!(age_text(86_400 * 800), "2 years ago");
    }

    #[test]
    fn sizes_read_naturally() {
        assert_eq!(size_text(812), "812 B");
        assert_eq!(size_text(12 * 1024), "12 KB");
        assert_eq!(size_text(1_500_000), "1.4 MB");
    }

    #[test]
    fn a_missing_file_says_so_and_a_template_is_marked() {
        let gone = info(Path::new("C:/nowhere/at/all/beam.ocalc"));
        assert_eq!(gone.name, "beam.ocalc");
        assert!(gone.found.is_none() && !gone.template);
        assert_eq!(meta_text(&gone, SystemTime::now()), "Missing");
        assert!(info(Path::new("x/starter.ocalct")).template);
    }

    #[test]
    fn the_preview_shows_the_top_of_a_tall_page_and_all_of_a_short_one() {
        // A page 280 by 396 shown 200 wide is 283 tall: a 112 high preview shows 40%.
        let f = visible_fraction(200.0, 112.0, [280, 396]);
        assert!((f - 0.396).abs() < 0.01, "{f}");
        // A landscape page is shorter than the preview: all of it shows.
        assert_eq!(visible_fraction(200.0, 112.0, [400, 100]), 1.0);
        assert!(visible_fraction(0.0, 112.0, [0, 0]) > 0.0);
    }
}
