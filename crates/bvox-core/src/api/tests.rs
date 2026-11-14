//! Tests for the api domain: transport, signing, text and URL handling.

use super::*;

use super::account::NavResponse;
use super::login::{cookie_from_set_cookies, cookie_from_url};
use super::sign::{encode_component, mixin_key};
use super::text::{clean_title, normalize_url};
use super::transport::cookie_field;
use super::video::normalize_duration;

#[test]
fn nav_response_parses_camel_case_login_flag() {
    // Real nav payload shape: `isLogin`/`uname`/`wbi_img` mixed cases.
    let json = r#"{
        "code": 0,
        "message": "0",
        "data": {
            "isLogin": true,
            "uname": "tester",
            "face": "https://i0.hdslb.com/bfs/face/abc.jpg",
            "vipStatus": 1,
            "vip": {"status": 1},
            "wbi_img": {
                "img_url": "https://i0.hdslb.com/bfs/wbi/aaa.png",
                "sub_url": "https://i0.hdslb.com/bfs/wbi/bbb.png"
            }
        }
    }"#;
    let resp: NavResponse = serde_json::from_str(json).unwrap();
    assert_eq!(resp.code, 0);
    let data = resp.data.expect("data 应存在");
    assert!(data.is_login, "isLogin 必须能反序列化");
    assert_eq!(data.uname.as_deref(), Some("tester"));
    assert_eq!(
        data.face.as_deref(),
        Some("https://i0.hdslb.com/bfs/face/abc.jpg")
    );
    assert_eq!(data.vip_status, 1, "vipStatus 是 camelCase");
    assert_eq!(data.vip.as_ref().map(|vip| vip.status), Some(1));
    assert!(data.wbi_img.is_some());
}

#[test]
fn wbi_mixin_key_matches_reference_vector() {
    let img_key = "7cd084941338484aae1ad9425b84077c";
    let sub_key = "4932caff0ff746eab6f01bf08b70ac45";
    assert_eq!(
        mixin_key(img_key, sub_key),
        "ea1db124af3c7062474693fa704f4ff8"
    );
}

#[test]
fn encoding_encodes_wbi_special_characters() {
    assert_eq!(encode_component("a b!'()*~"), "a%20b%21%27%28%29%2A~");
}

#[test]
fn title_tags_and_entities_are_stripped() {
    assert_eq!(
        clean_title("<em class=\"keyword\">周杰伦</em> &amp; 朋友"),
        "周杰伦 & 朋友"
    );
}

#[test]
fn duration_formatting() {
    assert_eq!(format_secs(213), "3:33");
    assert_eq!(format_secs(3671), "1:01:11");
    assert_eq!(normalize_duration("419:2"), "6:59:02");
    assert_eq!(normalize_duration("3:33"), "3:33");
}

#[test]
fn cookie_is_extracted_from_login_url() {
    let url = "https://www.bilibili.com/?SESSDATA=abc%2C123&bili_jct=xyz&DedeUserID=42&gourl=x";
    assert_eq!(
        cookie_from_url(url).unwrap(),
        "SESSDATA=abc%2C123; bili_jct=xyz; DedeUserID=42"
    );
}

#[test]
fn cookie_is_extracted_from_set_cookie_headers() {
    let headers = vec![
        "SESSDATA=abc%2C123; Path=/; Domain=.bilibili.com; HttpOnly".to_string(),
        "bili_jct=xyz; Path=/".to_string(),
        "DedeUserID=42; Path=/".to_string(),
    ];
    assert_eq!(
        cookie_from_set_cookies(&headers).unwrap(),
        "SESSDATA=abc%2C123; bili_jct=xyz; DedeUserID=42"
    );
}

#[test]
fn cookie_fields_are_extracted() {
    let cookie = "SESSDATA=abc%2C123; bili_jct=xyz; DedeUserID=42";
    assert_eq!(cookie_field(cookie, "bili_jct"), Some("xyz".to_string()));
    assert_eq!(cookie_field(cookie, "DedeUserID"), Some("42".to_string()));
    assert_eq!(cookie_field(cookie, "missing"), None);
}

#[test]
fn urls_are_normalized() {
    assert_eq!(
        normalize_url("//i0.hdslb.com/a.jpg"),
        "https://i0.hdslb.com/a.jpg"
    );
    assert_eq!(
        normalize_url("http://i0.hdslb.com/a.jpg"),
        "https://i0.hdslb.com/a.jpg"
    );
    assert_eq!(
        normalize_url("https://i0.hdslb.com/a.jpg"),
        "https://i0.hdslb.com/a.jpg"
    );
    assert_eq!(
        cover_thumbnail_url("http://i1.hdslb.com/a.jpg"),
        "https://i1.hdslb.com/a.jpg@640w_360h.jpg"
    );
    assert_eq!(
        cover_thumbnail_url("https://i0.hdslb.com/a.jpg@320w.jpg"),
        "https://i0.hdslb.com/a.jpg@320w.jpg"
    );
    assert_eq!(
        cover_thumbnail_url("https://i0.hdslb.com/a.jpg?x=1"),
        "https://i0.hdslb.com/a.jpg@640w_360h.jpg?x=1"
    );
}
