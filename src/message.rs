use crate::weather::{TodayWeather, WeatherTone};

const HERE_MENTION: &str = "<!here>";

pub fn compose_message(location_name: Option<&str>, forecast: &TodayWeather) -> String {
    let mut message = match forecast.tone {
        WeatherTone::Sunny => "晴れです".to_string(),
        WeatherTone::Cloudy => "曇りです".to_string(),
        WeatherTone::Snow => "雪が降りそうです".to_string(),
        WeatherTone::Rain => {
            if forecast.is_too_wet {
                format!(
                    "{HERE_MENTION} 滝が降ります、傘を持っていきましょう。\n出来ればリモートしましょう"
                )
            } else {
                format!("{HERE_MENTION} 雨です、傘を持っていきましょう")
            }
        }
        WeatherTone::Other => "天気を取得できませんでした".to_string(),
    };

    if let Some(name) = location_name {
        message = format!("{name}: {message}");
    }

    message
}

#[cfg(test)]
mod tests {
    use super::compose_message;
    use crate::weather::{TodayWeather, WeatherTone};

    #[test]
    fn sunny_has_location_prefix() {
        let weather = TodayWeather {
            tone: WeatherTone::Sunny,
            is_too_wet: false,
        };

        let msg = compose_message(Some("新宿"), &weather);
        assert_eq!(msg, "新宿: 晴れです");
    }

    #[test]
    fn rain_includes_here_mention() {
        let weather = TodayWeather {
            tone: WeatherTone::Rain,
            is_too_wet: false,
        };

        let msg = compose_message(None, &weather);
        assert_eq!(msg, "<!here> 雨です、傘を持っていきましょう");
    }

    #[test]
    fn heavy_rain_includes_remote_message() {
        let weather = TodayWeather {
            tone: WeatherTone::Rain,
            is_too_wet: true,
        };

        let msg = compose_message(Some("新宿"), &weather);
        assert_eq!(
            msg,
            "新宿: <!here> 滝が降ります、傘を持っていきましょう。\n出来ればリモートしましょう"
        );
    }
}
