use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cap {
    pub id: usize,
    pub cap_name: String,
    pub cap_password_hash: String,
}
