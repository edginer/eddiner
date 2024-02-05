CREATE TABLE IF NOT EXISTS threads (
    thread_number TEXT NOT NULL,
    title TEXT NOT NULL,
    response_count INTEGER NOT NULL,
    last_modified TEXT NOT NULL,
    board_id INTEGER NOT NULL,
    non_auth_thread INTEGER NOT NULL DEFAULT 0,
    archived INTEGER NOT NULL DEFAULT 0,
    active INTEGER NOT NULL DEFAULT 1,
    authed_cookie TEXT,
    metadent TEXT,
    no_pool INTEGER NOT NULL DEFAULT 0
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

CREATE UNIQUE INDEX threads_thread_number_idx ON threads(thread_number);