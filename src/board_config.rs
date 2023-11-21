use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BoardConfig<'a> {
    pub(crate) board_id: usize,
    pub(crate) board_key: &'a str,
    pub(crate) title: String,
    pub(crate) default_name: String,
}
