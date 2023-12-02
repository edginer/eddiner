use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tinker {
    pub authed_token: String,
    pub wrote_count: u32,
    pub created_thread_count: u32,
    pub level: u32,
    pub last_level_up_at: u64,
    pub last_wrote_at: u64,
}

impl Tinker {
    pub fn new(authed_token: String) -> Self {
        Self {
            authed_token,
            wrote_count: 0,
            created_thread_count: 0,
            level: 0,
            last_level_up_at: 0,
            last_wrote_at: 0,
        }
    }
}
