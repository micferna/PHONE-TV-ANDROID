use std::io::{BufRead, Write};
use std::path::PathBuf;

use crate::types::TvChannel;

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub dark_mode: bool,
    pub replay_ratio: f32,
    pub window_size: (f32, f32),
    #[serde(default)]
    pub openrouter_api_key: String,
    #[serde(default = "default_llm_model")]
    pub llm_model: String,
}

fn default_llm_model() -> String {
    "anthropic/claude-sonnet-4".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            dark_mode: true,
            replay_ratio: 12.0,
            window_size: (1000.0, 800.0),
            openrouter_api_key: String::new(),
            llm_model: default_llm_model(),
        }
    }
}

pub fn config_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("phone-tv");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn settings_path() -> PathBuf {
    config_dir().join("settings.toml")
}

pub fn load_settings() -> Settings {
    std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: &Settings) {
    if let Ok(s) = toml::to_string_pretty(settings) {
        let _ = std::fs::write(settings_path(), s);
    }
}

fn channels_path() -> PathBuf {
    config_dir().join("channels.txt")
}

pub fn default_channels() -> Vec<TvChannel> {
    vec![
        TvChannel {
            name: "TF1".into(),
            number: 1,
        },
        TvChannel {
            name: "France 2".into(),
            number: 2,
        },
        TvChannel {
            name: "France 3".into(),
            number: 3,
        },
        TvChannel {
            name: "France 4".into(),
            number: 4,
        },
        TvChannel {
            name: "France 5".into(),
            number: 5,
        },
        TvChannel {
            name: "M6".into(),
            number: 6,
        },
        TvChannel {
            name: "Arte".into(),
            number: 7,
        },
        TvChannel {
            name: "LCP".into(),
            number: 8,
        },
        TvChannel {
            name: "W9".into(),
            number: 9,
        },
        TvChannel {
            name: "TMC".into(),
            number: 10,
        },
        TvChannel {
            name: "TFX".into(),
            number: 11,
        },
        TvChannel {
            name: "Gulli".into(),
            number: 12,
        },
        TvChannel {
            name: "BFMTV".into(),
            number: 13,
        },
        TvChannel {
            name: "CNEWS".into(),
            number: 14,
        },
        TvChannel {
            name: "LCI".into(),
            number: 15,
        },
        TvChannel {
            name: "FranceInfo".into(),
            number: 16,
        },
        TvChannel {
            name: "CSTAR".into(),
            number: 17,
        },
        TvChannel {
            name: "CMI TV".into(),
            number: 18,
        },
        TvChannel {
            name: "TF1 SF".into(),
            number: 20,
        },
        TvChannel {
            name: "L'Équipe".into(),
            number: 21,
        },
        TvChannel {
            name: "6ter".into(),
            number: 22,
        },
        TvChannel {
            name: "RMC Story".into(),
            number: 23,
        },
        TvChannel {
            name: "RMC Déc".into(),
            number: 24,
        },
        TvChannel {
            name: "Chérie 25".into(),
            number: 25,
        },
    ]
}

pub fn load_channels() -> Vec<TvChannel> {
    let path = channels_path();
    if let Ok(file) = std::fs::File::open(&path) {
        let reader = std::io::BufReader::new(file);
        let mut channels = Vec::new();
        for line in reader.lines().map_while(Result::ok) {
            let line = line.trim().to_string();
            if let Some((num_str, name)) = line.split_once(':') {
                if let Ok(number) = num_str.parse::<u32>() {
                    channels.push(TvChannel {
                        name: name.to_string(),
                        number,
                    });
                }
            }
        }
        if !channels.is_empty() {
            return channels;
        }
    }
    let channels = default_channels();
    save_channels(&channels);
    channels
}

pub fn save_channels(channels: &[TvChannel]) {
    let path = channels_path();
    if let Ok(mut file) = std::fs::File::create(&path) {
        for ch in channels {
            let _ = writeln!(file, "{}:{}", ch.number, ch.name);
        }
    }
}

pub fn blacklist_path() -> PathBuf {
    config_dir().join("blacklist.txt")
}

pub fn load_blacklist() -> Vec<String> {
    std::fs::read_to_string(blacklist_path())
        .unwrap_or_default()
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn save_blacklist(blacklist: &[String]) {
    let _ = std::fs::write(blacklist_path(), blacklist.join("\n"));
}
