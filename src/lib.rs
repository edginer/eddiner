use board_config::BoardConfig;
use cookie::Cookie;
use routes::{
    auth::{route_auth_get, route_auth_post},
    auth_code::{route_auth_code_get, route_auth_code_post},
    bbs_cgi::route_bbs_cgi,
    dat_routing::route_dat,
    head_txt::route_head_txt,
    subject_txt::route_subject_txt,
    webui,
};
use worker::*;

mod authed_cookie;
pub(crate) mod inmemory_cache;
pub mod response;
mod thread;
mod utils;
pub(crate) mod routes {
    pub(crate) mod auth;
    pub(crate) mod auth_code;
    pub(crate) mod bbs_cgi;
    pub(crate) mod dat_routing;
    pub(crate) mod head_txt;
    pub(crate) mod setting_txt;
    pub(crate) mod subject_txt;
    pub(crate) mod webui;
}
pub(crate) mod board_config;
mod turnstile;

// TODO(kenmo-melon): 設定可能に? (コンパイル時定数? wrangler.toml?)
const SITE_TITLE: &'static str = "edgebb";
const SITE_NAME: &'static str = "エッヂ";
const SITE_DESCRIPTION: &'static str = "掲示板";
pub(crate) const BOARDS: &'static [BoardConfig] = &[BoardConfig {
    board_id: 1,
    title: "なんでも実況エッヂ",
    board_key: "liveedge",
    description: "エッヂ",
    default_name: "エッヂの名無し",
}];

fn get_secrets(env: &Env) -> Option<(String, String)> {
    let site_key = env.var("SITE_KEY").ok()?.to_string();
    let secret_key = env.var("SECRET_KEY").ok()?.to_string();
    Some((site_key, secret_key))
}

/// Find `edge-token` in cookies
fn get_token_cookies(req: &Request) -> Option<String> {
    let cookie_str = req.headers().get("Cookie").ok()??;
    for cookie in Cookie::split_parse(cookie_str).flatten() {
        if cookie.name() == "edge-token" {
            return Some(cookie.value().to_string());
        }
    }
    None
}

/// Returns true if --var=WEBUI:false is passed
fn check_webui_disabled(env: &Env) -> bool {
    match env.var("WEBUI") {
        Ok(var) => var.to_string() == "false",
        _ => false,
    }
}

#[event(fetch)]
async fn main(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let Some((site_key, secret_key)) = get_secrets(&env) else {
        return Response::error("internal server error", 500);
    };

    let cache = Cache::default();
    let token_cookie = get_token_cookies(&req);
    let ua = req.headers().get("User-Agent").ok().flatten();

    match &*req.path() {
        "/auth/" | "/auth" => {
            if req.method() == Method::Post {
                let Ok(db) = env.d1("DB") else {
                    return Response::error("internal server error: DB", 500);
                };
                route_auth_post(&mut req, &db, &secret_key).await
            } else if req.method() == Method::Get {
                route_auth_get(&req, &site_key)
            } else {
                Response::error("Bad request", 400)
            }
        }
        "/auth-code/" | "/auth-code" => {
            if req.method() == Method::Post {
                let Ok(db) = env.d1("DB") else {
                    return Response::error("internal server error: DB", 500);
                };
                route_auth_code_post(&mut req, &db, &secret_key).await
            } else if req.method() == Method::Get {
                route_auth_code_get(&site_key)
            } else {
                Response::error("Bad request", 400)
            }
        }
        "/" | "/index.html" => {
            if check_webui_disabled(&env) {
                return webui::webui_disabled(SITE_TITLE);
            }
            webui::route_index(SITE_TITLE, SITE_NAME, SITE_DESCRIPTION, &BOARDS)
                .map_err(|e| Error::RustError(format!("Error in index.rs {}", e)))
        }
        "/liveedge/" | "/liveedge" => {
            if check_webui_disabled(&env) {
                return webui::webui_disabled(SITE_TITLE);
            }
            let Ok(db) = env.d1("DB") else {
                return Response::error("internal server error - db", 500);
            };
            let host_url = match utils::get_host_url(&req) {
                Ok(url) => url,
                Err(res) => return res,
            };
            webui::route_board(&host_url, &BOARDS[0], &db).await
        }
        "/liveedge/SETTING.TXT" => routes::setting_txt::route_setting_txt(),
        "/liveedge/subject.txt" => {
            if let Ok(Some(s)) = cache.get(&req, false).await {
                return Ok(s);
            }
            let Ok(db) = env.d1("DB") else {
                return Response::error("internal server error: DB", 500);
            };
            let mut result = route_subject_txt(&db).await?;

            if let Ok(result) = result.cloned() {
                if result.status_code() == 200 {
                    let _ = cache.put(&req, result).await;
                }
            }

            Ok(result)
        }
        "/liveedge/head.txt" => route_head_txt(),
        "/test/bbs.cgi" => {
            if req.method() != Method::Post {
                return Response::error("Bad request", 400);
            }

            let Ok(db) = env.d1("DB") else {
                return Response::error("internal server error - db", 500);
            };

            route_bbs_cgi(&mut req, &env, ua, &db, token_cookie.as_deref()).await
        }
        e if e.starts_with("/liveedge/dat/") && e.ends_with(".dat") => {
            let Ok(db) = env.d1("DB") else {
                return Response::error("internal server error: DB", 500);
            };

            let range = req.headers().get("Range").ok().flatten();
            let if_modified_since = req.headers().get("If-Modified-Since").ok().flatten();

            if let Ok(Some(s)) = cache.get(&req, false).await {
                return Ok(s);
            }

            let Ok(Some(host_url)) = req.url().map(|url| url.host_str().map(ToOwned::to_owned))
            else {
                return Response::error("internal server error - failed to parse url", 500);
            };
            let mut result = route_dat(e, &ua, range, if_modified_since, &db, host_url).await?;

            if let Ok(result) = result.cloned() {
                if result.status_code() == 200 {
                    let _ = cache.put(&req, result).await;
                }
            }

            Ok(result)
        }
        e if e.starts_with("/liveedge/") || e.starts_with("/test/read.cgi/liveedge/") => {
            // TODO(kenmo-melon): これだと/liveedge/hogehogeのようなURLにもアクセスできるが、
            // DBにたくさんアクセスする羽目になるよりはマシ？
            if check_webui_disabled(&env) {
                return webui::webui_disabled(SITE_TITLE);
            }
            let board_idx = e.find("/liveedge/").unwrap();
            let rest_url = &e[board_idx + "/liveedge/".len()..];
            let slash_idx = if let Some(slash_idx) = rest_url.find("/") {
                match &rest_url[slash_idx..] {
                    "/" | "/index.html" => slash_idx,
                    _ => return Response::error("Not found", 404),
                }
            } else {
                rest_url.len()
            };
            let Ok(thread_id) = rest_url[..slash_idx].parse::<u64>() else {
                return Response::error("Not found", 404);
            };
            let Ok(db) = env.d1("DB") else {
                return Response::error("internal server error: DB", 500);
            };
            webui::route_thread(thread_id, &BOARDS[0], &db).await
        }
        _ => Response::error(format!("Not found - other route {}", req.path()), 404),
    }
}

#[event(scheduled)]
async fn scheduled(_req: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    let db = env.d1("DB").unwrap();

    db.prepare("UPDATE threads SET archived = 1 WHERE active = 0")
        .run()
        .await
        .unwrap();

    db.prepare(
        "UPDATE threads SET archived = 1, active = 0 WHERE thread_number IN (
        SELECT thread_number
        FROM threads WHERE board_id = 1 AND archived = 0
        ORDER BY CAST(last_modified AS INTEGER) DESC LIMIT 3000 OFFSET 70
    )",
    )
    .run()
    .await
    .unwrap();
}
