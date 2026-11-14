//! Tests for the cache.

use super::audio::{audio_file_name, parse_audio_file_name};
use super::*;
use crate::api::{LyricLine, VideoDetail};

#[test]
fn audio_file_names_roundtrip() {
    let name = audio_file_name("BV1xx411c7mD", 12345, 30280, 204800);
    assert_eq!(
        parse_audio_file_name(&name, "BV1xx411c7mD", 12345),
        Some((30280, 204800))
    );
}

#[test]
fn malformed_audio_file_names_are_rejected() {
    assert_eq!(
        parse_audio_file_name("BV1xx411c7mD_1_30280_204800.m4s.part", "BV1xx411c7mD", 1),
        None
    );
    assert_eq!(
        parse_audio_file_name("BV1xx411c7mD_2_30280_204800.m4s", "BV1xx411c7mD", 1),
        None
    );
    assert_eq!(
        parse_audio_file_name("BV1xx411c7mD_1_30280_x.m4s", "BV1xx411c7mD", 1),
        None
    );
}

fn test_cache() -> (Cache, PathBuf) {
    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "bvox-cache-test-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let cache = Cache::new(root.clone(), 1, CacheMode::Auto);
    (cache, root)
}

#[test]
fn stats_group_files_and_clear_keeps_active_audio() {
    let (cache, root) = test_cache();
    let audio = root.join("audio/BV1_1_30280_204800.m4s");
    let part = root.join("audio/BV2_2_30280_204800.m4s.42.part");
    let cover = root.join("covers/cover.img");
    let lyrics = root.join("lyrics/BV1_1.json");
    let view = root.join("view/BV1.json");
    for (path, size) in [
        (&audio, 10),
        (&part, 20),
        (&cover, 30),
        (&lyrics, 40),
        (&view, 50),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0u8; size]).unwrap();
    }

    let stats = cache.stats();
    assert_eq!(stats.total_bytes, 150);
    assert_eq!(stats.audio_bytes, 30, "completed file + in-flight part");
    assert_eq!(stats.cover_bytes, 30);
    assert_eq!(stats.lyrics_bytes, 40);
    assert_eq!(stats.view_bytes, 50);
    assert_eq!(stats.file_count, 5);

    // The playing track and a running download must survive a cleanup.
    let freed = cache.clear(Some(&audio));
    assert_eq!(freed, 120);
    assert!(audio.exists());
    assert!(part.exists());
    assert!(!cover.exists() && !lyrics.exists() && !view.exists());

    let stats = cache.stats();
    assert_eq!(stats.total_bytes, 30);
    assert_eq!(stats.audio_bytes, 30);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cached_bvids_lists_completed_audio_only() {
    let (cache, root) = test_cache();
    let audio = root.join("audio/BV1_1_30280_204800.m4s");
    let other_page = root.join("audio/BV2_9_30280_204800.m4s");
    let part = root.join("audio/BV3_3_30280_204800.m4s.42.part");
    let malformed = root.join("audio/not-a-track.m4s");
    for path in [&audio, &other_page, &part, &malformed] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"m4s").unwrap();
    }

    let bvids = cache.cached_bvids();
    assert!(bvids.contains("BV1"));
    assert!(bvids.contains("BV2"));
    assert!(!bvids.contains("BV3"));
    assert!(!bvids.contains("not-a-track"));
    assert_eq!(bvids.len(), 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cached_tracks_join_audio_with_view_metadata() {
    let (cache, root) = test_cache();
    let audio = root.join("audio/BV1_1_30280_204800.m4s");
    std::fs::create_dir_all(audio.parent().unwrap()).unwrap();
    std::fs::write(&audio, b"m4s").unwrap();

    // Without view metadata the file cannot be mapped to a page.
    assert!(cache.cached_tracks().is_empty());

    cache.put_view(
        "BV1",
        &VideoDetail {
            bvid: "BV1".to_string(),
            aid: 7,
            cid: 1,
            title: "<em>歌</em>".to_string(),
            author: "UP".to_string(),
            duration: 120,
            pages: vec![crate::api::VideoPage {
                cid: 1,
                duration: 120,
                part: "P1".to_string(),
            }],
            pic: Some("https://example.com/a.jpg".to_string()),
        },
    );

    let tracks = cache.cached_tracks();
    assert_eq!(tracks.len(), 1);
    assert_eq!(tracks[0].bvid, "BV1");
    assert_eq!(tracks[0].page_index, 0);
    assert_eq!(tracks[0].title, "歌");
    assert_eq!(tracks[0].author, "UP");
    assert_eq!(tracks[0].duration, "2:00");
    assert_eq!(tracks[0].part, "P1");
    assert_eq!(tracks[0].quality, "192K · 204 kbps");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lowering_the_limit_evicts_files_immediately() {
    let (cache, root) = test_cache();
    for index in 0..3 {
        let path = root.join(format!("covers/{index}.img"));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, vec![0u8; 500 * 1024]).unwrap();
    }
    assert_eq!(cache.stats().total_bytes, 3 * 500 * 1024);

    // Files written behind the cache's back still count once the user
    // lowers the cap through the UI; 1 MiB trims to 90% (921.6 KiB).
    let limit = cache.set_limit_mb(1, None);
    assert_eq!(limit, 1024 * 1024);
    let stats = cache.stats();
    assert_eq!(stats.limit_bytes, 1024 * 1024);
    assert_eq!(stats.total_bytes, 500 * 1024);
    assert_eq!(stats.file_count, 1);

    // Disabling keeps existing files so the user can clear them manually.
    cache.set_limit_mb(0, None);
    assert!(!cache.metadata_enabled());
    assert_eq!(cache.stats().total_bytes, 500 * 1024);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cache_mode_gates_audio_and_metadata_separately() {
    let (cache, root) = test_cache();
    let audio = root.join("audio/BV1_1_30280_204800.m4s");
    let cover = root.join("covers/cover.img");
    for path in [&audio, &cover] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"bytes").unwrap();
    }

    // Auto reads both categories.
    assert!(cache.audio_enabled() && cache.metadata_enabled());
    assert!(cache.find_audio("BV1", 1).is_some());
    assert!(cache.get_cover("https://i0.hdslb.com/a.jpg").is_none());
    cache.put_cover("https://i0.hdslb.com/a.jpg", b"image-bytes");
    assert!(cache.get_cover("https://i0.hdslb.com/a.jpg").is_some());

    // Small files: audio is neither read nor written, metadata still is.
    cache.set_mode(CacheMode::SmallFiles);
    assert!(!cache.audio_enabled() && cache.metadata_enabled());
    assert!(cache.find_audio("BV1", 1).is_none());
    assert!(cache.cached_bvids().is_empty());
    assert!(!cache.begin_download("BV1:1"));
    assert!(cache.get_cover("https://i0.hdslb.com/a.jpg").is_some());
    cache.put_lyrics(
        "BV1",
        1,
        &[LyricLine {
            from: 0.0,
            to: 1.0,
            text: "ok".to_string(),
        }],
    );
    assert!(cache.get_lyrics("BV1", 1).is_some());

    // Off: nothing is read or written, existing files stay on disk.
    cache.set_mode(CacheMode::Off);
    assert!(!cache.audio_enabled() && !cache.metadata_enabled());
    assert!(cache.get_cover("https://i0.hdslb.com/a.jpg").is_none());
    assert!(cache.get_lyrics("BV1", 1).is_none());
    cache.put_view(
        "BV1",
        &VideoDetail {
            bvid: "BV1".to_string(),
            aid: 1,
            cid: 1,
            title: "标题".to_string(),
            author: "UP".to_string(),
            duration: 60,
            pages: Vec::new(),
            pic: None,
        },
    );
    assert!(cache.get_view("BV1").is_none());
    assert!(audio.exists() && cover.exists());

    // Cycling is what the frontends use for their one-key switch.
    assert_eq!(CacheMode::Auto.next(), CacheMode::SmallFiles);
    assert_eq!(CacheMode::SmallFiles.next(), CacheMode::Off);
    assert_eq!(CacheMode::Off.next(), CacheMode::Auto);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn byte_sizes_are_human_readable() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1536), "1.5 KB");
    assert_eq!(format_bytes(3 * 1024 * 1024), "3.0 MB");
    assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.0 GB");
}

#[test]
fn audio_commit_and_lookup() {
    let (cache, root) = test_cache();
    let target = cache.audio_target("BV1xx411c7mD", 7, 30280, 204800);
    let part = cache.audio_part(&target);
    std::fs::create_dir_all(part.parent().unwrap()).unwrap();
    std::fs::write(&part, b"m4s-bytes").unwrap();

    cache
        .commit_audio(&part, "BV1xx411c7mD", 7, &target)
        .unwrap();

    let cached = cache.find_audio("BV1xx411c7mD", 7).unwrap();
    assert_eq!(cached.quality_id, 30280);
    assert_eq!(cached.bandwidth, 204800);
    assert_eq!(std::fs::read(&cached.path).unwrap(), b"m4s-bytes");
    assert!(!part.exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn lyrics_and_cover_roundtrip() {
    let (cache, root) = test_cache();
    let lines = vec![LyricLine {
        from: 1.5,
        to: 3.0,
        text: "第一句".to_string(),
    }];
    cache.put_lyrics("BV1xx411c7mD", 42, &lines);
    let loaded = cache.get_lyrics("BV1xx411c7mD", 42).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].text, "第一句");

    // Empty lyrics are deliberately not cached.
    cache.put_lyrics("BV1xx411c7mD", 43, &[]);
    assert!(cache.get_lyrics("BV1xx411c7mD", 43).is_none());

    // A manual refresh drops the cached copy before re-fetching.
    cache.remove_lyrics("BV1xx411c7mD", 42);
    assert!(cache.get_lyrics("BV1xx411c7mD", 42).is_none());

    let cover_url = "https://i0.hdslb.com/a.jpg@640w_360h.jpg";
    cache.put_cover(cover_url, b"image-bytes");
    assert_eq!(
        cache.get_cover(cover_url).as_deref(),
        Some(b"image-bytes".as_slice())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn legacy_lyrics_cache_is_ignored() {
    let (cache, root) = test_cache();
    // Version 1 stored a bare array; those files may hold another video's
    // subtitle and must not be served after the version bump.
    let path = root.join("lyrics").join("BV1xx411c7mD_42.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, br#"[{"from":0.0,"to":1.0,"text":"wrong song"}]"#).unwrap();
    assert!(cache.get_lyrics("BV1xx411c7mD", 42).is_none());

    cache.put_lyrics(
        "BV1xx411c7mD",
        42,
        &[LyricLine {
            from: 0.0,
            to: 1.0,
            text: "ok".to_string(),
        }],
    );
    assert_eq!(cache.get_lyrics("BV1xx411c7mD", 42).unwrap()[0].text, "ok");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn view_cache_respects_ttl_with_stale_fallback() {
    let (cache, root) = test_cache();
    let detail = VideoDetail {
        bvid: "BV1xx411c7mD".to_string(),
        aid: 1,
        cid: 42,
        title: "标题".to_string(),
        author: "UP".to_string(),
        duration: 60,
        pages: vec![crate::api::VideoPage {
            cid: 42,
            duration: 60,
            part: "P1".to_string(),
        }],
        pic: None,
    };
    cache.put_view("BV1xx411c7mD", &detail);

    let fresh = cache.get_view("BV1xx411c7mD").unwrap();
    assert_eq!(fresh.cid, 42);
    assert_eq!(fresh.title, "标题");

    // Age the entry past the 24h TTL.
    let path = root.join("view/BV1xx411c7mD.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value["fetched_at"] = serde_json::json!(0);
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();

    assert!(cache.get_view("BV1xx411c7mD").is_none());
    assert!(cache.get_stale_view("BV1xx411c7mD").is_some());
    let _ = std::fs::remove_dir_all(root);
}
