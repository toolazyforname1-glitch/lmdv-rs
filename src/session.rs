//! Session restore: open tabs, their scroll positions, and every document's
//! remembered reading position (lmdv's product rule).

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::prefs;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub tabs: Vec<SessionTab>,
    pub active: usize,
    pub positions: HashMap<String, usize>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionTab {
    pub path: String,
    pub scroll: usize,
}

pub fn load() -> Session {
    std::fs::read_to_string(path())
        .ok()
        .and_then(|text| serde_json::from_str::<Session>(&text).ok())
        .unwrap_or_default()
}

pub fn save(session: &Session) {
    let _ = std::fs::create_dir_all(prefs::data_dir());
    if let Ok(text) = serde_json::to_string_pretty(session) {
        let _ = std::fs::write(path(), text);
    }
}

fn path() -> PathBuf {
    prefs::data_dir().join("session.json")
}
