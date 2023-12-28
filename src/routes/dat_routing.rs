use std::str::FromStr;

use worker::*;

use crate::{
    board_config::BoardConfig, repositories::bbs_repository::BbsRepository,
    response::Ch5ResponsesFormatter,
};

pub struct DatRoutingThreadInfo<'a> {
    pub board_conf: &'a BoardConfig<'a>,
    pub thread_id: &'a str,
}

pub async fn route_dat(
    req: &Request,
    thread_info: DatRoutingThreadInfo<'_>,
    ua: &Option<String>,
    repo: &BbsRepository<'_>,
    bucket: &Option<Bucket>,
    host: String,
) -> Result<Response> {
    let range = req.headers().get("Range").ok().flatten();
    let if_modified_since = req.headers().get("If-Modified-Since").ok().flatten();

    let Ok(thread) = repo
        .get_thread(thread_info.board_conf.board_id, thread_info.thread_id)
        .await
    else {
        return Response::error("internal server error - get thread", 500);
    };

    let Some(thread) = thread else {
        return if bucket.is_some() {
            let generate_url = |thread_id: &str| {
                Url::from_str(&format!(
                    "http://bbs.eddibb.cc/{}/kako/{}/{}/{}.dat",
                    thread_info.board_conf.board_key,
                    &thread_id[0..4],
                    &thread_id[0..5],
                    thread_id
                ))
                .unwrap()
            };
            return Response::redirect(generate_url(thread_info.thread_id));
        } else {
            Response::error("Not found - dat", 404)
        };
    };
    if let Some(if_modified_since) = if_modified_since {
        if let Ok(parsed_date_time) =
            chrono::NaiveDateTime::parse_from_str(&if_modified_since, "%Y/%m/%d %H:%M:%S")
        {
            let remote_last_modified = parsed_date_time.timestamp() - 32400; // fix local time

            if remote_last_modified >= thread.last_modified.parse::<i64>().unwrap() {
                return Response::empty().map(|mut r| {
                    let _ = r.headers_mut().append("Cache-Control", "s-maxage=1");
                    r.with_status(304)
                });
            }
        }
    }

    let mut responses = match repo
        .get_responses(
            thread_info.board_conf.board_id,
            thread_info.thread_id,
            thread.modulo as usize,
        )
        .await
    {
        Ok(o) => o,
        Err(e) => return Response::error(format!("internal server error - {e}"), 500),
    };

    if host.contains("workers.dev") {
        if let Some(first_res) = responses.get_mut(0) {
            first_res.body
                .push_str(
                    "<br><br> 【以下運営からのメッセージ】<br>あなたは将来的に廃止される旧ドメインを使用しています。 <br>新ドメイン https://bbs.eddibb.cc/liveedge/ に移行してください<br>旧ドメインからの新規認証は終了しました。"
                )
        }
    }

    let body = responses.format_responses(&thread.title, &thread_info.board_conf.default_name);
    let sjis_body = encoding_rs::SHIFT_JIS.encode(&body).0.into_owned();

    let ranged_sjis_body = match (range, ua) {
        (Some(range), Some(ua)) if !ua.contains("Mate") && !ua.contains("Xeno") => {
            if let Some(range) = range.split('=').nth(1) {
                let range = range.split('-').collect::<Vec<_>>();
                let Some(start) = range.first().and_then(|x| x.parse::<usize>().ok()) else {
                    return Response::error("Bad request", 400);
                };

                Some(
                    sjis_body
                        .clone()
                        .into_iter()
                        .skip(start)
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            }
        }
        _ => None,
    };
    let Ok(mut resp) = Response::from_bytes(sjis_body) else {
        return Response::error("internal server error - converting sjis", 500);
    };

    let _ = resp.headers_mut().delete("Content-Type");
    let _ = resp.headers_mut().append("Content-Type", "text/plain");
    let _ = resp.headers_mut().append(
        "Cache-Control",
        if thread.active == 0 {
            "s-maxage=3600"
        } else {
            "s-maxage=1"
        },
    );

    if let Some(ranged_resp) = ranged_sjis_body {
        let Ok(mut ranged_resp) = Response::from_bytes(ranged_resp) else {
            return Response::error("internal server error - converting sjis", 500);
        };

        let _ = ranged_resp.headers_mut().delete("Content-Type");
        let _ = ranged_resp
            .headers_mut()
            .append("Content-Type", "text/plain");

        if resp.status_code() == 200 {
            let _ = Cache::default().put(req, resp).await;
        }

        Ok(ranged_resp.with_status(206))
    } else {
        if let Ok(result) = resp.cloned() {
            if result.status_code() == 200 {
                let _ = Cache::default().put(req, result).await;
            }
        }

        Ok(resp)
    }
}
