CREATE TABLE IF NOT EXISTS responses (
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

INSERT INTO
    responses (date, body, thread_id, ip_addr)
VALUES
    (
        'Mon, 02 Oct 2023 07:55:30 GMT',
        'testtest',
        '1696233330',
        '127.0.0.1'
    );

CREATE INDEX responses_thread_number_idx ON responses(thread_id);

CREATE INDEX responses_authed_token_idx ON responses(authed_token);