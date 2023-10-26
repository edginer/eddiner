use worker::*;

use crate::{
    repositories::bbs_repository::{BbsRepository, ThreadStatus},
    thread::Ch5ThreadFormatter,
    utils::response_shift_jis_text_plain_with_cache,
};

pub async fn route_subject_txt(repo: &BbsRepository<'_>) -> Result<Response> {
    let Ok(mut threads) = repo.get_threads(1, ThreadStatus::Unarchived).await else {
        return Response::error("internal server error", 500);
    };

    threads.sort_by_key(|x| u64::max_value() - x.last_modified.parse::<u64>().unwrap());

    let threads_body = threads.format_threads();
    response_shift_jis_text_plain_with_cache(threads_body, 1)
}
