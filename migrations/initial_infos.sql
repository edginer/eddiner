CREATE TABLE IF NOT EXISTS boards (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    local_rule TEXT,
    board_key TEXT
);

CREATE TABLE IF NOT EXISTS authed_cookies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cookie TEXT NOT NULL,
    authed_time TEXT,
    origin_ip TEXT NOT NULL,
    authed INTEGER NOT NULL,
    writed_time TEXT,
    auth_code TEXT,
    last_thread_creation TEXT
);

CREATE TABLE IF NOT EXISTS archives (
    thread_number TEXT NOT NULL,
    title TEXT NOT NULL,
    response_count INTEGER NOT NULL,
    board_id INTEGER NOT NULL,
    last_modified TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS caps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cap_name TEXT NOT NULL,
    cap_password_hash TEXT NOT NULL
);

INSERT INTO
    boards (name, local_rule, board_key)
VALUES
    (
        'なんでも実況エッヂ',
        '<hr>
<br>
<b>以下がローカルルールです<br><br>

<a href="/">全体の利用規約</a>などはこちらに<br>
<a href="https://github.com/edginer/eddiner/discussions/">運営への相談・報告はこちらへ</a>
<br>',
        'liveedge'
    );

CREATE INDEX authed_cookies_cookie_idx ON authed_cookies(cookie);

CREATE INDEX authed_cookies_origin_ip ON authed_cookies(origin_ip);