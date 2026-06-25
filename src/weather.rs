use crate::{
    config::{Config, TimeWindow},
    error::{AppError, Result},
};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Utc};
use chrono_tz::Tz;
use jp_holidays_lib::client::Client as HolidayClient;
use reqwest::Client;
use serde::Deserialize;
use std::str::FromStr;

#[derive(Deserialize)]
pub struct WeatherResponse {
    pub daily: DailyForecast,
    pub hourly: Option<HourlyForecast>,
}

#[derive(Deserialize)]
pub struct DailyForecast {
    pub time: Vec<String>,
    #[serde(rename = "weather_code", alias = "weathercode")]
    pub weather_code: Vec<u16>,
    #[serde(rename = "precipitation_sum")]
    pub precipitation_sum: Vec<Option<f64>>,
    #[serde(rename = "precipitation_probability_max")]
    pub precipitation_probability_max: Vec<Option<f64>>,
}

#[derive(Deserialize)]
pub struct HourlyForecast {
    pub time: Vec<String>,
    #[serde(rename = "weather_code", alias = "weathercode")]
    pub weather_code: Vec<u16>,
    pub precipitation: Vec<Option<f64>>,
    pub precipitation_probability: Vec<Option<f64>>,
    #[serde(default)]
    pub wind_gusts_10m: Vec<Option<f64>>,
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
    pub date_display: String,
    pub tone: WeatherTone,
    pub is_too_wet: bool,
    pub has_heavy_rain: bool,
    pub rain_periods: Vec<RainPeriod>,
    pub wind_periods: Vec<WindPeriod>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RainPeriod {
    pub start_display: String,
    pub end_display: String,
    pub is_too_wet: bool,
    pub impact_periods: Vec<RainImpactPeriod>,
    pub heavy_periods: Vec<TimePeriod>,
    pub thunderstorm_periods: Vec<TimePeriod>,
}

impl RainPeriod {
    pub fn time_display(&self) -> String {
        format!("{}-{}", self.start_display, self.end_display)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RainImpactPeriod {
    pub start_display: String,
    pub end_display: String,
    pub impact: RainImpact,
}

impl RainImpactPeriod {
    pub fn time_display(&self) -> String {
        format!("{}-{}", self.start_display, self.end_display)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindPeriod {
    pub start_display: String,
    pub end_display: String,
    pub max_gust_kmh: u32,
    pub storm_periods: Vec<TimePeriod>,
}

impl WindPeriod {
    pub fn time_display(&self) -> String {
        format!("{}-{}", self.start_display, self.end_display)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimePeriod {
    pub start_display: String,
    pub end_display: String,
}

impl TimePeriod {
    pub fn time_display(&self) -> String {
        format!("{}-{}", self.start_display, self.end_display)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RainImpact {
    EarlyCommute,
    Commute,
    Lunch,
    Return,
    LateReturn,
    Overtime,
    LowImpact,
}

pub fn build_weather_url(base: &str, lat: &str, lon: &str, tz: &str) -> String {
    format!(
        "{base}?latitude={lat}&longitude={lon}&daily=weather_code,precipitation_sum,precipitation_probability_max&hourly=weather_code,precipitation,precipitation_probability,wind_gusts_10m&timezone={tz}&forecast_days=2"
    )
}

pub fn is_weekday(date: &NaiveDate) -> bool {
    let weekday = date.weekday().number_from_monday();
    (1..=5).contains(&weekday)
}

pub fn is_weekday_in_timezone(timezone: &str) -> Result<bool> {
    let tz = Tz::from_str(timezone)
        .map_err(|_| AppError::new(format!("invalid timezone: {timezone}")))?;

    let now = Utc::now().with_timezone(&tz).date_naive();
    Ok(is_weekday(&now))
}

pub async fn is_holiday_in_timezone(timezone: &str) -> Result<bool> {
    let tz = Tz::from_str(timezone)
        .map_err(|_| AppError::new(format!("invalid timezone: {timezone}")))?;

    let today = Utc::now().with_timezone(&tz).date_naive();
    is_holiday_on_date(today).await
}

pub async fn is_holiday_on_date(date: NaiveDate) -> Result<bool> {
    let holiday_client = HolidayClient::init()
        .await
        .map_err(|err| AppError::new(err.to_string()))?;

    Ok(holiday_client.is_holiday(date))
}

pub fn determine_forecast_index(times: &[String], today: &str) -> Option<usize> {
    times.iter().position(|date| date == today).or({
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

fn is_significant_rain_code(code: u16) -> bool {
    matches!(code, 63 | 65 | 66 | 67 | 81 | 82)
}

fn is_thunderstorm_code(code: u16) -> bool {
    matches!(code, 95 | 96 | 99)
}

fn is_strong_wind_hour(wind_gust_kmh: f64) -> bool {
    wind_gust_kmh >= 54.0
}

fn is_storm_wind_hour(wind_gust_kmh: f64) -> bool {
    wind_gust_kmh >= 72.0
}

fn round_gust_kmh(wind_gust_kmh: f64) -> u32 {
    wind_gust_kmh.round().max(0.0) as u32
}

fn is_rain_hour(code: u16, precipitation: f64, precipitation_probability: f64) -> bool {
    let tone = classify_tone(code);
    tone == WeatherTone::Rain
        || (tone != WeatherTone::Snow && (precipitation > 0.0 || precipitation_probability >= 70.0))
}

fn parse_open_meteo_hour(value: &str) -> Result<NaiveDateTime> {
    NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M:%S"))
        .map_err(|_| AppError::new(format!("weather hourly time is invalid: {value}")))
}

fn day_bounds(today: &str) -> Result<(NaiveDateTime, NaiveDateTime)> {
    let date = NaiveDate::parse_from_str(today, "%Y-%m-%d")
        .map_err(|_| AppError::new(format!("today is invalid: {today}")))?;

    let start = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| AppError::new(format!("today is invalid: {today}")))?;
    Ok((start, start + Duration::days(1)))
}

fn first_hour_end(window: &TimeWindow) -> NaiveTime {
    let start_minutes = window.start.num_seconds_from_midnight() / 60;
    let end_minutes = window.end.num_seconds_from_midnight() / 60;
    let minutes = (start_minutes + 60).min(end_minutes);

    NaiveTime::from_num_seconds_from_midnight_opt(minutes * 60, 0)
        .expect("minutes from valid time window")
}

fn time_in_window(time: NaiveTime, window: &TimeWindow) -> bool {
    time >= window.start && time < window.end
}

fn clock_time(hour: u32, minute: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(hour, minute, 0).expect("valid clock time")
}

fn classify_rain_impact(
    slot_start: NaiveDateTime,
    commute_window: &TimeWindow,
    return_window: &TimeWindow,
) -> RainImpact {
    let time = slot_start.time();
    let early_commute_end = first_hour_end(commute_window);
    let return_first_hour_end = first_hour_end(return_window);

    if time >= commute_window.start && time < early_commute_end {
        RainImpact::EarlyCommute
    } else if time_in_window(time, commute_window) {
        RainImpact::Commute
    } else if time >= clock_time(12, 0) && time < clock_time(14, 0) {
        RainImpact::Lunch
    } else if time >= return_window.start && time < return_first_hour_end {
        RainImpact::Return
    } else if time_in_window(time, return_window) {
        RainImpact::LateReturn
    } else if time >= return_window.end {
        RainImpact::Overtime
    } else {
        RainImpact::LowImpact
    }
}

fn format_period_boundary(value: NaiveDateTime, day_end: NaiveDateTime) -> String {
    if value == day_end {
        "24:00".to_string()
    } else {
        value.time().format("%H:%M").to_string()
    }
}

fn classify_hourly_tone(codes: &[u16], has_rain: bool) -> WeatherTone {
    if has_rain {
        WeatherTone::Rain
    } else if codes
        .iter()
        .any(|code| classify_tone(*code) == WeatherTone::Snow)
    {
        WeatherTone::Snow
    } else if codes
        .iter()
        .any(|code| classify_tone(*code) == WeatherTone::Cloudy)
    {
        WeatherTone::Cloudy
    } else if codes
        .iter()
        .any(|code| classify_tone(*code) == WeatherTone::Sunny)
    {
        WeatherTone::Sunny
    } else {
        WeatherTone::Other
    }
}

struct RainHourSlot {
    start: NaiveDateTime,
    end: NaiveDateTime,
    impact: RainImpact,
    is_too_wet: bool,
    is_heavy: bool,
    is_thunderstorm: bool,
}

struct RainPeriodBuilder {
    start: NaiveDateTime,
    end: NaiveDateTime,
    is_too_wet: bool,
    impact_periods: Vec<RainImpactPeriodBuilder>,
    heavy_periods: Vec<TimePeriodBuilder>,
    thunderstorm_periods: Vec<TimePeriodBuilder>,
}

struct RainImpactPeriodBuilder {
    start: NaiveDateTime,
    end: NaiveDateTime,
    impact: RainImpact,
}

impl RainImpactPeriodBuilder {
    fn into_period(self, day_end: NaiveDateTime) -> RainImpactPeriod {
        RainImpactPeriod {
            start_display: format_period_boundary(self.start, day_end),
            end_display: format_period_boundary(self.end, day_end),
            impact: self.impact,
        }
    }
}

struct TimePeriodBuilder {
    start: NaiveDateTime,
    end: NaiveDateTime,
}

impl TimePeriodBuilder {
    fn into_period(self, day_end: NaiveDateTime) -> TimePeriod {
        TimePeriod {
            start_display: format_period_boundary(self.start, day_end),
            end_display: format_period_boundary(self.end, day_end),
        }
    }
}

fn push_time_period(
    periods: &mut Vec<TimePeriodBuilder>,
    start: NaiveDateTime,
    end: NaiveDateTime,
) {
    if let Some(period) = periods.last_mut() {
        if period.end == start {
            period.end = end;
            return;
        }
    }

    periods.push(TimePeriodBuilder { start, end });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RainImpactGroup {
    Commute,
    Lunch,
    Return,
    Overtime,
}

fn rain_impact_group(impact: RainImpact) -> Option<RainImpactGroup> {
    match impact {
        RainImpact::EarlyCommute | RainImpact::Commute => Some(RainImpactGroup::Commute),
        RainImpact::Lunch => Some(RainImpactGroup::Lunch),
        RainImpact::Return | RainImpact::LateReturn => Some(RainImpactGroup::Return),
        RainImpact::Overtime => Some(RainImpactGroup::Overtime),
        RainImpact::LowImpact => None,
    }
}

fn push_impact_period(
    periods: &mut Vec<RainImpactPeriodBuilder>,
    start: NaiveDateTime,
    end: NaiveDateTime,
    impact: RainImpact,
) {
    let Some(group) = rain_impact_group(impact) else {
        return;
    };

    if let Some(period) = periods.last_mut() {
        if period.end == start && rain_impact_group(period.impact) == Some(group) {
            period.end = end;
            return;
        }
    }

    periods.push(RainImpactPeriodBuilder { start, end, impact });
}

impl RainPeriodBuilder {
    fn into_period(self, day_end: NaiveDateTime) -> RainPeriod {
        RainPeriod {
            start_display: format_period_boundary(self.start, day_end),
            end_display: format_period_boundary(self.end, day_end),
            is_too_wet: self.is_too_wet,
            impact_periods: self
                .impact_periods
                .into_iter()
                .map(|period| period.into_period(day_end))
                .collect(),
            heavy_periods: self
                .heavy_periods
                .into_iter()
                .map(|period| period.into_period(day_end))
                .collect(),
            thunderstorm_periods: self
                .thunderstorm_periods
                .into_iter()
                .map(|period| period.into_period(day_end))
                .collect(),
        }
    }
}

fn rain_periods_from_slots(slots: Vec<RainHourSlot>, day_end: NaiveDateTime) -> Vec<RainPeriod> {
    let mut periods = Vec::new();
    let mut current: Option<RainPeriodBuilder> = None;

    for slot in slots {
        match current.as_mut() {
            Some(period) if period.end == slot.start => {
                period.end = slot.end;
                period.is_too_wet |= slot.is_too_wet;
                push_impact_period(
                    &mut period.impact_periods,
                    slot.start,
                    slot.end,
                    slot.impact,
                );
                if slot.is_heavy {
                    push_time_period(&mut period.heavy_periods, slot.start, slot.end);
                }
                if slot.is_thunderstorm {
                    push_time_period(&mut period.thunderstorm_periods, slot.start, slot.end);
                }
            }
            Some(_) => {
                let finished = current.take().expect("period exists");
                let mut impact_periods = Vec::new();
                push_impact_period(&mut impact_periods, slot.start, slot.end, slot.impact);
                let mut heavy_periods = Vec::new();
                if slot.is_heavy {
                    push_time_period(&mut heavy_periods, slot.start, slot.end);
                }
                let mut thunderstorm_periods = Vec::new();
                if slot.is_thunderstorm {
                    push_time_period(&mut thunderstorm_periods, slot.start, slot.end);
                }
                periods.push(finished.into_period(day_end));
                current = Some(RainPeriodBuilder {
                    start: slot.start,
                    end: slot.end,
                    is_too_wet: slot.is_too_wet,
                    impact_periods,
                    heavy_periods,
                    thunderstorm_periods,
                });
            }
            None => {
                let mut impact_periods = Vec::new();
                push_impact_period(&mut impact_periods, slot.start, slot.end, slot.impact);
                let mut heavy_periods = Vec::new();
                if slot.is_heavy {
                    push_time_period(&mut heavy_periods, slot.start, slot.end);
                }
                let mut thunderstorm_periods = Vec::new();
                if slot.is_thunderstorm {
                    push_time_period(&mut thunderstorm_periods, slot.start, slot.end);
                }
                current = Some(RainPeriodBuilder {
                    start: slot.start,
                    end: slot.end,
                    is_too_wet: slot.is_too_wet,
                    impact_periods,
                    heavy_periods,
                    thunderstorm_periods,
                });
            }
        }
    }

    if let Some(period) = current {
        periods.push(period.into_period(day_end));
    }

    periods
}

struct WindHourSlot {
    start: NaiveDateTime,
    end: NaiveDateTime,
    max_gust_kmh: u32,
    is_storm: bool,
}

struct WindPeriodBuilder {
    start: NaiveDateTime,
    end: NaiveDateTime,
    max_gust_kmh: u32,
    storm_periods: Vec<TimePeriodBuilder>,
}

impl WindPeriodBuilder {
    fn into_period(self, day_end: NaiveDateTime) -> WindPeriod {
        WindPeriod {
            start_display: format_period_boundary(self.start, day_end),
            end_display: format_period_boundary(self.end, day_end),
            max_gust_kmh: self.max_gust_kmh,
            storm_periods: self
                .storm_periods
                .into_iter()
                .map(|period| period.into_period(day_end))
                .collect(),
        }
    }
}

fn wind_periods_from_slots(slots: Vec<WindHourSlot>, day_end: NaiveDateTime) -> Vec<WindPeriod> {
    let mut periods = Vec::new();
    let mut current: Option<WindPeriodBuilder> = None;

    for slot in slots {
        match current.as_mut() {
            Some(period) if period.end == slot.start => {
                period.end = slot.end;
                period.max_gust_kmh = period.max_gust_kmh.max(slot.max_gust_kmh);
                if slot.is_storm {
                    push_time_period(&mut period.storm_periods, slot.start, slot.end);
                }
            }
            Some(_) => {
                let finished = current.take().expect("period exists");
                let mut storm_periods = Vec::new();
                if slot.is_storm {
                    push_time_period(&mut storm_periods, slot.start, slot.end);
                }
                periods.push(finished.into_period(day_end));
                current = Some(WindPeriodBuilder {
                    start: slot.start,
                    end: slot.end,
                    max_gust_kmh: slot.max_gust_kmh,
                    storm_periods,
                });
            }
            None => {
                let mut storm_periods = Vec::new();
                if slot.is_storm {
                    push_time_period(&mut storm_periods, slot.start, slot.end);
                }
                current = Some(WindPeriodBuilder {
                    start: slot.start,
                    end: slot.end,
                    max_gust_kmh: slot.max_gust_kmh,
                    storm_periods,
                });
            }
        }
    }

    if let Some(period) = current {
        periods.push(period.into_period(day_end));
    }

    periods
}

fn parse_hourly_forecast_for_today(
    hourly: &HourlyForecast,
    today: &str,
    date_display: &str,
    commute_window: &TimeWindow,
    return_window: &TimeWindow,
) -> Result<TodayWeather> {
    let (day_start, day_end) = day_bounds(today)?;
    let mut codes = Vec::new();
    let mut slots = Vec::new();
    let mut wind_slots = Vec::new();

    for (idx, raw_time) in hourly.time.iter().enumerate() {
        let slot_end = parse_open_meteo_hour(raw_time)?;
        let slot_start = slot_end - Duration::hours(1);
        if slot_start < day_start || slot_end > day_end {
            continue;
        }

        let code = hourly.weather_code.get(idx).copied().unwrap_or(0);
        let precipitation = hourly
            .precipitation
            .get(idx)
            .and_then(|value| *value)
            .unwrap_or(0.0);
        let precipitation_probability = hourly
            .precipitation_probability
            .get(idx)
            .and_then(|value| *value)
            .unwrap_or(0.0);
        let wind_gust_kmh = hourly
            .wind_gusts_10m
            .get(idx)
            .and_then(|value| *value)
            .unwrap_or(0.0);
        codes.push(code);

        if is_rain_hour(code, precipitation, precipitation_probability) {
            let is_heavy = precipitation >= 12.0 || is_significant_rain_code(code);
            let is_thunderstorm = is_thunderstorm_code(code);
            slots.push(RainHourSlot {
                start: slot_start,
                end: slot_end,
                impact: classify_rain_impact(slot_start, commute_window, return_window),
                is_too_wet: is_heavy || precipitation_probability >= 70.0 || is_thunderstorm,
                is_heavy,
                is_thunderstorm,
            });
        }

        if is_strong_wind_hour(wind_gust_kmh) {
            wind_slots.push(WindHourSlot {
                start: slot_start,
                end: slot_end,
                max_gust_kmh: round_gust_kmh(wind_gust_kmh),
                is_storm: is_storm_wind_hour(wind_gust_kmh),
            });
        }
    }

    if codes.is_empty() {
        return Err(AppError::new(
            "weather forecast does not have hourly data for today".to_string(),
        ));
    }

    let rain_periods = rain_periods_from_slots(slots, day_end);
    let wind_periods = wind_periods_from_slots(wind_slots, day_end);
    let tone = classify_hourly_tone(&codes, !rain_periods.is_empty());
    let has_heavy_rain = rain_periods
        .iter()
        .any(|period| !period.heavy_periods.is_empty());
    let is_too_wet = rain_periods.iter().any(|period| period.is_too_wet)
        || wind_periods
            .iter()
            .any(|period| !period.storm_periods.is_empty());

    Ok(TodayWeather {
        date_display: date_display.to_string(),
        tone,
        is_too_wet,
        has_heavy_rain,
        rain_periods,
        wind_periods,
    })
}

fn parse_daily_forecast_for_today(
    response: &WeatherResponse,
    today: &str,
    date_display: &str,
) -> Result<TodayWeather> {
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
    let has_heavy_rain = precipitation_sum >= 12.0 || is_significant_rain_code(code);
    let is_too_wet =
        has_heavy_rain || precipitation_probability >= 70.0 || is_thunderstorm_code(code);

    Ok(TodayWeather {
        date_display: date_display.to_string(),
        tone,
        is_too_wet,
        has_heavy_rain,
        rain_periods: Vec::new(),
        wind_periods: Vec::new(),
    })
}

pub fn parse_forecast_for_today(
    response: &WeatherResponse,
    today: &str,
    date_display: &str,
    commute_window: &TimeWindow,
    return_window: &TimeWindow,
) -> Result<TodayWeather> {
    if let Some(hourly) = &response.hourly {
        return parse_hourly_forecast_for_today(
            hourly,
            today,
            date_display,
            commute_window,
            return_window,
        );
    }

    parse_daily_forecast_for_today(response, today, date_display)
}

pub async fn fetch_weather(client: &Client, config: &Config) -> Result<TodayWeather> {
    let tz = Tz::from_str(&config.timezone)
        .map_err(|_| AppError::new(format!("invalid timezone: {}", config.timezone)))?;

    let today = Utc::now().with_timezone(&tz).date_naive();
    let today_for_forecast = today.to_string();
    let date_display = today.format("%-m/%-d").to_string();
    let url = build_weather_url(
        &config.weather_url,
        &config.weather_lat,
        &config.weather_lon,
        &config.timezone,
    );

    let response = client.get(url).send().await?.error_for_status()?;
    let body: WeatherResponse = response.json().await?;

    parse_forecast_for_today(
        &body,
        &today_for_forecast,
        &date_display,
        &config.commute_window,
        &config.return_window,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        build_weather_url, classify_tone, determine_forecast_index, is_weekday,
        parse_forecast_for_today, HourlyForecast, RainImpact,
    };
    use super::{DailyForecast, WeatherResponse, WeatherTone};
    use crate::config::TimeWindow;
    use chrono::{NaiveDate, NaiveTime};

    fn time_window(start_hour: u32, end_hour: u32) -> TimeWindow {
        TimeWindow {
            start: NaiveTime::from_hms_opt(start_hour, 0, 0).unwrap(),
            end: NaiveTime::from_hms_opt(end_hour, 0, 0).unwrap(),
        }
    }

    fn commute_window() -> TimeWindow {
        time_window(8, 10)
    }

    fn return_window() -> TimeWindow {
        time_window(19, 21)
    }

    fn parse(response: &WeatherResponse) -> super::Result<super::TodayWeather> {
        parse_forecast_for_today(
            response,
            "2026-06-10",
            "6/10",
            &commute_window(),
            &return_window(),
        )
    }

    #[test]
    fn weekday_date_classifier_distinguishes_weekend() {
        let wed = NaiveDate::from_ymd_opt(2026, 6, 10).expect("valid date");
        let sun = NaiveDate::from_ymd_opt(2026, 6, 14).expect("valid date");

        assert!(is_weekday(&wed));
        assert!(!is_weekday(&sun));
    }

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
            hourly: None,
        };

        let today = parse(&response).unwrap();
        assert_eq!(today.tone, WeatherTone::Rain);
        assert!(today.is_too_wet);
        assert!(!today.has_heavy_rain);
        assert!(today.rain_periods.is_empty());
    }

    #[test]
    fn daily_forecast_marks_precipitation_amount_as_heavy_rain() {
        let response = WeatherResponse {
            daily: DailyForecast {
                time: vec!["2026-06-10".to_string()],
                weather_code: vec![61],
                precipitation_sum: vec![Some(12.0)],
                precipitation_probability_max: vec![Some(60.0)],
            },
            hourly: None,
        };

        let today = parse(&response).unwrap();

        assert!(today.is_too_wet);
        assert!(today.has_heavy_rain);
    }

    #[test]
    fn hourly_forecast_reports_low_impact_rain_outside_work_windows() {
        let response = WeatherResponse {
            daily: DailyForecast {
                time: vec!["2026-06-10".to_string()],
                weather_code: vec![61],
                precipitation_sum: vec![Some(20.0)],
                precipitation_probability_max: vec![Some(90.0)],
            },
            hourly: Some(HourlyForecast {
                time: vec![
                    "2026-06-10T09:00".to_string(),
                    "2026-06-10T17:00".to_string(),
                ],
                weather_code: vec![1, 61],
                precipitation: vec![Some(0.0), Some(1.0)],
                precipitation_probability: vec![Some(0.0), Some(50.0)],
                wind_gusts_10m: vec![Some(10.0), Some(10.0)],
            }),
        };

        let today = parse(&response).unwrap();

        assert_eq!(today.tone, WeatherTone::Rain);
        assert_eq!(today.rain_periods.len(), 1);
        assert_eq!(today.rain_periods[0].time_display(), "16:00-17:00");
        assert!(today.rain_periods[0].impact_periods.is_empty());
    }

    #[test]
    fn hourly_forecast_reports_impact_windows() {
        let response = WeatherResponse {
            daily: DailyForecast {
                time: vec!["2026-06-10".to_string()],
                weather_code: vec![1],
                precipitation_sum: vec![Some(0.0)],
                precipitation_probability_max: vec![Some(0.0)],
            },
            hourly: Some(HourlyForecast {
                time: vec![
                    "2026-06-10T09:00".to_string(),
                    "2026-06-10T10:00".to_string(),
                    "2026-06-10T14:00".to_string(),
                    "2026-06-10T20:00".to_string(),
                    "2026-06-10T21:00".to_string(),
                    "2026-06-10T22:00".to_string(),
                    "2026-06-10T23:00".to_string(),
                    "2026-06-11T00:00".to_string(),
                ],
                weather_code: vec![61, 61, 61, 61, 61, 61, 61, 61],
                precipitation: vec![
                    Some(1.0),
                    Some(1.0),
                    Some(1.0),
                    Some(1.0),
                    Some(1.0),
                    Some(1.0),
                    Some(1.0),
                    Some(1.0),
                ],
                precipitation_probability: vec![
                    Some(50.0),
                    Some(50.0),
                    Some(50.0),
                    Some(50.0),
                    Some(50.0),
                    Some(50.0),
                    Some(50.0),
                    Some(50.0),
                ],
                wind_gusts_10m: vec![Some(10.0); 8],
            }),
        };

        let today = parse(&response).unwrap();

        assert_eq!(today.tone, WeatherTone::Rain);
        let actual = today
            .rain_periods
            .iter()
            .map(|period| period.time_display())
            .collect::<Vec<_>>();
        assert_eq!(
            actual,
            vec![
                "08:00-10:00".to_string(),
                "13:00-14:00".to_string(),
                "19:00-24:00".to_string(),
            ]
        );
        let impact_periods = today
            .rain_periods
            .iter()
            .flat_map(|period| period.impact_periods.iter())
            .map(|period| (period.time_display(), period.impact))
            .collect::<Vec<_>>();
        assert_eq!(
            impact_periods,
            vec![
                ("08:00-10:00".to_string(), RainImpact::EarlyCommute),
                ("13:00-14:00".to_string(), RainImpact::Lunch),
                ("19:00-21:00".to_string(), RainImpact::Return),
                ("21:00-24:00".to_string(), RainImpact::Overtime),
            ]
        );
    }

    #[test]
    fn hourly_forecast_keeps_contiguous_all_day_rain_as_one_period() {
        let response = WeatherResponse {
            daily: DailyForecast {
                time: vec!["2026-06-10".to_string()],
                weather_code: vec![61],
                precipitation_sum: vec![Some(10.0)],
                precipitation_probability_max: vec![Some(60.0)],
            },
            hourly: Some(HourlyForecast {
                time: (2..=24)
                    .map(|hour| {
                        if hour == 24 {
                            "2026-06-11T00:00".to_string()
                        } else {
                            format!("2026-06-10T{hour:02}:00")
                        }
                    })
                    .collect(),
                weather_code: vec![61; 23],
                precipitation: vec![Some(1.0); 23],
                precipitation_probability: vec![Some(50.0); 23],
                wind_gusts_10m: vec![Some(10.0); 23],
            }),
        };

        let today = parse(&response).unwrap();

        assert_eq!(today.rain_periods.len(), 1);
        assert_eq!(today.rain_periods[0].time_display(), "01:00-24:00");
        let impact_periods = today.rain_periods[0]
            .impact_periods
            .iter()
            .map(|period| (period.time_display(), period.impact))
            .collect::<Vec<_>>();
        assert_eq!(
            impact_periods,
            vec![
                ("08:00-10:00".to_string(), RainImpact::EarlyCommute),
                ("12:00-14:00".to_string(), RainImpact::Lunch),
                ("19:00-21:00".to_string(), RainImpact::Return),
                ("21:00-24:00".to_string(), RainImpact::Overtime),
            ]
        );
    }

    #[test]
    fn hourly_forecast_detects_early_commute_from_preceding_hour_precipitation() {
        let response = WeatherResponse {
            daily: DailyForecast {
                time: vec!["2026-06-10".to_string()],
                weather_code: vec![1],
                precipitation_sum: vec![Some(0.0)],
                precipitation_probability_max: vec![Some(0.0)],
            },
            hourly: Some(HourlyForecast {
                time: vec!["2026-06-10T09:00".to_string()],
                weather_code: vec![61],
                precipitation: vec![Some(1.0)],
                precipitation_probability: vec![Some(60.0)],
                wind_gusts_10m: vec![Some(10.0)],
            }),
        };
        let today = parse(&response).unwrap();

        assert_eq!(today.tone, WeatherTone::Rain);
        assert_eq!(today.rain_periods[0].time_display(), "08:00-09:00");
        assert_eq!(
            today.rain_periods[0].impact_periods[0].impact,
            RainImpact::EarlyCommute
        );
    }

    #[test]
    fn hourly_forecast_does_not_treat_before_commute_as_commute_rain() {
        let response = WeatherResponse {
            daily: DailyForecast {
                time: vec!["2026-06-10".to_string()],
                weather_code: vec![1],
                precipitation_sum: vec![Some(0.0)],
                precipitation_probability_max: vec![Some(0.0)],
            },
            hourly: Some(HourlyForecast {
                time: vec![
                    "2026-06-10T08:00".to_string(),
                    "2026-06-10T09:00".to_string(),
                ],
                weather_code: vec![61, 1],
                precipitation: vec![Some(3.0), Some(0.0)],
                precipitation_probability: vec![Some(90.0), Some(0.0)],
                wind_gusts_10m: vec![Some(10.0), Some(10.0)],
            }),
        };

        let today = parse(&response).unwrap();

        assert_eq!(today.rain_periods[0].time_display(), "07:00-08:00");
        assert!(today.rain_periods[0].is_too_wet);
        assert!(today.rain_periods[0].heavy_periods.is_empty());
        assert!(!today.has_heavy_rain);
        assert!(today.rain_periods[0].impact_periods.is_empty());
    }

    #[test]
    fn hourly_forecast_tracks_heavy_rain_periods_from_precipitation_amount() {
        let response = WeatherResponse {
            daily: DailyForecast {
                time: vec!["2026-06-10".to_string()],
                weather_code: vec![1],
                precipitation_sum: vec![Some(0.0)],
                precipitation_probability_max: vec![Some(0.0)],
            },
            hourly: Some(HourlyForecast {
                time: vec![
                    "2026-06-10T09:00".to_string(),
                    "2026-06-10T10:00".to_string(),
                ],
                weather_code: vec![61, 61],
                precipitation: vec![Some(12.0), Some(1.0)],
                precipitation_probability: vec![Some(60.0), Some(60.0)],
                wind_gusts_10m: vec![Some(10.0), Some(10.0)],
            }),
        };

        let today = parse(&response).unwrap();

        assert_eq!(today.rain_periods[0].time_display(), "08:00-10:00");
        assert!(today.has_heavy_rain);
        assert_eq!(
            today.rain_periods[0].heavy_periods[0].time_display(),
            "08:00-09:00"
        );
    }

    #[test]
    fn hourly_forecast_nests_thunderstorm_inside_rain_period() {
        let response = WeatherResponse {
            daily: DailyForecast {
                time: vec!["2026-06-10".to_string()],
                weather_code: vec![1],
                precipitation_sum: vec![Some(0.0)],
                precipitation_probability_max: vec![Some(0.0)],
            },
            hourly: Some(HourlyForecast {
                time: vec![
                    "2026-06-10T09:00".to_string(),
                    "2026-06-10T10:00".to_string(),
                ],
                weather_code: vec![95, 61],
                precipitation: vec![Some(1.0), Some(1.0)],
                precipitation_probability: vec![Some(60.0), Some(60.0)],
                wind_gusts_10m: vec![Some(10.0), Some(10.0)],
            }),
        };

        let today = parse(&response).unwrap();

        assert!(today.is_too_wet);
        assert_eq!(today.rain_periods[0].time_display(), "08:00-10:00");
        assert_eq!(
            today.rain_periods[0].thunderstorm_periods[0].time_display(),
            "08:00-09:00"
        );
    }

    #[test]
    fn hourly_forecast_groups_wind_periods_and_nests_storm_wind() {
        let response = WeatherResponse {
            daily: DailyForecast {
                time: vec!["2026-06-10".to_string()],
                weather_code: vec![1],
                precipitation_sum: vec![Some(0.0)],
                precipitation_probability_max: vec![Some(0.0)],
            },
            hourly: Some(HourlyForecast {
                time: vec![
                    "2026-06-10T09:00".to_string(),
                    "2026-06-10T10:00".to_string(),
                    "2026-06-10T11:00".to_string(),
                    "2026-06-10T12:00".to_string(),
                ],
                weather_code: vec![1, 1, 1, 1],
                precipitation: vec![Some(0.0), Some(0.0), Some(0.0), Some(0.0)],
                precipitation_probability: vec![Some(0.0), Some(0.0), Some(0.0), Some(0.0)],
                wind_gusts_10m: vec![Some(55.0), Some(74.0), Some(76.0), Some(40.0)],
            }),
        };

        let today = parse(&response).unwrap();

        assert!(today.rain_periods.is_empty());
        assert!(today.is_too_wet);
        assert_eq!(today.wind_periods[0].time_display(), "08:00-11:00");
        assert_eq!(today.wind_periods[0].max_gust_kmh, 76);
        assert_eq!(
            today.wind_periods[0].storm_periods[0].time_display(),
            "09:00-11:00"
        );
    }

    #[test]
    fn hourly_forecast_is_sunny_when_no_rain_periods_exist() {
        let response = WeatherResponse {
            daily: DailyForecast {
                time: vec!["2026-06-10".to_string()],
                weather_code: vec![61],
                precipitation_sum: vec![Some(20.0)],
                precipitation_probability_max: vec![Some(90.0)],
            },
            hourly: Some(HourlyForecast {
                time: vec!["2026-06-10T09:00".to_string()],
                weather_code: vec![1],
                precipitation: vec![Some(0.0)],
                precipitation_probability: vec![Some(0.0)],
                wind_gusts_10m: vec![Some(10.0)],
            }),
        };

        let today = parse(&response).unwrap();

        assert_eq!(today.tone, WeatherTone::Sunny);
        assert!(!today.is_too_wet);
        assert!(today.rain_periods.is_empty());
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
        assert!(url.contains("daily=weather_code,precipitation_sum,precipitation_probability_max"));
        assert!(url.contains(
            "hourly=weather_code,precipitation,precipitation_probability,wind_gusts_10m"
        ));
    }
}
