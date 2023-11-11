use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    pub id: usize,
    pub board_key: Option<String>,
    pub name: String,
    pub local_rule: String,
}
