pub(crate) fn parse_station_arg(arg: String) -> Option<String> {
    let station_name = arg.trim().to_string();
    (!station_name.is_empty()).then_some(station_name)
}

pub(crate) fn parse_station_threshold_args(arg: String) -> Option<(String, f64)> {
    let mut parts: Vec<&str> = arg.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let threshold_raw = parts.pop()?.replace(',', ".");
    let threshold = threshold_raw.parse::<f64>().ok()?;
    let station_name = parts.join(" ").trim().to_string();
    (!station_name.is_empty()).then_some((station_name, threshold))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_station_arg_rejects_blank() {
        assert_eq!(parse_station_arg("   ".to_string()), None);
    }

    #[test]
    fn parse_station_threshold_args_parses_station_and_threshold() {
        let parsed = parse_station_threshold_args("S. Carlo 2,5".to_string());
        assert_eq!(parsed, Some(("S. Carlo".to_string(), 2.5)));
    }

    #[test]
    fn parse_station_threshold_args_rejects_missing_threshold() {
        assert_eq!(parse_station_threshold_args("Cesena".to_string()), None);
    }
}
