//! Background tokio tasks. Each one runs an API call off the session
//! loop and reports the outcome back as a `Msg`.

pub(super) mod cache;
pub(super) mod cover;
pub(super) mod library;
pub(super) mod login;
pub(super) mod lyrics;
pub(super) mod playback;
pub(super) mod search;
pub(super) mod tracks;
