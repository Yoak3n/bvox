//! BV id and share-link parsing for user input.

/// Extract the first BV id from arbitrary text: a bare id or a video URL.
pub fn parse_bvid(input: &str) -> Option<String> {
    let start = input.find("BV")?;
    let candidate = &input[start..];
    if candidate.len() < 12 {
        return None;
    }
    let bvid = &candidate[..12];
    if bvid
        .chars()
        .all(|character| character.is_ascii_alphanumeric())
    {
        Some(bvid.to_string())
    } else {
        None
    }
}

/// Extract the first URL from pasted text. The app's share sheet wraps the
/// short link in a title (`【标题 - 哔哩哔哩】 https://b23.tv/xxxx`) and mobile
/// text can glue punctuation to the end, so stop at whitespace or CJK marks.
pub(in crate::session) fn share_link(input: &str) -> Option<&str> {
    let start = ["https://", "http://"]
        .iter()
        .filter_map(|prefix| input.find(prefix))
        .min()?;
    let url = &input[start..];
    let end = url
        .find(|character: char| {
            character.is_whitespace() || matches!(character, '】' | '，' | '。' | '、')
        })
        .unwrap_or(url.len());
    Some(&url[..end])
}
