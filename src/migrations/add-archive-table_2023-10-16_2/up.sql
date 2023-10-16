CREATE TABLE archives (
    thread_number TEXT NOT NULL,
    title TEXT NOT NULL,
    response_count INTEGER NOT NULL,
    board_id INTEGER NOT NULL,
    last_modified TEXT NOT NULL
);

CREATE INDEX archives_thread_number_idx ON archives (thread_number);
