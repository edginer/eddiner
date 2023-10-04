ALTER TABLE authed_cookies ADD COLUMN writed_time TEXT;
ALTER TABLE authed_cookies ADD COLUMN auth_code TEXT;

CREATE INDEX authed_cookies_cookie_idx ON authed_cookies(cookie);
CREATE INDEX authed_cookies_origin_ip ON authed_cookies(origin_ip);
