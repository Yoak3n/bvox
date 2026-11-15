//! Tests for search results, favorite folders and hidden playlists.

use super::account::{
    HiddenPlaylists, hidden_ids_for, load_hidden_playlists, save_hidden_playlists, set_hidden_id,
};
use super::playlist_form::{deal_body, folder_create_body, folder_delete_body, folder_edit_body};
use super::playlists::{
    FavoriteFoldersResponse, FavoriteItemsResponse, FavoriteMediaRaw, playlist_folders,
};
use super::search::SearchResponse;

#[test]
fn search_results_carry_aid() {
    let body = r#"{
        "code": 0,
        "data": {
            "result": [
                {
                    "id": 170001,
                    "aid": 170001,
                    "bvid": "BV1xx411c7mD",
                    "title": "<em>晴</em>天",
                    "author": "UP",
                    "duration": "4:29",
                    "pic": "//i0.hdslb.com/a.jpg"
                }
            ]
        }
    }"#;
    let resp: SearchResponse = serde_json::from_str(body).unwrap();
    let raw = resp.data.unwrap().result.unwrap().remove(0);
    let item = raw.into_item().unwrap();
    assert_eq!(item.aid, 170001);
    assert_eq!(item.bvid, "BV1xx411c7mD");
    assert_eq!(item.title, "晴天");
    assert_eq!(item.pic.as_deref(), Some("https://i0.hdslb.com/a.jpg"));
}

#[test]
fn search_response_carries_pagination() {
    let body = r#"{
        "code": 0,
        "data": {
            "page": 2,
            "pagesize": 20,
            "numResults": 57,
            "numPages": 3,
            "result": []
        }
    }"#;
    let resp: SearchResponse = serde_json::from_str(body).unwrap();
    let data = resp.data.unwrap();
    assert_eq!(data.page, 2);
    assert_eq!(data.page_size, 20);
    assert_eq!(data.num_pages, 3);
    assert_eq!(data.num_results, 57);
}

#[test]
fn search_aid_falls_back_to_id_when_aid_is_absent() {
    let body = r#"{
        "code": 0,
        "data": {
            "result": [
                {
                    "id": 170002,
                    "bvid": "BV1xx411c7mD",
                    "title": "t",
                    "author": "UP",
                    "duration": "4:29"
                }
            ]
        }
    }"#;
    let resp: SearchResponse = serde_json::from_str(body).unwrap();
    let raw = resp.data.unwrap().result.unwrap().remove(0);
    assert_eq!(raw.into_item().unwrap().aid, 170002);
}

#[test]
fn favorite_media_carries_aid_and_skips_invalid_entries() {
    let body = r#"{
        "code": 0,
        "data": {
            "has_more": false,
            "medias": [
                {
                    "id": 90001,
                    "type": 2,
                    "title": "第一首",
                    "duration": 213,
                    "cover": "http://i1.hdslb.com/b.jpg",
                    "upper": {"name": "UP"},
                    "attr": 0,
                    "bvid": "BV1xx411c7mD"
                },
                {
                    "id": 90002,
                    "type": 2,
                    "title": "失效",
                    "attr": 1,
                    "bvid": "BV1xx411c7mE"
                },
                {
                    "id": 90003,
                    "type": 12,
                    "title": "音频",
                    "attr": 0,
                    "bvid": "BV1xx411c7mF"
                }
            ]
        }
    }"#;
    let resp: FavoriteItemsResponse = serde_json::from_str(body).unwrap();
    let medias = resp.data.unwrap().medias.unwrap();
    assert_eq!(medias.len(), 3);
    let item = medias.into_iter().next().unwrap().into_item().unwrap();
    assert_eq!(item.aid, 90001);
    assert_eq!(item.title, "第一首");
    assert_eq!(item.duration, "3:33");

    let resp: FavoriteItemsResponse = serde_json::from_str(body).unwrap();
    let items = resp
        .data
        .unwrap()
        .medias
        .unwrap()
        .into_iter()
        .filter_map(FavoriteMediaRaw::into_item)
        .collect::<Vec<_>>();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].bvid, "BV1xx411c7mD");
}

#[test]
fn favorite_resource_list_carries_folder_info() {
    let body = r#"{
        "code": 0,
        "data": {
            "info": {"title": "公开收藏夹", "media_count": 12},
            "medias": [],
            "has_more": false
        }
    }"#;
    let resp: FavoriteItemsResponse = serde_json::from_str(body).unwrap();
    let info = resp.data.expect("data").info.expect("info");
    assert_eq!(info.title.as_deref(), Some("公开收藏夹"));
    assert_eq!(info.media_count, Some(12));
}

#[test]
fn favorite_resource_list_without_info_still_parses() {
    let body = r#"{"code": 0, "data": {"medias": [], "has_more": false}}"#;
    let resp: FavoriteItemsResponse = serde_json::from_str(body).unwrap();
    assert!(resp.data.expect("data").info.is_none());
}

#[test]
fn favorite_write_bodies_are_form_encoded() {
    assert_eq!(
        folder_create_body("我的 歌单&more", "简介", false, "csrf token"),
        "title=%E6%88%91%E7%9A%84%20%E6%AD%8C%E5%8D%95%26more&intro=%E7%AE%80%E4%BB%8B&privacy=0&csrf=csrf%20token"
    );
    assert_eq!(
        folder_create_body("歌单", "简介", true, "tok"),
        "title=%E6%AD%8C%E5%8D%95&intro=%E7%AE%80%E4%BB%8B&privacy=1&csrf=tok"
    );
    assert_eq!(
        folder_edit_body(42, "新名字", "tok"),
        "media_id=42&title=%E6%96%B0%E5%90%8D%E5%AD%97&csrf=tok"
    );
    assert_eq!(folder_delete_body(42, "tok"), "media_ids=42&csrf=tok");
    assert_eq!(
        deal_body(42, 90001, true, "tok"),
        "rid=90001&type=2&add_media_ids=42&del_media_ids=&csrf=tok"
    );
    assert_eq!(
        deal_body(42, 90001, false, "tok"),
        "rid=90001&type=2&add_media_ids=&del_media_ids=42&csrf=tok"
    );
}

#[test]
fn library_lists_every_favorite_folder() {
    let body = r#"{
        "code": 0,
        "data": {
            "list": [
                {"id": 1, "title": "默认收藏夹", "media_count": 0, "attr": 0},
                {"id": 2, "title": "我的歌单", "media_count": 3, "attr": 22},
                {"id": 3, "title": "稍后再听", "media_count": 1, "attr": 55}
            ]
        }
    }"#;
    let resp: FavoriteFoldersResponse = serde_json::from_str(body).unwrap();
    let folders = playlist_folders(resp.data.expect("data").list.expect("list"));
    assert_eq!(
        folders
            .iter()
            .map(|folder| folder.title.as_str())
            .collect::<Vec<_>>(),
        vec!["默认收藏夹", "我的歌单", "稍后再听"]
    );
}

#[test]
fn hidden_playlists_are_tracked_per_account() {
    let mut map = HiddenPlaylists::new();
    set_hidden_id(&mut map, 100, 7, true);
    set_hidden_id(&mut map, 100, 8, true);
    set_hidden_id(&mut map, 200, 9, true);
    assert_eq!(hidden_ids_for(&map, 100), vec![7, 8]);
    assert_eq!(hidden_ids_for(&map, 200), vec![9]);
    assert!(hidden_ids_for(&map, 300).is_empty());

    set_hidden_id(&mut map, 100, 7, false);
    assert_eq!(hidden_ids_for(&map, 100), vec![8]);

    // Unhiding something that was never hidden is a no-op.
    set_hidden_id(&mut map, 100, 42, false);
    assert_eq!(hidden_ids_for(&map, 100), vec![8]);
}

#[test]
fn hidden_playlist_file_roundtrips() {
    let root = std::env::temp_dir().join(format!(
        "bvox-hidden-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    ));
    let mut map = HiddenPlaylists::new();
    set_hidden_id(&mut map, 42, 7, true);
    save_hidden_playlists(&root, &map).unwrap();

    let loaded = load_hidden_playlists(&root);
    assert_eq!(hidden_ids_for(&loaded, 42), vec![7]);
    let _ = std::fs::remove_dir_all(root);
}
