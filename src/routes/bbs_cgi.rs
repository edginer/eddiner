use std::collections::HashMap;

use base64::{engine::general_purpose, Engine};
use jwt_simple::claims::Claims;
use jwt_simple::prelude::{HS256Key, MACLike};
use md5::{Digest, Md5};
use pwhash::unix;
use regex_lite::Regex;
use sha1::Sha1;
use worker::*;

use crate::get_board_info;
use crate::inmemory_cache::{maybe_reject_cookie, maybe_reject_ip, n_recent_auth};
use crate::repositories::bbs_repository::{
    BbsRepository, CreatingAuthedToken, CreatingRes, CreatingThread,
};
use crate::thread::MetadentType;
use crate::tinker::Tinker;
use crate::utils::{
    self, generate_six_digit_num, get_current_date_time, get_current_date_time_string,
    get_reduced_ip_addr, get_unix_timestamp_sec, response_shift_jis_text_html,
};

const WRITING_SUCCESS_HTML_RESPONSE: &str =
    include_str!("templates/writing_success_html_response.html");
const WRITING_FAILED_HTML_RESPONSE: &str =
    include_str!("templates/writing_failed_html_response.html");
const REQUEST_AUTHENTICATION_HTML: &str = include_str!("templates/request_authentication.html");
const REQUEST_AUTHENTICATION_CODE_HTML: &str =
    include_str!("templates/request_authentication_code.html");
const REQUEST_AUTHENTICATION_LOCAL: &str =
    include_str!("templates/request_authentication_local.html");

const RECENT_RES_SECONDS: u64 = 40;
const N_MAX_RECENT_AUTH_PER_IP: u32 = 3;

const CAP_ID_NONE: &str = "????";

pub struct TokenRemover {
    regex: Regex,
}

impl TokenRemover {
    pub(crate) fn new() -> TokenRemover {
        TokenRemover {
            regex: Regex::new(r"[a-z0-9]{30,}?").unwrap(),
        }
    }

    pub(crate) fn remove(&self, name: String) -> String {
        if name.len() >= 30 && self.regex.is_match(&name) {
            String::new()
        } else {
            name
        }
    }
}

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

impl BbsCgiForm {
    fn validate(&self) -> std::result::Result<(), &'static str> {
        if matches!(&self.subject, Some(subject) if subject.chars().count() > 96) {
            return Err("スレッドタイトルが長すぎます");
        }

        if self.name.chars().count() > 64 {
            return Err("名前が長すぎます");
        }

        if self.mail.chars().count() > 64 {
            return Err("メールアドレスが長すぎます");
        }

        let body_chars = self.body.chars().collect::<Vec<_>>();
        if body_chars.len() > 4096 {
            return Err("本文が長すぎます");
        }

        if body_chars.iter().filter(|&&x| x == '\n').count() > 32 {
            return Err("本文に改行が多すぎます");
        }

        Ok(())
    }
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
        Some(sanitize_thread_name(&result["subject"]).clone())
    } else {
        None
    };

    let name_segments = result["FROM"].split('#').collect::<Vec<_>>();
    let name = name_segments[0];
    let name = if name_segments.len() == 1 {
        let token_remover = TokenRemover::new();
        let name = token_remover.remove(name.to_string());
        sanitize(&name)
            .replace('◆', "◇")
            .replace("&#9670;", "◇")
            .replace('★', "☆")
            .replace("&#9733;", "☆")
    } else {
        // TODO: smell
        let trip = sanitize(&name_segments[1..].concat())
            .replace('◆', "◇")
            .replace("&#9670;", "◇");
        let trip = calculate_trip(&trip);
        format!("{name}◆{trip}")
    };

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
    env: &Env,
    board_keys: &HashMap<String, usize>,
    ua: Option<String>,
    repo: &BbsRepository<'_>,
    token_cookie: Option<&str>,
    tinker_token: Option<&str>,
) -> Result<Response> {
    let router =
        match BbsCgiRouter::new(req, env, repo, board_keys, token_cookie, tinker_token, ua).await {
            Ok(router) => router,
            Err(resp) => return resp,
        };

    router.route().await
}

struct BbsCgiRouter<'a> {
    repo: &'a BbsRepository<'a>,
    board_id: usize,
    token_cookie: Option<&'a str>,
    tinker_token: Option<&'a str>,
    tinker_secret: Option<String>,
    ip_addr: String,
    form: BbsCgiForm,
    unix_time: u64,
    id: Option<String>,
    ua: Option<String>,
    host_url: String,
    asn: u32,
    default_name: String,
    local_debugging: bool,
    using_hard_min_recent_res_span_cap: bool,
}

impl<'a> BbsCgiRouter<'a> {
    async fn new(
        req: &'a mut Request,
        env: &Env,
        repo: &'a BbsRepository<'a>,
        board_keys: &'a HashMap<String, usize>,
        token_cookie: Option<&'a str>,
        tinker_token: Option<&'a str>,
        ua: Option<String>,
    ) -> std::result::Result<BbsCgiRouter<'a>, Result<Response>> {
        let (ip_addr, local_debugging) =
            if let Ok(Some(ip_addr)) = req.headers().get("CF-Connecting-IP") {
                (ip_addr, false)
            } else {
                // Use DEBUG_IP if it is set
                if let Ok(ip_addr) = env.var("DEBUG_IP") {
                    (ip_addr.to_string(), true)
                } else {
                    return Err(Response::error(
                        "internal server error - cf-connecting-ip",
                        500,
                    ));
                }
            };

        let using_hard_min_recent_res_span_cap = env
            .var("HARD_MIN_RECENT_RES_SPAN_CAP")
            .ok()
            .map(|x| x.to_string() == "true")
            .unwrap_or(false);

        let Ok(req_bytes) = req.bytes().await else {
            return Err(Response::error("Bad request - read bytes", 400));
        };
        let form = match extract_forms(req_bytes) {
            Some(form) => form,
            None => return Err(Response::error("Bad request - extract forms", 400)),
        };
        let host_url = utils::get_host_url(req)?;

        if let Err(e) = form.validate() {
            return Err(response_shift_jis_text_html(
                WRITING_FAILED_HTML_RESPONSE.replace("{reason}", e),
            ));
        }

        let Some(board_id) = board_keys.get(&form.board_key) else {
            return Err(response_shift_jis_text_html(
                WRITING_FAILED_HTML_RESPONSE
                    .replace("{reason}", "書き込もうとしている板が存在しません"),
            ));
        };
        let Some(board_conf) = get_board_info(env, *board_id, &form.board_key) else {
            return Err(response_shift_jis_text_html(
                WRITING_FAILED_HTML_RESPONSE
                    .replace("{reason}", "書き込もうとしている板が存在しません"),
            ));
        };

        let tinker_secret = env.var("TINKER_SECRET").ok().map(|x| x.to_string());

        Ok(Self {
            repo,
            board_id: *board_id,
            token_cookie,
            tinker_token,
            tinker_secret,
            ip_addr,
            default_name: board_conf.default_name.clone(),
            form,
            unix_time: get_unix_timestamp_sec(),
            id: None,
            ua,
            host_url,
            local_debugging,
            asn: if local_debugging {
                0
            } else {
                req.cf().map(|x| x.asn()).unwrap_or_else(|| 0)
            },
            using_hard_min_recent_res_span_cap,
        })
    }

    async fn route(mut self) -> Result<Response> {
        // Reject too fast reponses by IP here
        if maybe_reject_ip(&self.ip_addr)? {
            return response_shift_jis_text_html(
                WRITING_FAILED_HTML_RESPONSE.replace("{reason}", "5秒以内の連続投稿はできません"),
            );
        }

        let moderator_cap = if let Some(cap) = &self.form.cap {
            if self.token_cookie.is_some() && cap.starts_with('@') {
                let mut hasher = sha2::Sha512::new();
                hasher.update(cap.as_str()[1..].as_bytes());
                let result = hasher.finalize();
                let hash = format!("{:x}", result);

                self.repo
                    .get_cap_by_password_hash(&hash)
                    .await
                    .ok()
                    .flatten()
            } else {
                None
            }
        } else {
            None
        };

        let (token_cookie_candidate, is_cap) = match (self.token_cookie, self.form.cap.as_deref()) {
            (_, Some(cap))
                if self
                    .ua
                    .as_ref()
                    .map(|x| x.contains("BathyScaphe"))
                    .unwrap_or(false) =>
            {
                (Some(cap), true)
            }
            (None, _)
                // For Cookie supporting browsers
                if self
                    .ua
                    .as_ref()
                    .map(|x| x.contains("2chMate"))
                    .unwrap_or(false) =>
            {
                (None, false)
            }
            (Some(cookie), _) => (Some(cookie), false),
            (None, Some(cap)) => (Some(cap), true),
            (None, None) => (None, false),
        };

        let authenticated_user_cookie = if let Some(tk) = token_cookie_candidate {
            let Ok(authed_token) = self.repo.get_authed_token(tk).await else {
                return Response::error("internal server error - check auth", 500);
            };
            if let Some(authed_token) = authed_token {
                if authed_token.authed == 1 {
                    Some(authed_token)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        let Some(authenticated_user_cookie) = authenticated_user_cookie else {
            if self.host_url.contains("workers.dev") {
                return response_shift_jis_text_html(WRITING_FAILED_HTML_RESPONSE.replace(
                    "{reason}",
                    "旧ドメインからの新規認証は終了しました。<br>新ドメインの板 https://bbs.eddibb.cc/liveedge/ を新規に外部板登録してから書き込んでください。",
                ));
            }

            // If the user is trying to get authed cookie too many times, it might be a script.
            // Even if not, it may be better to reject such access to reduce write access to db.
            let n_r_auth = n_recent_auth(&self.ip_addr)?;
            if n_r_auth >= N_MAX_RECENT_AUTH_PER_IP {
                return response_shift_jis_text_html(WRITING_FAILED_HTML_RESPONSE.replace(
                    "{reason}",
                    "発行ずみの認証トークンを使うか、時間を置いて再度アクセスして下さい",
                ));
            }
            let mut hasher: Md5 = Md5::new();
            hasher.update(&self.ip_addr);
            hasher.update(&self.unix_time.to_string());
            let hash = hasher.finalize();
            let token = format!("{:x}", hash);
            let auth_code = generate_six_digit_num();
            let writed_time = get_unix_timestamp_sec().to_string();

            let authed_token = CreatingAuthedToken {
                token: &token,
                origin_ip: &self.ip_addr,
                writed_time: &writed_time,
                auth_code: &auth_code,
            };
            if let Err(e) = self.repo.create_authed_token(authed_token).await {
                return Response::error(format!("internal server error - {e}"), 500);
            }

            let is_mate = self.ua.map(|x| x.contains("Mate")).unwrap_or(false);

            let auth_body = if self.local_debugging {
                REQUEST_AUTHENTICATION_LOCAL.replace("{token}", &token)
            } else if is_mate {
                REQUEST_AUTHENTICATION_HTML
                    .replace("{token}", &token)
                    .replace("{host_url}", &self.host_url)
            } else {
                REQUEST_AUTHENTICATION_CODE_HTML
                    .replace("{auth_code}", &auth_code)
                    .replace("{host_url}", &self.host_url)
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
        };

        let hs256_key = if let Some(tinker_secret) = &self.tinker_secret {
            if let Ok(key) =
                base64::engine::general_purpose::STANDARD.decode(tinker_secret.as_bytes())
            {
                Some(HS256Key::from_bytes(&key))
            } else {
                None
            }
        } else {
            None
        };

        let mut tinker = if let Some(hs256_key) = &hs256_key {
            Some(if let Some(tinker_tk) = self.tinker_token {
                let tinker = hs256_key.verify_token::<Tinker>(tinker_tk, None);

                match tinker {
                    Ok(tinker)
                        if tinker.custom.authed_token == authenticated_user_cookie.cookie =>
                    {
                        tinker.custom
                    }
                    _ => Tinker::new(authenticated_user_cookie.cookie.clone()),
                }
            } else {
                Tinker::new(authenticated_user_cookie.cookie.clone())
            })
        } else {
            None
        };

        // Reject too fast reponses by cookie here
        if maybe_reject_cookie(&authenticated_user_cookie.cookie)? {
            return response_shift_jis_text_html(
                WRITING_FAILED_HTML_RESPONSE.replace("{reason}", "5秒以内の連続投稿はできません"),
            );
        }

        if self.using_hard_min_recent_res_span_cap {
            let min_recent_res_span = match self
                .get_min_recent_res_span(&authenticated_user_cookie.cookie)
                .await
            {
                Ok(min_recent_res_span) => min_recent_res_span,
                Err(e) => return Response::error(e, 500),
            };
            if min_recent_res_span < 5 {
                return response_shift_jis_text_html(
                    WRITING_FAILED_HTML_RESPONSE
                        .replace("{reason}", "5秒以内の連続投稿はできません"),
                );
            }
        }

        if let Some(s) = &authenticated_user_cookie.last_thread_creation {
            if self.form.is_thread && self.unix_time - s.parse::<u64>().unwrap() < 120 {
                return response_shift_jis_text_html(
                    WRITING_FAILED_HTML_RESPONSE.replace("{reason}", "ちょっとスレ立てすぎ！"),
                );
            }
        }

        let datetime = get_current_date_time();
        let reduced_ip_addr = get_reduced_ip_addr(&authenticated_user_cookie.clone().origin_ip);
        let id = calculate_trip(&format!(
            "{}:{}:{}",
            reduced_ip_addr,
            datetime.date(),
            self.board_id
        ))
        .chars()
        .take(9)
        .collect::<String>();

        self.id = if moderator_cap.is_some() {
            Some(CAP_ID_NONE.to_string())
        } else {
            Some(id)
        };

        if let Some(tinker) = tinker.as_mut() {
            tinker.wrote_count += 1;

            if self.unix_time - tinker.last_wrote_at <= 5 {
                return response_shift_jis_text_html(
                    WRITING_FAILED_HTML_RESPONSE
                        .replace("{reason}", "5秒以内の連続投稿はできません"),
                );
            }

            tinker.last_wrote_at = self.unix_time;
            if self.form.is_thread {
                tinker.created_thread_count += 1;
            }
            if tinker.last_level_up_at + 60 * 60 * 23 < self.unix_time && tinker.level < 20 {
                tinker.level += 1;
                tinker.last_level_up_at = self.unix_time;
            }
        }

        let tinker_tk = if let (Some(tinker), Some(hs256_key)) = (&tinker, hs256_key) {
            if let Ok(tinker) = hs256_key.authenticate(Claims::with_custom_claims(
                tinker.clone(),
                jwt_simple::prelude::Duration::new(60 * 60 * 24 * 365, 0),
            )) {
                Some(tinker)
            } else {
                None
            }
        } else {
            None
        };

        if let Some(moderator_cap) = moderator_cap {
            self.form.name.push_str("★ ");
            self.form.name.push_str(&moderator_cap.cap_name);
        }

        let result = if self.form.is_thread {
            self.create_thread(&authenticated_user_cookie.cookie, &tinker)
                .await
        } else {
            self.create_response(&authenticated_user_cookie.cookie, &tinker)
                .await
        };

        if is_cap {
            let tk = authenticated_user_cookie.cookie;
            result.map(|mut x| {
                x.headers_mut()
                    .append(
                        "Set-Cookie",
                        &format!("edge-token={tk}; Max-Age=31536000; Path=/"),
                    )
                    .unwrap();
                x
            })
        } else {
            result
        }
        .map(|mut x| {
            if let Some(tinker) = tinker_tk {
                x.headers_mut()
                    .append(
                        "Set-Cookie",
                        &format!("tinker-token={tinker}; Max-Age=31536000; Path=/"),
                    )
                    .unwrap();
            }
            x
        })
    }

    /// Returns the number of recent responses per second for this token.
    async fn get_min_recent_res_span(
        &self,
        cookie: &str,
    ) -> std::result::Result<u64, &'static str> {
        let Ok(responses) = self
            .repo
            .get_responses_by_authed_token_and_timestamp(
                cookie,
                &(self.unix_time - RECENT_RES_SECONDS).to_string(),
            )
            .await
        else {
            return Err("internal server error - auth min response span");
        };

        if responses.is_empty() {
            return Ok(u64::max_value());
        }
        let mut time_stamps = responses
            .iter()
            .map(|res| res.timestamp)
            .collect::<Vec<_>>();
        time_stamps.push(self.unix_time);
        time_stamps.sort();
        let ts_min = time_stamps
            .iter()
            .zip(time_stamps.iter().skip(1))
            .map(|(&ts_i, &ts_i1)| ts_i1 - ts_i)
            .min()
            .unwrap();
        Ok(ts_min)
    }

    async fn create_thread(self, cookie: &str, tinker: &Option<Tinker>) -> Result<Response> {
        let BbsCgiForm {
            subject,
            name,
            mail,
            body,
            ..
        } = &self.form;

        let (body_opt, mt) = if body.contains("!metadent:vvv:") {
            (
                Some(body.replacen("!metadent:vvv:", "!metadent:vvv - configured", 1)),
                MetadentType::VVVerbose,
            )
        } else if body.contains("!metadent:vv:") {
            (
                Some(body.replacen("!metadent:vv:", "!metadent:vv - configured", 1)),
                MetadentType::VVerbose,
            )
        } else if body.contains("!metadent:v:") {
            (
                Some(body.replacen("!metadent:v:", "!metadent:v - configured", 1)),
                MetadentType::Verbose,
            )
        } else {
            (None, MetadentType::None)
        };

        let name = self.generate_name_with_metadent(name, &mt, tinker);

        let unix_time = self.unix_time.to_string();
        let thread = CreatingThread {
            title: subject.as_ref().unwrap(),
            unix_time: &unix_time,
            body: if let Some(body) = &body_opt {
                body
            } else {
                body
            },
            name: &name,
            mail,
            date_time: &get_current_date_time_string(true),
            author_ch5id: self.id.as_ref().unwrap(),
            authed_token: cookie,
            ip_addr: &self.ip_addr,
            board_id: self.board_id,
            metadent: mt,
        };

        match self.repo.create_thread(thread).await {
            Ok(_) => {
                let _ = self
                    .repo
                    .update_authed_token_last_thread_creation(cookie, &unix_time)
                    .await;
                response_shift_jis_text_html(WRITING_SUCCESS_HTML_RESPONSE.to_string())
            }
            Err(e) => {
                if e.to_string().contains("thread already exists") {
                    response_shift_jis_text_html(
                        WRITING_FAILED_HTML_RESPONSE
                            .replace("{reason}", "同じ時間に既にスレッドが立っています"),
                    )
                } else {
                    Response::error(format!("internal server error - {e}"), 500)
                }
            }
        }
    }

    async fn create_response(self, cookie: &str, tinker: &Option<Tinker>) -> Result<Response> {
        let BbsCgiForm {
            name,
            mail,
            body,
            thread_id,
            ..
        } = &self.form;

        let Ok(thread_info) = self
            .repo
            .get_thread(self.board_id, thread_id.as_ref().unwrap())
            .await
        else {
            return Response::error("internal server error - get thread", 500);
        };

        let thread_info = if let Some(thread_info) = thread_info {
            if thread_info.active == 0 {
                return response_shift_jis_text_html(WRITING_FAILED_HTML_RESPONSE.replace(
                    "{reason}",
                    "スレッドストッパーが働いたみたいなので書き込めません",
                ));
            }
            thread_info
        } else {
            return response_shift_jis_text_html(
                WRITING_FAILED_HTML_RESPONSE.replace("{reason}", "そのようなスレは存在しません"),
            );
        };

        let name = self.generate_name_with_metadent(name, &thread_info.metadent_type(), tinker);
        let res = CreatingRes {
            unix_time: &self.unix_time.to_string(),
            body,
            name: &name,
            mail,
            date_time: &get_current_date_time_string(true),
            authed_token: cookie,
            ip_addr: &self.ip_addr,
            board_id: self.board_id,
            author_ch5id: self.id.as_ref().unwrap(),
            thread_id: thread_id.as_ref().unwrap(),
        };

        match self
            .repo
            .create_response(res, thread_info.modulo as usize)
            .await
        {
            Ok(_) => response_shift_jis_text_html(WRITING_SUCCESS_HTML_RESPONSE.to_string()),
            Err(e) => Response::error(format!("internal server error - {e}"), 500),
        }
    }

    fn generate_name_with_metadent(
        &self,
        name: &str,
        metadent_type: &MetadentType,
        tinker: &Option<Tinker>,
    ) -> String {
        match (tinker, metadent_type) {
            (Some(_), MetadentType::None) | (None, _) => name.to_string(),
            (Some(tinker), MetadentType::Verbose) => {
                let mut name = if name.is_empty() {
                    self.default_name.clone()
                } else {
                    name.to_string()
                };
                name.push_str(&format!(" </b>(L{})<b>", tinker.level));
                name
            }
            (Some(_), MetadentType::VVerbose) => {
                let mut name = if name.is_empty() {
                    self.default_name.clone()
                } else {
                    name.to_string()
                };
                let metadent = generate_meta_ident(
                    self.asn,
                    &self.ip_addr,
                    self.ua.as_ref().unwrap_or(&"Unknown".to_string()),
                    generate_date_seed(),
                );
                name.push_str(&format!(
                    " </b>({})<b>",
                    if let Some(id) = &self.id {
                        if id == CAP_ID_NONE {
                            "????-????"
                        } else {
                            &metadent
                        }
                    } else {
                        &metadent
                    }
                ));
                name
            }
            (Some(tinker), MetadentType::VVVerbose) => {
                let mut name = if name.is_empty() {
                    self.default_name.clone()
                } else {
                    name.to_string()
                };
                let metadent = generate_meta_ident(
                    self.asn,
                    &self.ip_addr,
                    self.ua.as_ref().unwrap_or(&"Unknown".to_string()),
                    generate_date_seed(),
                );
                name.push_str(&format!(
                    " </b>(L{} {})<b>",
                    tinker.level,
                    if let Some(id) = &self.id {
                        if id == CAP_ID_NONE {
                            "????-????"
                        } else {
                            &metadent
                        }
                    } else {
                        &metadent
                    }
                ));
                name
            }
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

fn sanitize_thread_name(input: &str) -> String {
    let sanitized = sanitize(input);
    // Delete all of semicolon closing \n character references
    let re = Regex::new(r"&#([Xx]0*[aA]|0*10);").unwrap();
    let rn_sanitized = re.replace_all(&sanitized, "");

    sanitize_non_semi_closing_num_char_refs(&rn_sanitized)
}

// Delete all of non-semicolon closing numeric character references
fn sanitize_non_semi_closing_num_char_refs(target: &str) -> String {
    let mut sanitized = Vec::new();
    let mut ampersand_used = -1;
    let mut total_removed_len = 0;
    enum NumRefKind {
        Undef, // this state is only cause after reading "&#"
        Hex,
        Dec,
    }
    let mut in_num_ref = None;
    for (i, c) in target.chars().enumerate() {
        if let Some(kind) = &in_num_ref {
            if c == ';' {
                in_num_ref = None;
                sanitized.push(c);
            } else {
                match kind {
                    NumRefKind::Undef => {
                        match c {
                            'x' | 'X' => in_num_ref = Some(NumRefKind::Hex),
                            '0'..='9' => in_num_ref = Some(NumRefKind::Dec),
                            _ => in_num_ref = None,
                        };
                        sanitized.push(c);
                    }
                    NumRefKind::Hex => match c {
                        '0'..='9' | 'a'..='f' | 'A'..='F' => sanitized.push(c),
                        _ => {
                            // invalid non-semicolon closing numeric character references
                            in_num_ref = None;
                            sanitized =
                                sanitized[0..ampersand_used as usize - total_removed_len].to_vec();
                            total_removed_len += i - ampersand_used as usize;
                            sanitized.push(c);
                            if c == '&' {
                                ampersand_used = i as isize;
                            }
                        }
                    },
                    NumRefKind::Dec => match c {
                        '0'..='9' => sanitized.push(c),
                        _ => {
                            // invalid non-semicolon closing numeric character references
                            in_num_ref = None;
                            sanitized =
                                sanitized[0..ampersand_used as usize - total_removed_len].to_vec();
                            total_removed_len += i - ampersand_used as usize;
                            sanitized.push(c);
                            if c == '&' {
                                ampersand_used = i as isize;
                            }
                        }
                    },
                }
            }
        } else {
            sanitized.push(c);
            if c == '&' {
                ampersand_used = i as isize;
            } else if ampersand_used == (i as isize - 1) && c == '#' {
                in_num_ref = Some(NumRefKind::Undef);
            }
        }
    }

    if in_num_ref.is_some() {
        sanitized = sanitized[0..ampersand_used as usize - total_removed_len].to_vec();
    }

    sanitized.into_iter().collect::<String>()
}

// &str is utf-8 bytes
pub fn calculate_trip(target: &str) -> String {
    let bytes = encoding_rs::SHIFT_JIS.encode(target).0.into_owned();

    if bytes.len() >= 12 {
        let mut hasher = Sha1::new();
        hasher.update(&bytes);

        let calc_bytes = Vec::from(hasher.finalize().as_slice());
        let result = &general_purpose::STANDARD.encode(calc_bytes)[0..12];
        result.to_string().replace('+', ".")
    } else {
        let mut salt = Vec::from(if bytes.len() >= 3 { &bytes[1..=2] } else { &[] });
        salt.push(0x48);
        salt.push(0x2e);
        let salt = salt
            .into_iter()
            .map(|x| match x {
                0x3a..=0x40 => x + 7,
                0x5b..=0x60 => x + 6,
                46..=122 => x,
                _ => 0x2e,
            })
            .collect::<Vec<_>>();

        let salt = std::str::from_utf8(&salt).unwrap();
        let result = unix::crypt(bytes.as_slice(), salt).unwrap();
        result[3..].to_string()
    }
}

// for !metadent:vv, !metadent:vvv (vvv is currently disabled)
// (XXYY-zABB):
//   XX is generated from asn number ((asn + date_seed) % (len(a-zA-Z0-9))^2 to 2 byte char array to string)
//   YY is generated from ip_addr (if v6, only use first 4 segments)
//   z is 4 if v4, 6 if v6 (this segment does not use date_seed)
//   A is generated from type of Browser
//   BB is generated from UA
fn generate_meta_ident(asn: u32, ip_addr: &str, ua: &str, seed: u32) -> String {
    let alpha_char_62_to_ascii = |x: u8| match x {
        0..=9 => x + b'0',
        10..=35 => (x - 10) + b'A',
        36..=61 => (x - 36) + b'a',
        _ => b'0',
    };
    let num_to_2byte_chars = |x: u32| {
        let (first, second) = ((x / 62) as u8, (x % 62) as u8);
        vec![first, second]
            .into_iter()
            .map(alpha_char_62_to_ascii)
            .map(|x| x as char)
            .collect::<String>()
    };

    let xx = (asn + seed) % (62 * 62);
    let xx = num_to_2byte_chars(xx);

    let is_v6 = ip_addr.contains(':');
    let yy = ip_addr
        .split(if is_v6 { ':' } else { '.' })
        .take(4)
        .map(|x| {
            if is_v6 {
                if x.is_empty() {
                    0u64
                } else {
                    u64::from_str_radix(x, 16).unwrap_or(0)
                }
            } else {
                x.parse::<u64>().unwrap_or(0)
            }
        })
        .sum::<u64>()
        + seed as u64;

    let yy = (yy % (62 * 62)) as u32;
    let yy = num_to_2byte_chars(yy);
    let z = if is_v6 { 6 } else { 4 };

    let a = if ua.contains("Mate") {
        0
    } else if ua.contains("twinkle") {
        1
    } else if ua.contains("mae") {
        2
    } else if ua.contains("Siki") {
        3
    } else if ua.contains("Xeno") {
        4
    } else if ua.contains("ThreadMaster") {
        5
    } else {
        6
    } + seed;

    let a = (a % 62) as u8;
    let a = alpha_char_62_to_ascii(a) as char;

    let mut hasher = Md5::new();
    hasher.update(ua);
    let bb = hasher.finalize();

    let bb = bb
        .iter()
        .map(|x| *x as char)
        .filter(|x| x.is_ascii_alphanumeric())
        .take(2)
        .collect::<String>();

    format!("{xx}{yy}-{z}{a}{bb}",)
}

fn generate_date_seed() -> u32 {
    let n = get_unix_timestamp_sec();
    ((n / (60 * 60 * 24) / 7) % i32::max_value() as u64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_trip_over_12() {
        let test_cases = [
            ("aaaaaaaaaaaa", "OE/NFgqzszF0"),
            ("babababababababababa", "39J6Edxx77KI"),
            ("あああああああああああああああ", "3Djq3jN287f."),
        ];
        for (case, expected) in test_cases.iter() {
            assert_eq!(&calculate_trip(case), expected);
        }
    }

    #[test]
    fn test_calculate_trip_under_12() {
        let test_cases = [
            ("a", "ZnBI2EKkq."),
            ("あああ", "GJolKKvjNA"),
            ("aaあaあ", "oR7LYZCwJk"),
            ("6g9@Bt(6", "qCscNtsFCg"),
        ];

        for (case, expected) in test_cases.iter() {
            assert_eq!(&calculate_trip(case), expected);
        }
    }

    #[test]
    fn test_form_validation() {
        let test_cases = [
            (
                BbsCgiForm {
                    subject: Some("あ".repeat(97)),
                    name: "".to_string(),
                    mail: "".to_string(),
                    body: "a".repeat(12),
                    board_key: "abc".to_string(),
                    is_thread: true,
                    thread_id: None,
                    cap: None,
                },
                Err("スレッドタイトルが長すぎます"),
            ),
            (
                BbsCgiForm {
                    subject: None,
                    name: "".to_string(),
                    mail: "".to_string(),
                    body: "あい\n".repeat(60).to_string(),
                    board_key: "abc".to_string(),
                    is_thread: true,
                    thread_id: None,
                    cap: None,
                },
                Err("本文に改行が多すぎます"),
            ),
        ];

        for (case, expected) in test_cases.into_iter() {
            assert_eq!(expected, case.validate());
        }
    }

    #[test]
    fn test_metadent() {
        let n = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let seed = ((n / (60 * 60 * 24) / 7) % i32::max_value() as u64) as u32;
        let cands = [
            (17676, "127.0.0.1", "Mate/1.0.0"),
            (9605, "91b4:320f:123a:ff", "Xeno/1.0.0"),
            (2516, "866c:0fa3::f3:aa", "Mate/2.0.123"),
        ];

        for c in cands.iter() {
            let result = generate_meta_ident(c.0, c.1, c.2, seed);
            println!("{result}");
        }
    }

    #[test]
    fn test_sanitize_non_semi_closing_num_char_refs() {
        let test_cases = [
            // Test case 1
            (
                "This is a text with &#32; spaces.",
                "This is a text with &#32; spaces.",
            ),
            // Test case 2
            (
                "&#32;Hello, &#xa;world!&#65;",
                "&#32;Hello, &#xa;world!&#65;",
            ),
            // Test case 3
            (
                "This is an invalid numeric reference: &#32.",
                "This is an invalid numeric reference: .",
            ),
            // Test case 4
            (
                "Invalid numeric references: &#32&#65&#xa.",
                "Invalid numeric references: .",
            ),
            // Test case 5
            (
                "Mix of valid and invalid: &#32&#65;&#xa&#98aてすと&#99;.",
                "Mix of valid and invalid: &#65;aてすと&#99;.",
            ),
            // Test case 6
            (
                "Mix of valid and invalid: &#32Hello, &#xa;&#65;world!",
                "Mix of valid and invalid: Hello, &#xa;&#65;world!",
            ),
            // Test case 7
            ("", ""),
            // Test case 8
            (
                "No numeric references here. だよね",
                "No numeric references here. だよね",
            ),
            // Test case 9
            ("&#32&#xa", ""),
            // Test case 10
            ("&#32;&#xa;", "&#32;&#xa;"),
        ];

        for (input, expected) in test_cases.iter() {
            assert_eq!(*expected, sanitize_non_semi_closing_num_char_refs(input));
        }
    }
}
