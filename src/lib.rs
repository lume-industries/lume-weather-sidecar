use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForecastDay {
    pub day: String,
    pub condition: String,
    pub high: i32,
    pub low: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WeatherPayload {
    pub location: String,
    pub updated: String,
    pub days: Vec<ForecastDay>,
}

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    data: Vec<LocationEntry>,
}

#[derive(Deserialize)]
struct LocationEntry {
    geohash: String,
    name: String,
    state: Option<String>,
}

#[derive(Deserialize)]
struct ForecastResponse {
    #[serde(default)]
    data: Vec<ForecastEntry>,
}

#[derive(Deserialize)]
struct ForecastEntry {
    date: String,
    temp_max: Option<f32>,
    temp_min: Option<f32>,
    icon_descriptor: Option<String>,
}

fn weekday_abbrev_from_iso(iso_date: &str) -> Option<&'static str> {
    // Parse YYYY-MM-DD and compute day of week
    let parts: Vec<&str> = iso_date.split('-').collect();
    if parts.len() != 3 { return None; }
    let year: i32 = parts[0].parse().ok()?;
    let month: u8 = parts[1].parse().ok()?;
    let day: u8 = parts[2].parse().ok()?;
    let days = days_from_civil(year, month, day);
    // 1970-01-01 was a Thursday (day 4)
    let weekday = ((days + 4) % 7 + 7) % 7;
    Some(match weekday {
        0 => "Sun", 1 => "Mon", 2 => "Tue", 3 => "Wed",
        4 => "Thu", 5 => "Fri", 6 => "Sat",
        _ => "???",
    })
}

fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let year = i64::from(year) - i64::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let doy = (153 * (month + if month > 2 { -3 } else { 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn map_condition(icon: &str) -> String {
    let normalized = icon.to_ascii_lowercase().replace(['-', ' '], "_");
    match normalized.as_str() {
        "sunny" | "mostly_sunny" | "clear" => "sunny".to_string(),
        "partly_cloudy" => "part cloud".to_string(),
        "cloudy" => "cloudy".to_string(),
        "haze" | "hazy" | "fog" => "fog".to_string(),
        "light_shower" | "light_showers" | "light_rain" => "light rain".to_string(),
        "shower" | "showers" | "rain" => "rain".to_string(),
        "heavy_shower" | "heavy_showers" | "heavy_rain" => "heavy rain".to_string(),
        "storm" | "storms" | "thunderstorm" => "storm".to_string(),
        "wind" | "windy" | "dust" | "dusty" => "windy".to_string(),
        "snow" => "snow".to_string(),
        "frost" => "cold".to_string(),
        "cyclone" | "tropical_cyclone" => "cyclone".to_string(),
        _ => "cloudy".to_string(),
    }
}

pub fn parse_search_result(body: &str) -> Result<(String, String), String> {
    let response: SearchResponse = serde_json::from_str(body)
        .map_err(|error| format!("invalid weather search JSON: {error}"))?;
    let first = response
        .data
        .into_iter()
        .next()
        .ok_or_else(|| "weather search returned no locations".to_string())?;
    let label = match first.state {
        Some(state) if !state.is_empty() => format!("{}, {}", first.name, state),
        _ => first.name,
    };
    Ok((first.geohash, label))
}

pub fn parse_forecast_payload(
    location: String,
    body: &str,
    now_secs: u64,
) -> Result<WeatherPayload, String> {
    let response: ForecastResponse = serde_json::from_str(body)
        .map_err(|error| format!("invalid weather forecast JSON: {error}"))?;
    let mut days = Vec::new();
    for entry in response.data.into_iter().take(7) {
        let (Some(high), Some(low)) = (entry.temp_max, entry.temp_min) else {
            continue;
        };
        days.push(ForecastDay {
            day: weekday_abbrev_from_iso(&entry.date)
                .unwrap_or("???")
                .to_string(),
            condition: map_condition(entry.icon_descriptor.as_deref().unwrap_or("cloudy")),
            high: high.round() as i32,
            low: low.round() as i32,
        });
    }

    if days.is_empty() {
        return Err("weather forecast contained no usable daily entries".to_string());
    }

    let hh = (now_secs % 86400) / 3600;
    let mm = (now_secs % 3600) / 60;
    Ok(WeatherPayload {
        location,
        updated: format!("Updated {:02}:{:02}", hh, mm),
        days,
    })
}
