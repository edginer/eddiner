use worker::*;

use crate::{
    repositories::bbs_repository::BbsRepository,
    response::Ch5ResponsesFormatter,
    utils::{response_shift_jis_text_plain_with_cache, response_shift_jis_with_range},
};

pub async fn route_dat(
    path: &str,
    ua: &Option<String>,
    range: Option<String>,
    if_modified_since: Option<String>,
    repo: &BbsRepository<'_>,
    bucket: &Option<Bucket>,
    host: String,
) -> Result<Response> {
    let thread_id = path.replace(".dat", "").replace("/liveedge/dat/", "");
    let Ok(thread_id) = thread_id.parse::<u64>() else {
        return Response::error("Bad request - url parsing", 400);
    };
    let thread_id = thread_id.to_string();
    // TODO (kenmo-melon): Get the default name from the board config
    let default_name = "エッヂの名無し";

    let Ok(thread) = repo.get_thread(1, &thread_id).await else {
        return Response::error("internal server error - get thread", 500);
    };

    let Some(thread) = thread else {
        return if let Some(bucket) = bucket {
            let log = bucket
                .get(format!("liveedge/dat/{thread_id}.dat"))
                .execute()
                .await?;

            let Some(log) = log else {
                return Response::error("Not found - dat", 404);
            };
            let Some(log_body) = log.body() else {
                return Response::error("Internal server error - dat bucket", 500);
            };

            let log_text = log_body.text().await?;
            response_shift_jis_text_plain_with_cache(log_text, 86400)
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

    let mut responses = match repo.get_responses(1, &thread_id).await {
        Ok(o) => o,
        Err(e) => return Response::error(format!("internal server error - {e}"), 500),
    };

    if host.contains("workers.dev") {
        if let Some(first_res) = responses.get_mut(0) {
            first_res.body
                .push_str(
                    "<br><br> 【以下運営からのメッセージ】<br>あなたは将来的に廃止される旧ドメインを使用しています。 <br>新ドメイン https://bbs.eddibb.cc/liveedge/ に移行してください"
                )
        }
    }

    let body = responses.format_responses(&thread.title, default_name);

    match (range, ua) {
        (Some(range), Some(ua)) if !ua.contains("Mate") && !ua.contains("Xeno") => {
            if let Some(range) = range.split('=').nth(1) {
                let range = range.split('-').collect::<Vec<_>>();
                let Some(start) = range.first().and_then(|x| x.parse::<usize>().ok()) else {
                    return Response::error("Bad request", 400);
                };

                response_shift_jis_with_range(body, start).map(|x| x.with_status(206))
            } else {
                response_shift_jis_text_plain_with_cache(
                    body,
                    if thread.active == 0 { 86400 } else { 1 },
                )
            }
        }
        _ => response_shift_jis_text_plain_with_cache(
            body,
            if thread.active == 0 { 86400 } else { 1 },
        ),
    }
}
