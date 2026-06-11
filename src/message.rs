use crate::weather::{TodayWeather, WeatherTone};

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
            notes: &[
                "傘を持っていきましょう",
                "出来ればリモートしましょう",
            ],
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

pub fn compose_message(location_name: Option<&str>, forecast: &TodayWeather) -> String {
    let plan = message_plan(forecast);
    let mut lines = Vec::with_capacity(plan.notes.len() + 2);

    if plan.mention {
        lines.push(HERE_MENTION.to_string());
    }

    lines.push(format!("{}{}", message_prefix(location_name, &forecast.date_display), plan.main));
    lines.extend(plan.notes.iter().map(|note| note.to_string()));

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::compose_message;
    use crate::weather::{TodayWeather, WeatherTone};

    #[test]
    fn sunny_has_location_prefix() {
        let weather = TodayWeather {
            date_display: "6/11".to_string(),
            tone: WeatherTone::Sunny,
            is_too_wet: false,
        };

        let msg = compose_message(Some("新宿"), &weather);
        assert_eq!(msg, "本日(6/11)の新宿は晴れです");
    }

    #[test]
    fn rain_includes_here_mention() {
        let weather = TodayWeather {
            date_display: "6/11".to_string(),
            tone: WeatherTone::Rain,
            is_too_wet: false,
        };

        let msg = compose_message(None, &weather);
        assert_eq!(
            msg,
            "<!here>\n本日(6/11)は雨です\n傘を持っていきましょう"
        );
    }

    #[test]
    fn heavy_rain_includes_remote_message() {
        let weather = TodayWeather {
            date_display: "6/11".to_string(),
            tone: WeatherTone::Rain,
            is_too_wet: true,
        };

        let msg = compose_message(Some("新宿"), &weather);
        assert_eq!(
            msg,
            "<!here>\n本日(6/11)の新宿は滝が降ります\n傘を持っていきましょう\n出来ればリモートしましょう"
        );
    }
}
