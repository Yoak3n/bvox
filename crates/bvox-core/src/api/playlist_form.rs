//! Favorite-folder form bodies shared by the create/edit/delete endpoints.

use super::sign::encode_component;

pub(super) fn folder_create_body(title: &str, intro: &str, private: bool, csrf: &str) -> String {
    format!(
        "title={}&intro={}&privacy={}&csrf={}",
        encode_component(title),
        encode_component(intro),
        if private { "1" } else { "0" },
        encode_component(csrf)
    )
}

pub(super) fn folder_edit_body(media_id: i64, title: &str, csrf: &str) -> String {
    format!(
        "media_id={media_id}&title={}&csrf={}",
        encode_component(title),
        encode_component(csrf)
    )
}

pub(super) fn folder_delete_body(media_id: i64, csrf: &str) -> String {
    format!("media_ids={media_id}&csrf={}", encode_component(csrf))
}

pub(super) fn deal_body(folder_id: i64, aid: i64, add: bool, csrf: &str) -> String {
    let (add_ids, del_ids) = if add {
        (folder_id.to_string(), String::new())
    } else {
        (String::new(), folder_id.to_string())
    };
    format!(
        "rid={aid}&type=2&add_media_ids={add_ids}&del_media_ids={del_ids}&csrf={}",
        encode_component(csrf)
    )
}
