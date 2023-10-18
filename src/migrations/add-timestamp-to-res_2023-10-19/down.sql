ALTER TABLE authed_cookies ADD COLUMN last_wrote_time;
ALTER TABLE responses DROP COLUMN timestamp;
