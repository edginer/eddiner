ALTER TABLE authed_cookies DROP COLUMN writed_time;
ALTER TABLE authed_cookies DROP COLUMN auth_code;

DROP INDEX authed_cookies_cookie_idx;
DROP INDEX authed_cookies_origin_ip;
