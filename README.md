# Weather Slack Notifier

A small GitHub Actions job in Rust, scheduled at 04:30 (Asia/Tokyo), that posts the daily weather forecast to Slack.

- sunny -> `晴れです`
- cloudy -> `曇りです`
- rain -> `<!here> 雨の時間帯があります`
- heavy rain -> `<!here> 滝が降ります、傘を持っていきましょう。できればリモートしましょう`

When hourly forecast data is available, the message is built around commute
impact and always includes the concrete rain time ranges:

- every rainy message includes `雨の時間帯: HH:MM-HH:MM` so readers know when it rains
- `<!here>` is used only when rain or severe weather actually hits the commute,
  return-commute, lunch, or after-work windows — rain that ends within the first
  hour of the commute buffer (before most departures) does not mention
- all-day rain -> `一日雨の予報です` + `雨の時間帯: 01:00-24:00` + `出勤・退勤とも傘が必要です`
  (no redundant per-window lines when it rains all day)
- all-day heavy rain -> `一日滝が降る予報です` + `できればリモートしましょう`
- commute rain -> `出勤時間帯に雨が降りそうです` + `傘を持っていきましょう` (+ `退勤時間帯は止む見込みです` when it clears)
- return rain -> `退勤時間帯に雨が降りそうです` + `帰りに降る見込みです。朝から傘を持っていきましょう`
- lunch rain -> `昼に雨が降りそうです` + `昼(12:00-13:00)に外へ出るなら傘があると安心です`
- heavy rain only after the return window -> `退勤後の夜に激しい雨が降りそうです` + `荒れるのは退勤後です。残業せず早めに帰りましょう`
- rain ending before most departures -> `朝早くに雨が降りそうです` + `出勤時間帯には止む見込みです（早く出るなら傘を）` (no `<!here>`)
- low-impact rain -> `雨の時間帯: 16:00-17:00（外出予定がなければ影響は小さめです）`
- every rainy message also includes an hourly probability sparkline, e.g.
  `降水確率: ▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▁▃▅▆▅▃▁▁▁ 最大80%（18時・1.2mm）`
  (one block character per hour 00-23; omitted when no hour has rain probability)

Open-Meteo does not identify named typhoons or official warnings in this
forecast response. Special notes are derived from forecast variables:

- heavy rain -> hourly precipitation from 12 mm/h or WMO rain codes 63, 65, 66, 67, 81, 82
- thunderstorm -> WMO weather codes 95, 96, 99
- strong wind -> hourly wind gusts from 54 km/h
- storm wind -> hourly wind gusts from 72 km/h

## What is configured by environment

| required | variable |
| - | - |
| yes | `SLACK_BOT_TOKEN` |
| yes | `SLACK_CHANNEL_ID` |
| yes | `WEATHER_LAT` |
| yes | `WEATHER_LON` |

### Optional

| optional | variable |
| - | - |
| | `WEATHER_NAME` |
| | `WEATHER_API_URL` (default: `https://api.open-meteo.com/v1/forecast`) |
| | `WEATHER_TIMEZONE` (default: `Asia/Tokyo`) |
| | `WEATHER_WORK_START_TIME` (default: `10:00`) |
| | `WEATHER_WORK_END_TIME` (default: `19:00`) |
| | `WEATHER_WORK_BUFFER_HOURS` (default: `2`) |

CLI options:

- `--work-start-time <HH:MM>`
  - default: `WEATHER_WORK_START_TIME` or `10:00`
- `--work-end-time <HH:MM>`
  - default: `WEATHER_WORK_END_TIME` or `19:00`
- `--work-buffer-hours <HOURS>`
  - default: `WEATHER_WORK_BUFFER_HOURS` or `2`
  - `10:00-19:00` with buffer `2` checks `08:00-10:00` for commute and `19:00-21:00` for return commute
- `--skip-weekday-check`
  - default: false (weekday filtering ON)
  - when set, weekend posting check is skipped
- `--skip-holiday-check`
  - default: false (holiday filtering ON)
  - when set, holiday filtering is skipped

## How to run locally

```bash
# Required env vars
export SLACK_BOT_TOKEN="xoxb-..."
export SLACK_CHANNEL_ID="C092..."
export WEATHER_LAT="35.6895"
export WEATHER_LON="139.6917"

# Post using current location label
cargo run -- --lat "$WEATHER_LAT" --lon "$WEATHER_LON" --name "Shinjuku"
```

You can also pass each value directly with CLI flags:

```bash
cargo run -- --lat 35.6895 --lon 139.6917 --name "Shinjuku" --api-url "https://api.open-meteo.com/v1/forecast" --timezone "Asia/Tokyo" --work-start-time "10:00" --work-end-time "19:00" --work-buffer-hours "2"
```

## Security notes

- Store `SLACK_BOT_TOKEN` and `SLACK_CHANNEL_ID` in GitHub Secrets.
- Keep location values as repository variables only if shared, and rotate tokens immediately if leaked.
- Message text is intentionally simple and can be extended by updating tests and constants.

## GitHub Actions

- Runs at 04:30 JST (`30 19 * * *` UTC).
- Supports `workflow_dispatch` for manual runs.
- The workflow passes environment variables and runs:

```bash
cargo run --release -- --lat "$WEATHER_LAT" --lon "$WEATHER_LON" --api-url "$WEATHER_API_URL" --timezone "$WEATHER_TIMEZONE" --work-start-time "$WEATHER_WORK_START_TIME" --work-end-time "$WEATHER_WORK_END_TIME" --work-buffer-hours "$WEATHER_WORK_BUFFER_HOURS" [--name "$WEATHER_NAME"]
```
