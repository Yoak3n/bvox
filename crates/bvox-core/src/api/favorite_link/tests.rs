use super::*;

#[test]
fn bare_ids_are_accepted() {
    assert_eq!(parse_favorite_media_id("123456"), Some(123_456));
    assert_eq!(parse_favorite_media_id(" 42 "), Some(42));
    assert_eq!(parse_favorite_media_id("0"), None);
    assert_eq!(parse_favorite_media_id("-1"), None);
}

#[test]
fn favlist_links_use_the_fid_query() {
    assert_eq!(
        parse_favorite_media_id("https://space.bilibili.com/2/favlist?fid=456789&ftype=create"),
        Some(456_789)
    );
    assert_eq!(
        parse_favorite_media_id("https://m.bilibili.com/space/2?fid=456789"),
        Some(456_789)
    );
    assert_eq!(
        parse_favorite_media_id("https://example.com/favlist?media_id=321"),
        Some(321)
    );
}

#[test]
fn medialist_links_use_the_ml_path() {
    assert_eq!(
        parse_favorite_media_id("https://www.bilibili.com/medialist/detail/ml987654?type=1"),
        Some(987_654)
    );
    assert_eq!(
        parse_favorite_media_id("https://www.bilibili.com/list/ml987654"),
        Some(987_654)
    );
    assert_eq!(
        parse_favorite_media_id("https://www.bilibili.com/medialist/detail/ml987654/"),
        Some(987_654)
    );
}

#[test]
fn raw_query_fragments_are_accepted() {
    assert_eq!(parse_favorite_media_id("fid=777"), Some(777));
    assert_eq!(parse_favorite_media_id("media_id=888"), Some(888));
}

#[test]
fn unrelated_input_is_rejected() {
    assert_eq!(parse_favorite_media_id(""), None);
    assert_eq!(parse_favorite_media_id("not a link"), None);
    assert_eq!(
        parse_favorite_media_id("https://www.bilibili.com/video/BV1xx411c7mD"),
        None
    );
}

#[test]
fn short_link_hosts_are_detected() {
    assert!(is_short_link("https://b23.tv/abc123"));
    assert!(is_short_link("https://www.bili2233.cn/abc123"));
    assert!(!is_short_link("https://space.bilibili.com/2/favlist?fid=1"));
    assert!(!is_short_link("123456"));
}
