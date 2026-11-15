//! URL and response-snippet helpers shared by the API parsers.

pub(super) fn normalize_url(url: &str) -> String {
    if let Some(rest) = url.strip_prefix("//") {
        format!("https://{rest}")
    } else if let Some(rest) = url.strip_prefix("http://") {
        format!("https://{rest}")
    } else {
        url.to_string()
    }
}

pub(super) fn snippet(body: &str) -> String {
    let compact = body.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() > 160 {
        format!("{}…", compact.chars().take(160).collect::<String>())
    } else {
        compact
    }
}
