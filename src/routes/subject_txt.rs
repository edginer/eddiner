use worker::*;

use crate::{
    thread::{Ch5ThreadFormatter, Thread},
    utils::response_shift_jis_text_plain_with_cache,
};

pub async fn route_subject_txt(db: &D1Database) -> Result<Response> {
    let Ok(threads) = db
        .prepare("SELECT * FROM threads WHERE archived = 0")
        .all()
        .await
    else {
        return Response::error("internal server error: select", 500);
    };

    let Ok(mut threads) = threads.results::<Thread>() else {
        return Response::error("internal server error: convertion", 500);
    };

    threads.sort_by_key(|x| u64::max_value() - x.last_modified.parse::<u64>().unwrap());

    let threads_body = threads.format_threads();
    response_shift_jis_text_plain_with_cache(threads_body)
}
