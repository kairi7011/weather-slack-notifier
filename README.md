# Weather Slack Notifier

A small GitHub Actions job in Rust, scheduled at 04:30 (Asia/Tokyo), that posts the daily rain-impact forecast to Slack.

- sunny -> `晴れです`
- cloudy -> `曇りです`
- rain -> `<!here> 雨の時間帯があります`
- heavy rain -> `<!here> 滝が降ります、傘を持っていきましょう。出来ればリモートしましょう`

When hourly forecast data is available, rain is reported by affected time band:

- `08:00-09:00`: early commute
- `09:00-10:00`: commute
- `12:00-14:00`: lunch
- `19:00-20:00`: return commute
- `20:00-21:00`: late return commute
- `21:00-24:00`: long-overtime return
- other hours: lower-impact rain unless you have a planned trip

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
