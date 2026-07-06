use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR: &str = "boulder-relay";
const CONFIG_FILE: &str = "settings.toml";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ServerAccount {
    pub nick: String,
    pub password: String,
    pub service: String,
    pub auth_method: String,
}

impl ServerAccount {
    pub fn load_password(_server: &str, _nick: &str) -> String {
        String::new()
    }

    pub fn save_password(_server: &str, _nick: &str, _password: &str) {
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub nickname: String,
    pub server: String,
    pub password: String,
    pub favorites: Vec<String>,
    pub extra_channels: Vec<String>,
    pub last_channel: String,
    pub notifications_enabled: bool,
    pub background_on_close: bool,
    pub nick_colors_enabled: bool,
    pub timestamp_format: String,
    pub account_service: String,
    pub auth_method: String,
    pub accounts: HashMap<String, ServerAccount>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            nickname: String::from("SisyphusCode"),
            server: String::from("irc.libera.chat"),
            password: String::new(),
            favorites: vec![String::from("Server")],
            extra_channels: Vec::new(),
            last_channel: String::from("Server"),
            notifications_enabled: true,
            background_on_close: true,
            nick_colors_enabled: true,
            timestamp_format: "%H:%M".to_string(),
            account_service: String::from("NickServ"),
            auth_method: String::from("nickserv"),
            accounts: HashMap::new(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = config_path();
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        toml::from_str(&content).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let body = toml::to_string_pretty(&self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        fs::write(&path, body)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
        }

        Ok(())
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|home| PathBuf::from(home).join(".config"))
                .unwrap_or_else(|_| PathBuf::from(".config"))
        });
    base.join(CONFIG_DIR).join(CONFIG_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_settings_toml() {
        let settings = Settings {
            nickname: String::from("testnick"),
            server: String::from("irc.libera.chat"),
            ..Settings::default()
        };
        let serialized = toml::to_string_pretty(&settings).unwrap();
        let parsed: Settings = toml::from_str(&serialized).unwrap();
        assert_eq!(parsed.nickname, "testnick");
        assert_eq!(parsed.server, "irc.libera.chat");
    }

    #[test]
    fn default_settings_are_sane() {
        let s = Settings::default();
        assert_eq!(s.nickname, "SisyphusCode");
        assert!(s.notifications_enabled);
        assert!(s.nick_colors_enabled);
        assert_eq!(s.timestamp_format, "%H:%M");
    }
}