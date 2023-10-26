use worker::D1Database;

use crate::{authed_cookie::AuthedCookie, response::Res};

pub struct BbsRepository<'a> {
    db: &'a D1Database,
}

impl<'a> BbsRepository<'a> {
    pub fn new(db: &'a D1Database) -> BbsRepository<'a> {
        BbsRepository { db }
    }
}

impl BbsRepository<'_> {
    pub async fn get_thread(
        &self,
        board_id: usize,
        thread_id: &str,
    ) -> anyhow::Result<Option<crate::thread::Thread>> {
        let Ok(stmt) = self
            .db
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

    async fn get_threads(
        &self,
        board_id: usize,
        status: ThreadStatus,
    ) -> anyhow::Result<Vec<crate::thread::Thread>> {
        let Ok(stmt) = self
            .db
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

    async fn get_responses(&self, board_id: usize, thread_id: &str) -> anyhow::Result<Vec<Res>> {
        let Ok(stmt) = self
            .db
            .prepare("SELECT * FROM responses WHERE thread_id = ? AND board_id = ?")
            .bind(&[thread_id.into(), board_id.into()])
        else {
            return Err(anyhow::anyhow!("failed to bind thread_id and board_id"));
        };
        let Ok(responses) = stmt.all().await.and_then(|res| res.results::<Res>()) else {
            return Err(anyhow::anyhow!("failed to fetch responses"));
        };

        Ok(responses)
    }

    pub async fn get_responses_by_authed_token_and_timestamp(
        &self,
        authed_token: &str,
        min_timestamp: &str,
    ) -> anyhow::Result<Vec<Res>> {
        let Ok(stmt) = self
            .db
            .prepare("SELECT * FROM responses WHERE authed_token = ? AND timestamp > ?")
            .bind(&[authed_token.into(), min_timestamp.to_string().into()])
        else {
            return Err(anyhow::anyhow!("failed to bind authed_token and timestamp"));
        };

        if let Ok(responses) = stmt.all().await.and_then(|res| res.results::<Res>()) {
            Ok(responses)
        } else {
            Err(anyhow::anyhow!("failed to fetch responses"))
        }
    }

    pub async fn get_authed_token(&self, token: &str) -> anyhow::Result<Option<AuthedCookie>> {
        let Ok(stmt) = self
            .db
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

    pub async fn create_thread(&self, thread: CreatingThread<'_>) -> anyhow::Result<()> {
        let th_stmt = self
            .db
            .prepare(
                "INSERT INTO threads
                (thread_number, title, response_count, board_id, last_modified, authed_cookie)
                VALUES (?, ?, 1, 1, ?, ?)",
            )
            .bind(&[
                thread.unix_time.into(),
                thread.title.into(),
                thread.unix_time.into(),
                thread.authed_token.into(),
            ]);

        let res_stmt = self
            .db
            .prepare(
                "INSERT INTO responses 
                (name, mail, date, author_id, body, thread_id, ip_addr, authed_token, timestamp)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
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

    pub async fn create_response(&self, res: CreatingRes<'_>) -> anyhow::Result<()> {
        let update_th_stmt = self
            .db
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
            WHERE thread_number = ?",
            ) // 999 means thread stopper 1000
            .bind(&[res.unix_time.into(), res.thread_id.into()]);
        let res_stmt = self
            .db
            .prepare(
                "INSERT INTO responses 
                (name, mail, date, author_id, body, thread_id, ip_addr, authed_token, timestamp) 
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
            ]);

        match (update_th_stmt, res_stmt) {
            (Ok(update_th_stmt), Ok(res_stmt)) => {
                if (self.db.batch(vec![update_th_stmt, res_stmt]).await).is_err() {
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
            .db
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
            .db
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
}

#[derive(Debug, Clone, Copy)]
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
