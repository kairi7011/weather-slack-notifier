use crate::weather::{RainImpact, RainPeriod, TimePeriod, TodayWeather, WeatherTone, WindPeriod};

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
        WeatherTone::Rain if forecast.has_heavy_rain => MessagePlan {
            mention: true,
            main: "滝が降ります",
            notes: &["傘を持っていきましょう", "できればリモートしましょう"],
        },
        WeatherTone::Rain if forecast.is_too_wet => MessagePlan {
            mention: true,
            main: "雨に注意が必要です",
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

/// Which routine time windows the rain hits.
///
/// `early_commute` (the first hour of the commute window) is kept separate
/// from `commute` on purpose: rain that ends within that first hour misses
/// most departures, so it must not trigger commute alerts or a mention.
#[derive(Debug, Default)]
struct ImpactBuckets {
    early_commute: bool,
    commute: bool,
    lunch: bool,
    return_commute: bool,
    overtime: bool,
    /// A severe period that overlaps none of the classified windows
    /// (e.g. storm wind at midday without rain).
    unclassified: bool,
}

impl ImpactBuckets {
    fn hits_commute(&self) -> bool {
        self.commute || self.return_commute
    }
}

fn record_impact(buckets: &mut ImpactBuckets, impact: RainImpact) {
    match impact {
        RainImpact::EarlyCommute => buckets.early_commute = true,
        RainImpact::Commute => buckets.commute = true,
        RainImpact::Lunch => buckets.lunch = true,
        RainImpact::Return | RainImpact::LateReturn => buckets.return_commute = true,
        RainImpact::Overtime => buckets.overtime = true,
        RainImpact::LowImpact => {}
    }
}

fn rain_impact_buckets(periods: &[RainPeriod]) -> ImpactBuckets {
    let mut buckets = ImpactBuckets::default();
    for impact_period in periods
        .iter()
        .flat_map(|period| period.impact_periods.iter())
    {
        record_impact(&mut buckets, impact_period.impact);
    }
    buckets
}

fn severe_impact_buckets(
    rain_periods: &[RainPeriod],
    severe_periods: &[TimePeriod],
) -> ImpactBuckets {
    let mut buckets = ImpactBuckets::default();

    for severe in severe_periods {
        let mut classified = false;
        for impact_period in rain_periods
            .iter()
            .flat_map(|period| period.impact_periods.iter())
        {
            if time_ranges_overlap(
                &severe.start_display,
                &severe.end_display,
                &impact_period.start_display,
                &impact_period.end_display,
            ) {
                record_impact(&mut buckets, impact_period.impact);
                classified = true;
            }
        }
        if !classified {
            buckets.unclassified = true;
        }
    }

    buckets
}

fn lunch_ranges(periods: &[RainPeriod]) -> Vec<String> {
    periods
        .iter()
        .flat_map(|period| period.impact_periods.iter())
        .filter(|impact_period| impact_period.impact == RainImpact::Lunch)
        .map(|impact_period| impact_period.time_display())
        .collect()
}

fn severe_rain_time_periods(periods: &[RainPeriod]) -> Vec<TimePeriod> {
    periods
        .iter()
        .flat_map(|period| {
            period
                .heavy_periods
                .iter()
                .chain(period.thunderstorm_periods.iter())
        })
        .cloned()
        .collect()
}

fn rain_times_line(periods: &[RainPeriod], low_impact: bool) -> String {
    let ranges = periods
        .iter()
        .map(RainPeriod::time_display)
        .collect::<Vec<_>>()
        .join(", ");
    if low_impact {
        format!("雨の時間帯: {ranges}（外出予定がなければ影響は小さめです）")
    } else {
        format!("雨の時間帯: {ranges}")
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
        Some(format!("滝が降る時間帯: {}", ranges.join(", ")))
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

fn is_heavy_rain(forecast: &TodayWeather) -> bool {
    forecast
        .rain_periods
        .iter()
        .any(|period| !period.heavy_periods.is_empty())
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
        start <= 6 * 60 && end >= 22 * 60
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

fn rain_header_subject(
    all_day: bool,
    heavy: bool,
    has_severe: bool,
    buckets: &ImpactBuckets,
    severe_buckets: &ImpactBuckets,
) -> &'static str {
    if all_day {
        if heavy {
            "一日滝が降る予報です"
        } else {
            "一日雨の予報です"
        }
    } else if buckets.commute && buckets.return_commute {
        "出勤・退勤時間帯に雨が降りそうです"
    } else if buckets.commute {
        "出勤時間帯に雨が降りそうです"
    } else if buckets.return_commute {
        "退勤時間帯に雨が降りそうです"
    } else if severe_buckets.overtime {
        "退勤後の夜に激しい雨が降りそうです"
    } else if buckets.lunch {
        "昼に雨が降りそうです"
    } else if buckets.early_commute {
        "朝早くに雨が降りそうです"
    } else if has_severe {
        "悪天候の時間帯があります"
    } else {
        "雨の時間帯があります"
    }
}

fn add_umbrella_advice(lines: &mut Vec<String>, all_day: bool, buckets: &ImpactBuckets) {
    if all_day || (buckets.commute && buckets.return_commute) {
        lines.push("出勤・退勤とも傘が必要です".to_string());
    } else if buckets.commute {
        lines.push("傘を持っていきましょう".to_string());
        if !buckets.return_commute && !buckets.overtime {
            lines.push("退勤時間帯は止む見込みです".to_string());
        }
    } else if buckets.return_commute {
        lines.push("帰りに降る見込みです。朝から傘を持っていきましょう".to_string());
    } else if buckets.early_commute {
        lines.push("出勤時間帯には止む見込みです（早く出るなら傘を）".to_string());
    }
}

fn add_severe_advice(
    lines: &mut Vec<String>,
    all_day: bool,
    buckets: &ImpactBuckets,
    severe_buckets: &ImpactBuckets,
) {
    if all_day || (severe_buckets.commute && severe_buckets.return_commute) {
        lines.push("できればリモートしましょう".to_string());
    } else if severe_buckets.commute {
        lines.push("出勤は荒れる時間帯を避けるのがよさそうです".to_string());
    } else if severe_buckets.return_commute {
        lines.push("退勤は荒れる時間帯を避けるのがよさそうです".to_string());
    } else if severe_buckets.overtime {
        lines.push("荒れるのは退勤後です。残業せず早めに帰りましょう".to_string());
    } else if severe_buckets.lunch {
        lines.push("昼の外出は避けるのがよさそうです".to_string());
    } else if severe_buckets.unclassified {
        lines.push("その時間帯の外出は避けてください".to_string());
    } else if severe_buckets.early_commute && !buckets.early_commute {
        lines.push("出勤時間帯には収まる見込みです".to_string());
    }
}

fn compose_wind_only_message(location_name: Option<&str>, forecast: &TodayWeather) -> String {
    let storm_notes = storm_wind_notes(&forecast.wind_periods, &forecast.rain_periods);
    let prefix = message_prefix(location_name, &forecast.date_display);
    let mut lines = Vec::new();

    if storm_notes.is_empty() {
        lines.push(format!("{prefix}風が強い予報です"));
        lines.extend(forecast.wind_periods.iter().map(wind_period_note));
    } else {
        lines.push(HERE_MENTION.to_string());
        lines.push(format!("{prefix}悪天候の時間帯があります"));
        lines.extend(storm_notes);
        lines.push("その時間帯の外出は避けてください".to_string());
    }

    lines.join("\n")
}

fn compose_period_message(location_name: Option<&str>, forecast: &TodayWeather) -> String {
    if forecast.rain_periods.is_empty() {
        return compose_wind_only_message(location_name, forecast);
    }

    let all_day = is_all_day_rain(&forecast.rain_periods);
    let heavy = is_heavy_rain(forecast);
    let buckets = rain_impact_buckets(&forecast.rain_periods);

    let mut severe_periods = severe_rain_time_periods(&forecast.rain_periods);
    severe_periods.extend(
        forecast
            .wind_periods
            .iter()
            .flat_map(|period| period.storm_periods.iter())
            .cloned(),
    );
    let severe_buckets = severe_impact_buckets(&forecast.rain_periods, &severe_periods);
    let has_severe = !severe_periods.is_empty();

    let mention = all_day
        || buckets.hits_commute()
        || severe_buckets.hits_commute()
        || severe_buckets.lunch
        || severe_buckets.overtime
        || severe_buckets.unclassified;

    let mut lines = Vec::new();
    if mention {
        lines.push(HERE_MENTION.to_string());
    }

    lines.push(format!(
        "{}{}",
        message_prefix(location_name, &forecast.date_display),
        rain_header_subject(all_day, heavy, has_severe, &buckets, &severe_buckets)
    ));

    let low_impact = !all_day
        && !has_severe
        && !buckets.hits_commute()
        && !buckets.early_commute
        && !buckets.lunch
        && !buckets.overtime;
    lines.push(rain_times_line(&forecast.rain_periods, low_impact));

    if let Some(note) = heavy_rain_note(&forecast.rain_periods) {
        lines.push(note);
    }
    if let Some(note) = thunderstorm_note(&forecast.rain_periods) {
        lines.push(note);
    }
    lines.extend(storm_wind_notes(
        &forecast.wind_periods,
        &forecast.rain_periods,
    ));

    add_umbrella_advice(&mut lines, all_day, &buckets);

    if !all_day && buckets.lunch && !(buckets.commute && buckets.return_commute) {
        let ranges = lunch_ranges(&forecast.rain_periods).join(", ");
        lines.push(format!("昼({ranges})に外へ出るなら傘があると安心です"));
    }

    if has_severe {
        add_severe_advice(&mut lines, all_day, &buckets, &severe_buckets);
    }

    if !has_severe && !forecast.wind_periods.is_empty() {
        lines.push("風の注意:".to_string());
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

    fn weather(tone: WeatherTone, is_too_wet: bool, has_heavy_rain: bool) -> TodayWeather {
        TodayWeather {
            date_display: "6/11".to_string(),
            tone,
            is_too_wet,
            has_heavy_rain,
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
        let weather = weather(WeatherTone::Sunny, false, false);

        let msg = compose_message(Some("新宿"), &weather);
        assert_eq!(msg, "本日(6/11)の新宿は晴れです");
    }

    #[test]
    fn rain_includes_here_mention() {
        let weather = weather(WeatherTone::Rain, false, false);

        let msg = compose_message(None, &weather);
        assert_eq!(msg, "<!here>\n本日(6/11)は雨です\n傘を持っていきましょう");
    }

    #[test]
    fn heavy_rain_includes_remote_message() {
        let weather = weather(WeatherTone::Rain, true, true);

        let msg = compose_message(Some("新宿"), &weather);
        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の新宿は滝が降ります\n傘を持っていきましょう\nできればリモートしましょう"
        );
    }

    #[test]
    fn high_probability_rain_without_heavy_amount_does_not_say_waterfall() {
        let weather = weather(WeatherTone::Rain, true, false);

        let msg = compose_message(Some("新宿"), &weather);
        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の新宿は雨に注意が必要です\n傘を持っていきましょう\nできればリモートしましょう"
        );
    }

    #[test]
    fn all_day_rain_shows_time_range_and_skips_redundant_lunch_note() {
        let mut weather = weather(WeatherTone::Rain, false, false);
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
            "<!here>\n本日(6/11)は一日雨の予報です\n雨の時間帯: 01:00-24:00\n出勤・退勤とも傘が必要です"
        );
    }

    #[test]
    fn all_day_high_probability_rain_is_not_called_heavy_without_heavy_periods() {
        let mut weather = weather(WeatherTone::Rain, false, false);
        let mut period = rain_period("01:00", "24:00", RainImpact::LowImpact);
        period.is_too_wet = true;
        weather.rain_periods = vec![period];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は一日雨の予報です\n雨の時間帯: 01:00-24:00\n出勤・退勤とも傘が必要です"
        );
    }

    #[test]
    fn all_day_heavy_rain_shows_heavy_time_range_and_recommends_remote() {
        let mut weather = weather(WeatherTone::Rain, true, true);
        let mut period = rain_period("01:00", "24:00", RainImpact::LowImpact);
        period.heavy_periods = vec![TimePeriod {
            start_display: "08:00".to_string(),
            end_display: "12:00".to_string(),
        }];
        weather.rain_periods = vec![period];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は一日滝が降る予報です\n雨の時間帯: 01:00-24:00\n滝が降る時間帯: 08:00-12:00\n出勤・退勤とも傘が必要です\nできればリモートしましょう"
        );
    }

    #[test]
    fn low_impact_rain_does_not_tell_people_to_bring_an_umbrella() {
        let mut weather = weather(WeatherTone::Rain, false, false);
        weather.rain_periods = vec![rain_period("16:00", "17:00", RainImpact::LowImpact)];

        let msg = compose_message(Some("新宿"), &weather);

        assert_eq!(
            msg,
            "本日(6/11)の新宿は雨の時間帯があります\n雨の時間帯: 16:00-17:00（外出予定がなければ影響は小さめです）"
        );
    }

    #[test]
    fn outbound_commute_rain_shows_times_and_lunch_range() {
        let mut weather = weather(WeatherTone::Rain, false, false);
        weather.rain_periods = vec![rain_period("00:00", "18:00", RainImpact::Commute)];
        weather.rain_periods[0].impact_periods = vec![
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
        ];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は出勤時間帯に雨が降りそうです\n雨の時間帯: 00:00-18:00\n傘を持っていきましょう\n退勤時間帯は止む見込みです\n昼(12:00-14:00)に外へ出るなら傘があると安心です"
        );
    }

    #[test]
    fn lunch_rain_is_reported_as_lower_priority() {
        let mut weather = weather(WeatherTone::Rain, false, false);
        weather.rain_periods = vec![rain_period("12:00", "13:00", RainImpact::Lunch)];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "本日(6/11)の西新宿は昼に雨が降りそうです\n雨の時間帯: 12:00-13:00\n昼(12:00-13:00)に外へ出るなら傘があると安心です"
        );
    }

    #[test]
    fn commute_heavy_rain_shows_times_and_avoidance_advice() {
        let mut weather = weather(WeatherTone::Rain, true, true);
        let mut period = rain_period("08:00", "10:00", RainImpact::Commute);
        period.heavy_periods = vec![TimePeriod {
            start_display: "08:00".to_string(),
            end_display: "10:00".to_string(),
        }];
        weather.rain_periods = vec![period];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は出勤時間帯に雨が降りそうです\n雨の時間帯: 08:00-10:00\n滝が降る時間帯: 08:00-10:00\n傘を持っていきましょう\n退勤時間帯は止む見込みです\n出勤は荒れる時間帯を避けるのがよさそうです"
        );
    }

    #[test]
    fn return_commute_heavy_rain_uses_return_wording() {
        let mut weather = weather(WeatherTone::Rain, true, true);
        let mut period = rain_period("19:00", "21:00", RainImpact::Return);
        period.impact_periods = vec![RainImpactPeriod {
            start_display: "19:00".to_string(),
            end_display: "21:00".to_string(),
            impact: RainImpact::Return,
        }];
        period.heavy_periods = vec![TimePeriod {
            start_display: "19:00".to_string(),
            end_display: "21:00".to_string(),
        }];
        weather.rain_periods = vec![period];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は退勤時間帯に雨が降りそうです\n雨の時間帯: 19:00-21:00\n滝が降る時間帯: 19:00-21:00\n帰りに降る見込みです。朝から傘を持っていきましょう\n退勤は荒れる時間帯を避けるのがよさそうです"
        );
    }

    #[test]
    fn heavy_rain_after_return_window_advises_leaving_on_time() {
        let mut weather = weather(WeatherTone::Rain, true, true);
        let mut period = rain_period("19:00", "24:00", RainImpact::Return);
        period.impact_periods = vec![
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
        period.heavy_periods = vec![TimePeriod {
            start_display: "21:00".to_string(),
            end_display: "24:00".to_string(),
        }];
        weather.rain_periods = vec![period];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は退勤時間帯に雨が降りそうです\n雨の時間帯: 19:00-24:00\n滝が降る時間帯: 21:00-24:00\n帰りに降る見込みです。朝から傘を持っていきましょう\n荒れるのは退勤後です。残業せず早めに帰りましょう"
        );
    }

    #[test]
    fn overtime_only_heavy_rain_advises_leaving_early_without_commute_alert() {
        let mut weather = weather(WeatherTone::Rain, true, true);
        let mut period = rain_period("21:00", "24:00", RainImpact::Overtime);
        period.heavy_periods = vec![TimePeriod {
            start_display: "21:00".to_string(),
            end_display: "24:00".to_string(),
        }];
        weather.rain_periods = vec![period];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の西新宿は退勤後の夜に激しい雨が降りそうです\n雨の時間帯: 21:00-24:00\n滝が降る時間帯: 21:00-24:00\n荒れるのは退勤後です。残業せず早めに帰りましょう"
        );
    }

    #[test]
    fn heavy_rain_ending_in_early_commute_hour_does_not_mention_here() {
        let mut weather = weather(WeatherTone::Rain, true, true);
        let mut period = rain_period("03:00", "08:00", RainImpact::LowImpact);
        period.impact_periods = vec![RainImpactPeriod {
            start_display: "07:00".to_string(),
            end_display: "08:00".to_string(),
            impact: RainImpact::EarlyCommute,
        }];
        period.heavy_periods = vec![TimePeriod {
            start_display: "03:00".to_string(),
            end_display: "08:00".to_string(),
        }];
        weather.rain_periods = vec![period];

        let msg = compose_message(Some("西新宿"), &weather);

        assert_eq!(
            msg,
            "本日(6/11)の西新宿は朝早くに雨が降りそうです\n雨の時間帯: 03:00-08:00\n滝が降る時間帯: 03:00-08:00\n出勤時間帯には止む見込みです（早く出るなら傘を）"
        );
    }

    #[test]
    fn early_commute_thunderstorm_shows_times_without_mention() {
        let mut weather = weather(WeatherTone::Rain, true, false);
        let mut period = rain_period("08:00", "09:00", RainImpact::EarlyCommute);
        period.thunderstorm_periods = vec![TimePeriod {
            start_display: "08:00".to_string(),
            end_display: "09:00".to_string(),
        }];
        weather.rain_periods = vec![period];

        let msg = compose_message(None, &weather);

        assert_eq!(
            msg,
            "本日(6/11)は朝早くに雨が降りそうです\n雨の時間帯: 08:00-09:00\n雷雨: 08:00-09:00\n出勤時間帯には止む見込みです（早く出るなら傘を）"
        );
    }

    #[test]
    fn all_day_bad_weather_lists_severe_time_ranges() {
        let mut weather = weather(WeatherTone::Rain, true, false);
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
            "<!here>\n本日(6/11)の西新宿は一日雨の予報です\n雨の時間帯: 01:00-24:00\n雷雨: 08:00-09:00\n暴風雨: 11:00-12:00\n出勤・退勤とも傘が必要です\nできればリモートしましょう"
        );
    }

    #[test]
    fn wind_periods_are_reported_without_rain_periods() {
        let mut weather = weather(WeatherTone::Cloudy, false, false);
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
            "<!here>\n本日(6/11)の新宿は悪天候の時間帯があります\n暴風: 11:00-12:00\nその時間帯の外出は避けてください"
        );
    }

    #[test]
    fn strong_wind_without_storm_is_reported_without_mention() {
        let mut weather = weather(WeatherTone::Cloudy, false, false);
        weather.wind_periods = vec![WindPeriod {
            start_display: "09:00".to_string(),
            end_display: "13:00".to_string(),
            max_gust_kmh: 60,
            storm_periods: Vec::new(),
        }];

        let msg = compose_message(Some("新宿"), &weather);

        assert_eq!(
            msg,
            "本日(6/11)の新宿は風が強い予報です\n09:00-13:00: 強風に注意してください（最大60km/h）"
        );
    }

    #[test]
    fn storm_wind_outside_rain_impact_warns_without_commute_advice() {
        let mut weather = weather(WeatherTone::Rain, true, false);
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
            "<!here>\n本日(6/11)の西新宿は悪天候の時間帯があります\n雨の時間帯: 16:00-17:00\n暴風: 09:00-10:00\nその時間帯の外出は避けてください"
        );
    }
}
