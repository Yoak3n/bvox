//! Tests for the app state machine.

use crate::api::SearchPage;

use super::*;

fn item(bvid: &str) -> VideoItem {
    VideoItem {
        aid: 0,
        bvid: bvid.to_string(),
        title: bvid.to_string(),
        author: String::new(),
        duration: "0:00".to_string(),
        pic: None,
    }
}

fn queue(mode: PlayMode) -> App {
    let mut app = App::new();
    app.play_mode = mode;
    app.replace_results(vec![item("BV1"), item("BV2")], "测试", None);
    app.play_at(0);
    app
}

fn folder(id: i64, title: &str, count: i64) -> FavoriteFolder {
    FavoriteFolder {
        id,
        title: title.to_string(),
        count,
    }
}

fn open_playlist(app: &mut App, folder: FavoriteFolder, items: Vec<VideoItem>) {
    let seq = app.begin_playlist_open();
    app.on_playlist_opened(seq, folder, Ok(items), false);
}

fn search_page(items: Vec<VideoItem>, page: u32, pages: u32, total: u64) -> SearchPage {
    SearchPage {
        items,
        page,
        pages,
        total,
    }
}

#[test]
fn opening_a_playlist_makes_it_the_auto_advancing_queue() {
    let mut app = App::new();
    open_playlist(
        &mut app,
        folder(7, "歌单", 2),
        vec![item("BV1"), item("BV2")],
    );

    assert_eq!(app.view, MainView::Queue);
    assert_eq!(app.results_title, "歌单 · 歌单");
    assert_eq!(
        app.current_playlist.as_ref().map(|folder| folder.id),
        Some(7)
    );

    let request = app.auto_advance().expect("playlist should advance");
    assert_eq!(request.item.bvid, "BV2");
}

#[test]
fn search_results_clear_the_current_playlist() {
    let mut app = App::new();
    open_playlist(&mut app, folder(7, "歌单", 1), vec![item("BV1")]);
    app.search_seq = 1;
    app.on_search_done(
        1,
        "晴天".to_string(),
        Ok(search_page(vec![item("BV10")], 1, 1, 1)),
    );

    assert_eq!(app.view, MainView::Queue);
    assert!(app.current_playlist.is_none());
    assert!(app.auto_advance().is_none());
}

#[test]
fn search_reports_more_pages_and_appends_them() {
    let mut app = App::new();
    app.search_seq = 1;
    app.on_search_done(
        1,
        "晴天".to_string(),
        Ok(search_page(vec![item("BV1")], 1, 2, 2)),
    );

    assert!(app.has_more_results());
    assert_eq!(app.search_keyword, "晴天");
    assert_eq!(app.search_total, 2);

    app.loading_more = true;
    app.on_search_more(1, Ok(search_page(vec![item("BV2")], 2, 2, 2)));

    assert!(!app.loading_more);
    assert!(!app.has_more_results());
    assert_eq!(
        app.results
            .iter()
            .map(|item| item.bvid.as_str())
            .collect::<Vec<_>>(),
        ["BV1", "BV2"]
    );
}

#[test]
fn search_more_dedupes_repeated_hits() {
    let mut app = App::new();
    app.search_seq = 1;
    app.on_search_done(
        1,
        "晴天".to_string(),
        Ok(search_page(vec![item("BV1"), item("BV2")], 1, 2, 3)),
    );

    app.on_search_more(1, Ok(search_page(vec![item("BV2"), item("BV3")], 2, 2, 3)));

    assert_eq!(
        app.results
            .iter()
            .map(|item| item.bvid.as_str())
            .collect::<Vec<_>>(),
        ["BV1", "BV2", "BV3"]
    );
}

#[test]
fn loading_more_results_extends_the_shuffle_bag() {
    let mut app = App::new();
    app.play_mode = PlayMode::Shuffle;
    app.search_seq = 1;
    app.on_search_done(
        1,
        "晴天".to_string(),
        Ok(search_page(vec![item("BV1")], 1, 2, 2)),
    );
    assert_eq!(app.shuffle_remaining.len(), 1);

    app.on_search_more(1, Ok(search_page(vec![item("BV2")], 2, 2, 2)));

    assert_eq!(app.shuffle_remaining.len(), 2);
    assert!(app.shuffle_remaining.contains(&1));
}

#[test]
fn stale_search_pages_are_ignored() {
    let mut app = App::new();
    app.search_seq = 2;
    app.on_search_done(
        2,
        "晴天".to_string(),
        Ok(search_page(vec![item("BV1")], 1, 2, 2)),
    );

    app.on_search_more(1, Ok(search_page(vec![item("BV9")], 2, 2, 2)));

    assert_eq!(app.results.len(), 1);
    assert_eq!(app.results[0].bvid, "BV1");
    assert!(app.has_more_results());
}

#[test]
fn opening_a_playlist_clears_search_paging() {
    let mut app = App::new();
    app.search_seq = 1;
    app.on_search_done(
        1,
        "晴天".to_string(),
        Ok(search_page(vec![item("BV1")], 1, 2, 2)),
    );

    open_playlist(&mut app, folder(7, "歌单", 1), vec![item("BV2")]);

    assert!(!app.has_more_results());
    assert!(app.search_keyword.is_empty());
}

#[test]
fn removing_a_track_keeps_indices_consistent() {
    let mut app = App::new();
    open_playlist(
        &mut app,
        folder(7, "歌单", 3),
        vec![item("BV1"), item("BV2"), item("BV3")],
    );
    app.current = Some(2);
    app.selected = Some(2);

    app.on_track_removed(7, "BV1".to_string(), true, Ok(()));

    assert_eq!(
        app.results
            .iter()
            .map(|item| item.bvid.as_str())
            .collect::<Vec<_>>(),
        ["BV2", "BV3"]
    );
    assert_eq!(app.current, Some(1));
    assert_eq!(app.selected, Some(1));
    assert_eq!(
        app.current_playlist.as_ref().map(|folder| folder.count),
        Some(2)
    );
}

#[test]
fn removing_the_playing_track_clears_its_queue_index() {
    let mut app = App::new();
    open_playlist(
        &mut app,
        folder(7, "歌单", 2),
        vec![item("BV1"), item("BV2")],
    );
    app.current = Some(1);

    app.on_track_removed(7, "BV2".to_string(), true, Ok(()));

    assert_eq!(app.results.len(), 1);
    assert!(app.current.is_none());
    assert_eq!(app.selected, Some(0));
}

#[test]
fn renaming_a_playlist_updates_the_open_queue_title() {
    let mut app = App::new();
    app.playlists = vec![folder(7, "旧名", 1)];
    open_playlist(&mut app, folder(7, "旧名", 1), vec![item("BV1")]);

    app.on_playlist_renamed(7, "新名".to_string(), Ok(()));

    assert_eq!(app.playlists[0].title, "新名");
    assert_eq!(
        app.current_playlist
            .as_ref()
            .map(|folder| folder.title.as_str()),
        Some("新名")
    );
    assert_eq!(app.results_title, "歌单 · 新名");
}

#[test]
fn deleting_the_open_playlist_keeps_the_local_queue() {
    let mut app = App::new();
    app.playlists = vec![folder(7, "歌单", 1)];
    open_playlist(&mut app, folder(7, "歌单", 1), vec![item("BV1")]);

    app.on_playlist_deleted(7, Ok(()));

    assert!(app.playlists.is_empty());
    assert!(app.current_playlist.is_none());
    assert_eq!(app.results.len(), 1);
}

#[test]
fn picker_opens_before_the_folder_list_arrives() {
    let mut app = App::new();
    app.open_picker("BV1".to_string());
    assert_eq!(
        app.picker
            .as_ref()
            .map(|picker| picker.target_bvid.as_str()),
        Some("BV1")
    );
    assert!(app.picker_folder().is_none());

    let seq = app.begin_playlists_load();
    app.on_playlists_loaded(seq, Ok((vec![folder(7, "歌单", 0)], Vec::new())));

    assert_eq!(app.picker_folder().map(|folder| folder.id), Some(7));
}

#[test]
fn hidden_playlists_are_filtered_out_of_the_library() {
    let mut app = App::new();
    let seq = app.begin_playlists_load();
    app.on_playlists_loaded(
        seq,
        Ok((
            vec![
                folder(7, "稍后再听", 3),
                folder(8, "视频收藏", 20),
                folder(9, "纯音乐", 5),
            ],
            vec![8],
        )),
    );

    assert_eq!(
        app.library_playlists()
            .iter()
            .map(|folder| folder.id)
            .collect::<Vec<_>>(),
        vec![7, 9]
    );
    assert_eq!(app.playlist_selected, Some(0));

    // Revealing hidden folders restores the entry with its hidden mark.
    app.toggle_show_hidden();
    assert_eq!(app.library_playlists().len(), 3);
    assert!(app.is_hidden(8));

    // There is no special-cased folder anymore: every folder can hide.
    app.toggle_show_hidden();
    app.apply_playlist_hidden(7, true);
    assert!(!app.library_playlists().iter().any(|folder| folder.id == 7));
}

#[test]
fn importing_without_a_playlist_parks_the_track_for_a_new_playlist() {
    let mut app = App::new();
    app.on_bv_imported(Ok(BvImport {
        item: item("BV1"),
        message: "未打开歌单".to_string(),
        error: false,
        playlist: None,
        logged_in: true,
    }));

    assert_eq!(
        app.pending_import
            .as_ref()
            .map(|pending| pending.bvid.as_str()),
        Some("BV1")
    );
    app.cancel_pending_import();
    assert!(app.pending_import.is_none());
}

#[test]
fn logged_out_or_playlist_imports_do_not_park_a_track() {
    let mut app = App::new();
    app.on_bv_imported(Ok(BvImport {
        item: item("BV1"),
        message: "未登录，仅加入本地列表".to_string(),
        error: false,
        playlist: None,
        logged_in: false,
    }));
    assert!(app.pending_import.is_none());

    app.on_bv_imported(Ok(BvImport {
        item: item("BV2"),
        message: "已加入歌单「歌单」".to_string(),
        error: false,
        playlist: Some(folder(7, "歌单", 0)),
        logged_in: true,
    }));
    assert!(app.pending_import.is_none());
}

#[test]
fn offline_refresh_keeps_the_cached_folder_list() {
    let mut app = App::new();
    app.load_cached_playlists(vec![folder(7, "缓存歌单", 2)], Vec::new());
    assert_eq!(app.library_playlists().len(), 1);

    let seq = app.begin_playlists_load();
    app.on_playlists_loaded(seq, Err("网络不可用".to_string()));

    assert_eq!(app.library_playlists().len(), 1);
    assert_eq!(app.status, "离线：显示缓存的歌单");
    assert!(!app.error);
}

#[test]
fn offline_playlist_open_is_marked_in_the_status() {
    let mut app = App::new();
    let seq = app.begin_playlist_open();
    app.on_playlist_opened(seq, folder(7, "缓存歌单", 1), Ok(vec![item("BV1")]), true);

    assert_eq!(app.results.len(), 1);
    assert!(app.status.contains("离线缓存"));
}

#[test]
fn stale_playlist_responses_are_ignored() {
    let mut app = App::new();
    let stale_open = app.begin_playlist_open();
    let _fresh_open = app.begin_playlist_open();
    app.on_playlist_opened(
        stale_open,
        folder(7, "旧歌单", 1),
        Ok(vec![item("BV1")]),
        false,
    );
    assert!(app.current_playlist.is_none());
    assert!(app.results.is_empty());

    let stale_list = app.begin_playlists_load();
    let _fresh_list = app.begin_playlists_load();
    app.on_playlists_loaded(stale_list, Ok((vec![folder(7, "旧列表", 1)], Vec::new())));
    assert!(app.playlists.is_empty());
}

#[test]
fn sequential_advances_then_stops_at_the_end() {
    let mut app = queue(PlayMode::Sequential);
    let request = app.auto_advance().expect("sequential should advance");
    assert_eq!(request.item.bvid, "BV2");
    assert!(app.auto_advance().is_none());
}

#[test]
fn repeat_all_wraps_to_the_first_track() {
    let mut app = queue(PlayMode::RepeatAll);
    let request = app.auto_advance().expect("advance to BV2");
    assert_eq!(request.item.bvid, "BV2");
    let request = app.auto_advance().expect("wrap to BV1");
    assert_eq!(request.item.bvid, "BV1");
}

#[test]
fn repeat_one_replays_the_current_page() {
    let mut app = queue(PlayMode::RepeatOne);
    let request = app.auto_advance().expect("repeat one");
    assert_eq!(request.item.bvid, "BV1");
    assert_eq!(request.page_index, 0);

    // Even a multi-page video repeats the current page.
    app.page_count = 2;
    app.now_playing = Some(item("BV1"));
    let request = app.auto_advance().expect("repeat the page");
    assert_eq!(request.item.bvid, "BV1");
    assert_eq!(request.page_index, 0);
}

#[test]
fn failed_loads_skip_to_the_next_queue_entry() {
    let mut app = queue(PlayMode::Sequential);
    let request = app.skip_failed().expect("try BV2");
    assert_eq!(request.item.bvid, "BV2");

    // The second failure exhausts the two-track queue.
    assert!(app.skip_failed().is_none());
    assert!(app.status.contains("没有可播放"));
}

#[test]
fn failed_loads_skip_even_in_repeat_one() {
    let mut app = queue(PlayMode::RepeatOne);
    let request = app
        .skip_failed()
        .expect("repeat-one should still skip a broken track");
    assert_eq!(request.item.bvid, "BV2");
}

#[test]
fn shuffle_visits_every_track_before_repeating() {
    let mut app = App::new();
    app.play_mode = PlayMode::Shuffle;
    app.replace_results(
        (0..6).map(|index| item(&format!("BV{index}"))).collect(),
        "测试",
        None,
    );
    app.play_at(0);

    let mut seen = vec!["BV0".to_string()];
    for _ in 0..5 {
        let request = app.auto_advance().expect("shuffle should continue");
        seen.push(request.item.bvid);
    }
    seen.sort();
    seen.dedup();
    assert_eq!(seen.len(), 6, "shuffle should cover the queue once");

    // A new pass starts instead of stopping.
    assert!(app.auto_advance().is_some());
}

#[test]
fn shuffle_previous_walks_the_history() {
    let mut app = App::new();
    app.play_mode = PlayMode::Shuffle;
    app.replace_results(vec![item("BV1"), item("BV2"), item("BV3")], "测试", None);
    app.play_at(0);

    let next = app.step(1).expect("shuffle next");
    assert_ne!(next.item.bvid, "BV1");
    let previous = app.step(-1).expect("shuffle previous");
    assert_eq!(previous.item.bvid, "BV1");
}

#[test]
fn multi_page_videos_auto_continue_inside_any_mode() {
    let mut app = queue(PlayMode::Sequential);
    app.page = 0;
    app.page_count = 2;
    app.now_playing = Some(item("BV1"));

    let request = app.auto_advance().expect("pages should keep playing");
    assert_eq!(request.item.bvid, "BV1");
    assert_eq!(request.page_index, 1);
}
