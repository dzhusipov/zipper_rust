use std::sync::{Arc, Mutex};

use futures_channel::mpsc::UnboundedSender;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct FormData {
    pub url: String,
}

pub type ProgressSender = UnboundedSender<String>;

pub struct AppState {
    pub progress_senders: Arc<Mutex<Vec<ProgressSender>>>,
}