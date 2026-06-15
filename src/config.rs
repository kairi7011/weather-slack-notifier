use crate::error::{AppError, Result};
use chrono::{NaiveTime, Timelike};
use chrono_tz::Tz;
use clap::Parser;
use std::env;
use std::str::FromStr;

const DEFAULT_WEATHER_API_URL: &str = "https://api.open-meteo.com/v1/forecast";
const DEFAULT_WEATHER_TIMEZONE: &str = "Asia/Tokyo";
const DEFAULT_WORK_START_TIME: &str = "10:00";
const DEFAULT_WORK_END_TIME: &str = "19:00";
const DEFAULT_WORK_BUFFER_HOURS: u32 = 2;

#[derive(Parser, Debug)]
#[command(name = "weather-forecast-to-slack")]
pub struct CliArgs {
    #[arg(long)]
    pub lat: Option<String>,

    #[arg(long)]
    pub lon: Option<String>,

    #[arg(long)]
    pub name: Option<String>,

    #[arg(long = "api-url")]
    pub api_url: Option<String>,

    #[arg(long)]
    pub timezone: Option<String>,

    #[arg(long = "work-start-time")]
    pub work_start_time: Option<String>,

    #[arg(long = "work-end-time")]
    pub work_end_time: Option<String>,

    #[arg(long = "work-buffer-hours")]
    pub work_buffer_hours: Option<String>,

    #[arg(long)]
    pub skip_weekday_check: bool,

    #[arg(long)]
    pub skip_holiday_check: bool,
}

#[derive(Debug)]
pub struct Config {
    pub slack_bot_token: String,
    pub slack_channel_id: String,
    pub weather_url: String,
    pub weather_lat: String,
    pub weather_lon: String,
    pub weather_name: Option<String>,
    pub timezone: String,
    pub commute_window: TimeWindow,
    pub return_window: TimeWindow,
    pub weekday_only: bool,
    pub holiday_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeWindow {
    pub start: NaiveTime,
    pub end: NaiveTime,
}

impl TimeWindow {
    pub fn display(&self) -> String {
        format!(
            "{}-{}",
            self.start.format("%H:%M"),
            self.end.format("%H:%M")
        )
    }
}

impl Config {
    pub fn from_args(args: CliArgs) -> Result<Self> {
        let slack_bot_token = require_env("SLACK_BOT_TOKEN")?;
        let slack_channel_id = require_env("SLACK_CHANNEL_ID")?;

        let weather_lat = choose_value("WEATHER_LAT", args.lat)?;
        let weather_lon = choose_value("WEATHER_LON", args.lon)?;

        validate_coordinate(&weather_lat, "WEATHER_LAT", -90.0, 90.0)?;
        validate_coordinate(&weather_lon, "WEATHER_LON", -180.0, 180.0)?;

        let weather_name = sanitize_optional_label(choose_value("WEATHER_NAME", args.name).ok());

        let weather_url = args
            .api_url
            .or_else(|| env::var("WEATHER_API_URL").ok())
            .unwrap_or_else(|| DEFAULT_WEATHER_API_URL.to_string());
        validate_https_url(&weather_url, "WEATHER_API_URL")?;

        let timezone = args
            .timezone
            .or_else(|| env::var("WEATHER_TIMEZONE").ok())
            .unwrap_or_else(|| DEFAULT_WEATHER_TIMEZONE.to_string());
        validate_timezone(&timezone)?;

        let work_start_time = args
            .work_start_time
            .or_else(|| env::var("WEATHER_WORK_START_TIME").ok())
            .unwrap_or_else(|| DEFAULT_WORK_START_TIME.to_string());
        let work_end_time = args
            .work_end_time
            .or_else(|| env::var("WEATHER_WORK_END_TIME").ok())
            .unwrap_or_else(|| DEFAULT_WORK_END_TIME.to_string());
        let work_buffer_hours = args
            .work_buffer_hours
            .or_else(|| env::var("WEATHER_WORK_BUFFER_HOURS").ok())
            .map(|value| parse_work_buffer_hours(&value))
            .unwrap_or(Ok(DEFAULT_WORK_BUFFER_HOURS))?;
        let (commute_window, return_window) =
            derive_work_windows(&work_start_time, &work_end_time, work_buffer_hours)?;

        Ok(Self {
            slack_bot_token,
            slack_channel_id,
            weather_url,
            weather_lat,
            weather_lon,
            weather_name,
            timezone,
            commute_window,
            return_window,
            weekday_only: !args.skip_weekday_check,
            holiday_only: !args.skip_holiday_check,
        })
    }
}

pub(crate) fn require_env(name: &str) -> Result<String> {
    env::var(name).map_err(|_| AppError::new(format!("{name} is required (environment variable)")))
}

pub(crate) fn choose_value(name: &str, cli_value: Option<String>) -> Result<String> {
    cli_value
        .or_else(|| env::var(name).ok())
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| {
            AppError::new(format!(
                "{name} is required (env or --{})",
                name.to_lowercase()
            ))
        })
}

fn validate_coordinate(value: &str, name: &str, min: f64, max: f64) -> Result<()> {
    let parsed: f64 = value
        .trim()
        .parse()
        .map_err(|_| AppError::new(format!("{name} must be a decimal number: {value}")))?;

    if !(min..=max).contains(&parsed) {
        return Err(AppError::new(format!(
            "{name} must be in range [{min}, {max}]. got {value}"
        )));
    }

    Ok(())
}

fn validate_https_url(value: &str, name: &str) -> Result<()> {
    if value.starts_with("https://") {
        return Ok(());
    }

    Err(AppError::new(format!(
        "{name} must start with https:// to avoid unsafe transport"
    )))
}

fn validate_timezone(value: &str) -> Result<()> {
    Tz::from_str(value)
        .map(|_| ())
        .map_err(|_| AppError::new(format!("WEATHER_TIMEZONE is invalid: {value}")))
}

fn derive_work_windows(
    start_raw: &str,
    end_raw: &str,
    buffer_hours: u32,
) -> Result<(TimeWindow, TimeWindow)> {
    let work_start = parse_clock_time(start_raw, "WEATHER_WORK_START_TIME")?;
    let work_end = parse_clock_time(end_raw, "WEATHER_WORK_END_TIME")?;

    if work_start >= work_end {
        return Err(AppError::new(format!(
            "WEATHER_WORK_START_TIME must be before WEATHER_WORK_END_TIME: {} >= {}",
            work_start.format("%H:%M"),
            work_end.format("%H:%M")
        )));
    }

    let buffer_minutes = buffer_hours * 60;
    let work_start_minutes = work_start.num_seconds_from_midnight() / 60;
    let work_end_minutes = work_end.num_seconds_from_midnight() / 60;
    let day_end_minutes = 24 * 60;

    if work_start_minutes < buffer_minutes {
        return Err(AppError::new(format!(
            "WEATHER_WORK_BUFFER_HOURS is too large for WEATHER_WORK_START_TIME: {buffer_hours}"
        )));
    }
    if work_end_minutes + buffer_minutes >= day_end_minutes {
        return Err(AppError::new(format!(
            "WEATHER_WORK_BUFFER_HOURS is too large for WEATHER_WORK_END_TIME: {buffer_hours}"
        )));
    }

    Ok((
        TimeWindow {
            start: time_from_minutes(work_start_minutes - buffer_minutes),
            end: work_start,
        },
        TimeWindow {
            start: work_end,
            end: time_from_minutes(work_end_minutes + buffer_minutes),
        },
    ))
}

fn parse_work_buffer_hours(value: &str) -> Result<u32> {
    let parsed = value.trim().parse::<u32>().map_err(|_| {
        AppError::new(format!(
            "WEATHER_WORK_BUFFER_HOURS must be a positive integer: {value}"
        ))
    })?;

    if parsed == 0 || parsed > 6 {
        return Err(AppError::new(format!(
            "WEATHER_WORK_BUFFER_HOURS must be in range [1, 6]. got {value}"
        )));
    }

    Ok(parsed)
}

fn parse_clock_time(value: &str, name: &str) -> Result<NaiveTime> {
    let trimmed = value.trim();
    let parts = trimmed.split(':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err(AppError::new(format!("{name} must be HH:MM: {value}")));
    }

    let hour = parts[0]
        .parse::<u32>()
        .map_err(|_| AppError::new(format!("{name} hour is invalid: {value}")))?;
    let minute = parts[1]
        .parse::<u32>()
        .map_err(|_| AppError::new(format!("{name} minute is invalid: {value}")))?;

    NaiveTime::from_hms_opt(hour, minute, 0)
        .ok_or_else(|| AppError::new(format!("{name} must be a valid 24-hour time: {value}")))
}

fn time_from_minutes(minutes: u32) -> NaiveTime {
    NaiveTime::from_num_seconds_from_midnight_opt(minutes * 60, 0)
        .expect("minutes are already validated")
}

fn sanitize_optional_label(raw: Option<String>) -> Option<String> {
    let value = raw?.trim().to_string();
    if value.is_empty() {
        return None;
    }

    let filtered = value
        .chars()
        .filter(|ch| !matches!(ch, '\n' | '\r'))
        .collect::<String>();
    let trimmed = filtered.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.chars().take(64).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        derive_work_windows, parse_work_buffer_hours, sanitize_optional_label, validate_coordinate,
        validate_https_url,
    };
    use crate::error::AppError;

    #[test]
    fn coordinate_is_valid_in_range() {
        assert!(validate_coordinate("35.6895", "WEATHER_LAT", -90.0, 90.0).is_ok());
        assert!(validate_coordinate("-180", "WEATHER_LON", -180.0, 180.0).is_ok());
        assert!(validate_coordinate("181", "WEATHER_LON", -180.0, 180.0).is_err());
        assert!(validate_coordinate("abc", "WEATHER_LAT", -90.0, 90.0).is_err());
    }

    #[test]
    fn url_must_be_https() {
        assert!(validate_https_url("https://example.com", "WEATHER_API_URL").is_ok());
        assert!(validate_https_url("http://example.com", "WEATHER_API_URL").is_err());
    }

    #[test]
    fn work_windows_are_derived_from_regular_hours() {
        let (commute, return_window) = derive_work_windows("10:00", "19:00", 2).unwrap();

        assert_eq!(commute.display(), "08:00-10:00");
        assert_eq!(return_window.display(), "19:00-21:00");
    }

    #[test]
    fn work_window_buffer_is_validated() {
        assert_eq!(parse_work_buffer_hours("2").unwrap(), 2);
        assert!(parse_work_buffer_hours("0").is_err());
        assert!(parse_work_buffer_hours("7").is_err());
        assert!(derive_work_windows("01:00", "19:00", 2).is_err());
        assert!(derive_work_windows("10:00", "23:00", 2).is_err());
    }

    #[test]
    fn sanitize_label_removes_newlines_and_limits_length() {
        let long = "abcdefghij".repeat(10);
        let sanitized = sanitize_optional_label(Some(format!("\n {long} \r\n"))).unwrap();

        assert!(!sanitized.contains('\n'));
        assert_eq!(sanitized.len(), 64);
        assert_eq!(
            sanitized,
            "abcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcdefghijabcd"
        );
    }

    #[test]
    fn sanitize_label_empty_becomes_none() {
        assert!(sanitize_optional_label(Some("   \n".to_string())).is_none());
        assert!(sanitize_optional_label(None).is_none());
    }

    #[test]
    fn app_error_is_a_string_message() {
        let err = AppError::new("oops");
        assert_eq!(err.to_string(), "oops");
    }
}
