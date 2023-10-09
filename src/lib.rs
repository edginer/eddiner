use cookie::Cookie;
use routes::{
    auth::{route_auth_get, route_auth_post},
    auth_code::{route_auth_code_get, route_auth_code_post},
    bbs_cgi::route_bbs_cgi,
    dat_routing::route_dat,
    head_txt::route_head_txt,
    liveedge::route_liveedge,
    subject_txt::route_subject_txt,
};

use worker::*;

mod authed_cookie;
pub mod response;
mod thread;
mod utils;
pub(crate) mod routes {
    pub(crate) mod auth;
    pub(crate) mod auth_code;
    pub(crate) mod bbs_cgi;
    pub(crate) mod dat_routing;
    pub(crate) mod head_txt;
    pub(crate) mod liveedge;
    pub(crate) mod setting_txt;
    pub(crate) mod subject_txt;
}
mod turnstile;

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

#[event(fetch)]
async fn main(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let Some((site_key, secret_key)) = get_secrets(&env) else {
        return Response::error("internal server error", 500);
    };

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
        "/liveedge/" | "/liveedge" => route_liveedge(),
        "/liveedge/SETTING.TXT" => routes::setting_txt::route_setting_txt(),
        "/liveedge/subject.txt" => {
            let Ok(db) = env.d1("DB") else {
                return Response::error("internal server error: DB", 500);
            };
            route_subject_txt(&db).await
        }
        "/liveedge/head.txt" => route_head_txt(),
        "/test/bbs.cgi" => {
            if req.method() != Method::Post {
                return Response::error("Bad request", 400);
            }

            let Ok(db) = env.d1("DB") else {
                return Response::error("internal server error - db", 500);
            };

            route_bbs_cgi(&mut req, ua, &db, token_cookie.as_deref()).await
        }
        e if e.starts_with("/liveedge/dat/") && e.ends_with(".dat") => {
            let Ok(db) = env.d1("DB") else {
                return Response::error("internal server error: DB", 500);
            };

            let range = req.headers().get("Range").ok().flatten();
            let if_modified_since = req.headers().get("If-Modified-Since").ok().flatten();

            route_dat(e, ua, range, if_modified_since, &db).await
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
