//! Tests for audio candidate ordering, quality prefs and lyrics parsing.

use super::*;

use super::audio::order_audio_candidates;
use super::lyrics::{SubtitleRaw, choose_subtitle, parse_lyrics, subtitle_belongs_to_video};
use super::playurl::AudioTrack;

fn audio_track(id: i64, bandwidth: i64) -> AudioTrack {
    AudioTrack {
        id,
        bandwidth,
        base_url: None,
        backup_url: Vec::new(),
    }
}

fn candidate_ids(candidates: Vec<AudioTrack>) -> Vec<i64> {
    candidates.into_iter().map(|track| track.id).collect()
}

#[test]
fn vip_auto_prefers_lossless_then_dolby_then_aac() {
    let candidates = |vip| {
        order_audio_candidates(
            vec![audio_track(30216, 44_000), audio_track(30280, 204_800)],
            Some(audio_track(30251, 1_400_000)),
            vec![audio_track(30250, 448_000)],
            AudioQuality::Auto,
            vip,
        )
    };
    assert_eq!(candidate_ids(candidates(true)), vec![30251, 30250, 30280]);
    // Non-VIP keeps AAC first; premium tracks stay as a last resort.
    assert_eq!(candidate_ids(candidates(false)), vec![30280, 30251, 30250]);
}

#[test]
fn explicit_preferences_fall_back_to_aac() {
    let candidates = |preference| {
        order_audio_candidates(
            vec![audio_track(30280, 204_800)],
            Some(audio_track(30251, 1_400_000)),
            vec![audio_track(30250, 448_000)],
            preference,
            true,
        )
    };
    assert_eq!(
        candidate_ids(candidates(AudioQuality::Aac)),
        vec![30280, 30251, 30250]
    );
    assert_eq!(
        candidate_ids(candidates(AudioQuality::Dolby)),
        vec![30250, 30280, 30251]
    );
    assert_eq!(
        candidate_ids(candidates(AudioQuality::HiRes)),
        vec![30251, 30280, 30250]
    );
}

#[test]
fn audio_candidates_are_deduplicated() {
    let candidates = order_audio_candidates(
        vec![audio_track(30250, 448_000)],
        None,
        vec![audio_track(30250, 448_000)],
        AudioQuality::Auto,
        true,
    );
    assert_eq!(candidate_ids(candidates), vec![30250]);
}

#[test]
fn audio_quality_preference_roundtrips() {
    for quality in [
        AudioQuality::Auto,
        AudioQuality::Aac,
        AudioQuality::Dolby,
        AudioQuality::HiRes,
    ] {
        assert_eq!(AudioQuality::parse(quality.as_str()), Some(quality));
    }
    assert_eq!(AudioQuality::parse("unknown"), None);
    assert_eq!(AudioQuality::Auto.next(), AudioQuality::Aac);
    assert_eq!(AudioQuality::Aac.next(), AudioQuality::Dolby);
    assert_eq!(AudioQuality::Dolby.next(), AudioQuality::HiRes);
    assert_eq!(AudioQuality::HiRes.next(), AudioQuality::Auto);
}

#[test]
fn play_mode_preference_roundtrips() {
    for mode in [
        PlayMode::Sequential,
        PlayMode::RepeatAll,
        PlayMode::RepeatOne,
        PlayMode::Shuffle,
    ] {
        assert_eq!(PlayMode::parse(mode.as_str()), Some(mode));
    }
    assert_eq!(PlayMode::parse("unknown"), None);
    assert_eq!(PlayMode::Sequential.next(), PlayMode::RepeatAll);
    assert_eq!(PlayMode::RepeatAll.next(), PlayMode::RepeatOne);
    assert_eq!(PlayMode::RepeatOne.next(), PlayMode::Shuffle);
    assert_eq!(PlayMode::Shuffle.next(), PlayMode::Sequential);
}

#[test]
fn lyrics_json_is_parsed_and_blank_lines_dropped() {
    let body = r#"[
        {"from": 0.0, "to": 2.5, "content": "  "},
        {"from": 2.5, "to": 5.0, "content": "第一句"},
        {"from": 5.0, "to": 8.0, "content": "第二句 "}
    ]"#;
    let lines = parse_lyrics(body).unwrap();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0].text, "第一句");
    assert_eq!(lines[1].from, 5.0);
}

#[test]
fn ai_subtitle_object_body_is_parsed() {
    let body = r#"{
        "type": "AIsubtitle",
        "lang": "zh",
        "body": [
            {"from": 12.56, "to": 17.62, "sid": 1, "content": "啊啊啊"}
        ]
    }"#;
    let lines = parse_lyrics(body).unwrap();
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].from, 12.56);
    assert_eq!(lines[0].text, "啊啊啊");
}

fn subtitle(lan: &str, url: &str) -> SubtitleRaw {
    SubtitleRaw {
        lan: lan.to_string(),
        subtitle_url: Some(url.to_string()),
    }
}

#[test]
fn uploaded_subtitles_are_preferred_over_ai_ones() {
    let ai = subtitle("ai-zh", "//aisubtitle.hdslb.com/bfs/ai_subtitle/prod/1");
    let human = subtitle("zh-CN", "//i0.hdslb.com/bfs/subtitle/a.json");
    let english = subtitle("en-US", "//i0.hdslb.com/bfs/subtitle/b.json");

    let candidates = vec![&ai, &english, &human];
    assert_eq!(choose_subtitle(&candidates).unwrap().lan, "zh-CN");

    // AI Chinese still beats an uploaded track in another language and
    // beats an AI transcript of another language.
    let candidates = vec![&english, &ai];
    assert_eq!(choose_subtitle(&candidates).unwrap().lan, "ai-zh");

    let ai_english = subtitle("ai-en", "//aisubtitle.hdslb.com/bfs/ai_subtitle/prod/2");
    let candidates = vec![&ai_english, &english];
    assert_eq!(choose_subtitle(&candidates).unwrap().lan, "en-US");
}

#[test]
fn ai_subtitle_urls_are_checked_against_video_ids() {
    let aid = 668_754_300;
    let cid = 212_930_870;
    let own = subtitle(
        "ai-zh",
        &format!("//aisubtitle.hdslb.com/bfs/ai_subtitle/prod/{aid}{cid}abc?auth_key=1"),
    );
    // Real shape of a mismatched entry: the file name is not this aid/cid.
    let other = subtitle(
        "ai-zh",
        "//aisubtitle.hdslb.com/bfs/ai_subtitle/prod/11335002542065426408127325b4?auth_key=1",
    );
    // Uploaded stems carry no ids, so they cannot be checked here.
    let uploaded = subtitle("zh-CN", "//i0.hdslb.com/bfs/subtitle/x.json");

    assert!(subtitle_belongs_to_video(aid, cid, &own));
    assert!(!subtitle_belongs_to_video(aid, cid, &other));
    assert!(subtitle_belongs_to_video(aid, cid, &uploaded));
    // Without an aid, an AI URL cannot be verified.
    assert!(!subtitle_belongs_to_video(0, cid, &own));
}
