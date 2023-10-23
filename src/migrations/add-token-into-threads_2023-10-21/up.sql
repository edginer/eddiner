ALTER TABLE threads ADD COLUMN authed_cookie TEXT; 
CREATE INDEX threads_authed_token_idx ON responses(authed_token);
