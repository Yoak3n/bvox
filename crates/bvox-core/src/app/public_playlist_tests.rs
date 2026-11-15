use super::*;
use crate::api::{FavoriteContent, FavoriteFolder, SearchPage, VideoItem};
use crate::resume::SavedPlayback;

fn item(bvid: &str) -> VideoItem {
    VideoItem {
        aid: 1,
        bvid: bvid.to_string(),
        title: bvid.to_string(),
        author: "UP".to_string(),
        duration: "03:00".to_string(),
        pic: None,
    }
}

fn folder(id: i64, title: &str) -> FavoriteFolder {
    FavoriteFolder {
        id,
        title: title.to_string(),
        count: 1,
    }
}

fn open_public(app: &mut App) {
    let seq = app.begin_playlist_open();
    app.on_public_playlist_opened(
        seq,
        Ok(FavoriteContent {
            folder: folder(9, "别人的歌单"),
            items: vec![item("BV1")],
        }),
    );
}

#[test]
fn public_playlist_becomes_a_read_only_queue() {
    let mut app = App::new();
    open_public(&mut app);

    assert_eq!(app.view, MainView::Queue);
    assert!(app.current_playlist_public);
    assert_eq!(app.results_title, "公开收藏夹 · 别人的歌单");
    assert_eq!(
        app.current_playlist.as_ref().map(|folder| folder.id),
        Some(9)
    );
}

#[test]
fn search_results_clear_the_public_flag() {
    let mut app = App::new();
    open_public(&mut app);

    app.search_seq = 1;
    app.on_search_done(
        1,
        "晴天".to_string(),
        Ok(SearchPage {
            items: vec![item("BV2")],
            page: 1,
            pages: 1,
            total: 1,
        }),
    );

    assert!(app.current_playlist.is_none());
    assert!(!app.current_playlist_public);
}

#[test]
fn resume_keeps_the_public_flag_and_title() {
    let mut app = App::new();
    open_public(&mut app);
    app.results_title.clear();

    let saved = SavedPlayback::capture(&app);
    let encoded = serde_json::to_vec(&saved).unwrap();
    let decoded: SavedPlayback = serde_json::from_slice(&encoded).unwrap();
    let mut restored = App::new();
    decoded.apply(&mut restored);

    assert!(restored.current_playlist_public);
    assert_eq!(restored.results_title, "公开收藏夹 · 别人的歌单");
}
