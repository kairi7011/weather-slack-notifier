use crate::weather::{RainPeriod, TimePeriod, TodayWeather, WeatherTone, WindPeriod};

const HERE_MENTION: &str = "<!here>";

struct MessagePlan {
    mention: bool,
    main: &'static str,
    notes: &'static [&'static str],
}

fn message_plan(forecast: &TodayWeather) -> MessagePlan {
    match forecast.tone {
        WeatherTone::Sunny => MessagePlan {
            mention: false,
            main: "晴れです",
            notes: &[],
        },
        WeatherTone::Cloudy => MessagePlan {
            mention: false,
            main: "曇りです",
            notes: &[],
        },
        WeatherTone::Snow => MessagePlan {
            mention: false,
            main: "雪が降りそうです",
            notes: &[],
        },
        WeatherTone::Rain if forecast.is_too_wet => MessagePlan {
            mention: true,
            main: "雨が強い予報です",
            notes: &["傘を持っていきましょう", "できればリモートしましょう"],
        },
        WeatherTone::Rain => MessagePlan {
            mention: true,
            main: "雨です",
            notes: &["傘を持っていきましょう"],
        },
        WeatherTone::Other => MessagePlan {
            mention: false,
            main: "天気を取得できませんでした",
            notes: &[],
        },
    }
}

fn message_prefix(location_name: Option<&str>, date_display: &str) -> String {
    match location_name {
        Some(name) => format!("本日({})の{}は", date_display, name),
        None => format!("本日({})は", date_display),
    }
}

fn rainy_header(location_name: Option<&str>, date_display: &str) -> String {
    match location_name {
        Some(name) => format!("本日({})の{}は雨の時間帯があります", date_display, name),
        None => format!("本日({})は雨の時間帯があります", date_display),
    }
}

fn all_day_rain_header(location_name: Option<&str>, date_display: &str, heavy: bool) -> String {
    let suffix = if heavy {
        "一日強い雨の予報です"
    } else {
        "一日雨の予報です"
    };
    match location_name {
        Some(name) => format!("本日({date_display})の{name}は{suffix}"),
        None => format!("本日({date_display})は{suffix}"),
    }
}

fn bad_weather_header(location_name: Option<&str>, date_display: &str, all_day: bool) -> String {
    let suffix = if all_day {
        "一日悪天候です"
    } else {
        "悪天候の時間帯があります"
    };
    match location_name {
        Some(name) => format!("本日({date_display})の{name}は{suffix}"),
        None => format!("本日({date_display})は{suffix}"),
    }
}

fn rain_periods_note(periods: &[RainPeriod]) -> String {
    let ranges = periods
        .iter()
        .map(format_rain_period)
        .collect::<Vec<_>>()
        .join(", ");
    format!("雨の時間帯: {ranges}")
}

fn format_rain_period(period: &RainPeriod) -> String {
    let mut details = Vec::new();
    if !period.heavy_periods.is_empty() {
        details.push(format!(
            "強い雨: {}",
            format_time_periods(&period.heavy_periods)
        ));
    }
    if !period.thunderstorm_periods.is_empty() {
        details.push(format!(
            "雷雨: {}",
            format_time_periods(&period.thunderstorm_periods)
        ));
    }

    if details.is_empty() {
        period.time_display()
    } else {
        format!("{}（{}）", period.time_display(), details.join("、"))
    }
}

fn wind_period_note(period: &WindPeriod) -> String {
    let detail = if period_is_fully_covered(
        &period.storm_periods,
        &period.start_display,
        &period.end_display,
    ) {
        format!("暴風に注意してください（最大{}km/h）", period.max_gust_kmh)
    } else if period.storm_periods.is_empty() {
        format!("強風に注意してください（最大{}km/h）", period.max_gust_kmh)
    } else {
        format!(
            "強風に注意してください（暴風: {}、最大{}km/h）",
            format_time_periods(&period.storm_periods),
            period.max_gust_kmh
        )
    };

    format!("{}: {}", period.time_display(), detail)
}

fn storm_wind_notes(periods: &[WindPeriod], rain_periods: &[RainPeriod]) -> Vec<String> {
    let mut storm_wind_ranges = Vec::new();
    let mut storm_rain_ranges = Vec::new();

    for period in periods {
        for storm_period in &period.storm_periods {
            if time_period_overlaps_rain(storm_period, rain_periods) {
                storm_rain_ranges.push(storm_period.time_display());
            } else {
                storm_wind_ranges.push(storm_period.time_display());
            }
        }
    }

    let mut notes = Vec::new();
    if !storm_rain_ranges.is_empty() {
        notes.push(format!("暴風雨: {}", storm_rain_ranges.join(", ")));
    }
    if !storm_wind_ranges.is_empty() {
        notes.push(format!("暴風: {}", storm_wind_ranges.join(", ")));
    }
    notes
}

fn heavy_rain_note(periods: &[RainPeriod]) -> Option<String> {
    let ranges = periods
        .iter()
        .flat_map(|period| period.heavy_periods.iter())
        .map(TimePeriod::time_display)
        .collect::<Vec<_>>();

    if ranges.is_empty() {
        None
    } else {
        Some(format!("強い雨: {}", ranges.join(", ")))
    }
}

fn thunderstorm_note(periods: &[RainPeriod]) -> Option<String> {
    let ranges = periods
        .iter()
        .flat_map(|period| period.thunderstorm_periods.iter())
        .map(TimePeriod::time_display)
        .collect::<Vec<_>>();

    if ranges.is_empty() {
        None
    } else {
        Some(format!("雷雨: {}", ranges.join(", ")))
    }
}

fn has_storm_weather(forecast: &TodayWeather) -> bool {
    forecast
        .rain_periods
        .iter()
        .any(|period| !period.thunderstorm_periods.is_empty())
        || forecast
            .wind_periods
            .iter()
            .any(|period| !period.storm_periods.is_empty())
}

fn is_heavy_rain(forecast: &TodayWeather) -> bool {
    forecast
        .rain_periods
        .iter()
        .any(|period| !period.heavy_periods.is_empty())
}

fn period_minutes(start_display: &str, end_display: &str) -> Option<u32> {
    let start = parse_time_minutes(start_display)?;
    let end = parse_time_minutes(end_display)?;
    end.checked_sub(start)
}

fn time_ranges_overlap(
    first_start_display: &str,
    first_end_display: &str,
    second_start_display: &str,
    second_end_display: &str,
) -> bool {
    let Some(first_start) = parse_time_minutes(first_start_display) else {
        return false;
    };
    let Some(first_end) = parse_time_minutes(first_end_display) else {
        return false;
    };
    let Some(second_start) = parse_time_minutes(second_start_display) else {
        return false;
    };
    let Some(second_end) = parse_time_minutes(second_end_display) else {
        return false;
    };

    first_start < second_end && second_start < first_end
}

fn time_period_overlaps_rain(period: &TimePeriod, rain_periods: &[RainPeriod]) -> bool {
    rain_periods.iter().any(|rain_period| {
        time_ranges_overlap(
            &period.start_display,
            &period.end_display,
            &rain_period.start_display,
            &rain_period.end_display,
        )
    })
}

fn parse_time_minutes(value: &str) -> Option<u32> {
    if value == "24:00" {
        return Some(24 * 60);
    }
    let (hour, minute) = value.split_once(':')?;
    let hour = hour.parse::<u32>().ok()?;
    let minute = minute.parse::<u32>().ok()?;
    if hour < 24 && minute < 60 {
        Some(hour * 60 + minute)
    } else {
        None
    }
}

fn is_all_day_rain(periods: &[RainPeriod]) -> bool {
    periods.iter().any(|period| {
        let Some(start) = parse_time_minutes(&period.start_display) else {
            return false;
        };
        let Some(end) = parse_time_minutes(&period.end_display) else {
            return false;
        };
        let duration = period_minutes(&period.start_display, &period.end_display).unwrap_or(0);
        duration >= 18 * 60 || (start <= 6 * 60 && end >= 22 * 60)
    })
}

fn format_time_periods(periods: &[TimePeriod]) -> String {
    periods
        .iter()
        .map(TimePeriod::time_display)
        .collect::<Vec<_>>()
        .join(", ")
}

fn period_is_fully_covered(periods: &[TimePeriod], start_display: &str, end_display: &str) -> bool {
    periods.len() == 1
        && periods[0].start_display == start_display
        && periods[0].end_display == end_display
}

fn compose_period_message(location_name: Option<&str>, forecast: &TodayWeather) -> String {
    let all_day_rain = is_all_day_rain(&forecast.rain_periods);
    let storm_weather = has_storm_weather(forecast);
    let heavy_rain = is_heavy_rain(forecast);
    let bad_weather = storm_weather || (!all_day_rain && heavy_rain);
    let mut lines = Vec::with_capacity(forecast.wind_periods.len() + 5);

    if bad_weather || all_day_rain || heavy_rain || !forecast.rain_periods.is_empty() {
        lines.push(HERE_MENTION.to_string());
    }

    if bad_weather {
        lines.push(bad_weather_header(
            location_name,
            &forecast.date_display,
            all_day_rain,
        ));
        let mut severe_notes = Vec::new();
        if let Some(note) = heavy_rain_note(&forecast.rain_periods) {
            severe_notes.push(note);
        }
        if let Some(note) = thunderstorm_note(&forecast.rain_periods) {
            severe_notes.push(note);
        }
        severe_notes.extend(storm_wind_notes(
            &forecast.wind_periods,
            &forecast.rain_periods,
        ));
        if all_day_rain {
            let kinds = severe_notes
                .iter()
                .map(|note| note.split(':').next().unwrap_or(note))
                .collect::<Vec<_>>()
                .join("、");
            if !kinds.is_empty() {
                lines.push(format!("{kinds}の予報が出ています"));
            }
        } else {
            lines.extend(severe_notes);
        }
        lines.push("できればリモートしましょう".to_string());
    } else if all_day_rain {
        if heavy_rain {
            lines.push(all_day_rain_header(
                location_name,
                &forecast.date_display,
                true,
            ));
            lines.push("傘を持っていきましょう".to_string());
            lines.push("できればリモートしましょう".to_string());
        } else {
            lines.push(all_day_rain_header(
                location_name,
                &forecast.date_display,
                false,
            ));
            lines.push("傘を持っていきましょう".to_string());
        }
    } else if !forecast.rain_periods.is_empty() {
        lines.push(rainy_header(location_name, &forecast.date_display));
        lines.push(rain_periods_note(&forecast.rain_periods));
        lines.push("傘を持っていきましょう".to_string());
    } else if !forecast.wind_periods.is_empty() {
        match location_name {
            Some(name) => lines.push(format!(
                "本日({})の{}は風が強い予報です",
                forecast.date_display, name
            )),
            None => lines.push(format!("本日({})は風が強い予報です", forecast.date_display)),
        }
    }

    if !bad_weather && !forecast.wind_periods.is_empty() {
        if !forecast.rain_periods.is_empty() {
            lines.push("風の注意:".to_string());
        }
        lines.extend(forecast.wind_periods.iter().map(wind_period_note));
    }

    lines.join("\n")
}

pub fn compose_message(location_name: Option<&str>, forecast: &TodayWeather) -> String {
    if !forecast.rain_periods.is_empty() || !forecast.wind_periods.is_empty() {
        return compose_period_message(location_name, forecast);
    }

    let plan = message_plan(forecast);
    let mut lines = Vec::with_capacity(plan.notes.len() + 2);

    if plan.mention {
        lines.push(HERE_MENTION.to_string());
    }

    lines.push(format!(
        "{}{}",
        message_prefix(location_name, &forecast.date_display),
        plan.main
    ));
    lines.extend(plan.notes.iter().map(|note| note.to_string()));

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::compose_message;
    use crate::weather::{
        RainImpact, RainImpactPeriod, RainPeriod, TimePeriod, TodayWeather, WeatherTone, WindPeriod,
    };

    fn weather(tone: WeatherTone, is_too_wet: bool) -> TodayWeather {
        TodayWeather {
            date_display: "6/11".to_string(),
            tone,
            is_too_wet,
            rain_periods: Vec::new(),
            wind_periods: Vec::new(),
        }
    }

    fn rain_period(start: &str, end: &str, impact: RainImpact) -> RainPeriod {
        let impact_periods = if impact == RainImpact::LowImpact {
            Vec::new()
        } else {
            vec![RainImpactPeriod {
                start_display: start.to_string(),
                end_display: end.to_string(),
                impact,
            }]
        };

        RainPeriod {
            start_display: start.to_string(),
            end_display: end.to_string(),
            is_too_wet: false,
            impact_periods,
            heavy_periods: Vec::new(),
            thunderstorm_periods: Vec::new(),
        }
    }

    #[test]
    fn sunny_has_location_prefix() {
        let weather = weather(WeatherTone::Sunny, false);

        let msg = compose_message(Some("新宿"), &weather);
        assert_eq!(msg, "本日(6/11)の新宿は晴れです");
    }

    #[test]
    fn rain_includes_here_mention() {
        let weather = weather(WeatherTone::Rain, false);

        let msg = compose_message(None, &weather);
        assert_eq!(msg, "<!here>\n本日(6/11)は雨です\n傘を持っていきましょう");
    }

    #[test]
    fn heavy_rain_includes_remote_message() {
        let weather = weather(WeatherTone::Rain, true);

        let msg = compose_message(Some("新宿"), &weather);
        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の新宿は雨が強い予報です\n傘を持っていきましょう\nできればリモートしましょう"
        );
    }

    #[test]
    fn all_day_rain_summarizes_without_impact_windows() {
        let mut weather = weather(WeatherTone::Rain, false);
        let mut period = rain_period("01:00", "24:00", RainImpact::LowImpact);
        period.impact_periods = vec![
            RainImpactPeriod {
                start_display: "08:00".to_string(),
                end_display: "10:00".to_string(),
                impact: RainImpact::Commute,
            },
            RainImpactPeriod {
                start_display: "12:00".to_string(),
                end_display: "14:00".to_string(),
                impact: RainImpact::Lunch,
            },
            RainImpactPeriod {
                start_display: "19:00".to_string(),
                end_display: "21:00".to_string(),
                impact: RainImpact::Return,
            },
            RainImpactPeriod {
                start_display: "21:00".to_string(),
                end_display: "24:00".to_string(),
                impact: RainImpact::Overtime,
            },
        ];
        weather.rain_periods = vec![period];

        let msg = compose_message(None, &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)は一日雨の予報です\n傘を持っていきましょう"
        );
    }

    #[test]
    fn all_day_high_probability_rain_is_not_called_heavy_without_heavy_periods() {
        let mut weather = weather(WeatherTone::Rain, false);
        let mut period = rain_period("01:00", "24:00", RainImpact::LowImpact);
        period.is_too_wet = true;
        weather.rain_periods = vec![period];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は一日雨の予報です\n傘を持っていきましょう"
        );
    }

    #[test]
    fn all_day_heavy_rain_uses_direct_heavy_rain_summary() {
        let mut weather = weather(WeatherTone::Rain, true);
        let mut period = rain_period("01:00", "24:00", RainImpact::LowImpact);
        period.heavy_periods = vec![TimePeriod {
            start_display: "08:00".to_string(),
            end_display: "12:00".to_string(),
        }];
        weather.rain_periods = vec![period];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は一日強い雨の予報です\n傘を持っていきましょう\nできればリモートしましょう"
        );
    }

    #[test]
    fn partial_rain_reports_only_the_rain_range() {
        let mut weather = weather(WeatherTone::Rain, false);
        weather.rain_periods = vec![rain_period("16:00", "17:00", RainImpact::LowImpact)];

        let msg = compose_message(Some("新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の新宿は雨の時間帯があります\n雨の時間帯: 16:00-17:00\n傘を持っていきましょう"
        );
    }

    #[test]
    fn partial_heavy_rain_is_not_nested_inside_plain_rain_range() {
        let mut weather = weather(WeatherTone::Rain, true);
        let mut period = rain_period("08:00", "10:00", RainImpact::Commute);
        period.heavy_periods = vec![TimePeriod {
            start_display: "08:00".to_string(),
            end_display: "10:00".to_string(),
        }];
        weather.rain_periods = vec![period];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は悪天候の時間帯があります\n強い雨: 08:00-10:00\nできればリモートしましょう"
        );
    }

    #[test]
    fn rain_periods_include_thunderstorm_inside_the_period_note() {
        let mut weather = weather(WeatherTone::Rain, true);
        let mut period = rain_period("08:00", "09:00", RainImpact::EarlyCommute);
        period.thunderstorm_periods = vec![TimePeriod {
            start_display: "08:00".to_string(),
            end_display: "09:00".to_string(),
        }];
        weather.rain_periods = vec![period];

        let msg = compose_message(None, &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)は悪天候の時間帯があります\n雷雨: 08:00-09:00\nできればリモートしましょう"
        );
    }

    #[test]
    fn all_day_bad_weather_summarizes_to_weather_type_without_time_ranges() {
        let mut weather = weather(WeatherTone::Rain, true);
        let mut period = rain_period("01:00", "24:00", RainImpact::LowImpact);
        period.thunderstorm_periods = vec![TimePeriod {
            start_display: "08:00".to_string(),
            end_display: "09:00".to_string(),
        }];
        weather.rain_periods = vec![period];
        weather.wind_periods = vec![WindPeriod {
            start_display: "09:00".to_string(),
            end_display: "13:00".to_string(),
            max_gust_kmh: 76,
            storm_periods: vec![TimePeriod {
                start_display: "11:00".to_string(),
                end_display: "12:00".to_string(),
            }],
        }];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は一日悪天候です\n雷雨、暴風雨の予報が出ています\nできればリモートしましょう"
        );
    }

    #[test]
    fn wind_periods_are_reported_without_rain_periods() {
        let mut weather = weather(WeatherTone::Cloudy, false);
        weather.wind_periods = vec![WindPeriod {
            start_display: "09:00".to_string(),
            end_display: "13:00".to_string(),
            max_gust_kmh: 76,
            storm_periods: vec![TimePeriod {
                start_display: "11:00".to_string(),
                end_display: "12:00".to_string(),
            }],
        }];

        let msg = compose_message(Some("新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の新宿は悪天候の時間帯があります\n暴風: 11:00-12:00\nできればリモートしましょう"
        );
    }

    #[test]
    fn storm_wind_is_called_storm_rain_only_when_it_overlaps_rain() {
        let mut weather = weather(WeatherTone::Rain, true);
        weather.rain_periods = vec![rain_period("16:00", "17:00", RainImpact::LowImpact)];
        weather.wind_periods = vec![WindPeriod {
            start_display: "09:00".to_string(),
            end_display: "11:00".to_string(),
            max_gust_kmh: 76,
            storm_periods: vec![TimePeriod {
                start_display: "09:00".to_string(),
                end_display: "10:00".to_string(),
            }],
        }];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は悪天候の時間帯があります\n暴風: 09:00-10:00\nできればリモートしましょう"
        );
    }
}
