//! WBI request signing: mixin key derivation and MD5 query signatures.

/// Permutation table used to derive the WBI mixin key.
const MIXIN_KEY_ENC_TAB: [usize; 64] = [
    46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9, 42, 19, 29,
    28, 14, 39, 12, 38, 41, 13, 37, 48, 7, 16, 24, 55, 40, 61, 26, 17, 0, 1, 60, 51, 30, 4, 22, 25,
    54, 21, 56, 59, 6, 63, 57, 62, 11, 36, 20, 34, 44, 52,
];
pub(super) fn signed_query(mut params: Vec<(&str, String)>, mixin_key: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    params.push(("wts", timestamp.to_string()));
    params.sort_by(|left, right| left.0.cmp(right.0));

    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", encode_component(key), encode_component(value)))
        .collect::<Vec<_>>()
        .join("&");
    let w_rid = format!("{:x}", md5::compute(format!("{query}{mixin_key}")));

    format!("{query}&w_rid={w_rid}")
}

pub(super) fn mixin_key(img_key: &str, sub_key: &str) -> String {
    let source = format!("{img_key}{sub_key}");
    let bytes = source.as_bytes();
    MIXIN_KEY_ENC_TAB
        .iter()
        .take(32)
        .filter_map(|&index| bytes.get(index).map(|byte| *byte as char))
        .collect()
}

pub(super) fn extract_key(url: &str) -> Option<String> {
    url.rsplit('/')
        .next()
        .and_then(|file_name| file_name.split('.').next())
        .filter(|key| !key.is_empty())
        .map(ToOwned::to_owned)
}

/// Percent-encode everything except RFC 3986 unreserved characters.
/// `!'()*` are intentionally encoded as well, matching Bilibili's WBI examples.
pub(super) fn encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}
