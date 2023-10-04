use worker::*;

use crate::utils::response_shift_jis_text_plain;

pub fn route_liveedge() -> Result<Response> {
    let builder = String::from("liveedge");
    response_shift_jis_text_plain(builder)
}
