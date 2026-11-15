//! Title cleanup shared by API parsers, cache and session state.

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
