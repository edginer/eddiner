use worker::*;

use crate::{
    repositories::bbs_repository::BbsRepository, services::auth_verification,
    utils::get_unix_timestamp_sec,
};

const AUTH_GETTING_HTML: &str = include_str!("templates/auth_code_getting.html");
const AUTH_FAILED_HTML: &str = include_str!("templates/auth_failed.html");
const AUTH_SUCCESSFUL_HTML: &str = include_str!("templates/auth_successful.html");

pub fn route_auth_code_get(site_key: &str, g_recaptcha_site_key: &str) -> Result<Response> {
    Response::from_html(
        AUTH_GETTING_HTML
            .replace("{site_key}", site_key)
            .replace("{recaptcha_site_key}", g_recaptcha_site_key),
    )
}

pub async fn route_auth_code_post(
    req: &mut Request,
    repo: &BbsRepository<'_>,
    secret_key: &str,
    g_recaptcha_secret_key: &str,
) -> Result<Response> {
    let Ok(body) = req.form_data().await else {
        return Response::error("Bad request", 400);
    };

    let Some(FormEntry::Field(turnstile_response_tk)) = body.get("cf-turnstile-response") else {
        return Response::error("Bad request", 400);
    };
    let Some(FormEntry::Field(grecaptcha_response_tk)) = body.get("g-recaptcha-response") else {
        return Response::error("Bad request", 400);
    };
    let Ok(Some(ip)) = req.headers().get("CF-Connecting-IP") else {
        return Response::error("Bad request", 400);
    };

    let result = match auth_verification::verify_auth_resp(
        secret_key,
        Some(g_recaptcha_secret_key),
        &ip,
        &turnstile_response_tk,
        Some(&grecaptcha_response_tk),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => return e,
    };

    if result {
        let Some(FormEntry::Field(auth_code)) = body.get("auth-code") else {
            return Response::error("Bad request", 400);
        };

        let Ok(result) = repo
            .get_authed_token_by_origin_ip_and_auth_code(&ip, &auth_code)
            .await
        else {
            return Response::error("internal server error: DB", 500);
        };

        let Some(authed_cookie) = result else {
            return Response::from_html(
                AUTH_FAILED_HTML
                    .replace("{reason}", "認証コード、もしくはIPアドレスが一致しません"),
            )
            .map(|r| r.with_status(400));
        };
        let current_unix_time_sec = get_unix_timestamp_sec();
        let writed_time = authed_cookie.writed_time.parse::<u64>().unwrap();
        if current_unix_time_sec - writed_time > 60 * 5 {
            return Response::from_html(
                AUTH_FAILED_HTML.replace("{reason}", "認証コードの有効期限が切れています"),
            )
            .map(|r| r.with_status(400));
        }

        if repo
            .update_authed_status(&authed_cookie.cookie, &get_unix_timestamp_sec().to_string())
            .await
            .is_err()
        {
            return Response::error("internal server error: DB", 500);
        }

        Response::from_html(AUTH_SUCCESSFUL_HTML.replace("{token}", &authed_cookie.cookie))
    } else {
        Response::from_html(AUTH_FAILED_HTML.replace("{reason}", "Cloudflareの認証に失敗しました"))
            .map(|r| r.with_status(400))
    }
}
