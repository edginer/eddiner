use worker::*;

use crate::{repositories::bbs_repository::BbsRepository, utils::response_shift_jis_text_plain};

pub async fn route_head_txt(board_id: usize, repo: &BbsRepository<'_>) -> Result<Response> {
    let Ok(Some(board_info)) = repo.get_board_info(board_id).await else {
        return Response::error("internal server error - failed to find board", 500);
    };

    response_shift_jis_text_plain(board_info.local_rule)
}
