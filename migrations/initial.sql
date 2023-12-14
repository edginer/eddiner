DROP TABLE IF EXISTS threads;

DROP TABLE IF EXISTS boards;

DROP TABLE IF EXISTS responses;

DROP TABLE IF EXISTS authed_cookies;

CREATE TABLE threads (
    thread_number TEXT NOT NULL,
    title TEXT NOT NULL,
    response_count INTEGER NOT NULL,
    last_modified TEXT NOT NULL,
    board_id INTEGER NOT NULL,
    non_auth_thread INTEGER NOT NULL DEFAULT 0,
    archived INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1,
    authed_cookie TEXT
);

CREATE TABLE responses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT,
    -- author name
    mail TEXT,
    date TEXT NOT NULL,
    author_id TEXT,
    body TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    ip_addr TEXT NOT NULL,
    authed_token TEXT,
    timestamp INTEGER DEFAULT 0,
    board_id INTEGER NOT NULL DEFAULT 1,
    is_abone INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE boards (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    local_rule TEXT,
    board_key TEXT
);

CREATE TABLE authed_cookies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cookie TEXT NOT NULL,
    authed_time TEXT,
    origin_ip TEXT NOT NULL,
    authed INTEGER NOT NULL,
    writed_time TEXT,
    auth_code TEXT,
    last_thread_creation TEXT
);

CREATE TABLE archives (
    thread_number TEXT NOT NULL,
    title TEXT NOT NULL,
    response_count INTEGER NOT NULL,
    board_id INTEGER NOT NULL,
    last_modified TEXT NOT NULL
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
<a href="https://git.3chan.cc/edginer/eddiner/issues">運営・開発への要望・相談・報告はこちらへ</a>
<br>',
        'liveedge'
    );

INSERT INTO
    threads (
        thread_number,
        title,
        response_count,
        last_modified,
        board_id
    )
VALUES
    ('1696233330', 'test', 1, '1696233330', 1);

INSERT INTO
    responses (date, body, thread_id, ip_addr)
VALUES
    (
        'Mon, 02 Oct 2023 07:55:30 GMT',
        'testtest',
        '1696233330',
        '127.0.0.1'
    );

CREATE INDEX authed_cookies_cookie_idx ON authed_cookies(cookie);

CREATE INDEX authed_cookies_origin_ip ON authed_cookies(origin_ip);

CREATE UNIQUE INDEX threads_thread_number_idx ON threads(thread_number);

CREATE INDEX threads_authed_token_idx ON responses(authed_token);

CREATE INDEX responses_thread_number_idx ON responses(thread_id);

CREATE INDEX responses_authed_token_idx ON responses(authed_token);

CREATE INDEX archives_thread_number_idx ON archives(thread_number);