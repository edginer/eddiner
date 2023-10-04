use std::collections::HashMap;

use worker::*;

use crate::{
    authed_cookie::AuthedCookie, turnstile::TurnstileResponse, utils::get_unix_timetamp_sec,
};

const AUTH_GETTING_HTML: &str = include_str!("templates/auth_code_getting.html");
const AUTH_FAILED_HTML: &str = include_str!("templates/auth_failed.html");
const AUTH_SUCCESSFUL_HTML: &str = include_str!("templates/auth_successful.html");

pub fn route_auth_code_get(site_key: &str) -> Result<Response> {
    Response::from_html(AUTH_GETTING_HTML.replace("{site_key}", site_key))
}

pub async fn route_auth_code_post(
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
        let Some(FormEntry::Field(auth_code)) = body.get("auth-code") else {
            return Response::error("Bad request", 400);
        };

        let Ok(stmt) = db
            .prepare("SELECT * FROM authed_cookies WHERE origin_ip = ? AND auth_code = ?")
            .bind(&[ip.into(), auth_code.clone().into()])
        else {
            return Response::error("internal server error: DB", 500);
        };
        let Ok(result) = stmt.first::<AuthedCookie>(None).await else {
            return Response::error("internal server error: DB", 500);
        };

        let Some(authed_cookie) = result else {
            return Response::from_html(
                AUTH_FAILED_HTML
                    .replace("{reason}", "認証コード、もしくはIPアドレスが一致しません"),
            )
            .map(|r| r.with_status(400));
        };
        let current_unix_time_sec = get_unix_timetamp_sec();
        let writed_time = authed_cookie.writed_time.parse::<u64>().unwrap();
        if current_unix_time_sec - writed_time > 60 * 5 {
            return Response::from_html(
                AUTH_FAILED_HTML.replace("{reason}", "認証コードの有効期限が切れています"),
            )
            .map(|r| r.with_status(400));
        }

        let Ok(stmt) = db
            .prepare("UPDATE authed_cookies SET authed = ?, authed_time = ? WHERE cookie = ?")
            .bind(&[
                1.into(),
                get_unix_timetamp_sec().to_string().into(),
                authed_cookie.cookie.clone().into(),
            ])
        else {
            return Response::error("internal server error: DB", 500);
        };
        if stmt.run().await.is_err() {
            return Response::error("internal server error: DB", 500);
        }

        Response::from_html(AUTH_SUCCESSFUL_HTML.replace("{token}", &authed_cookie.cookie))
    } else {
        Response::from_html(AUTH_FAILED_HTML.replace("{reason}", "Cloudflareの認証に失敗しました"))
            .map(|r| r.with_status(400))
    }
}
