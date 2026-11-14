//! Tests for session input parsing.

use super::parse::{parse_bvid, share_link};
use super::tasks::playback::track_display_title;

#[test]
fn parses_bvid_from_share_text() {
    let text = "【标题 - 哔哩哔哩】 https://www.bilibili.com/video/BV1xx411c7mD?p=2";
    assert_eq!(parse_bvid(text).as_deref(), Some("BV1xx411c7mD"));
}

#[test]
fn multi_page_display_title_prefers_the_part_name() {
    assert_eq!(
        track_display_title("某合集", "第二话 歌名", 3),
        "第二话 歌名"
    );
    // No part name (or a single-P video) falls back to the video title,
    // which is cleaned of markup like search results are.
    assert_eq!(track_display_title("单曲", "", 1), "单曲");
    assert_eq!(track_display_title("某合集", "", 3), "某合集");
    assert_eq!(track_display_title("A&amp;B", "", 1), "A&B");
}

#[test]
fn short_link_exposes_url_but_no_bvid() {
    let text = "【标题 - 哔哩哔哩】 https://b23.tv/AbCdEf】";
    assert_eq!(parse_bvid(text), None);
    assert_eq!(share_link(text), Some("https://b23.tv/AbCdEf"));
}

#[test]
fn share_link_stops_at_punctuation() {
    assert_eq!(
        share_link("http://b23.tv/AbCdEf，看看这个"),
        Some("http://b23.tv/AbCdEf")
    );
    assert_eq!(share_link("没有链接"), None);
}
