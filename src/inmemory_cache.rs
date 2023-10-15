use crate::utils::get_current_date_time;
use chrono::NaiveDateTime;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use worker::*;

fn get_cached_lwt_per_ip() -> &'static Mutex<HashMap<String, NaiveDateTime>> {
    static LAST_WROTE_TIME: OnceLock<Mutex<HashMap<String, NaiveDateTime>>> = OnceLock::new();
    LAST_WROTE_TIME.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_cached_lwt_per_cookie() -> &'static Mutex<HashMap<String, NaiveDateTime>> {
    static LAST_WROTE_TIME: OnceLock<Mutex<HashMap<String, NaiveDateTime>>> = OnceLock::new();
    LAST_WROTE_TIME.get_or_init(|| Mutex::new(HashMap::new()))
}

fn reject_common(
    key: &str,
    lwt_map: &'static Mutex<HashMap<String, NaiveDateTime>>,
) -> Result<bool> {
    let mut lock = lwt_map
        .lock()
        .map_err(|_| Error::RustError("Mutex is Poisoned".to_string()))?;
    let now = get_current_date_time();
    match lock.entry(key.to_string()) {
        Entry::Occupied(mut lwt) => {
            let diff = now - *lwt.get();
            *lwt.get_mut() = now;
            Ok(diff.num_seconds() < 5)
        }
        Entry::Vacant(e) => {
            e.insert(now);
            Ok(false)
        }
    }
}

pub(crate) fn maybe_reject_cookie(cookie: &str) -> Result<bool> {
    reject_common(cookie, get_cached_lwt_per_cookie())
}

pub(crate) fn maybe_reject_ip(ip: &str) -> Result<bool> {
    reject_common(ip, get_cached_lwt_per_ip())
}
