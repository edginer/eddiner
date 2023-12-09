use worker::*;

use crate::{
    repositories::bbs_repository::BbsRepository,
    services::auth_verification,
    utils::{equals_ip_addr, get_unix_timestamp_sec},
};

const AUTH_GETTING_HTML: &str = include_str!("templates/auth_getting.html");
const AUTH_FAILED_HTML: &str = include_str!("templates/auth_failed.html");
const AUTH_SUCCESSFUL_HTML: &str = include_str!("templates/auth_successful.html");

pub async fn route_auth_post(
    req: &mut Request,
    repo: &BbsRepository<'_>,
    secret_key: &str,
    g_recaptcha_secret_key: &str,
) -> Result<Response> {
    let Ok(body) = req.form_data().await else {
        return Response::error("Bad request", 400);
    };

    let Some(FormEntry::Field(cf_turnstile_response)) = body.get("cf-turnstile-response") else {
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
        &cf_turnstile_response,
        Some(&grecaptcha_response_tk),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => return e,
    };

    if result {
        let Some(FormEntry::Field(edge_token)) = body.get("edge-token") else {
            return Response::error("Bad request", 400);
        };

        let Ok(Some(result)) = repo.get_authed_token(&edge_token).await else {
            return Response::error("internal server error: DB get authed token", 500);
        };
        if !equals_ip_addr(&result.origin_ip, &ip) {
            return Response::from_html(AUTH_FAILED_HTML.replace(
                "{reason}",
                &format!("IPが一致していません: {} <-> {}", result.origin_ip, ip),
            ))
            .map(|r| r.with_status(400));
        }

        if repo
            .update_authed_status(&edge_token, &get_unix_timestamp_sec().to_string())
            .await
            .is_err()
        {
            return Response::error("internal server error: DB update authed token", 500);
        }

        Response::from_html(AUTH_SUCCESSFUL_HTML.replace("{token}", &edge_token))
    } else {
        Response::from_html(AUTH_FAILED_HTML.replace("{reason}", "Cloudflareの認証に失敗しました"))
            .map(|r| r.with_status(400))
    }
}

pub fn route_auth_get(
    req: &Request,
    site_key: &str,
    g_recaptcha_site_key: &str,
) -> Result<Response> {
    let url = req.url()?;
    // get y in x=y
    let Some(Some(token)) = url.query().map(|e| e.split('=').nth(1)) else {
        return Response::error("Bad request", 400);
    };

    Response::from_html(
        AUTH_GETTING_HTML
            .replace("{site_key}", site_key)
            .replace("{token}", token)
            .replace("{recaptcha_site_key}", g_recaptcha_site_key),
    )
}
