ALTER TABLE authed_cookies DROP COLUMN last_wrote_time;
ALTER TABLE responses ADD COLUMN timestamp INTEGER default 0;
CREATE INDEX responses_authed_token_idx ON responses(authed_token);
