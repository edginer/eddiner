use worker::*;

use crate::utils::response_shift_jis_text_plain;

const HEAD_TXT: &str = include_str!("templates/head.txt");

pub fn route_head_txt() -> Result<Response> {
    response_shift_jis_text_plain(HEAD_TXT.to_string())
}
