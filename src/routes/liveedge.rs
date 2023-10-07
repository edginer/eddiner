use worker::*;

use crate::utils::response_shift_jis_text_plain;

const LIVEEDGE_HTML: &str = include_str!("templates/liveedge.html");

pub fn route_liveedge() -> Result<Response> {
    response_shift_jis_text_plain(LIVEEDGE_HTML.to_string())
}
