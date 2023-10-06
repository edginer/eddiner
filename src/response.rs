use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Res {
    pub name: Option<String>, // author name
    pub mail: Option<String>,
    pub date: String,
    pub author_id: Option<String>,
    pub body: String,
    pub thread_id: String,
    pub ip_addr: String,
}

pub trait Ch5ResponsesFormatter {
    fn format_responses(&self, thread_title: &str) -> String;
}

impl Ch5ResponsesFormatter for Vec<Res> {
    fn format_responses(&self, thread_title: &str) -> String {
        let mut builder = String::new();

        for (idx, r) in self.iter().enumerate() {
            builder.push_str(&format!(
                "{}<>{}<>{} ID:{}<> {} <>{}",
                r.name
                    .clone()
                    .map(|x| if x.is_empty() {
                        "エッヂの名無し".to_string()
                    } else {
                        x
                    })
                    .unwrap_or("エッヂの名無し".to_string())
                    .replace('\n', ""),
                r.mail.clone().unwrap_or("".to_string()).replace('\n', ""),
                r.date,
                r.author_id.clone().unwrap_or("".to_string()),
                r.body.replace('\n', "<br>"),
                if idx == 0 {
                    thread_title.replace('\n', "")
                } else {
                    "".to_string()
                }
            ));
            builder.push('\n');
        }

        builder
    }
}
