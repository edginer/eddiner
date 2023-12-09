use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GrecaptchaResponse {
    pub success: bool,
    #[serde(rename = "error-codes")]
    pub error_codes: Option<Vec<String>>,
    pub challenge_ts: String,
    pub hostname: String,
}
