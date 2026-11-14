//! Small duration helpers shared by the API and the app state.

pub fn format_secs(total: i64) -> String {
    let total = total.max(0);
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

pub(crate) fn parse_duration(text: &str) -> f64 {
    text.split(':')
        .filter_map(|part| part.parse::<f64>().ok())
        .fold(0.0, |total, part| total * 60.0 + part)
}
