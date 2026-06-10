# Weather Slack Notifier

A small GitHub Actions job in Rust that posts the daily weather forecast to Slack at 08:00 (Asia/Tokyo).

- sunny -> `晴れです`
- cloudy -> `曇りです`
- rain -> `<!here> 雨です、傘を持っていきましょう`
- heavy rain -> `<!here> 滝が降ります、傘を持っていきましょう。出来ればリモートしましょう`

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

CLI options:

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
cargo run -- --lat 35.6895 --lon 139.6917 --name "Shinjuku" --api-url "https://api.open-meteo.com/v1/forecast" --timezone "Asia/Tokyo"
```

## Security notes

- Store `SLACK_BOT_TOKEN` and `SLACK_CHANNEL_ID` in GitHub Secrets.
- Keep location values as repository variables only if shared, and rotate tokens immediately if leaked.
- Message text is intentionally simple and can be extended by updating tests and constants.

## GitHub Actions

- Runs at 08:00 JST (`0 23 * * *` UTC).
- Supports `workflow_dispatch` for manual runs.
- The workflow passes environment variables and runs:

```bash
cargo run --release -- --lat "$WEATHER_LAT" --lon "$WEATHER_LON" --api-url "$WEATHER_API_URL" --timezone "$WEATHER_TIMEZONE" [--name "$WEATHER_NAME"]
```