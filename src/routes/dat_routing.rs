use worker::*;

use crate::{
    response::{Ch5ResponsesFormatter, Res},
    thread::Thread,
    utils::{response_shift_jis_text_plain_with_cache, response_shift_jis_with_range},
};

pub async fn route_dat(
    path: &str,
    range: Option<String>,
    if_modified_since: Option<String>,
    db: &D1Database,
) -> Result<Response> {
    let thread_id = path.replace(".dat", "").replace("/liveedge/dat/", "");
    let Ok(thread_id) = thread_id.parse::<u64>() else {
        return Response::error("Bad request", 400);
    };
    let thread_id = thread_id.to_string();
    // TODO (kenmo-melon): Get the default name from the board config
    let default_name = "エッヂの名無し";

    let Ok(binded_stmt) = db
        .prepare("SELECT * FROM threads WHERE thread_number = ?")
        .bind(&[thread_id.to_string().into()])
    else {
        return Response::error("internal server error", 500);
    };
    let Ok(thread) = binded_stmt.first::<Thread>(None).await else {
        return Response::error("internal server error", 500);
    };
    let Some(thread) = thread else {
        return Response::error("Not found - dat", 404);
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

    let Ok(responses_binded_stmt) = db
        .prepare("SELECT * FROM responses WHERE thread_id = ?")
        .bind(&[thread_id.into()])
    else {
        return Response::error("internal server error", 500);
    };
    let Ok(responses) = responses_binded_stmt.all().await else {
        return Response::error("internal server error", 500);
    };
    let Ok(responses): Result<Vec<Res>> = responses.results::<Res>() else {
        return Response::error("internal server error", 500);
    };

    let body = responses.format_responses(&thread.title, default_name);

    match range {
        Some(range) => {
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
        None => response_shift_jis_text_plain_with_cache(body),
    }
}
