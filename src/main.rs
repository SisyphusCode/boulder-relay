mod channels;
mod config;
mod notify;
mod theme;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use config::Settings;
use futures::prelude::*;
use adw;
use gtk::glib::{self, DateTime};
use gtk::prelude::*;
use adw::prelude::*;
use irc::client::prelude::*;
use notify::NotifyKind;
use relm4::{gtk, ComponentParts, ComponentSender, RelmApp, RelmWidgetExt, SimpleComponent};
use std::collections::HashMap;
use std::thread;

const DEFAULT_SERVER: &str = "irc.libera.chat";
const DEFAULT_NICKNAME: &str = "SisyphusCode";
const DEFAULT_PORT: u16 = 6697;
const SERVER_TAB: &str = "Server";
const DEFAULT_ACCOUNT_SERVICE: &str = "NickServ";

/// Gruvbox-inspired palette for per-nickname colors.
const NICK_COLORS: [&str; 8] = [
    "#fabd2f", // yellow
    "#b8bb26", // green
    "#83a598", // blue
    "#d3869b", // purple
    "#fe8019", // orange
    "#8ec07c", // aqua
    "#fb4934", // red
    "#d79921", // bright yellow
];

#[derive(Copy, Clone, PartialEq, Eq)]
enum LineStyle {
    Normal,
    SelfMsg,
    System,
    Mention,
}

#[derive(Clone)]
struct ChatLine {
    timestamp: String,
    user: Option<String>,
    body: String,
    style: LineStyle,
}

const HELP_TEXT: &str = "\
Commands: /join #chan [,#chan2], /j #chan, /part [#chan], /msg nick text, /me text,\n\
  /list, /clear, /nick name, /whois nick, /away [message], /back, /topic [text], /help\n\
Join box: #channel (or nick for DM). Comma-separate for multi-join: #foo,#bar\n\
Sidebar filter searches your joined list.\n\
\"Register new account\u2026\" for NickServ registration + email verification.\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionState {
    Offline,
    Connecting,
    Connected,
}

#[derive(Debug, Clone)]
pub enum AppInput {
    UpdateNickname(String),
    UpdateServer(String),
    UpdatePassword(String),
    Connect,
    Disconnect,
    /// Intentional user-initiated disconnect (suppresses auto-reconnect)
    UserDisconnect,
    NetworkStatus(String),
    NetworkConnected(irc::client::Sender),
    SelectChannel(String),
    JoinChannel(String),
    PartChannel(String),
    ClearChannel(String),
    ToggleFavorite(String),
    ToggleMute { channel: String, user: String },
    IgnoreUser(String),
    UnignoreUser(String),
    ReceiveMessage { channel: String, user: String, body: String },
    ReceiveServerMessage(String),
    BatchAddUsers { channel: String, users: Vec<String> },
    UserJoined { channel: String, user: String },
    UserLeft { channel: String, user: String },
    UserQuit { user: String },
    UserRenamed { old: String, new: String },
    SendMessage(String),
    JoinEntry(String),
    UpdateNotificationsEnabled(bool),
    UpdateBackgroundOnClose(bool),
    UpdateChannelFilter(String),
    BrowseChannels,
    ChannelListEntry { name: String, users: u32, topic: String },
    ChannelListEnd,
    ChannelTopic { channel: String, topic: String },
    OpenPreferences,
    UpdateNickColorsEnabled(bool),
    UpdateTimestampFormat(String),
    OpenRegisterDialog,
    SubmitRegistration { nick: String, password: String, email: String },
    SubmitVerification { nick: String, code: String },
    UpdateAccountService(String),
    UpdateAuthMethod(String),
    SendRawPrivmsg { target: String, msg: String },
    AddServer(String),
    SwitchServer(String),
    OpenAccountManager,
    OpenLogViewer,
    MarkChannelRead(String),
    Quit,
    SaveSettings,
}

struct AppModel {
    // Multi-server
    servers: Vec<String>,
    current_server: String,
    senders: HashMap<String, Option<irc::client::Sender>>,
    server_states: HashMap<String, ConnectionState>,
    // Current connection
    connection: ConnectionState,
    /// True when the user explicitly clicked Disconnect — suppresses auto-reconnect.
    user_disconnected: bool,
    status: String,
    active_channel: String,
    channels: Vec<String>,
    favorite_channels: Vec<String>,
    muted_users: HashMap<String, Vec<String>>,
    ignored_users: std::collections::HashSet<String>,
    /// Per-channel unread message count (reset on SelectChannel)
    unread_counts: HashMap<String, u32>,
    /// Per-channel pending mention count
    mention_counts: HashMap<String, u32>,
    chat_histories: HashMap<String, Vec<ChatLine>>,
    channel_users: HashMap<String, Vec<String>>,
    irc_sender: Option<irc::client::Sender>,
    nickname: String,
    server: String,
    password: String,
    channel_box: gtk::ListBox,
    user_box: gtk::ListBox,
    chat_view: gtk::TextView,
    window: adw::Window,
    notifications_enabled: bool,
    background_on_close: bool,
    channel_filter: String,
    channel_list_results: Vec<(String, u32, String)>,
    channel_topics: HashMap<String, String>,
    nick_colors_enabled: bool,
    timestamp_format: String,
    account_service: String,
    auth_method: String,
    accounts: HashMap<String, config::ServerAccount>,
    pending_register_email: Option<String>,
}

impl AppModel {
    fn normalized_nick(user: &str) -> String {
        user.trim_start_matches(['@', '+', '%', '~', '&']).to_string()
    }

    fn nick_color_index(nick: &str) -> usize {
        let clean = Self::normalized_nick(nick);
        let hash = clean
            .bytes()
            .fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
        (hash as usize) % NICK_COLORS.len()
    }

    fn nick_color_tag(nick: &str) -> String {
        format!("nick-{}", Self::nick_color_index(nick))
    }

    fn is_muted(&self, channel: &str, user: &str) -> bool {
        let clean = Self::normalized_nick(user);
        self.muted_users
            .get(channel)
            .map(|users| users.iter().any(|u| u == &clean))
            .unwrap_or(false)
    }

    fn timestamp_prefix(&self) -> String {
        DateTime::now_local()
            .map(|dt| {
                format!(
                    "[{}] ",
                    dt.format(&self.timestamp_format).unwrap_or_default()
                )
            })
            .unwrap_or_else(|_| String::from("[??:??] "))
    }

    fn extra_channels(&self) -> Vec<String> {
        self.channels
            .iter()
            .filter(|c| **c != SERVER_TAB)
            .cloned()
            .collect()
    }

    fn settings_snapshot(&self) -> Settings {
        let mut snapshot = Settings {
            nickname: self.nickname.clone(),
            server: self.server.clone(),
            password: self.password.clone(),
            favorites: self.favorite_channels.clone(),
            extra_channels: self.extra_channels(),
            last_channel: self.active_channel.clone(),
            notifications_enabled: self.notifications_enabled,
            background_on_close: self.background_on_close,
            nick_colors_enabled: self.nick_colors_enabled,
            timestamp_format: self.timestamp_format.clone(),
            account_service: self.account_service.clone(),
            auth_method: self.auth_method.clone(),
            accounts: self.accounts.clone(),
        };
        snapshot.accounts.insert(
            self.server.clone(),
            config::ServerAccount {
                nick: self.nickname.clone(),
                password: self.password.clone(),
                service: self.account_service.clone(),
                auth_method: self.auth_method.clone(),
            },
        );
        snapshot
    }

    fn sync_account_for_server(&mut self, server: &str) {
        self.accounts.insert(
            server.to_string(),
            config::ServerAccount {
                nick: self.nickname.clone(),
                password: self.password.clone(),
                service: self.account_service.clone(),
                auth_method: self.auth_method.clone(),
            },
        );
    }

    fn load_account_for_server(&mut self, server: &str) {
        if let Some(acc) = self.accounts.get(server) {
            if !acc.nick.is_empty() {
                self.nickname = acc.nick.clone();
            }
            if !acc.password.is_empty() {
                self.password = acc.password.clone();
            }
            if !acc.service.is_empty() {
                self.account_service = acc.service.clone();
            }
            if !acc.auth_method.is_empty() {
                self.auth_method = acc.auth_method.clone();
            }
        }
    }

    fn should_notify(&self, channel: &str, user: &str, style: LineStyle) -> bool {
        if !self.notifications_enabled || user == "System" || style == LineStyle::SelfMsg {
            return false;
        }
        let hidden = !self.window.is_visible();
        let inactive = channel != self.active_channel;
        let dm = !channels::is_channel_target(channel);
        let mention = style == LineStyle::Mention;
        hidden || inactive || dm || mention
    }

    fn notify_kind(&self, channel: &str, style: LineStyle) -> NotifyKind {
        if !channels::is_channel_target(channel) {
            NotifyKind::DirectMessage
        } else if style == LineStyle::Mention {
            NotifyKind::Mention
        } else {
            NotifyKind::Activity
        }
    }

    fn send_irc_join(&self, target: &str) {
        if let Some(tx) = &self.irc_sender {
            if channels::is_channel_target(target) {
                let _ = tx.send_join(target);
            }
        }
    }

    fn style_tag(style: LineStyle) -> &'static str {
        match style {
            LineStyle::Normal => "normal",
            LineStyle::SelfMsg => "self-msg",
            LineStyle::System => "system",
            LineStyle::Mention => "mention",
        }
    }

    fn setup_chat_tags(view: &gtk::TextView) {
        let buffer = view.buffer();
        let table = buffer.tag_table();
        for (name, fg, bg) in [
            ("normal", "#ebdbb2", None),
            ("self-msg", "#10B981", None),
            ("system", "#928374", None),
            ("mention", "#fe8019", Some("#3c3836")),
        ] {
            let tag = gtk::TextTag::new(Some(name));
            tag.set_foreground(Some(fg));
            if let Some(bg) = bg {
                tag.set_background(Some(bg));
            }
            table.add(&tag);
        }
        for (i, &color) in NICK_COLORS.iter().enumerate() {
            let 