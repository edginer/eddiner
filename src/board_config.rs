use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BoardConfig {
    pub(crate) board_id: u32,
    pub(crate) title: &'static str,
    pub(crate) board_key: &'static str,
    pub(crate) description: &'static str,
    pub(crate) default_name: &'static str,
}
