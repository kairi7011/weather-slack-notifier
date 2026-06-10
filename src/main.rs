use chrono::{Datelike, Utc};
use chrono_tz::Tz;
use reqwest::Client;
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Debug)]
struct AppError(String);

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Error for AppError {}

#[derive(Clone)]
struct Config {
    slack_bot_token: String,
    slack_channel_id: String,
    weather_url: String,
    weather_lat: String,
    weather_lon: String,
    weather_name: Option<String>,
    timezone: String,
    weekday_only: bool,
}

#[derive(Deserialize)]
struct WeatherResponse {
    daily: DailyForecast,
}

#[derive(Deserialize)]
struct DailyForecast {
    time: Vec<String>,
    #[serde(rename = "weathercode")]
    weather_code: Vec<u16>,
    #[serde(rename = "precipitation_sum")]
    precipitation_sum: Vec<Option<f64>>,
    #[serde(rename = "precipitation_probability_max")]
    precipitation_probability_max: Vec<Option<f64>>,
}

#[derive(Deserialize)]
struct SlackResponse {
    ok: bool,
    error: Option<String>,
}

#[derive(Clone, Copy)]
enum WeatherTone {
    Sunny,
    Cloudy,
    Rain,
    Snow,
    Other,
}

#[derive(Clone)]
struct TodayWeather {
    tone: WeatherTone,
    is_too_wet: bool,
}

#[derive(Default)]
struct CliOptions {
    lat: Option<String>,
    lon: Option<String>,
    name: Option<String>,
    api_url: Option<String>,
    timezone: Option<String>,
    skip_weekday_check: bool,
    show_help: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = CliOptions::from_args()?;
    if args.show_help {
        println!("{}", usage());
        return Ok(());
    }

    let config = Config::from_args(&args)?;

    if config.weekday_only && !is_weekday_in_timezone(&config.timezone)? {
        return Ok(());
    }

    let client = Client::new();
    let weather = fetch_weather(&client, &config).await?;
    let message = compose_message(&config.weather_name, weather);
    post_to_slack(&client, &config, &message).await?;

    Ok(())
}

impl CliOptions {
    fn from_args() -> Result<Self, AppError> {
        let mut raw = env::args().skip(1);
        let mut result = CliOptions::default();

        while let Some(arg) = raw.next() {
            match arg.as_str() {
                "--lat" => {
                    result.lat = Some(require_value("--lat", raw.next())?);
                }
                "--lon" => {
                    result.lon = Some(require_value("--lon", raw.next())?);
                }
                "--name" => {
                    result.name = Some(require_value("--name", raw.next())?);
                }
                "--api-url" => {
                    result.api_url = Some(require_value("--api-url", raw.next())?);
                }
                "--timezone" => {
                    result.timezone = Some(require_value("--timezone", raw.next())?);
                }
                "--skip-weekday-check" => {
                    result.skip_weekday_check = true;
                }
                "--help" | "-h" => {
                    result.show_help = true;
                }
                s if s.starts_with("--lat=") => {
                    result.lat = Some(s.trim_start_matches("--lat=").to_string());
                }
                s if s.starts_with("--lon=") => {
                    result.lon = Some(s.trim_start_matches("--lon=").to_string());
                }
                s if s.starts_with("--name=") => {
                    result.name = Some(s.trim_start_matches("--name=").to_string());
                }
                s if s.starts_with("--api-url=") => {
                    result.api_url = Some(s.trim_start_matches("--api-url=").to_string());
                }
                s if s.starts_with("--timezone=") => {
                    result.timezone = Some(s.trim_start_matches("--timezone=").to_string());
                }
                _ => {
                    return Err(AppError(format!("unknown argument: {arg}")));
                }
            }
        }

        Ok(result)
    }
}

fn require_value(flag: &str, value: Option<String>) -> Result<String, AppError> {
    value.ok_or_else(|| AppError(format!("{flag} requires a value")))
}

impl Config {
    fn env_var(name: &str) -> Option<String> {
        env::var(name).ok().filter(|v| !v.trim().is_empty())
    }

    fn from_args(args: &CliOptions) -> Result<Self, Box<dyn Error>> {
        let slack_bot_token = Self::env_var("SLACK_BOT_TOKEN").ok_or_else(|| {
            AppError("SLACK_BOT_TOKEN is required (environment variable)".to_string()) as Box<dyn Error>
        })?;

        let slack_channel_id = Self::env_var("SLACK_CHANNEL_ID").ok_or_else(|| {
            AppError("SLACK_CHANNEL_ID is required (environment variable)".to_string()) as Box<dyn Error>
        })?;

        let weather_lat = args
            .lat
            .clone()
            .or_else(|| Self::env_var("WEATHER_LAT"))
            .ok_or_else(|| {
                AppError("WEATHER_LAT is required (env or --lat)".to_string()) as Box<dyn Error>
            })?;

        let weather_lon = args
            .lon
            .clone()
            .or_else(|| Self::env_var("WEATHER_LON"))
            .ok_or_else(|| {
                AppError("WEATHER_LON is required (env or --lon)".to_string()) as Box<dyn Error>
            })?;

        validate_coordinate(&weather_lat, "WEATHER_LAT", -90.0, 90.0)?;
        validate_coordinate(&weather_lon, "WEATHER_LON", -180.0, 180.0)?;

        let weather_name = sanitize_optional_label(args.name.clone().or_else(|| Self::env_var("WEATHER_NAME")));

        let weather_url = args
            .api_url
            .clone()
            .or_else(|| Self::env_var("WEATHER_API_URL"))
            .unwrap_or_else(|| "https://api.open-meteo.com/v1/forecast".to_string());
        validate_https_url(&weather_url, "WEATHER_API_URL")?;

        let timezone = args
            .timezone
            .clone()
            .or_else(|| Self::env_var("WEATHER_TIMEZONE"))
            .unwrap_or_else(|| "Asia/Tokyo".to_string());
        validate_timezone(&timezone)?;

        Ok(Self {
            slack_bot_token,
            slack_channel_id,
            weather_url,
            weather_lat,
            weather_lon,
            weather_name,
            timezone,
            weekday_only: !args.skip_weekday_check,
        })
    }
}

fn sanitize_optional_label(raw: Option<String>) -> Option<String> {
    let value = raw?.trim().to_string();
    if value.is_empty() {
        return None;
    }
    let filtered: String = value
        .chars()
        .filter(|ch| !matches!(ch, '\n' | '\r'))
        .collect();
    let trimmed = filtered.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(64).collect())
    }
}

fn validate_coordinate(value: &str, name: &str, min: f64, max: f64) -> Result<(), AppError> {
    let parsed: f64 = value
        .trim()
        .parse()
        .map_err(|_| AppError(format!("{name} must be a decimal number: {value}")))?;

    if !(min..=max).contains(&parsed) {
        return Err(AppError(format!("{name} must be in range [{min}, {max}]")));
    }

    Ok(())
}

fn validate_https_url(value: &str, name: &str) -> Result<(), AppError> {
    if value.starts_with("https://") {
        return Ok(());
    }

    Err(AppError(format!("{name} must start with https://")))
}

fn validate_timezone(value: &str) -> Result<(), AppError> {
    Tz::from_str(value)
        .map(|_| ())
        .map_err(|_| AppError(format!("WEATHER_TIMEZONE is invalid: {value}")))
}

fn is_weekday_in_timezone(timezone: &str) -> Result<bool, AppError> {
    let tz = Tz::from_str(timezone).map_err(|_| AppError(format!("invalid timezone: {timezone}")))?;
    let now = Utc::now().with_timezone(&tz);
    let weekday = now.weekday().number_from_monday();
    Ok((1..=5).contains(&weekday))
}

fn usage() -> &'static str {
    "Usage:\n\
  cargo run --release -- \\\n\
    --lat <latitude> --lon <longitude> [--name <label>] [--api-url <url>] [--timezone <tz>] [--skip-weekday-check]\n\
    --help\n\
    [or set config via env vars]\n\
    env vars:\n\
    WEATHER_LAT, WEATHER_LON, WEATHER_NAME, WEATHER_API_URL, WEATHER_TIMEZONE\n\
    Required env vars:\n\
    SLACK_BOT_TOKEN, SLACK_CHANNEL_ID"
}

async fn fetch_weather(client: &Client, config: &Config) -> Result<TodayWeather, Box<dyn Error>> {
    let tz = Tz::from_str(&config.timezone).map_err(|_| {
        Box::new(AppError(format!("invalid timezone: {}", config.timezone))) as Box<dyn Error>
    })?;

    let today = Utc::now()
        .with_timezone(&tz)
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();

    let url = format!(
        "{base}?latitude={lat}&longitude={lon}&daily=weathercode,precipitation_sum,precipitation_probability_max&timezone={tz}&forecast_days=2",
        base = config.weather_url,
        lat = config.weather_lat,
        lon = config.weather_lon,
        tz = config.timezone,
    );

    let resp = client.get(url).send().await?;
    let resp = resp.error_for_status()?;
    let body: WeatherResponse = resp.json().await?;

    let idx = body
        .daily
        .time
        .iter()
        .position(|date| date == &today)
        .or_else(|| body.daily.time.first().map(|_| 0))
        .ok_or_else(|| {
            AppError("weather forecast does not have any daily data".to_string())
        })?;

    let code = body.daily.weather_code.get(idx).copied().unwrap_or(0);
    let precipitation_sum = body.daily.precipitation_sum.get(idx).and_then(|v| *v).unwrap_or(0.0);
    let precipitation_probability = body
        .daily
        .precipitation_probability_max
        .get(idx)
        .and_then(|v| *v)
        .unwrap_or(0.0);

    let tone = classify_tone(code);
    let is_too_wet = precipitation_sum >= 12.0
        || precipitation_probability >= 70.0
        || matches!(code, 61 | 63 | 65 | 66 | 67 | 80 | 81 | 82 | 95 | 96 | 99);

    Ok(TodayWeather { tone, is_too_wet })
}

fn classify_tone(code: u16) -> WeatherTone {
    if code == 0 || code == 1 {
        WeatherTone::Sunny
    } else if code == 2 || code == 3 || code == 45 || code == 48 {
        WeatherTone::Cloudy
    } else if matches!(
        code,
        51 | 53 | 55 | 56 | 57 | 61 | 63 | 65 | 66 | 67 | 80 | 81 | 82 | 95 | 96 | 99
    ) {
        WeatherTone::Rain
    } else if matches!(code, 71 | 73 | 75 | 77 | 85 | 86) {
        WeatherTone::Snow
    } else {
        WeatherTone::Other
    }
}

fn compose_message(location_name: &Option<String>, forecast: TodayWeather) -> String {
    let mut message = match forecast.tone {
        WeatherTone::Sunny => "晴れです".to_string(),
        WeatherTone::Cloudy => "曇りです".to_string(),
        WeatherTone::Snow => "雪です".to_string(),
        WeatherTone::Rain => {
            if forecast.is_too_wet {
                "@here 滝が降ります、傘を持っていきましょう\n出来ればリモートしましょう".to_string()
            } else {
                "@here 雨です、傘を持っていきましょう".to_string()
            }
        }
        WeatherTone::Other => "天候が取得できません".to_string(),
    };

    if !matches!(forecast.tone, WeatherTone::Rain) {
        if let Some(name) = location_name.as_deref() {
            message = format!("{name}: {message}");
        }
    }

    message
}

async fn post_to_slack(client: &Client, config: &Config, text: &str) -> Result<(), Box<dyn Error>> {
    let response = client
        .post("https://slack.com/api/chat.postMessage")
        .bearer_auth(&config.slack_bot_token)
        .json(&serde_json::json!({
            "channel": config.slack_channel_id,
            "text": text,
        }))
        .send()
        .await?;

    let response = response.error_for_status()?;
    let body: SlackResponse = response.json().await?;
    if body.ok {
        return Ok(());
    }

    Err(Box::new(AppError(format!(
        "slack API error: {}",
        body.error.unwrap_or_else(|| "unknown".to_string())
    ))))
}

