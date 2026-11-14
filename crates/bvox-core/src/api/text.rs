//! Text and URL normalization helpers shared by the API parsers.

pub(crate) fn clean_title(title: &str) -> String {
    let mut without_tags = String::with_capacity(title.len());
    let mut inside_tag = false;
    for character in title.chars() {
        match character {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => without_tags.push(character),
            _ => {}
        }
    }
    without_tags
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

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
