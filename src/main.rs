use chrono::{Datelike, Utc};
use chrono_tz::Asia::Tokyo;
use reqwest::Client;
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fmt::{Display, Formatter};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;

    if !is_weekday_jst() {
        return Ok(());
    }

    let client = Client::new();
    let today = fetch_weather(&client, &config).await?;
    let message = compose_message(&config.weather_name, today);
    post_to_slack(&client, &config, &message).await?;

    Ok(())
}

fn is_weekday_jst() -> bool {
    let now = Utc::now().with_timezone(&Tokyo);
    let weekday = now.weekday().number_from_monday();
    (1..=5).contains(&weekday)
}

impl Config {
    fn get_env(name: &str) -> Result<String, Box<dyn Error>> {
        env::var(name).map_err(|_| {
            Box::new(AppError(format!(
                "environment variable `{}` is required",
                name
            ))) as Box<dyn Error>
        })
    }

    fn from_env() -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            slack_bot_token: Self::get_env("SLACK_BOT_TOKEN")?,
            slack_channel_id: Self::get_env("SLACK_CHANNEL_ID")?,
            weather_lat: env::var("WEATHER_LAT").unwrap_or_else(|_| "35.6895".to_string()),
            weather_lon: env::var("WEATHER_LON").unwrap_or_else(|_| "139.6917".to_string()),
            weather_name: env::var("WEATHER_NAME").unwrap_or_else(|_| "今日の天気".to_string()),
            weather_url: env::var("WEATHER_API_URL")
                .unwrap_or_else(|_| "https://api.open-meteo.com/v1/forecast".to_string()),
        })
    }
}

async fn fetch_weather(client: &Client, config: &Config) -> Result<TodayWeather, Box<dyn Error>> {
    let url = format!(
        "{base}?latitude={lat}&longitude={lon}&daily=weathercode,precipitation_sum,precipitation_probability_max&timezone=Asia%2FTokyo&forecast_days=2",
        base = config.weather_url,
        lat = config.weather_lat,
        lon = config.weather_lon
    );

    let resp = client.get(url).send().await?;
    let resp = resp.error_for_status()?;
    let body: WeatherResponse = resp.json().await?;
    let today = Utc::now().with_timezone(&Tokyo).date_naive();
    let today = today.format("%Y-%m-%d").to_string();

    let idx = body
        .daily
        .time
        .iter()
        .position(|date| date == &today)
        .or_else(|| body.daily.time.first().map(|_| 0))
        .ok_or_else(|| {
            Box::new(AppError("weather forecast does not have any daily data".to_string())) as Box<dyn Error>
        })?;

    let code = body
        .daily
        .weather_code
        .get(idx)
        .copied()
        .unwrap_or(0);
    let precipitation_sum = body.daily.precipitation_sum.get(idx).and_then(|v| *v).unwrap_or(0.0);
    let precipitation_probability = body
        .daily
        .precipitation_probability_max
        .get(idx)
        .and_then(|v| *v)
        .unwrap_or(0.0);

    let weather_tone = classify_tone(code);
    let is_too_wet = matches!(weather_tone, WeatherTone::HeavyRain)
        || precipitation_sum >= 12.0
        || precipitation_probability >= 70.0
        || matches!(code, 65 | 82 | 95 | 96 | 99);

    Ok(TodayWeather { code, is_too_wet })
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
        WeatherTone::Sunny => "晴れです".to_string(),
        WeatherTone::Cloudy => "曇りです".to_string(),
        WeatherTone::Snow => "雪です".to_string(),
        WeatherTone::Rain => {
            if forecast.is_too_wet {
                "滝が降ります".to_string()
            } else {
                "雨です".to_string()
            }
        }
        WeatherTone::Other => "天候が分かりません".to_string(),
    };

    let mut message = format!("{location_name}: {body}");

    match tone {
        WeatherTone::Rain => {
            if forecast.is_too_wet {
                message = "@here 滝が降ります、傘を持っていきましょう\n出来ればリモートしましょう".to_string();
            } else {
                message = "@here 雨です、傘を持っていきましょう".to_string();
            }
        }
        _ => {}
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
