use crate::weather::{
    RainImpact, RainImpactPeriod, RainPeriod, TimePeriod, TodayWeather, WeatherTone, WindPeriod,
};

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
            main: "滝が降ります",
            notes: &["傘を持っていきましょう", "出来ればリモートしましょう"],
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

fn rain_period_header(location_name: Option<&str>, date_display: &str) -> String {
    match location_name {
        Some(name) => format!("本日({})の{}は雨の時間帯があります", date_display, name),
        None => format!("本日({})は雨の時間帯があります", date_display),
    }
}

fn period_header(location_name: Option<&str>, forecast: &TodayWeather) -> String {
    if !forecast.rain_periods.is_empty() {
        return rain_period_header(location_name, &forecast.date_display);
    }

    match location_name {
        Some(name) => format!(
            "本日({})の{}は風の注意があります",
            forecast.date_display, name
        ),
        None => format!("本日({})は風の注意があります", forecast.date_display),
    }
}

fn should_mention_for_periods(forecast: &TodayWeather) -> bool {
    forecast
        .rain_periods
        .iter()
        .any(|period| !period.impact_periods.is_empty() || period.is_too_wet)
        || forecast
            .wind_periods
            .iter()
            .any(|period| !period.storm_periods.is_empty())
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

fn impact_periods_note(periods: &[RainPeriod]) -> Option<String> {
    let impacts = periods
        .iter()
        .flat_map(|period| period.impact_periods.iter())
        .map(format_impact_period)
        .collect::<Vec<_>>();

    if impacts.is_empty() {
        None
    } else {
        Some(format!("影響: {}", impacts.join("、")))
    }
}

fn format_impact_period(period: &RainImpactPeriod) -> String {
    format!(
        "{} {}",
        rain_impact_label(period.impact),
        period.time_display()
    )
}

fn rain_impact_label(impact: RainImpact) -> &'static str {
    match impact {
        RainImpact::EarlyCommute | RainImpact::Commute => "出勤",
        RainImpact::Lunch => "昼",
        RainImpact::Return | RainImpact::LateReturn => "退勤",
        RainImpact::Overtime => "残業",
        RainImpact::LowImpact => "低影響",
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
    let mut lines =
        Vec::with_capacity(forecast.rain_periods.len() + forecast.wind_periods.len() + 4);

    if should_mention_for_periods(forecast) {
        lines.push(HERE_MENTION.to_string());
    }

    lines.push(period_header(location_name, forecast));
    if !forecast.rain_periods.is_empty() {
        lines.push(rain_periods_note(&forecast.rain_periods));
    }
    if let Some(note) = impact_periods_note(&forecast.rain_periods) {
        lines.push(note);
    }
    if !forecast.wind_periods.is_empty() {
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
            "<!here>\n本日(6/11)の新宿は滝が降ります\n傘を持っていきましょう\n出来ればリモートしましょう"
        );
    }

    #[test]
    fn rain_periods_explain_impact_windows() {
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
            "<!here>\n本日(6/11)は雨の時間帯があります\n雨の時間帯: 01:00-24:00\n影響: 出勤 08:00-10:00、昼 12:00-14:00、退勤 19:00-21:00、残業 21:00-24:00"
        );
    }

    #[test]
    fn low_impact_rain_period_does_not_mention_here() {
        let mut weather = weather(WeatherTone::Rain, false);
        weather.rain_periods = vec![rain_period("16:00", "17:00", RainImpact::LowImpact)];

        let msg = compose_message(Some("新宿"), &weather);

        assert_eq!(
            msg,
            "本日(6/11)の新宿は雨の時間帯があります\n雨の時間帯: 16:00-17:00"
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
            "<!here>\n本日(6/11)は雨の時間帯があります\n雨の時間帯: 08:00-09:00（雷雨: 08:00-09:00）\n影響: 出勤 08:00-09:00"
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
            "<!here>\n本日(6/11)の新宿は風の注意があります\n09:00-13:00: 強風に注意してください（暴風: 11:00-12:00、最大76km/h）"
        );
    }
}
