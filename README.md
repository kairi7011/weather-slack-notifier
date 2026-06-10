# Weather Slack Notifier

A small GitHub Actions job in Rust that posts the daily weather forecast to Slack at 08:00 Asia/Tokyo.

- sunny -> `晴れです`
- cloudy -> `曇りです`
- rain -> `<!here> 雨です、傘を持っていきましょう`
- heavy rain -> `<!here> 滝が降ります、傘を持っていきましょう。\n出来ればリモートしましょう`

## What is configured by environment

| required | variable |
| - | - |
| ✓ | `SLACK_BOT_TOKEN` |
| ✓ | `SLACK_CHANNEL_ID` |
| ✓ | `WEATHER_LAT` |
| ✓ | `WEATHER_LON` |

### Optional

| optional | variable |
| - | - |
| | `WEATHER_NAME` |
| | `WEATHER_API_URL` (default: `https://api.open-meteo.com/v1/forecast`) |
| | `WEATHER_TIMEZONE` (default: `Asia/Tokyo`) |

## How to run locally

- `SLACK_BOT_TOKEN` and `SLACK_CHANNEL_ID` are required in env
- pass location as CLI args to avoid shell history leakage:

```bash
$env:SLACK_BOT_TOKEN = "xoxb-..."
$env:SLACK_CHANNEL_ID = "C092..."
$env:WEATHER_LAT = "35.6895"
$env:WEATHER_LON = "139.6917"
cargo run -- --lat 35.6895 --lon 139.6917 --name "Shinjuku"
```

Or, if preferred, pass `--lat/--lon/--name` and keep API values in env:

```bash
cargo run -- --lat 35.6895 --lon 139.6917 --name "Shinjuku" --api-url "https://api.open-meteo.com/v1/forecast" --timezone "Asia/Tokyo"
```

## Security notes

- Never commit secrets. Store Slack token and channel in GitHub Secrets.
- Location names and coordinates are treated as non-secret input and can be moved to repository variables if shared.
- Message content, threshold rules, and timezone are configurable in code constants and tests.

## GitHub Actions

Workflow runs:

- Scheduled at 08:00 JST (`0 23 * * *` in UTC)
- On manual `workflow_dispatch`

The workflow passes secrets/vars into environment and runs:

```bash
cargo run --release -- --lat "$WEATHER_LAT" --lon "$WEATHER_LON" --api-url "$WEATHER_API_URL" --timezone "$WEATHER_TIMEZONE" [--name "$WEATHER_NAME"]
```