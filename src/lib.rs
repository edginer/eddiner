use cookie::Cookie;
use routes::{
    auth::{route_auth_get, route_auth_post},
    auth_code::{route_auth_code_get, route_auth_code_post},
    bbs_cgi::route_bbs_cgi,
    dat_routing::route_dat,
    liveedge::route_liveedge,
    subject_txt::route_subject_txt,
};
use thread::Thread;
use worker::*;

mod authed_cookie;
mod response;
mod thread;
mod utils;
pub(crate) mod routes {
    pub(crate) mod auth;
    pub(crate) mod auth_code;
    pub(crate) mod bbs_cgi;
    pub(crate) mod dat_routing;
    pub(crate) mod liveedge;
    pub(crate) mod setting_txt;
    pub(crate) mod subject_txt;
}
mod turnstile;

fn get_secrets(env: &Env) -> Option<(String, String)> {
    let Ok(site_key) = env.var("SITE_KEY") else {
        return None;
    };
    let site_key = site_key.to_string();
    let Ok(secret_key) = env.var("SECRET_KEY") else {
        return None;
    };
    let secret_key = secret_key.to_string();
    Some((site_key, secret_key))
}

fn get_token_cookies(req: &Request) -> Option<String> {
    let Ok(cookie) = req.headers().get("Cookie") else {
        return None;
    };

    let cookie = cookie.map(|x| Cookie::split_parse(x.to_string()));
    let token_cookie = cookie.map(|x| {
        x.filter_map(|x| {
            if let Ok(x) = x {
                if x.name() != "edge-token" {
                    return None;
                }
                Some((x.name().to_string(), x.value().to_string()))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
    });
    token_cookie.and_then(|x| x.get(0).cloned()).map(|x| x.1)
}

#[event(fetch)]
async fn main(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let Some((site_key, secret_key)) = get_secrets(&env) else {
        return Response::error("internal server error", 500);
    };

    let token_cookie = get_token_cookies(&req);

    match &req.path() as &str {
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
        "/test/bbs.cgi" => {
            if req.method() != Method::Post {
                return Response::error("Bad request", 400);
            }

            let Ok(db) = env.d1("DB") else {
                return Response::error("internal server error - db", 500);
            };

            route_bbs_cgi(&mut req, &db, &token_cookie).await
        }
        e if e.starts_with("/liveedge/dat/") && e.ends_with(".dat") => {
            let Ok(db) = env.d1("DB") else {
                return Response::error("internal server error: DB", 500);
            };
            route_dat(e, &db).await
        }
        _ => Response::error(format!("Not found - other route {}", req.path()), 404),
    }
}

#[event(scheduled)]
async fn scheduled(_req: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    let db = env.d1("DB").unwrap();

    let mut current_threads = db
        .prepare("SELECT * FROM threads WHERE board_id = 1 AND archived = 0")
        .all()
        .await
        .unwrap()
        .results::<Thread>()
        .unwrap();
    if current_threads.len() > 75 {
        current_threads.sort_by_key(|x| x.last_modified.parse::<u64>().unwrap());
        current_threads.reverse();
        let mut threads_to_archive = current_threads.split_off(70);
        threads_to_archive.append(
            &mut current_threads
                .iter()
                .filter(|x| x.active == 0)
                .cloned()
                .collect::<Vec<_>>(),
        );

        let in_tokens = threads_to_archive
            .iter()
            .map(|x| format!("'{}'", x.thread_number))
            .collect::<Vec<_>>()
            .join(",");
        let Ok(stmt) = db
            .prepare(format!(
                "UPDATE threads SET archived = 1 WHERE thread_number IN ({in_tokens})",
            ))
            .bind(&[in_tokens.into()])
        else {
            return;
        };
        stmt.run().await.unwrap();
    }
}
