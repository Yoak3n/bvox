//! Per-view key handlers, dispatched from `handle_key`.

mod confirm;
mod input;
mod library;
mod login;
mod picker;
mod queue;

pub(super) use confirm::handle_confirm_key;
pub(super) use input::handle_input_key;
pub(super) use library::handle_library_key;
pub(super) use login::handle_login_key;
pub(super) use picker::handle_picker_key;
pub(super) use queue::handle_queue_key;
