use std::collections::HashMap;

use tokio::join;
use worker::{Response, Result};

use crate::{grecaptcha::GrecaptchaResponse, turnstile::TurnstileResponse};

pub async fn verify_auth_resp(
    turnstile_secret_key: &str,
    grecaptcha_secret_key: Option<&str>,
    ip: &str,
    turnstile_response_tk: &str,
    grecaptcha_response_tk: Option<&str>,
) -> std::result::Result<bool, Result<Response>> {
    let mut ts_form_data = HashMap::new();
    ts_form_data.insert("secret", turnstile_secret_key);
    ts_form_data.insert("response", turnstile_response_tk);
    ts_form_data.insert("remoteip", ip);

    let mut gr_form_data = HashMap::new();
    if let (Some(grecaptcha_secret_key), Some(grecaptcha_response_tk)) =
        (grecaptcha_secret_key, grecaptcha_response_tk)
    {
        gr_form_data.insert("secret", grecaptcha_secret_key);
        gr_form_data.insert("response", grecaptcha_response_tk);
        gr_form_data.insert("remoteip", ip);
    }

    let turnstile_req = send_auth_request(
        &ts_form_data,
        "https://challenges.cloudflare.com/turnstile/v0/siteverify",
    );
    if grecaptcha_secret_key.is_some() {
        let grecaptcha_req = send_auth_request(
            &gr_form_data,
            "https://www.google.com/recaptcha/api/siteverify",
        );
        let (turnstile_resp, gcap_resp) = join!(turnstile_req, grecaptcha_req);
        let (turnstile_text, gcap_text) = (turnstile_resp?, gcap_resp?);

        let Ok(turnstile_result) = serde_json::from_str::<TurnstileResponse>(&turnstile_text)
        else {
            return Err(Response::error(
                format!("internal server error - json parsing {turnstile_text}"),
                500,
            ));
        };
        let Ok(gcap_result) = serde_json::from_str::<GrecaptchaResponse>(&gcap_text) else {
            return Err(Response::error(
                format!("internal server error - json parsing {gcap_text}"),
                500,
            ));
        };

        Ok(turnstile_result.success && gcap_result.success)
    } else {
        let turnstile_text = turnstile_req.await?;
        let Ok(turnstile_result) = serde_json::from_str::<TurnstileResponse>(&turnstile_text)
        else {
            return Err(Response::error(
                format!("internal server error - json parsing {turnstile_text}"),
                500,
            ));
        };

        Ok(turnstile_result.success)
    }
}

async fn send_auth_request(
    form_data: &HashMap<&str, &str>,
    url: &str,
) -> std::result::Result<String, Result<Response>> {
    let resp = reqwest::Client::new()
        .post(url)
        .form(&form_data)
        .send()
        .await;
    let Ok(resp) = resp else {
        return Err(Response::error(
            "internal server error - reqwest cloudflare",
            500,
        ));
    };
    let Ok(text) = resp.text().await else {
        return Err(Response::error(
            "internal server error - cloudflare response",
            500,
        ));
    };

    Ok(text)
}
