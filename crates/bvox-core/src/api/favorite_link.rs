//! Parse favorite-folder links pasted by the user.

use reqwest::Url;

/// Extract a favorite folder's `media_id` from a bare id, a `fid=` query or
/// an `ml<id>` path segment. Short links need a network round trip and are
/// expanded by `ApiClient::resolve_favorite_media_id`.
pub(crate) fn parse_favorite_media_id(input: &str) -> Option<i64> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    if let Some(id) = parse_id(input) {
        return Some(id);
    }
    for prefix in ["fid=", "media_id="] {
        if let Some(rest) = input.strip_prefix(prefix)
            && let Some(id) = parse_id(rest)
        {
            return Some(id);
        }
    }

    let url = Url::parse(input).ok()?;
    for (key, value) in url.query_pairs() {
        if matches!(key.as_ref(), "fid" | "media_id")
            && let Some(id) = parse_id(&value)
        {
            return Some(id);
        }
    }

    let segment = url
        .path_segments()?
        .rev()
        .find(|segment| !segment.is_empty())?;
    for prefix in ["ml", "fav"] {
        if let Some(rest) = segment.strip_prefix(prefix)
            && let Some(id) = parse_id(rest)
        {
            return Some(id);
        }
    }
    None
}

/// Hosts whose links must be resolved before the folder id is visible.
pub(crate) fn is_short_link(input: &str) -> bool {
    let Ok(url) = Url::parse(input.trim()) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    host == "b23.tv"
        || host.ends_with(".b23.tv")
        || host == "bili2233.cn"
        || host.ends_with(".bili2233.cn")
}

fn parse_id(value: &str) -> Option<i64> {
    value.trim().parse::<i64>().ok().filter(|id| *id > 0)
}

#[cfg(test)]
mod tests;
