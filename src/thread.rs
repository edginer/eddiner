use minijinja::{context, AutoEscape, Environment};
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
    pub metadent: Option<String>,
    pub no_pool: u32,
}

impl Thread {
    pub fn metadent_type(&self) -> MetadentType {
        match self.metadent.as_deref() {
            Some("v") => MetadentType::Verbose,
            Some("vv") => MetadentType::VVerbose,
            Some("vvv") => MetadentType::VVVerbose,
            _ => MetadentType::None,
        }
    }
}

impl From<MetadentType> for Option<&str> {
    fn from(value: MetadentType) -> Self {
        match value {
            MetadentType::None => None,
            MetadentType::Verbose => Some("v"),
            MetadentType::VVerbose => Some("vv"),
            MetadentType::VVVerbose => Some("vvv"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetadentType {
    None,
    Verbose,
    VVerbose,
    VVVerbose,
}

pub trait Ch5ThreadFormatter {
    fn format_threads(&self) -> String;
}

const SUBJECT_TEMPLATE: &str = "
{%- for thread in threads -%}
  {{ thread.thread_number }}.dat<>{{ thread.title | replace('\n', '') }} ({{ thread.response_count }})
{% endfor -%}
";

impl Ch5ThreadFormatter for Vec<Thread> {
    fn format_threads(&self) -> String {
        let mut env = Environment::new();
        // TODO(kenmo-melon): Need to escape anything?
        env.set_auto_escape_callback(|_| AutoEscape::None);
        env.add_template("subject.txt", SUBJECT_TEMPLATE).unwrap();
        let tmpl = env.get_template("subject.txt").unwrap();
        tmpl.render(context!(threads => self)).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::{Ch5ThreadFormatter, Thread};

    fn make_test_thread(title: &str, number: u64, count: u32) -> Thread {
        Thread {
            title: title.to_owned(),
            response_count: count,
            thread_number: number.to_string(),
            last_modified: format!("{}", 36000 + count),
            board_id: 1,
            non_auth_thread: 0,
            archived: 0,
            active: 1,
            metadent: None,
            no_pool: 0,
        }
    }
    #[test]
    fn test_render_subject_txt() {
        let thread_1 = make_test_thread("実況スレ", 1666666666, 334);
        let thread_2 = make_test_thread("雑談 \n スレ", 1666666667, 88);
        let threads = vec![thread_1, thread_2];
        let formatted = threads.format_threads();
        assert_eq!(
            formatted,
            r"1666666666.dat<>実況スレ (334)
1666666667.dat<>雑談  スレ (88)
"
        )
    }
}
