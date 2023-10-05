use worker::*;

use crate::{
    response::{Ch5ResponsesFormatter, Res},
    thread::Thread,
    utils::response_shift_jis_text_plain,
};

pub async fn route_dat(
    path: &str,
    ua: Option<String>,
    range: Option<String>,
    _if_modified_since: Option<String>,
    db: &D1Database,
) -> Result<Response> {
    let thread_id = path.replace(".dat", "").replace("/liveedge/dat/", "");
    let Ok(thread_id) = thread_id.parse::<u64>() else {
        return Response::error("Bad request", 400);
    };
    let thread_id = thread_id.to_string();

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

    let body = responses.format_responses(&thread.title);
    response_shift_jis_text_plain(body).map(|x| {
        if matches!((ua, range), (Some(ua), Some(_)) if ua.contains("twinkle")) {
            x.with_status(416)
        } else {
            x
        }
    })
}
