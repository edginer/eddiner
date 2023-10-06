use std::collections::HashMap;

use chrono::NaiveDateTime;
use rand::Rng;
use worker::{Date, Response};

pub fn shift_jis_url_encodeded_body_to_vec(
    data: &str,
) -> std::result::Result<HashMap<&str, String>, ()> {
    fn ascii_hex_digit_to_byte(value: u8) -> Result<u8, ()> {
        if value.is_ascii_hexdigit() {
            if value.is_ascii_digit() {
                // U+0030 '0' - U+0039 '9',
                Ok(value - 0x30)
            } else if value.is_ascii_uppercase() {
                // U+0041 'A' - U+0046 'F',
                Ok(value - 0x41 + 0xa)
            } else if value.is_ascii_lowercase() {
                // U+0061 'a' - U+0066 'f',
                Ok(value - 0x61 + 0xa)
            } else {
                Err(())
            }
        } else {
            Err(())
        }
    }

    data.split('&')
        .map(|x| {
            let split = x.split('=').collect::<Vec<_>>();
            if split.len() != 2 {
                return std::result::Result::Err(());
            }
            let (key, value) = (split[0], split[1]);
            let bytes = value.as_bytes();
            let len = bytes.len();
            let mut i = 0;
            let mut result = Vec::new();
            while i < len {
                let item = bytes[i];
                if item == 0x25 {
                    // Look up the next two bytes from 0x25
                    if let Some([next1, next2]) = bytes.get(i + 1..i + 3) {
                        let first_byte = ascii_hex_digit_to_byte(*next1)?;
                        let second_byte = ascii_hex_digit_to_byte(*next2)?;
                        let code = first_byte * 0x10_u8 + second_byte;
                        result.push(code);
                    }
                    i += 2;
                } else if item == 0x2b {
                    result.push(0x20);
                } else {
                    result.push(bytes[i]);
                }
                i += 1;
            }
            let result = encoding_rs::SHIFT_JIS.decode(&result).0.to_string();
            Ok((key, result))
        })
        .collect::<Result<HashMap<_, _>, ()>>()
}

pub fn get_unix_timetamp_sec() -> u64 {
    Date::now().as_millis() / 1000
}

pub fn response_shift_jis_text_plain(body: String) -> worker::Result<Response> {
    let data = encoding_rs::SHIFT_JIS.encode(&body).0.into_owned();
    let Ok(mut resp) = Response::from_bytes(data) else {
        return Response::error("internal server error - converting sjis", 500);
    };
    let _ = resp.headers_mut().delete("Content-Type");
    let _ = resp.headers_mut().append("Content-Type", "text/plain");
    Ok(resp)
}

pub fn response_shift_jis_with_range(body: String, start_range: usize) -> worker::Result<Response> {
    let data = encoding_rs::SHIFT_JIS.encode(&body).0.into_owned();
    let Ok(mut resp) = Response::from_bytes(data.into_iter().skip(start_range).collect::<Vec<_>>())
    else {
        return Response::error("internal server error - converting sjis", 500);
    };
    let _ = resp.headers_mut().delete("Content-Type");
    let _ = resp.headers_mut().append("Content-Type", "text/plain");
    Ok(resp)
}

pub fn response_shift_jis_text_html(body: String) -> worker::Result<Response> {
    let data = encoding_rs::SHIFT_JIS.encode(&body).0.into_owned();
    let Ok(mut resp) = Response::from_bytes(data) else {
        return Response::error("internal server error - converting sjis", 500);
    };
    let _ = resp.headers_mut().delete("Content-Type");
    let _ = resp
        .headers_mut()
        .append("Content-Type", "text/html; charset=x-sjis");
    Ok(resp)
}

pub fn get_current_date_time() -> chrono::NaiveDateTime {
    let date = NaiveDateTime::from_timestamp_millis(Date::now().as_millis() as i64).unwrap();
    date.checked_add_signed(chrono::Duration::hours(9)).unwrap()
}

pub fn get_current_date_time_string() -> String {
    get_current_date_time()
        .format("%Y/%m/%d(%a) %H:%M:%S.%3f")
        .to_string()
}

fn unix_ts_to_bytes(ts: u64) -> [u8; 32] {
    let mut bytes = [0; 32];

    for (i, byte) in bytes.iter_mut().enumerate().take(8) {
        *byte = (ts >> (56 - i * 8)) as u8;
    }

    bytes
}

pub fn generate_six_digit_num() -> String {
    let milli = Date::now().as_millis();

    let mut rng: rand::rngs::StdRng = rand::SeedableRng::from_seed(unix_ts_to_bytes(milli));
    let num = rng.gen_range(0..1000000);
    format!("{:06}", num)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_siki_shift_jis_encodeded_body_to_vec() {
        let data = "submit=%8f%91%82%ab%8d%9e%82%de&time=%31%36%39%36%32%37%30%31%34%38&bbs=%6c%69%76%65%65%64%67%65&key=%31%36%39%36%32%35%31%38%35%39&MESSAGE=%82%c4%82%93%82%94&FROM=&mail=";
        let result = shift_jis_url_encodeded_body_to_vec(data);
        println!("{:?}", result);
    }
}
