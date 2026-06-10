# Weather Slack Notifier (GitHub Actions)

## 概要

Open-Meteo の1日天気予報を取得して、平日のみ Slack に投稿する Rust 製ジョブです。
週末・祝日は投稿しません（平日チェックはデフォルトON）。

## 投稿メッセージ

- 晴れ: `晴れです`
- 曇り: `曇りです`
- 雨: `@here 雨です、傘を持っていきましょう`
- 非常に強い雨: `@here 滝が降ります、傘を持っていきましょう`と`出来ればリモートしましょう`

## 設定

このワークフローは以下の2種類の値を使います。

- シークレット（暗号化・マスク）
  - `SLACK_BOT_TOKEN`
  - `SLACK_CHANNEL_ID`
- 変数（公開情報）
  - `WEATHER_LAT`
  - `WEATHER_LON`
  - `WEATHER_NAME`（任意・表示名）
  - `WEATHER_API_URL`（任意、未設定時: `https://api.open-meteo.com/v1/forecast`）
  - `WEATHER_TIMEZONE`（任意、未設定時: `Asia/Tokyo`）

### 実行時引数

以下はワークフローから実行されるときに `--lat` / `--lon` / `--name` / `--api-url` / `--timezone` として引数化されています。

- 引数が未指定でも、同名の環境変数から補完されます。
- `SLACK_BOT_TOKEN`, `SLACK_CHANNEL_ID` は環境変数経由のみ。
- `--help` を付けると使い方を表示します。

## セキュリティ上の注意

- トークンはリポジトリシークレットにのみ保存し、コミットに含めない。
- 公開リポジトリでは `vars`（リポジトリ変数）は機密情報として扱わない。
- 固有名詞（`WEATHER_NAME` など）は必要最小限にし、公開情報に残す内容を意識する。
- このページで共有されたトークン文字列は公開された情報なので、すぐにローテーションしてください。

## 運用

- スケジュール実行: 毎日 08:00 (JST)
- 手動実行: `workflow_dispatch`

