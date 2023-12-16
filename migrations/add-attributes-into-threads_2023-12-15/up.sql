ALTER TABLE
    threads
ADD
    COLUMN metadent TEXT;

ALTER TABLE
    threads
ADD
    COLUMN no_pool INTEGER NOT NULL DEFAULT 0;

CREATE TABLE caps (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    cap_name TEXT NOT NULL,
    cap_password_hash TEXT NOT NULL
);