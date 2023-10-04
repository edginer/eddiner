use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthedCookie {
    pub id: i64,
    pub cookie: String,
    pub authed_time: Option<String>,
    pub origin_ip: String,
    pub authed: i32,
    pub writed_time: String,
    pub auth_code: String,
}
