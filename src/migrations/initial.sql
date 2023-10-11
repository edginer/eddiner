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
    active INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE responses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT, -- author name
    mail TEXT,
    date TEXT NOT NULL,
    author_id TEXT,
    body TEXT NOT NULL,
    thread_id TEXT NOT NULL,
    ip_addr TEXT NOT NULL
);

CREATE TABLE boards (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL
);

CREATE TABLE authed_cookies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cookie TEXT NOT NULL,
    authed_time TEXT,
    origin_ip TEXT NOT NULL,
    authed INTEGER NOT NULL
);

INSERT INTO boards (name) VALUES ('なんでも実況エッヂ');
INSERT INTO threads (thread_number, title, response_count, last_modified, board_id)
VALUES ('1696233330', 'test', 1, '1696233330', 1);
INSERT INTO responses (
    date,
    body,
    thread_id,
    ip_addr
) VALUES (
    'Mon, 02 Oct 2023 07:55:30 GMT',
    'testtest',
    '1696233330',
    '127.0.0.1'
);

