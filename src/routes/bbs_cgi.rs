use md5::{Digest, Md5};
use worker::*;

use crate::{
    authed_cookie::AuthedCookie,
    thread::Thread,
    utils::{
        self, generate_six_digit_num, get_current_date_time, get_current_date_time_string,
        get_unix_timetamp_sec, response_shift_jis_text_html,
    },
};

const WRITING_SUCCESS_HTML_RESPONSE: &str =
    include_str!("templates/writing_success_html_response.html");
const WRITING_FAILED_HTML_RESPONSE: &str =
    include_str!("templates/writing_failed_html_response.html");
const REQUEST_AUTHENTICATION_HTML: &str = include_str!("templates/request_authentication.html");
const REQUEST_AUTHENTICATION_CODE_HTML: &str =
    include_str!("templates/request_authentication_code.html");

#[derive(Debug, Clone)]
struct BbsCgiForm {
    subject: Option<String>,
    name: String,
    mail: String,
    body: String,
    board_key: String,
    is_thread: bool,
    thread_id: Option<String>,
    cap: Option<String>,
}

fn extract_forms(bytes: Vec<u8>) -> Option<BbsCgiForm> {
    let data = encoding_rs::SHIFT_JIS.decode(&bytes).0.to_string();

    // TODO: replace ApplicationError such as malformed form
    let Ok(result) = utils::shift_jis_url_encodeded_body_to_vec(&data) else {
        return None;
    };
    let is_thread = {
        let submit = &result["submit"];
        match submit as &str {
            "書き込む" => false,
            "新規スレッド作成" => true,
            // TODO: above comment
            _ => return None,
        }
    };

    let mail_segments = result["mail"].split('#').collect::<Vec<_>>();
    let mail = mail_segments[0];
    let cap = if mail_segments.len() == 1 {
        None
    } else {
        Some(sanitize(&mail_segments[1..].concat()))
    };

    let subject = if is_thread {
        Some(sanitize(&result["subject"]).clone())
    } else {
        None
    };
    let name = sanitize(&result["FROM"]).clone();
    let mail = sanitize(mail).to_string();
    let body = sanitize(&result["MESSAGE"]).clone();
    let board_key = result["bbs"].clone();

    let thread_id = if is_thread {
        None
    } else {
        Some(result["key"].clone())
    };

    Some(BbsCgiForm {
        subject,
        name,
        mail,
        body,
        board_key,
        is_thread,
        thread_id,
        cap,
    })
}

pub async fn route_bbs_cgi(
    req: &mut Request,
    ua: Option<String>,
    db: &D1Database,
    token_cookie: Option<&str>,
) -> Result<Response> {
    let router = match BbsCgiRouter::new(req, db, token_cookie, ua).await {
        Ok(router) => router,
        Err(resp) => return resp,
    };

    router.route().await
}

struct BbsCgiRouter<'a> {
    db: &'a D1Database,
    token_cookie: Option<&'a str>,
    ip_addr: String,
    form: BbsCgiForm,
    unix_time: u64,
    id: Option<String>,
    ua: Option<String>,
}

impl<'a> BbsCgiRouter<'a> {
    async fn new(
        req: &'a mut Request,
        db: &'a D1Database,
        token_cookie: Option<&'a str>,
        ua: Option<String>,
    ) -> std::result::Result<BbsCgiRouter<'a>, Result<Response>> {
        let Ok(Some(ip_addr)) = req.headers().get("CF-Connecting-IP") else {
            return Err(Response::error(
                "internal server error - cf-connecting-ip",
                500,
            ));
        };

        let Ok(req_bytes) = req.bytes().await else {
            return Err(Response::error("Bad request", 400));
        };
        let form = match extract_forms(req_bytes) {
            Some(form) => form,
            None => return Err(Response::error("Bad request", 400)),
        };

        Ok(Self {
            db,
            token_cookie,
            ip_addr,
            form,
            unix_time: get_unix_timetamp_sec(),
            id: None,
            ua,
        })
    }

    async fn route(mut self) -> Result<Response> {
        if self.form.board_key != "liveedge" {
            return Response::error("Bad request", 400);
        }

        let (token_cookie_candidate, is_cap) =
            match (self.token_cookie.as_deref(), self.form.cap.as_deref()) {
                (Some(_), Some(cap)) => (Some(cap), true),
                (Some(cookie), None) => (Some(cookie), false),
                (None, Some(cap)) => (Some(cap), true),
                (None, None) => (None, false),
            };

        let authenticated_user_cookie = if let Some(tk) = token_cookie_candidate {
            let Ok(stmt) = self
                .db
                .prepare("SELECT * FROM authed_cookies WHERE cookie = ?")
                .bind(&[tk.into()])
            else {
                return Response::error("internal server error - check auth bind", 500);
            };

            if let Ok(Some(r)) = stmt.first::<AuthedCookie>(None).await {
                if r.authed == 1 {
                    Some(r)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if authenticated_user_cookie.is_none() {
            let mut hasher: Md5 = Md5::new();
            hasher.update(&self.ip_addr);
            hasher.update(&self.unix_time.to_string());
            let hash = hasher.finalize();
            let token = format!("{:x}", hash);
            let auth_code = generate_six_digit_num();
            let writed_time = get_unix_timetamp_sec().to_string();

            let Ok(stmt) = self
                .db
                .prepare("INSERT INTO authed_cookies (cookie, origin_ip, authed, auth_code, writed_time) VALUES (?, ?, ?, ?, ?)")
                .bind(&[
                    token.to_string().into(),
                    self.ip_addr.to_string().into(),
                    0.into(),
                    auth_code.clone().into(),
                    writed_time.into(),
                ])
            else {
                return Response::error("internal server error - auth bind", 500);
            };

            if stmt.run().await.is_err() {
                return Response::error("internal server error - db", 500);
            }

            let is_mate = self.ua.map(|x| x.contains("Mate")).unwrap_or(false);

            let auth_body = if is_mate {
                REQUEST_AUTHENTICATION_HTML.replace("{token}", &token)
            } else {
                REQUEST_AUTHENTICATION_CODE_HTML
                    .replace("{token}", &token)
                    .replace("{auth_code}", &auth_code)
            };

            let resp = response_shift_jis_text_html(auth_body.clone()).map(|mut x| {
                x.headers_mut()
                    .append(
                        "Set-Cookie",
                        &format!("edge-token={token}; Max-Age=31536000; Path=/"),
                    )
                    .unwrap();
                x
            });

            return resp;
        }

        let mut hasher = Md5::new();
        hasher.update(&authenticated_user_cookie.clone().unwrap().cookie);
        let datetime = get_current_date_time();
        hasher.update(datetime.date().to_string());
        let hash = hasher.finalize();
        let id = format!("{:x}", hash).chars().take(10).collect::<String>();

        self.id = Some(id);

        let result = if self.form.is_thread {
            self.create_thread().await
        } else {
            self.create_response().await
        };

        if is_cap {
            let tk = authenticated_user_cookie.unwrap().cookie;
            result.map(|mut x| {
                x.headers_mut()
                    .append(
                        "Set-Cookie",
                        &format!("edge-token={}; Max-Age=31536000; Path=/", tk),
                    )
                    .unwrap();
                x
            })
        } else {
            result
        }
    }

    async fn create_thread(self) -> Result<Response> {
        let BbsCgiForm {
            subject,
            name,
            mail,
            body,
            ..
        } = self.form;

        let thread = self.db.prepare(
            "INSERT INTO threads (thread_number, title, response_count, board_id, last_modified) VALUES (?, ?, 1, 1, ?)",
        ).bind(&[self.unix_time.to_string().into(), subject.clone().unwrap().into(), self.unix_time.to_string().into()]);

        let response = self.db.prepare(
            "INSERT INTO responses (name, mail, date, author_id, body, thread_id, ip_addr) VALUES (?, ?, ?, ?, ?, ?, ?)"
        ).bind(&[name.into(),
            mail.into(),
            get_current_date_time_string().into(),
            self.id.unwrap().into(),
            body.into(),
            self.unix_time.to_string().into(),
            self.ip_addr.into(),
        ]);

        match (thread, response) {
            (Ok(thread), Ok(response)) => {
                if self.db.batch(vec![thread, response]).await.is_err() {
                    Response::error("internal server error", 500)
                } else {
                    response_shift_jis_text_html(WRITING_SUCCESS_HTML_RESPONSE.to_string())
                }
            }
            _ => Response::error("internal server error", 500),
        }
    }

    async fn create_response(self) -> Result<Response> {
        let BbsCgiForm {
            name,
            mail,
            body,
            thread_id,
            ..
        } = self.form;

        let thread_id = thread_id.clone().unwrap();

        let Ok(get_thread_stmt) = self
            .db
            .prepare("SELECT * FROM threads WHERE thread_number = ? AND archived = 0")
            .bind(&[thread_id.clone().into()])
        else {
            return Response::error("Bad request - get_thread_stmt.is_err()", 400);
        };
        let Ok(thread_info) = get_thread_stmt.first::<Thread>(None).await else {
            return Response::error("Bad request - get_thread_stmt.first().is_err()", 400);
        };
        if let Some(thread_info) = thread_info {
            if thread_info.active == 0 {
                return response_shift_jis_text_html(WRITING_FAILED_HTML_RESPONSE.replace(
                    "{reason}",
                    "スレッドストッパーが働いたみたいなので書き込めません",
                ));
            }
        } else {
            return response_shift_jis_text_html(
                WRITING_FAILED_HTML_RESPONSE.replace("{reason}", "そのようなスレは存在しません"),
            );
        }

        let update_thread_stmt = self
            .db
            .prepare(
                "UPDATE threads SET response_count = response_count + 1,
            last_modified = ?,
            active = (
                CASE
                    WHEN response_count >= 999 THEN 0
                    ELSE 1
                END
            )
            WHERE thread_number = ?",
            ) // 999 means thread stopper 1000
            .bind(&[self.unix_time.to_string().into(), thread_id.clone().into()]);

        if let Err(e) = update_thread_stmt {
            return Response::error(format!("Bad request - thread.is_err() {e}"), 400);
        }

        let response = self.db.prepare(
            "INSERT INTO responses (name, mail, date, author_id, body, thread_id, ip_addr) VALUES (?, ?, ?, ?, ?, ?, ?)"
        ).bind(&[
            name.into(),
            mail.into(),
            get_current_date_time_string().into(),
            self.id.unwrap().into(),
            body.into(),
            thread_id.into(),
            self.ip_addr.into()
        ]);
        if response.is_err() {
            return Response::error("Bad request - response.is_err()", 400);
        }

        match (update_thread_stmt, response) {
            (Ok(thread), Ok(response)) => {
                if self.db.batch(vec![thread, response]).await.is_err() {
                    Response::error("internal server error - thread creation batch", 500)
                } else {
                    response_shift_jis_text_html(WRITING_SUCCESS_HTML_RESPONSE.to_string())
                }
            }
            _ => Response::error("internal server error - resp prep", 500),
        }
    }
}

fn sanitize(input: &str) -> String {
    input
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "<br>")
        .replace('\r', "")
        .replace("&#10;", "")
}
