use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, OnceLock, RwLock},
};

use tokio::join;
use worker::{console_log, Date};

use crate::{authed_cookie::AuthedCookie, response::Res, thread::MetadentType, DbOrchestrator};

const RESPONSES_CACHE_EXPIRE_TIME: u64 = 1000; // same as s-maxage=1
const CLEAR_RESPONSES_CACHE_INTERVAL: u64 = 1000 * 60 * 5;

type ResponsesCacheMap = HashMap<(usize, String, usize), (u64, Vec<Res>)>;

fn get_responses_cache_map() -> &'static RwLock<ResponsesCacheMap> {
    static RESPONSES_CACHE: OnceLock<RwLock<ResponsesCacheMap>> = OnceLock::new();
    RESPONSES_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn put_responses_cache(board_id: usize, thread_id: &str, modulo: usize, responses: Vec<Res>) {
    let current_millis = Date::now().as_millis();
    static LAST_REFRESHED: OnceLock<AtomicU64> = OnceLock::new();
    let last_refreshed = LAST_REFRESHED.get_or_init(|| AtomicU64::new(current_millis));

    let mut lock = get_responses_cache_map().write().unwrap();
    if current_millis - last_refreshed.load(std::sync::atomic::Ordering::Relaxed)
        > CLEAR_RESPONSES_CACHE_INTERVAL
    {
        lock.clear();
        last_refreshed.store(current_millis, std::sync::atomic::Ordering::Relaxed);
        console_log!("cache refreshed");
    }

    lock.insert(
        (board_id, thread_id.to_string(), modulo),
        (current_millis, responses),
    );
}

fn get_responses_cache(board_id: usize, thread_id: &str, modulo: usize) -> Option<Vec<Res>> {
    let t = {
        let lock = get_responses_cache_map().read().unwrap();
        lock.get(&(board_id, thread_id.to_string(), modulo))
            .cloned()
    };
    if let Some((timestamp, responses)) = t {
        if Date::now().as_millis() - timestamp > RESPONSES_CACHE_EXPIRE_TIME {
            console_log!("cache expired {board_id}/{thread_id} ({modulo})");
            None
        } else {
            console_log!("cache hit {board_id}/{thread_id} ({modulo})");
            Some(responses)
        }
    } else {
        None
    }
}

pub struct BbsRepository<'a> {
    dbo: &'a DbOrchestrator,
}

impl<'a> BbsRepository<'a> {
    pub fn new(dbo: &'a DbOrchestrator) -> BbsRepository<'a> {
        BbsRepository { dbo }
    }
}

impl BbsRepository<'_> {
    pub async fn get_board_info(
        &self,
        board_id: usize,
    ) -> anyhow::Result<Option<crate::board::Board>> {
        let Ok(stmt) = self
            .dbo
            .infos_db
            .prepare("SELECT * FROM boards WHERE id = ?")
            .bind(&[board_id.into()])
        else {
            return Err(anyhow::anyhow!("failed to bind id"));
        };
        let Ok(board) = stmt.first::<crate::board::Board>(None).await else {
            return Err(anyhow::anyhow!("failed to fetch board"));
        };

        Ok(board)
    }

    pub async fn get_thread(
        &self,
        board_id: usize,
        thread_id: &str,
    ) -> anyhow::Result<Option<crate::thread::Thread>> {
        let Ok(stmt) = self
            .dbo
            .threads_db
            .prepare("SELECT * FROM threads WHERE thread_number = ? AND board_id = ?")
            .bind(&[thread_id.into(), board_id.into()])
        else {
            return Err(anyhow::anyhow!("failed to bind thread_number and board_id"));
        };
        let Ok(thread) = stmt.first::<crate::thread::Thread>(None).await else {
            return Err(anyhow::anyhow!("failed to fetch thread"));
        };

        Ok(thread)
    }

    pub async fn get_threads(
        &self,
        board_id: usize,
        status: ThreadStatus,
    ) -> anyhow::Result<Vec<crate::thread::Thread>> {
        let Ok(stmt) = self
            .dbo
            .threads_db
            .prepare(match status {
                ThreadStatus::Active => "SELECT * FROM threads WHERE board_id = ? AND active = 1",
                ThreadStatus::Inactive => {
                    "SELECT * FROM threads WHERE board_id = ? AND active = 0 AND archived = 0"
                }
                ThreadStatus::Archived => {
                    "SELECT * FROM threads WHERE board_id = ? AND archived = 1"
                }
                ThreadStatus::Unarchived => {
                    "SELECT * FROM threads WHERE board_id = ? AND archived = 0"
                }
            })
            .bind(&[board_id.into()])
        else {
            return Err(anyhow::anyhow!("failed to bind board_id"));
        };
        let Ok(threads) = stmt
            .all()
            .await
            .and_then(|res| res.results::<crate::thread::Thread>())
        else {
            return Err(anyhow::anyhow!("failed to fetch threads"));
        };

        Ok(threads)
    }

    pub async fn get_responses(
        &self,
        board_id: usize,
        thread_id: &str,
        modulo: usize,
    ) -> anyhow::Result<Vec<Res>> {
        let cached_resps = get_responses_cache(board_id, thread_id, modulo);
        if let Some(resps) = cached_resps {
            return Ok(resps);
        }
        let Ok(stmt) = self
            .dbo
            .get_responses_db(modulo)
            .prepare("SELECT * FROM responses WHERE thread_id = ? AND board_id = ?")
            .bind(&[thread_id.into(), board_id.into()])
        else {
            return Err(anyhow::anyhow!("failed to bind thread_id and board_id"));
        };
        let Ok(responses) = stmt.all().await.and_then(|res| res.results::<Res>()) else {
            return Err(anyhow::anyhow!("failed to fetch responses"));
        };

        put_responses_cache(board_id, thread_id, modulo, responses.clone());
        Ok(responses)
    }

    pub async fn get_responses_by_authed_token_and_timestamp(
        &self,
        authed_token: &str,
        min_timestamp: &str,
    ) -> anyhow::Result<Vec<Res>> {
        let mut responses = Vec::new();
        for m in &self.dbo.responses_db {
            let Ok(stmt) = m
                .prepare("SELECT * FROM responses WHERE authed_token = ? AND timestamp > ?")
                .bind(&[authed_token.into(), min_timestamp.into()])
            else {
                return Err(anyhow::anyhow!("failed to bind authed_token and timestamp"));
            };
            if let Ok(resps) = stmt.all().await.and_then(|res| res.results::<Res>()) {
                responses.extend(resps);
            } else {
                return Err(anyhow::anyhow!("failed to fetch responses"));
            }
        }

        Ok(responses)
    }

    pub async fn get_authed_token(&self, token: &str) -> anyhow::Result<Option<AuthedCookie>> {
        let Ok(stmt) = self
            .dbo
            .infos_db
            .prepare("SELECT * FROM authed_cookies WHERE cookie = ?")
            .bind(&[token.into()])
        else {
            return Err(anyhow::anyhow!("failed to bind token"));
        };

        if let Ok(authed_cookie) = stmt.first::<AuthedCookie>(None).await {
            Ok(authed_cookie)
        } else {
            Err(anyhow::anyhow!("failed to fetch authed_cookie"))
        }
    }

    pub async fn get_authed_token_by_origin_ip_and_auth_code(
        &self,
        ip: &str,
        auth_code: &str,
    ) -> anyhow::Result<Option<AuthedCookie>> {
        let Ok(stmt) = self
            .dbo
            .infos_db
            .prepare("SELECT * FROM authed_cookies WHERE origin_ip = ? AND auth_code = ?")
            .bind(&[ip.into(), auth_code.into()])
        else {
            return Err(anyhow::anyhow!("failed to bind ip and auth_code"));
        };

        if let Ok(authed_cookie) = stmt.first::<AuthedCookie>(None).await {
            Ok(authed_cookie)
        } else {
            Err(anyhow::anyhow!("failed to fetch authed_cookie"))
        }
    }

    pub async fn create_thread(&self, thread: CreatingThread<'_>) -> anyhow::Result<()> {
        let metadent: Option<&str> = thread.metadent.into();
        let metadent = metadent.unwrap_or("");
        let modulo = thread.unix_time.parse::<usize>().unwrap() % self.dbo.responses_db.len();
        let th_stmt = self
            .dbo
            .threads_db
            .prepare(
                "INSERT INTO threads
                (thread_number, title, response_count, board_id, last_modified, authed_cookie, metadent, modulo)
                VALUES (?, ?, 1, ?, ?, ?, ?, ?)",
            )
            .bind(&[
                thread.unix_time.into(),
                thread.title.into(),
                thread.board_id.into(),
                thread.unix_time.into(),
                thread.authed_token.into(),
                metadent.into(),
                modulo.into(),
            ]);

        let res_stmt = self
            .dbo
            .get_responses_db(modulo)
            .prepare(
                "INSERT INTO responses 
                (name, mail, date, author_id, body, thread_id, ip_addr, authed_token, timestamp, board_id)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&[
                thread.name.into(),
                thread.mail.into(),
                thread.date_time.into(),
                thread.author_ch5id.into(),
                thread.body.into(),
                thread.unix_time.into(),
                thread.ip_addr.into(),
                thread.authed_token.into(),
                thread.unix_time.into(),
                thread.board_id.into(),
            ]);

        match (th_stmt, res_stmt) {
            (Ok(th_stmt), Ok(res_stmt)) => {
                if let Err(e) = th_stmt.run().await {
                    if e.to_string().to_lowercase().contains("unique") {
                        Err(anyhow::anyhow!("thread already exists"))
                    } else {
                        Err(anyhow::anyhow!("failed to insert thread"))
                    }
                } else if res_stmt.run().await.is_err() {
                    Err(anyhow::anyhow!("failed to insert response"))
                } else {
                    Ok(())
                }
            }
            _ => Err(anyhow::anyhow!("failed to bind in thread creation")),
        }
    }

    pub async fn create_response(&self, res: CreatingRes<'_>, modulo: usize) -> anyhow::Result<()> {
        let update_th_stmt = self
            .dbo
            .threads_db
            .prepare(
                "UPDATE threads SET 
            response_count = response_count + 1,
            last_modified = ?,
            active = (
                CASE
                    WHEN response_count >= 999 THEN 0
                    ELSE 1
                END
            )
            WHERE thread_number = ? AND board_id = ?",
            ) // 999 means thread stopper 1000
            .bind(&[
                res.unix_time.into(),
                res.thread_id.into(),
                res.board_id.into(),
            ]);

        let res_stmt = self
            .dbo
            .get_responses_db(modulo)
            .prepare(
                "INSERT INTO responses 
                (name, mail, date, author_id, body, thread_id, ip_addr, authed_token, timestamp, board_id) 
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&[
                res.name.into(),
                res.mail.into(),
                res.date_time.into(),
                res.author_ch5id.into(),
                res.body.into(),
                res.thread_id.into(),
                res.ip_addr.into(),
                res.authed_token.into(),
                res.unix_time.into(),
                res.board_id.into(),
            ]);

        match (update_th_stmt, res_stmt) {
            (Ok(update_th_stmt), Ok(res_stmt)) => {
                let (th_res, res_res) = join!(update_th_stmt.run(), res_stmt.run(),);
                if th_res.is_err() {
                    Err(anyhow::anyhow!("failed to update thread"))
                } else if res_res.is_err() {
                    Err(anyhow::anyhow!("failed to insert response"))
                } else {
                    Ok(())
                }
            }
            _ => Err(anyhow::anyhow!("failed to bind in response creation")),
        }
    }

    pub async fn create_authed_token(
        &self,
        authed_token: CreatingAuthedToken<'_>,
    ) -> anyhow::Result<()> {
        let Ok(stmt) = self
            .dbo
            .infos_db
            .prepare(
                "INSERT INTO authed_cookies (cookie, origin_ip, authed, auth_code, writed_time) 
                VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&[
                authed_token.token.into(),
                authed_token.origin_ip.into(),
                0.into(),
                authed_token.auth_code.into(),
                authed_token.writed_time.into(),
            ])
        else {
            return Err(anyhow::anyhow!("failed to bind authed_token"));
        };
        if stmt.run().await.is_err() {
            Err(anyhow::anyhow!("failed to insert authed_token"))
        } else {
            Ok(())
        }
    }

    pub async fn update_authed_token_last_thread_creation(
        &self,
        token: &str,
        unix_time: &str,
    ) -> anyhow::Result<()> {
        let Ok(stmt) = self
            .dbo
            .infos_db
            .prepare("UPDATE authed_cookies SET last_thread_creation = ? WHERE cookie = ?")
            .bind(&[unix_time.into(), token.into()])
        else {
            return Err(anyhow::anyhow!("failed to bind token"));
        };

        if stmt.run().await.is_err() {
            Err(anyhow::anyhow!("failed to update authed_token"))
        } else {
            Ok(())
        }
    }

    pub async fn update_authed_status(&self, token: &str, authed_time: &str) -> anyhow::Result<()> {
        let Ok(stmt) = self
            .dbo
            .infos_db
            .prepare("UPDATE authed_cookies SET authed = ?, authed_time = ? WHERE cookie = ?")
            .bind(&[1.into(), authed_time.into(), token.into()])
        else {
            return Err(anyhow::anyhow!("failed to bind token"));
        };

        if stmt.run().await.is_err() {
            Err(anyhow::anyhow!("failed to update authed_token"))
        } else {
            Ok(())
        }
    }

    pub async fn get_cap_by_password_hash(
        &self,
        hash: &str,
    ) -> anyhow::Result<Option<crate::cap::Cap>> {
        let Ok(stmt) = self
            .dbo
            .infos_db
            .prepare("SELECT * FROM caps WHERE cap_password_hash = ?")
            .bind(&[hash.into()])
        else {
            return Err(anyhow::anyhow!("failed to bind hash"));
        };

        if let Ok(cap) = stmt.first::<crate::cap::Cap>(None).await {
            Ok(cap)
        } else {
            Err(anyhow::anyhow!("failed to fetch cap"))
        }
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // TODO: Remove this
pub enum ThreadStatus {
    // Show in the thread list
    Active,
    // Not show in the thread list and can't be posted (will be archived via eddiner-archiver)
    Archived,
    // Show in the thread list but can't be posted
    Inactive,
    // Show in the thread list, and it contains the thread that is inactive but not archived
    Unarchived,
}

#[derive(Debug, Clone)]
pub struct CreatingThread<'a> {
    pub title: &'a str,
    pub unix_time: &'a str,
    pub body: &'a str,
    pub name: &'a str,
    pub mail: &'a str,
    pub date_time: &'a str,
    pub author_ch5id: &'a str,
    pub authed_token: &'a str,
    pub ip_addr: &'a str,
    pub board_id: usize,
    pub metadent: MetadentType,
}

#[derive(Debug, Clone)]
pub struct CreatingRes<'a> {
    pub unix_time: &'a str,
    pub body: &'a str,
    pub name: &'a str,
    pub mail: &'a str,
    pub date_time: &'a str,
    pub author_ch5id: &'a str,
    pub authed_token: &'a str,
    pub ip_addr: &'a str,
    pub thread_id: &'a str,
    pub board_id: usize,
}

#[derive(Debug, Clone)]
pub struct CreatingAuthedToken<'a> {
    pub token: &'a str,
    pub origin_ip: &'a str,
    pub writed_time: &'a str,
    pub auth_code: &'a str,
}
