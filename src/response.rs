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
    pub authed_token: Option<String>,
    pub timestamp: u64,
    pub is_abone: u32,
}

pub trait Ch5ResponsesFormatter {
    fn format_responses(&self, thread_title: &str, default_name: &str) -> String;
}

impl Ch5ResponsesFormatter for Vec<Res> {
    fn format_responses(&self, thread_title: &str, default_name: &str) -> String {
        let thread_title = thread_title.replace('\n', "");
        let mut builder = String::new();
        for (idx, r) in self.iter().enumerate() {
            if r.is_abone == 1 {
                builder.push_str(&format!(
                    "あぼーん<>あぼーん<> <> あぼーん<>{}",
                    if idx == 0 { &thread_title } else { "" }
                ));
            } else {
                builder.push_str(&format!(
                    "{}<><>{} ID:{}<> {}<>{}",
                    r.name
                        .as_ref()
                        .map(|x| if x.is_empty() { default_name } else { x })
                        .unwrap_or(default_name)
                        .replace('\n', ""),
                    r.date,
                    r.author_id.as_deref().unwrap_or(""),
                    r.body
                        .replace('\n', "<br>")
                        .replace("edge.edgebb.workers.dev", "bbs.eddibb.cc"),
                    if idx == 0 { &thread_title } else { "" }
                ));
            }

            builder.push('\n');
        }
        builder
    }
}

#[cfg(test)]
mod tests {
    use super::{Ch5ResponsesFormatter, Res};

    fn make_test_res(name: Option<&str>, body: &str, sec: u32, is_abone: bool) -> Res {
        Res {
            name: name.map(ToOwned::to_owned),
            mail: None,
            date: format!("2099/9/09(金) 0:0:{}.00", sec),
            author_id: Some(format!("abC/DEf{}", sec)),
            body: body.to_owned(),
            thread_id: "1666666666".to_owned(),
            ip_addr: "1.1.1.1".to_owned(),
            authed_token: None,
            timestamp: 0,
            is_abone: if is_abone { 1 } else { 0 },
        }
    }
    #[test]
    fn test_render_dat() {
        let res_1 = make_test_res(Some("コテハン"), "ええ？", 10, false);
        let res_2 = make_test_res(Some(""), "うん？\n。。。", 20, false);
        let res_3 = make_test_res(None, "そう...", 30, false);
        let res_4 = make_test_res(Some("#abcdefg"), "認証てすと", 40, false);
        // name.len() == 30
        let res_5 = make_test_res(
            Some("a0b1c2d3e4f5g6h7i8j9k10l11m12n"),
            "認証できた？",
            50,
            false,
        );
        let responses = vec![res_1, res_2, res_3, res_4, res_5];
        let formatted = responses.format_responses("実況スレ", "デフォルト名無し");
        assert_eq!(
            r"コテハン<><>2099/9/09(金) 0:0:10.00 ID:abC/DEf10<> ええ？<>実況スレ
デフォルト名無し<><>2099/9/09(金) 0:0:20.00 ID:abC/DEf20<> うん？<br>。。。<>
デフォルト名無し<><>2099/9/09(金) 0:0:30.00 ID:abC/DEf30<> そう...<>
#abcdefg<><>2099/9/09(金) 0:0:40.00 ID:abC/DEf40<> 認証てすと<>
a0b1c2d3e4f5g6h7i8j9k10l11m12n<><>2099/9/09(金) 0:0:50.00 ID:abC/DEf50<> 認証できた？<>
",
            formatted,
        )
    }
}
