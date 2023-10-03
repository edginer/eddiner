# Re-eddi (eddi-chan)
なんEとほぼ同等の技術スタックを用いてCloudflare workers上に5ch型掲示板を作ろうのレポジトリです

## インストール方法
### 事前条件
事前にNode.jsとRustをインストールしている必要があります

### 手順
1. wrangler.toml.sampleをwrangler.tomlに名前変更して適切な箇所を埋める
   - いずれもCloudflareのサイト上もしくはwrangler CLIで取得できます
2. `npx wrangler d1 execute zerochedge-d1 --file=./src/schema.sql`でDB初期化
3. `npx wrangler deploy`でデプロイ

## ライセンス
現状ではAGPLです
しかしながら状況の変化によってはライセンスの変更（MITなどへの変更）が生じる可能性があるため、コントリビュート（おもにプルリク）の際にはこの旨に了承しているものと考えます
