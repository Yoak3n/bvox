//! Human-readable formatting shared by the API, cache and frontends.

/// Short label for a Bilibili audio quality id.
pub fn quality_label(id: i64) -> &'static str {
    match id {
        30216 => "64K",
        30232 => "132K",
        30280 => "192K",
        30250 => "Dolby",
        30251 => "Hi-Res",
        _ => "AAC",
    }
}

/// Human-readable size for UI labels (`12.3 MB`).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}
