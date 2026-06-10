use crate::{
    config::Config,
    error::{AppError, Result},
};
use chrono::{Datelike, Utc};
use chrono_tz::Tz;
use reqwest::Client;
use serde::Deserialize;
use std::str::FromStr;

#[derive(Deserialize)]
pub struct WeatherResponse {
    pub daily: DailyForecast,
}

#[derive(Deserialize)]
pub struct DailyForecast {
    pub time: Vec<String>,
    #[serde(rename = "weathercode")]
    pub weather_code: Vec<u16>,
    #[serde(rename = "precipitation_sum")]
    pub precipitation_sum: Vec<Option<f64>>,
    #[serde(rename = "precipitation_probability_max")]
    pub precipitation_probability_max: Vec<Option<f64>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherTone {
    Sunny,
    Cloudy,
    Rain,
    Snow,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodayWeather {
    pub tone: WeatherTone,
    pub is_too_wet: bool,
}

pub fn build_weather_url(base: &str, lat: &str, lon: &str, tz: &str) -> String {
    format!(
        "{base}?latitude={lat}&longitude={lon}&daily=weathercode,precipitation_sum,precipitation_probability_max&timezone={tz}&forecast_days=2"
    )
}

pub fn is_weekday_in_timezone(timezone: &str) -> Result<bool> {
    let tz = Tz::from_str(timezone)
        .map_err(|_| AppError::new(format!("invalid timezone: {timezone}")))?;

    let now = Utc::now().with_timezone(&tz);
    let weekday = now.weekday().number_from_monday();
    Ok((1..=5).contains(&weekday))
}

pub fn determine_forecast_index(times: &[String], today: &str) -> Option<usize> {
    times.iter().position(|date| date == today).or_else(|| {
        if times.is_empty() {
            None
        } else {
            Some(0)
        }
    })
}

pub fn classify_tone(code: u16) -> WeatherTone {
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

pub fn parse_forecast_for_today(response: &WeatherResponse, today: &str) -> Result<TodayWeather> {
    let idx = determine_forecast_index(&response.daily.time, today).ok_or_else(|| {
        AppError::new("weather forecast does not have any daily data".to_string())
    })?;

    let code = response.daily.weather_code.get(idx).copied().unwrap_or(0);
    let precipitation_sum = response
        .daily
        .precipitation_sum
        .get(idx)
        .and_then(|value| *value)
        .unwrap_or(0.0);
    let precipitation_probability = response
        .daily
        .precipitation_probability_max
        .get(idx)
        .and_then(|value| *value)
        .unwrap_or(0.0);

    let tone = classify_tone(code);
    let is_too_wet = precipitation_sum >= 12.0
        || precipitation_probability >= 70.0
        || matches!(code, 61 | 63 | 65 | 66 | 67 | 80 | 81 | 82 | 95 | 96 | 99);

    Ok(TodayWeather { tone, is_too_wet })
}

pub async fn fetch_weather(client: &Client, config: &Config) -> Result<TodayWeather> {
    let tz = Tz::from_str(&config.timezone)
        .map_err(|_| AppError::new(format!("invalid timezone: {}", config.timezone)))?;

    let today = Utc::now().with_timezone(&tz).date_naive().to_string();
    let url = build_weather_url(
        &config.weather_url,
        &config.weather_lat,
        &config.weather_lon,
        &config.timezone,
    );

    let response = client.get(url).send().await?.error_for_status()?;
    let body: WeatherResponse = response.json().await?;

    parse_forecast_for_today(&body, &today)
}

#[cfg(test)]
mod tests {
    use super::{
        build_weather_url, classify_tone, determine_forecast_index, parse_forecast_for_today,
    };
    use super::{DailyForecast, WeatherResponse, WeatherTone};

    #[test]
    fn prefer_today_when_present() {
        let times = vec!["2026-06-08".to_string(), "2026-06-09".to_string()];
        assert_eq!(determine_forecast_index(&times, "2026-06-09"), Some(1));
    }

    #[test]
    fn fallback_to_first_when_today_not_found() {
        let times = vec!["2026-06-08".to_string()];
        assert_eq!(determine_forecast_index(&times, "2026-06-10"), Some(0));
    }

    #[test]
    fn classify_tone_mapping() {
        assert_eq!(classify_tone(0), WeatherTone::Sunny);
        assert_eq!(classify_tone(3), WeatherTone::Cloudy);
        assert_eq!(classify_tone(61), WeatherTone::Rain);
        assert_eq!(classify_tone(75), WeatherTone::Snow);
        assert_eq!(classify_tone(999), WeatherTone::Other);
    }

    #[test]
    fn can_parse_today_forecast_and_detect_heavy_rain() {
        let response = WeatherResponse {
            daily: DailyForecast {
                time: vec!["2026-06-10".to_string(), "2026-06-11".to_string()],
                weather_code: vec![61, 1],
                precipitation_sum: vec![Some(0.3), Some(0.0)],
                precipitation_probability_max: vec![Some(90.0), Some(10.0)],
            },
        };

        let today = parse_forecast_for_today(&response, "2026-06-10").unwrap();
        assert_eq!(today.tone, WeatherTone::Rain);
        assert!(today.is_too_wet);
    }

    #[test]
    fn weather_url_is_constructed_with_params() {
        let url = build_weather_url(
            "https://api.example.com/forecast",
            "35.0",
            "139.0",
            "Asia/Tokyo",
        );
        assert!(url.starts_with("https://api.example.com/forecast?latitude=35.0"));
        assert!(url.contains("timezone=Asia/Tokyo"));
    }
}
