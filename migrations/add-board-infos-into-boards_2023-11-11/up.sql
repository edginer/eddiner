ALTER TABLE boards ADD COLUMN local_rule TEXT;
ALTER TABLE boards ADD COLUMN board_key TEXT;

UPDATE boards SET local_rule = '<hr>
<br>
<b>以下がローカルルールです<br><br>

<a href="/">全体の利用規約</a>などはこちらに<br>
<a href="https://git.3chan.cc/edginer/eddiner/issues">運営・開発への要望・相談・報告はこちらへ</a>
<br>' WHERE id = 1;
UPDATE boards SET board_key = 'liveedge' WHERE id = 1;
