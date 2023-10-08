use worker::*;

const LIVEEDGE_HTML: &str = include_str!("templates/liveedge.html");

pub fn route_liveedge() -> Result<Response> {
    Response::from_html(LIVEEDGE_HTML)
}
