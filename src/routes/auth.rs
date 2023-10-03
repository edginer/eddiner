use std::collections::HashMap;

use worker::*;

use crate::{
    authed_cookie::AuthedCookie, turnstile::TurnstileResponse, utils::get_unix_timetamp_sec,
};

const AUTH_GETTING_HTML: &str = include_str!("templates/auth_getting.html");
const AUTH_FAILED_HTML: &str = include_str!("templates/auth_failed.html");
const AUTH_SUCCESSFUL_HTML: &str = include_str!("templates/auth_successful.html");

pub async fn route_auth_post(
    req: &mut Request,
    db: &D1Database,
    secret_key: &str,
) -> Result<Response> {
    let Ok(body) = req.form_data().await else {
        return Response::error("Bad request", 400);
    };

    let Some(FormEntry::Field(token)) = body.get("cf-turnstile-response") else {
        return Response::error("Bad request", 400);
    };
    let Ok(Some(ip)) = req.headers().get("CF-Connecting-IP") else {
        return Response::error("Bad request", 400);
    };

    // Validate the token by calling the `/siteverify` API.
    // `secret_key` here is set using Wrangler secrets
    let mut form_data = HashMap::new();
    form_data.insert("secret", secret_key);
    form_data.insert("response", &token);
    form_data.insert("remoteip", &ip);

    let Ok(response) = reqwest::Client::new()
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&form_data)
        .send()
        .await
    else {
        return Response::error("internal server error - reqwest cloudflare", 500);
    };
    let Ok(text) = response.text().await else {
        return Response::error("internal server error - cloudflare response", 500);
    };
    let Ok(result) = serde_json::from_str::<TurnstileResponse>(&text) else {
        return Response::error(format!("internal server error - json parsing {text}"), 500);
    };
    if result.success {
        let Some(FormEntry::Field(edge_token)) = body.get("edge-token") else {
            return Response::error("Bad request", 400);
        };

        let Ok(stmt) = db
            .prepare("SELECT * FROM authed_cookies WHERE cookie = ?")
            .bind(&[edge_token.clone().into()])
        else {
            return Response::error("internal server error: DB", 500);
        };
        let Ok(Some(result)) = stmt.first::<AuthedCookie>(None).await else {
            return Response::error("internal server error: DB", 500);
        };
        if result.origin_ip != ip {
            return Response::from_html(
                AUTH_FAILED_HTML.replace("{reason}", "IPが一致していません"),
            )
            .map(|r| r.with_status(400));
        }

        let Ok(stmt) = db
            .prepare("UPDATE authed_cookies SET authed = ?, authed_time = ? WHERE cookie = ?")
            .bind(&[
                1.into(),
                get_unix_timetamp_sec().to_string().into(),
                edge_token.clone().into(),
            ])
        else {
            return Response::error("internal server error: DB", 500);
        };
        if stmt.run().await.is_err() {
            return Response::error("internal server error: DB", 500);
        }

        Response::from_html(AUTH_SUCCESSFUL_HTML.replace("{token}", &edge_token))
    } else {
        Response::from_html(AUTH_FAILED_HTML.replace("{reason}", "Cloudflareの認証に失敗しました"))
            .map(|r| r.with_status(400))
    }
}

pub fn route_auth_get(req: &Request, site_key: &str) -> Result<Response> {
    let url = req.url().unwrap();
    let Some(token) = url.query().map(|e| e.split('=').collect::<Vec<&str>>()) else {
        return Response::error("Bad request", 400);
    };
    let Some(token) = token.get(1) else {
        return Response::error("Bad request", 400);
    };

    Response::from_html(
        AUTH_GETTING_HTML
            .replace("{site_key}", site_key)
            .replace("{token}", token),
    )
}
