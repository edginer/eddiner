use worker::*;

use crate::{
    response::{Ch5ResponsesFormatter, Res},
    thread::Thread,
    utils::{response_shift_jis_text_plain_with_cache, response_shift_jis_with_range},
};

pub(crate) async fn get_all_responses(
    thread_id: &str,
    db: &D1Database,
) -> std::result::Result<Vec<Res>, Result<Response>> {
    let Ok(responses_binded_stmt) = db
        .prepare("SELECT * FROM responses WHERE thread_id = ?")
        .bind(&[thread_id.into()])
    else {
        return Err(Response::error("internal server error", 500));
    };
    let Ok(responses) = responses_binded_stmt
        .all()
        .await
        .and_then(|res| res.results::<Res>())
    else {
        return Err(Response::error("internal server error", 500));
    };
    Ok(responses)
}

pub async fn route_dat(
    path: &str,
    ua: &Option<String>,
    range: Option<String>,
    if_modified_since: Option<String>,
    db: &D1Database,
    host: String,
) -> Result<Response> {
    let thread_id = path.replace(".dat", "").replace("/liveedge/dat/", "");
    let Ok(thread_id) = thread_id.parse::<u64>() else {
        return Response::error("Bad request - url parsing", 400);
    };
    let thread_id = thread_id.to_string();
    // TODO (kenmo-melon): Get the default name from the board config
    let default_name = "エッヂの名無し";

    let Ok(binded_stmt) = db
        .prepare("SELECT * FROM threads WHERE thread_number = ?")
        .bind(&[thread_id.clone().into()])
    else {
        return Response::error("internal server error", 500);
    };
    let thread = match binded_stmt.first::<Thread>(None).await {
        Ok(Some(thread)) => thread,
        Ok(None) => return Response::error("internal server error", 500),
        Err(e) => return Response::error(format!("DB error {}", e), 500),
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

    let mut responses = match get_all_responses(&thread_id, &db).await {
        Ok(responses) => responses,
        Err(e) => return e,
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
                response_shift_jis_text_plain_with_cache(body)
            }
        }
        _ => response_shift_jis_text_plain_with_cache(body),
    }
}
