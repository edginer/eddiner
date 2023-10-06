use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thread {
    pub title: String,
    pub response_count: u32,
    pub thread_number: String,
    pub last_modified: String,
    pub board_id: u32,
    pub non_auth_thread: u32,
    pub archived: u32,
    pub active: u32,
}

pub trait Ch5ThreadFormatter {
    fn format_threads(&self) -> String;
}

impl Ch5ThreadFormatter for Vec<Thread> {
    fn format_threads(&self) -> String {
        let mut builder = String::new();
        for t in self {
            builder.push_str(&t.thread_number.to_string());
            builder.push_str(".dat<>");
            builder.push_str(&t.title.replace('\n', ""));
            builder.push_str(" (");
            builder.push_str(&t.response_count.to_string());
            builder.push_str(")\n");
        }

        builder
    }
}
