use std::collections::HashMap;

use board_config::BoardConfig;
use cookie::Cookie;
use repositories::bbs_repository::BbsRepository;
use routes::{
    analyze_route,
    auth::{route_auth_get, route_auth_post},
    auth_code::{route_auth_code_get, route_auth_code_post},
    bbs_cgi::route_bbs_cgi,
    dat_routing::{route_dat, DatRoutingThreadInfo},
    head_txt::route_head_txt,
    subject_txt::route_subject_txt,
    webui,
};
use utils::response_shift_jis_text_plain_with_cache;
use worker::*;

mod authed_cookie;
mod board;
pub(crate) mod board_config;
pub(crate) mod inmemory_cache;
pub mod response;
pub mod routes;
mod thread;
mod turnstile;
mod utils;
pub(crate) mod repositories {
    pub(crate) mod bbs_repository;
}

// TODO(kenmo-melon): 設定可能に? (コンパイル時定数? wrangler.toml?)
const SITE_TITLE: &str = "edgebb";
const SITE_NAME: &str = "エッヂ";
const SITE_DESCRIPTION: &str = "掲示板";

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

fn get_board_keys(env: &Env) -> Option<HashMap<String, usize>> {
    let Ok(board_keys) = env.var("BOARD_KEYS") else {
        return None;
    };
    Some(
        board_keys
            .to_string()
            .split(',')
            .enumerate()
            .map(|(id, key)| (key.to_string(), id + 1))
            .collect::<HashMap<_, _>>(),
    )
}

fn get_board_info<'a>(env: &Env, board_id: usize, board_key: &'a str) -> Option<BoardConfig<'a>> {
    let Ok(board_info) = env.var(board_key) else {
        return None;
    };
    let board_info = board_info.to_string();
    let info_split = board_info.split(',').collect::<Vec<_>>();
    if info_split.len() < 2 {
        None
    } else {
        Some(BoardConfig {
            board_id,
            board_key,
            title: info_split[0].to_string(),
            default_name: info_split[1].to_string(),
        })
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
    let Ok(db) = env.d1("DB") else {
        return Response::error("internal server error: DB", 500);
    };
    let repo = BbsRepository::new(&db);
    let Some(board_keys) = get_board_keys(&env) else {
        return Response::error(
            "internal server error: failed to load environment settings",
            500,
        );
    };

    match analyze_route(req.path().as_str(), &board_keys) {
        routes::Route::Index => {
            if check_webui_disabled(&env) {
                return webui::webui_disabled(SITE_TITLE);
            }
            let board_infos = board_keys
                .iter()
                .filter_map(|(board_key, board_id)| get_board_info(&env, *board_id, board_key))
                .collect::<Vec<_>>();

            webui::route_index(SITE_TITLE, SITE_NAME, SITE_DESCRIPTION, &board_infos)
                .map_err(|e| Error::RustError(format!("Error in index.rs {}", e)))
        }
        routes::Route::Auth => {
            if req.method() == Method::Post {
                route_auth_post(&mut req, &repo, &secret_key).await
            } else if req.method() == Method::Get {
                route_auth_get(&req, &site_key)
            } else {
                Response::error("Bad request", 400)
            }
        }
        routes::Route::AuthCode => {
            if req.method() == Method::Post {
                route_auth_code_post(&mut req, &repo, &secret_key).await
            } else if req.method() == Method::Get {
                route_auth_code_get(&site_key)
            } else {
                Response::error("Bad request", 400)
            }
        }
        routes::Route::BbsCgi => {
            if req.method() != Method::Post {
                return Response::error("Bad request", 400);
            }
            route_bbs_cgi(
                &mut req,
                &env,
                &board_keys,
                ua,
                &repo,
                token_cookie.as_deref(),
            )
            .await
        }
        routes::Route::Dat {
            board_key,
            thread_id,
            board_id,
        } => {
            if let Ok(Some(s)) = cache.get(&req, false).await {
                return Ok(s);
            }

            let bucket = env.bucket("ARCHIVE_BUCKET").ok();

            let range = req.headers().get("Range").ok().flatten();
            let if_modified_since = req.headers().get("If-Modified-Since").ok().flatten();

            let Ok(Some(host_url)) = req.url().map(|url| url.host_str().map(ToOwned::to_owned))
            else {
                return Response::error("internal server error - failed to parse url", 500);
            };

            let Some(board_conf) = get_board_info(&env, board_id, board_key) else {
                return Response::error("internal server error - failed to load board info", 500);
            };
            let mut result = route_dat(
                DatRoutingThreadInfo {
                    board_conf: &board_conf,
                    thread_id,
                },
                &ua,
                range,
                if_modified_since,
                &repo,
                &bucket,
                host_url,
            )
            .await?;
            if let Ok(result) = result.cloned() {
                if result.status_code() == 200 {
                    let _ = cache.put(&req, result).await;
                }
            }

            Ok(result)
        }
        routes::Route::KakoDat {
            board_key,
            thread_id,
            board_id: _,
        } => {
            if let Ok(Some(s)) = cache.get(&req, false).await {
                return Ok(s);
            }

            let Some(bucket) = env.bucket("ARCHIVE_BUCKET").ok() else {
                return Response::error("internal server error - bucket", 500);
            };

            let log = bucket
                .get(format!("{board_key}/dat/{thread_id}.dat"))
                .execute()
                .await?;

            let Some(log) = log else {
                return Response::error("Not found - dat", 404);
            };
            let Some(log_body) = log.body() else {
                return Response::error("Internal server error - dat bucket", 500);
            };

            let log_text = log_body.text().await?;
            let mut result = response_shift_jis_text_plain_with_cache(log_text, 86400)?;
            if let Ok(result) = result.cloned() {
                if result.status_code() == 200 {
                    let _ = cache.put(&req, result).await;
                }
            }

            Ok(result)
        }
        routes::Route::SettingTxt {
            board_key,
            board_id,
        } => {
            let Some(board_conf) = get_board_info(&env, board_id, board_key) else {
                return Response::error("internal server error - failed to load board info", 500);
            };
            routes::setting_txt::route_setting_txt(&board_conf)
        }
        routes::Route::SubjectTxt {
            board_key: _,
            board_id,
        } => {
            if let Ok(Some(s)) = cache.get(&req, false).await {
                return Ok(s);
            }
            let mut result = route_subject_txt(&repo, board_id).await?;

            if let Ok(result) = result.cloned() {
                if result.status_code() == 200 {
                    let _ = cache.put(&req, result).await;
                }
            }

            Ok(result)
        }
        routes::Route::HeadTxt {
            board_key: _,
            board_id,
        } => route_head_txt(board_id, &repo).await,
        routes::Route::BoardIndex {
            board_key,
            board_id,
        } => {
            if check_webui_disabled(&env) {
                return webui::webui_disabled(SITE_TITLE);
            }

            if let Ok(Some(s)) = cache.get(&req, false).await {
                return Ok(s);
            }

            let host_url = match utils::get_host_url(&req) {
                Ok(url) => url,
                Err(res) => return res,
            };
            let Some(board_config) = get_board_info(&env, board_id, board_key) else {
                return Response::error("internal server error - failed to load board info", 500);
            };
            let mut resp = webui::route_board(&host_url, &board_config, &repo).await?;
            if let Ok(result) = resp.cloned() {
                if result.status_code() == 200 {
                    let _ = cache.put(&req, result).await;
                }
            }

            Ok(resp)
        }
        routes::Route::ThreadWebUI {
            board_key,
            thread_id,
            board_id,
        } => {
            // TODO(kenmo-melon): これだと/liveedge/hogehogeのようなURLにもアクセスできるが、
            // DBにたくさんアクセスする羽目になるよりはマシ？
            if check_webui_disabled(&env) {
                return webui::webui_disabled(SITE_TITLE);
            }

            let Ok(Some(host_url)) = req.url().map(|url| url.host_str().map(ToOwned::to_owned))
            else {
                return Response::error("internal server error - failed to parse url", 500);
            };
            if let Ok(Some(s)) = cache.get(&req, false).await {
                return Ok(s);
            }
            let Ok(thread_id) = thread_id.parse::<u64>() else {
                return Response::error("Not found", 404);
            };
            let Some(board_config) = get_board_info(&env, board_id, board_key) else {
                return Response::error("internal server error - failed to load board info", 500);
            };

            let mut resp = webui::route_thread(thread_id, &board_config, &repo, &host_url).await?;
            if let Ok(result) = resp.cloned() {
                if result.status_code() == 200 {
                    let _ = cache.put(&req, result).await;
                }
            }
            Ok(resp)
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
        ORDER BY CAST(last_modified AS INTEGER) DESC LIMIT 3000 OFFSET 60
    )",
    )
    .run()
    .await
    .unwrap();
}
