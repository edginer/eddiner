use minijinja::{context, AutoEscape, Environment};
use regex_lite::Regex;
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
    fn format_responses(&self, thread_title: &str, default_name: &str) -> String;
}

const DAT_TEMPLATE: &str = "
{%- for res in responses -%}
  {%- if res.name is not none and res.name|length > 1 -%}
    {{ res.name | remove_token }}
  {%- else -%}
    {{ default_name }}
  {%- endif -%}
  <><>{{ res.date }} ID:{{ res.author_id if res.author_id is not none }}\
  <> {{ res.body | replace('\n', '<br>') }}<>\
  {{ thread_title if loop.index == 1 }}
{% endfor %}
";

pub(crate) struct TokenRemover {
    regex: Regex,
    default: String,
}

impl TokenRemover {
    pub(crate) fn new(default: &str) -> TokenRemover {
        TokenRemover {
            regex: Regex::new(r"[a-z0-9]{30,}?").unwrap(),
            default: default.to_owned(),
        }
    }

    pub(crate) fn remove(&self, name: String) -> String {
        if name.starts_with('#') || (name.len() >= 30 && self.regex.is_match(&name)) {
            self.default.clone()
        } else {
            name
        }
    }
}

impl Ch5ResponsesFormatter for Vec<Res> {
    fn format_responses(&self, thread_title: &str, default_name: &str) -> String {
        let mut env = Environment::new();
        // TODO(kenmo-melon): Need to escape anything?
        env.set_auto_escape_callback(|_| AutoEscape::None);
        let token_remover = TokenRemover::new(default_name);
        env.add_filter("remove_token", move |name| token_remover.remove(name));
        env.add_template("0000000000.dat", DAT_TEMPLATE).unwrap();
        let tmpl = env.get_template("0000000000.dat").unwrap();
        let ress = self
            .iter()
            .map(|x| Res {
                name: x.name.clone().map(|name| name.chars().take(32).collect()),
                body: x
                    .body
                    .replace("edge.edgebb.workers.dev", "bbs.eddibb.cc")
                    .chars()
                    .take(4096)
                    .collect(),
                ..x.clone()
            })
            .collect::<Vec<_>>();
        tmpl.render(context!(responses => ress, thread_title, default_name))
            .unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::{Ch5ResponsesFormatter, Res};

    fn make_test_res(name: Option<&str>, body: &str, sec: u32) -> Res {
        Res {
            name: name.map(ToOwned::to_owned),
            mail: None,
            date: format!("2099/9/09(金) 0:0:{}.00", sec),
            author_id: Some(format!("abC/DEf{}", sec)),
            body: body.to_owned(),
            thread_id: "1666666666".to_owned(),
            ip_addr: "1.1.1.1".to_owned(),
        }
    }
    #[test]
    fn test_render_dat() {
        let res_1 = make_test_res(Some("コテハン"), "ええ？", 10);
        let res_2 = make_test_res(Some(""), "うん？\n。。。", 20);
        let res_3 = make_test_res(None, "そう...", 30);
        let res_4 = make_test_res(Some("#abcdefg"), "認証てすと", 40);
        // name.len() == 30
        let res_5 = make_test_res(Some("a0b1c2d3e4f5g6h7i8j9k10l11m12n"), "認証できた？", 50);
        let responses = vec![res_1, res_2, res_3, res_4, res_5];
        let formatted = responses.format_responses("実況スレ", "デフォルト名無し");
        assert_eq!(
            formatted,
            r"コテハン<><>2099/9/09(金) 0:0:10.00 ID:abC/DEf10<> ええ？<>実況スレ
デフォルト名無し<><>2099/9/09(金) 0:0:20.00 ID:abC/DEf20<> うん？<br>。。。<>
デフォルト名無し<><>2099/9/09(金) 0:0:30.00 ID:abC/DEf30<> そう...<>
デフォルト名無し<><>2099/9/09(金) 0:0:40.00 ID:abC/DEf40<> 認証てすと<>
デフォルト名無し<><>2099/9/09(金) 0:0:50.00 ID:abC/DEf50<> 認証できた？<>
"
        )
    }
}
