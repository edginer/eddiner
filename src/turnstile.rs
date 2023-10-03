use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct TurnstileResponse {
    pub success: bool,
    #[serde(rename = "error-codes")]
    pub error_codes: Vec<String>,
    pub challenge_ts: String,
    pub hostname: String,
}
