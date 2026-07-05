use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const CONFIG_DIR: &str = "boulder-relay";
const CONFIG_FILE: &str = "settings.conf";

#[derive(Debug, Clone, Default)]
pub struct ServerAccount {
    pub nick: String,
    pub password: String,
    pub service: String,
    pub auth_method: String, // "nickserv", "sasl_plain", "sasl_external"
}

#[derive(Debug, Clone)]
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
    pub accounts: std::collections::HashMap<String, ServerAccount>,
    pub auth_method: String,
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
            accounts: std::collections::HashMap::new(),
            auth_method: "nickserv".to_string(),
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = config_path();
        let Ok(content) = fs::read_to_string(&path) else {
            return Self::default();
        };

        let mut values = parse_key_values(&content);
        let mut settings = Self::default();

        if let Some(nickname) = values.remove("nickname") {
            settings.nickname = nickname;
        }
        if let Some(server) = values.remove("server") {
            settings.server = server;
        }
        if let Some(password) = values.remove("password") {
            settings.password = password;
        }
        if let Some(favorites) = values.remove("favorites") {
            settings.favorites = favorites
                .split('|')
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect();
        }
        if let Some(extra_channels) = values.remove("extra_channels") {
            settings.extra_channels = extra_channels
                .split('|')
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect();
        }
        if let Some(last_channel) = values.remove("last_channel") {
            settings.last_channel = last_channel;
        }
        if let Some(notifications_enabled) = values.remove("notifications_enabled") {
            settings.notifications_enabled = parse_bool(&notifications_enabled, true);
        }
        if let Some(background_on_close) = values.remove("background_on_close") {
            settings.background_on_close = parse_bool(&background_on_close, true);
        }
        if let Some(nick_colors) = values.remove("nick_colors_enabled") {
            settings.nick_colors_enabled = parse_bool(&nick_colors, true);
        }
        if let Some(ts_format) = values.remove("timestamp_format") {
            settings.timestamp_format = ts_format;
        }
        if let Some(service) = values.remove("account_service") {
            if !service.is_empty() {
                settings.account_service = service;
            }
        }
        if let Some(method) = values.remove("auth_method") {
            if !method.is_empty() {
                settings.auth_method = method;
            }
        }
        // Accounts are stored as: server\0nick\0password\0service\0method entries joined by \n
        if let Some(accounts_str) = values.remove("accounts") {
            for entry in accounts_str.split('\n') {
                if entry.is_empty() {
                    continue;
                }
                let parts: Vec<&str> = entry.splitn(5, '\0').collect();
                if parts.len() == 5 {
                    let acc = ServerAccount {
                        nick: parts[1].to_string(),
                        password: parts[2].to_string(),
                        service: parts[3].to_string(),
                        auth_method: parts[4].to_string(),
                    };
                    settings.accounts.insert(parts[0].to_string(), acc);
                }
            }
        }

        settings
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let favorites = self.favorites.join("|");
        let extra_channels = self.extra_channels.join("|");

        // Accounts encoded as null-separated fields, one entry per line.
        // Null bytes are safe here since server/nick/service cannot contain them.
        let accounts_str = self
            .accounts
            .iter()
            .map(|(s, a)| {
                format!(
                    "{}\0{}\0{}\0{}\0{}",
                    s, a.nick, a.password, a.service, a.auth_method
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let body = format!(
            "nickname={}\nserver={}\npassword={}\nfavorites={}\nextra_channels={}\nlast_channel={}\nnotifications_enabled={}\nbackground_on_close={}\nnick_colors_enabled={}\ntimestamp_format={}\naccount_service={}\nauth_method={}\naccounts={}\n",
            escape_value(&self.nickname),
            escape_value(&self.server),
            escape_value(&self.password),
            escape_value(&favorites),
            escape_value(&extra_channels),
            escape_value(&self.last_channel),
            if self.notifications_enabled { "true" } else { "false" },
            if self.background_on_close { "true" } else { "false" },
            if self.nick_colors_enabled { "true" } else { "false" },
            escape_value(&self.timestamp_format),
            escape_value(&self.account_service),
            escape_value(&self.auth_method),
            escape_value(&accounts_str),
        );

        fs::write(&path, body)?;

        // Restrict config file to owner read/write only (contains passwords).
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

fn parse_key_values(content: &str) -> HashMap<String, String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), unescape_value(value.trim())))
        })
        .collect()
}

fn parse_bool(value: &str, default: bool) -> bool {
    match value.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => default,
    }
}

fn escape_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\n', "\\n")
}

fn unescape_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_escaped_values() {
        let settings = Settings {
            nickname: String::from("test\\nick"),
            password: String::from("sec\\ret"),
            ..Settings::default()
        };
        let encoded = format!(
            "nickname={}\npassword={}\n",
            escape_value(&settings.nickname),
            escape_value(&settings.password),
        );
        let parsed = parse_key_values(&encoded);
        assert_eq!(parsed["nickname"], "test\\nick");
        assert_eq!(parsed["password"], "sec\\ret");
    }

    #[test]
    fn accounts_round_trip_special_chars() {
        let mut settings = Settings::default();
        settings.accounts.insert(
            "irc.libera.chat".to_string(),
            ServerAccount {
                nick: "testnick".to_string(),
                password: "p@ss|word,with,commas".to_string(),
                service: "NickServ".to_string(),
                auth_method: "nickserv".to_string(),
            },
        );

        let accounts_str = settings
            .accounts
            .iter()
            .map(|(s, a)| {
                format!("{}\0{}\0{}\0{}\0{}", s, a.nick, a.password, a.service, a.auth_method)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let encoded = format!("accounts={}\n", escape_value(&accounts_str));
        let parsed = parse_key_values(&encoded);

        let raw = parsed["accounts"].as_str();
        for entry in raw.split('\n') {
            if entry.is_empty() {
                continue;
            }
            let parts: Vec<&str> = entry.splitn(5, '\0').collect();
            assert_eq!(parts.len(), 5);
            assert_eq!(parts[2], "p@ss|word,with,commas");
        }
    }
}
