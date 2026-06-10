use chrono::{Datelike, Utc};
use chrono_tz::{Asia::Tokyo, Tz};
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
    weather_name: String,
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
    code: u16,
    is_too_wet: bool,
}

#[derive(Default)]
struct CliOptions {
    bot_token: Option<String>,
    channel_id: Option<String>,
    lat: Option<String>,
    lon: Option<String>,
    name: Option<String>,
    api_url: Option<String>,
    timezone: Option<String>,
    skip_weekday_check: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = CliOptions::from_args();
    let config = Config::from_args(&args)?;

    if config.weekday_only && !is_weekday_in_timezone(&config.timezone) {
        return Ok(());
    }

    let client = Client::new();
    let weather = fetch_weather(&client, &config).await?;
    let message = compose_message(&config.weather_name, weather);
    post_to_slack(&client, &config, &message).await?;

    Ok(())
}

impl CliOptions {
    fn from_args() -> Self {
        let mut raw = env::args().skip(1);
        let mut result = CliOptions::default();

        while let Some(arg) = raw.next() {
            match arg.as_str() {
                "--bot-token" | "--token" | "--slack-bot-token" => {
                    result.bot_token = raw.next();
                }
                "--channel-id" | "--channel" => {
                    result.channel_id = raw.next();
                }
                "--lat" => {
                    result.lat = raw.next();
                }
                "--lon" => {
                    result.lon = raw.next();
                }
                "--name" => {
                    result.name = raw.next();
                }
                "--api-url" => {
                    result.api_url = raw.next();
                }
                "--timezone" => {
                    result.timezone = raw.next();
                }
                "--skip-weekday-check" => {
                    result.skip_weekday_check = true;
                }
                "--weekday-only" => {
                    result.skip_weekday_check = false;
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
                _ => {}
            }
        }

        result
    }
}

impl Config {
    fn env_var(name: &str) -> Option<String> {
        env::var(name).ok()
    }

    fn from_args(args: &CliOptions) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            slack_bot_token: args
                .bot_token
                .clone()
                .or_else(|| Self::env_var("SLACK_BOT_TOKEN"))
                .ok_or_else(|| {
                    Box::new(AppError(
                        "SLACK_BOT_TOKEN is required (env or --bot-token)".to_string(),
                    )) as Box<dyn Error>
                })?,
            slack_channel_id: args
                .channel_id
                .clone()
                .or_else(|| Self::env_var("SLACK_CHANNEL_ID"))
                .ok_or_else(|| {
                    Box::new(AppError(
                        "SLACK_CHANNEL_ID is required (env or --channel-id)".to_string(),
                    )) as Box<dyn Error>
                })?,
            weather_lat: args
                .lat
                .clone()
                .or_else(|| Self::env_var("WEATHER_LAT"))
                .unwrap_or_else(|| "35.6895".to_string()),
            weather_lon: args
                .lon
                .clone()
                .or_else(|| Self::env_var("WEATHER_LON"))
                .unwrap_or_else(|| "139.6917".to_string()),
            weather_name: args
                .name
                .clone()
                .or_else(|| Self::env_var("WEATHER_NAME"))
                .unwrap_or_else(|| "\u897f\u65b0\u5bbf".to_string()),
            weather_url: args
                .api_url
                .clone()
                .or_else(|| Self::env_var("WEATHER_API_URL"))
                .unwrap_or_else(|| "https://api.open-meteo.com/v1/forecast".to_string()),
            timezone: args
                .timezone
                .clone()
                .or_else(|| Self::env_var("WEATHER_TIMEZONE"))
                .unwrap_or_else(|| "Asia/Tokyo".to_string()),
            weekday_only: !args.skip_weekday_check,
        })
    }
}

fn is_weekday_in_timezone(timezone: &str) -> bool {
    let tz = Tz::from_str(timezone).unwrap_or(Tokyo);
    let now = Utc::now().with_timezone(&tz);
    let weekday = now.weekday().number_from_monday();
    (1..=5).contains(&weekday)
}

async fn fetch_weather(client: &Client, config: &Config) -> Result<TodayWeather, Box<dyn Error>> {
    let tz = Tz::from_str(&config.timezone).unwrap_or(Tokyo);
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
        tz = config.timezone
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
            Box::new(AppError("weather forecast does not have any daily data".to_string())) as Box<dyn Error>
        })?;

    let code = body.daily.weather_code.get(idx).copied().unwrap_or(0);
    let precipitation_sum = body.daily.precipitation_sum.get(idx).and_then(|v| *v).unwrap_or(0.0);
    let precipitation_probability = body
        .daily
        .precipitation_probability_max
        .get(idx)
        .and_then(|v| *v)
        .unwrap_or(0.0);

    let weather_tone = classify_tone(code);
    let is_too_wet = precipitation_sum >= 12.0
        || precipitation_probability >= 70.0
        || matches!(code, 61 | 63 | 65 | 66 | 67 | 80 | 81 | 82 | 95 | 96 | 99);

    Ok(TodayWeather {
        code,
        is_too_wet,
    })
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

fn compose_message(location_name: &str, forecast: TodayWeather) -> String {
    let tone = classify_tone(forecast.code);
    let body = match tone {
        WeatherTone::Sunny => "\u6674\u308c\u3067\u3059".to_string(),
        WeatherTone::Cloudy => "\u66ec\u308a\u3077\u308c\u3067\u3059".to_string(),
        WeatherTone::Snow => "\u96ea\u3067\u3059".to_string(),
        WeatherTone::Rain => {
            if forecast.is_too_wet {
                "\u6d6a\u304c\u964d\u308a\u307e\u3059".to_string()
            } else {
                "\u96e8\u3067\u3059".to_string()
            }
        }
        WeatherTone::Other => "\u5929\u5019\u3092\u53d6\u5f97\u3067\u304d\u307e\u305b\u3093".to_string(),
    };

    let mut message = format!("{location_name}: {body}");

    if let WeatherTone::Rain = tone {
        if forecast.is_too_wet {
            message =
                "@here \u6d6a\u304c\u964d\u308a\u307e\u3059\u3001\u96f0\u3092\u6301\u3063\u3066\u304d\u307e\u3057\u3087\u3046\n\u51fa\u6765\u308c\u3070\u30ea\u30e2\u30fc\u30c8\u3057\u307e\u3057\u3087\u3046"
                    .to_string();
        } else {
            message =
                "@here \u96e8\u3067\u3059\u3001\u96f0\u3092\u6301\u3063\u3066\u304d\u307e\u3057\u3087\u3046".to_string();
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
