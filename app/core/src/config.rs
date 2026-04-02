const DEFAULT_SCAN_PAGE_SIZE: i32 = 25;
const MAX_SCAN_PAGE_SIZE: i32 = 100;

const ALERTS_TABLE_NAME_ENV: &str = "ALERTS_TABLE_NAME";
const CHATS_TABLE_NAME_ENV: &str = "CHATS_TABLE_NAME";
const EMILIA_ROMAGNA_STATIONS_TABLE_NAME_ENV: &str = "EMILIA_ROMAGNA_STATIONS_TABLE_NAME";
const MARCHE_STATIONS_TABLE_NAME_ENV: &str = "MARCHE_STATIONS_TABLE_NAME";
const REGION_EMILIA_ROMAGNA_KEY_ENV: &str = "REGION_EMILIA_ROMAGNA_KEY";
const REGION_EMILIA_ROMAGNA_LABEL_ENV: &str = "REGION_EMILIA_ROMAGNA_LABEL";
const REGION_MARCHE_KEY_ENV: &str = "REGION_MARCHE_KEY";
const REGION_MARCHE_LABEL_ENV: &str = "REGION_MARCHE_LABEL";
const STATIONS_SCAN_PAGE_SIZE_ENV: &str = "STATIONS_SCAN_PAGE_SIZE";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionConfig {
    pub key: String,
    pub label: String,
    pub table_name: String,
}

impl RegionConfig {
    pub fn matches_key(&self, key: &str) -> bool {
        key.eq_ignore_ascii_case(self.key.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionsConfig {
    pub emilia_romagna: RegionConfig,
    pub marche: RegionConfig,
}

impl RegionsConfig {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            emilia_romagna: RegionConfig {
                key: require_env(REGION_EMILIA_ROMAGNA_KEY_ENV)?,
                label: require_env(REGION_EMILIA_ROMAGNA_LABEL_ENV)?,
                table_name: require_env(EMILIA_ROMAGNA_STATIONS_TABLE_NAME_ENV)?,
            },
            marche: RegionConfig {
                key: require_env(REGION_MARCHE_KEY_ENV)?,
                label: require_env(REGION_MARCHE_LABEL_ENV)?,
                table_name: require_env(MARCHE_STATIONS_TABLE_NAME_ENV)?,
            },
        })
    }

    pub fn find_by_key(&self, key: &str) -> Option<&RegionConfig> {
        if self.emilia_romagna.matches_key(key) {
            Some(&self.emilia_romagna)
        } else if self.marche.matches_key(key) {
            Some(&self.marche)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StationsTablesConfig {
    pub emilia_romagna: String,
    pub marche: String,
}

impl StationsTablesConfig {
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            emilia_romagna: require_env(EMILIA_ROMAGNA_STATIONS_TABLE_NAME_ENV)?,
            marche: require_env(MARCHE_STATIONS_TABLE_NAME_ENV)?,
        })
    }
}

pub fn stations_scan_page_size_from_env() -> i32 {
    let parsed = std::env::var(STATIONS_SCAN_PAGE_SIZE_ENV)
        .ok()
        .and_then(|value| value.trim().parse::<i32>().ok());
    parsed
        .unwrap_or(DEFAULT_SCAN_PAGE_SIZE)
        .clamp(1, MAX_SCAN_PAGE_SIZE)
}

pub fn require_env(name: &str) -> Result<String, String> {
    env_var(name).ok_or_else(|| format!("Missing env var: {name}"))
}

pub fn env_var(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub const ALERTS_TABLE_NAME_ENV_NAME: &str = ALERTS_TABLE_NAME_ENV;
pub const CHATS_TABLE_NAME_ENV_NAME: &str = CHATS_TABLE_NAME_ENV;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_config_matches_key_ignoring_case() {
        let region = RegionConfig {
            key: "marche".to_string(),
            label: "Marche".to_string(),
            table_name: "Marche-Stations".to_string(),
        };

        assert!(region.matches_key("Marche"));
        assert!(!region.matches_key("emilia-romagna"));
    }

    #[test]
    fn regions_config_finds_known_key() {
        let config = RegionsConfig {
            emilia_romagna: RegionConfig {
                key: "emilia-romagna".to_string(),
                label: "Emilia-Romagna".to_string(),
                table_name: "EmiliaRomagna-Stations".to_string(),
            },
            marche: RegionConfig {
                key: "marche".to_string(),
                label: "Marche".to_string(),
                table_name: "Marche-Stations".to_string(),
            },
        };

        assert_eq!(
            config
                .find_by_key("MARCHE")
                .map(|region| region.label.as_str()),
            Some("Marche")
        );
        assert!(config.find_by_key("unknown").is_none());
    }
}
