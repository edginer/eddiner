use crate::repositories::bbs_repository::ThreadStatus;
use crate::response::TokenRemover;
use crate::utils::into_workers_err;
use crate::{board_config::BoardConfig, repositories::bbs_repository::BbsRepository};

use minijinja::{context, Environment};
use worker::{Response, Result};

const BOARD_HTML: &str = include_str!("templates/board.html");
const INDEX_HTML: &str = include_str!("templates/index.html");
const THREAD_HTML: &str = include_str!("templates/thread.html");
const WEBUI_DISABLED_HTML: &str = include_str!("templates/webui_disabled.html");

pub(crate) fn webui_disabled(site_title: &str) -> Result<Response> {
    let html = WEBUI_DISABLED_HTML.replace("{site_title}", site_title);
    Response::from_html(html)
}

pub(crate) fn route_index(
    site_title: &str,
    site_name: &str,
    site_description: &str,
    boards: &[BoardConfig],
) -> Result<Response> {
    let mut env = Environment::new();
    env.add_template("index.html", INDEX_HTML)
        .map_err(into_workers_err)?;
    let tmpl = env.get_template("index.html").map_err(into_workers_err)?;
    let html = tmpl
        .render(context!(site_title, site_name, site_description, boards))
        .map_err(into_workers_err)?;
    Response::from_html(html)
}

pub(crate) async fn route_board(
    host_url: &str,
    board: &BoardConfig,
    repo: &BbsRepository<'_>,
) -> Result<Response> {
    // TODO: this restriction is only for eddi. It should be removed in the future.
    if host_url.contains("workers.dev") {
        return webui_disabled("edgebb");
    }

    // Get threads from db
    let Ok(threads) = repo
        .get_threads(board.board_id as usize, ThreadStatus::Active)
        .await
    else {
        return Response::error("internal server error: convertion", 500);
    };

    let mut env = Environment::new();
    env.add_template("board.html", BOARD_HTML)
        .map_err(into_workers_err)?;
    let tmpl = env.get_template("board.html").map_err(into_workers_err)?;
    let html = tmpl
        .render(context!(host_url, board, threads))
        .map_err(into_workers_err)?;
    Response::from_html(html).map(|mut x| {
        let _ = x.headers_mut().append("Cache-Control", "s-maxage=10");
        x
    })
}

pub(crate) async fn route_thread(
    thread_id: u64,
    board: &BoardConfig,
    repo: &BbsRepository<'_>,
    host_url: &str,
) -> Result<Response> {
    // TODO: this restriction is only for eddi. It should be removed in the future.
    if host_url.contains("workers.dev") {
        return webui_disabled("edgebb");
    }

    let thread_id = thread_id.to_string();
    // Get threads from db
    let thread = match repo.get_thread(board.board_id as usize, &thread_id).await {
        Ok(Some(thread)) => thread,
        Ok(None) => return Response::error("internal server error", 500),
        Err(e) => return Response::error(format!("DB error {}", e), 500),
    };
    let responses = match repo.get_responses(1, &thread_id).await {
        Ok(responses) => responses,
        Err(e) => return Response::error(format!("DB error {}", e), 500),
    };

    let mut env = Environment::new();
    env.add_template("thread.html", THREAD_HTML)
        .map_err(into_workers_err)?;
    let token_remover = TokenRemover::new(board.default_name);
    env.add_filter("remove_token", move |name| token_remover.remove(name));
    let tmpl = env.get_template("thread.html").map_err(into_workers_err)?;
    let html = tmpl
        .render(context!(board, thread, responses))
        .map_err(into_workers_err)?;
    Response::from_html(html).map(|mut x| {
        let _ = x.headers_mut().append("Cache-Control", "s-maxage=5");
        x
    })
}
