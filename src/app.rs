use crate::channels;
use crate::config::Settings;
use crate::discord::{
    bridge_discord_events, ChannelRegistry as DiscordChannelRegistry, DiscordClient, DiscordEvent,
};
use crate::irc::commands::{self, SlashCommand};
use crate::irc::connection::IrcConnection;
use crate::matrix::client::{MatrixClient, MatrixEvent};
use crate::matrix::rooms::RoomRegistry;
use crate::matrix::sync::bridge_matrix_events;
use crate::notify::{self, NotifyKind};
use crate::runtime;
use crate::theme;
use crate::ui::{chat_view, dialogs};
use adw;
use adw::prelude::*;
use gtk::glib::{self, DateTime};
use relm4::{gtk, ComponentParts, ComponentSender, RelmWidgetExt, SimpleComponent};
use std::collections::HashMap;
use tokio::sync::mpsc;

pub const DEFAULT_SERVER: &str = "irc.libera.chat";
pub const DEFAULT_NICKNAME: &str = "SisyphusAeolides";
pub const DEFAULT_PORT: u16 = 6697;
pub const SERVER_TAB: &str = "Server";
pub const DEFAULT_ACCOUNT_SERVICE: &str = "NickServ";

/// Which protocol owns a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protocol {
    Irc,
    Matrix { room_id: String },
    Discord { channel_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolFilter {
    All,
    Irc,
    Matrix,
    Discord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Offline,
    Connecting,
    Connected,
}

#[derive(Debug, Clone)]
pub struct ChatLine {
    pub timestamp: String,
    pub user: String,
    pub body: String,
    pub style: chat_view::LineStyle,
}

#[derive(Clone)]
pub enum AppInput {
    // ── IRC ──────────────────────────────────────────────────────────
    UpdateNickname(String),
    UpdateServer(String),
    UpdatePassword(String),
    Connect,
    Disconnect,
    UserDisconnect,
    NetworkStatus(String),
    NetworkConnected(irc::client::Sender),
    SelectChannel(String),
    JoinChannel(String),
    PartChannel(String),
    ClearChannel(String),
    ToggleFavorite(String),
    ToggleMute {
        channel: String,
        user: String,
    },
    IgnoreUser(String),
    UnignoreUser(String),
    ReceiveMessage {
        channel: String,
        user: String,
        body: String,
        protocol: Protocol,
    },
    ReceiveServerMessage(String),
    BatchAddUsers {
        channel: String,
        users: Vec<String>,
    },
    UserJoined {
        channel: String,
        user: String,
    },
    UserLeft {
        channel: String,
        user: String,
    },
    UserQuit {
        user: String,
    },
    UserRenamed {
        old: String,
        new: String,
    },
    SendMessage(String),
    JoinEntry(String),
    UpdateNotificationsEnabled(bool),
    UpdateBackgroundOnClose(bool),
    UpdateChannelFilter(String),
    SetProtocolFilter(ProtocolFilter),
    BrowseChannels,
    ChannelListEntry {
        name: String,
        users: u32,
        topic: String,
    },
    ChannelListEnd,
    ChannelTopic {
        channel: String,
        topic: String,
    },
    OpenPreferences,
    UpdateNickColorsEnabled(bool),
    UpdateTimestampFormat(String),
    OpenRegisterDialog,
    SubmitRegistration {
        nick: String,
        password: String,
        email: String,
    },
    SubmitVerification {
        nick: String,
        code: String,
    },
    UpdateAccountService(String),
    UpdateAuthMethod(String),
    SendRawPrivmsg {
        target: String,
        msg: String,
    },
    AddServer(String),
    SwitchServer(String),
    OpenAccountManager,
    OpenIrcLogin,
    OpenLogViewer,
    MarkChannelRead(String),
    Quit,
    SaveSettings,
    ComposerSendClicked,
    // ── Matrix ───────────────────────────────────────────────────────
    OpenMatrixLogin,
    OpenMatrixJoin,
    MatrixLogin {
        homeserver: String,
        username: String,
        password: String,
        remember: bool,
    },
    ClearMatrixAccount,
    MatrixConnected {
        user_id: String,
    },
    MatrixStoreClient(MatrixClient),
    MatrixRoomJoined {
        room_id: String,
        room_name: String,
    },
    MatrixRoomLeft {
        room_id: String,
    },
    MatrixJoinRoom(String),
    MatrixSendMessage {
        room_id: String,
        body: String,
    },
    // ── Discord (bot accounts only) ────────────────────────────────────
    OpenDiscordLogin,
    DiscordButtonClicked,
    DiscordLogin {
        bot_token: String,
        remember: bool,
    },
    ClearDiscordAccount,
    DiscordStoreClient(DiscordClient),
    DiscordConnected {
        user_id: String,
    },
    DiscordChannelDiscovered {
        channel_id: String,
        display_name: String,
    },
    DiscordMessage {
        channel_id: String,
        dm_display_name: Option<String>,
        sender: String,
        body: String,
    },
    DiscordChannelDeleted {
        channel_id: String,
    },
    DiscordError(String),
    DiscordDisconnected,
    DisconnectDiscord,
}

impl std::fmt::Debug for AppInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Opaque: AppInput carries non-Debug MatrixClient on one variant.
        write!(f, "AppInput")
    }
}

pub struct AppModel {
    // IRC state
    pub servers: Vec<String>,
    pub current_server: String,
    pub senders: HashMap<String, Option<irc::client::Sender>>,
    pub server_states: HashMap<String, ConnectionState>,
    pub connection: ConnectionState,
    pub user_disconnected: bool,
    pub status: String,
    pub active_channel: String,
    pub channels: Vec<String>,
    pub favorite_channels: Vec<String>,
    pub muted_users: HashMap<String, Vec<String>>,
    pub ignored_users: std::collections::HashSet<String>,
    pub unread_counts: HashMap<String, u32>,
    pub mention_counts: HashMap<String, u32>,
    pub chat_histories: HashMap<String, Vec<ChatLine>>,
    pub channel_users: HashMap<String, Vec<String>>,
    pub irc_sender: Option<irc::client::Sender>,
    pub nickname: String,
    pub server: String,
    pub password: String,
    pub irc_port: u16,
    pub irc_use_tls: bool,
    pub notifications_enabled: bool,
    pub background_on_close: bool,
    pub channel_filter: String,
    pub protocol_filter: ProtocolFilter,
    pub channel_list_results: Vec<(String, u32, String)>,
    pub channel_topics: HashMap<String, String>,
    pub nick_colors_enabled: bool,
    pub timestamp_format: String,
    pub account_service: String,
    pub auth_method: String,
    pub accounts: HashMap<String, crate::config::ServerAccount>,
    pub pending_register_email: Option<String>,
    pub matrix_account: crate::config::MatrixAccount,
    pub discord_account: crate::config::DiscordAccount,
    // Matrix state
    pub matrix_client: Option<MatrixClient>,
    pub matrix_user_id: Option<String>,
    pub matrix_rooms: RoomRegistry,
    pub matrix_connected: bool,
    // Discord state
    pub discord_client: Option<DiscordClient>,
    pub discord_user_id: Option<String>,
    pub discord_channels: DiscordChannelRegistry,
    pub discord_connection: ConnectionState,
    pub discord_status: String,
    // GTK widget refs
    pub channel_box: gtk::ListBox,
    pub user_box: gtk::ListBox,
    pub chat_view: gtk::TextView,
    pub composer_entry: gtk::Entry,
    pub window: gtk::Window,
}

/// True when a NetworkStatus string means the IRC session is dead and Connect
/// may run again (must cover pre-connect failures, not only "Connection failed").
pub fn is_terminal_irc_status(s: &str) -> bool {
    s == "Disconnected"
        || s.starts_with("Connection failed")
        || s.starts_with("NickServ auth failed")
        || s.starts_with("Auth failed")
        || s.starts_with("IRC error")
}

impl AppModel {
    pub fn normalized_nick(user: &str) -> String {
        user.trim_start_matches(['@', '+', '%', '~', '&'])
            .to_string()
    }

    pub fn timestamp_prefix(&self) -> String {
        DateTime::now_local()
            .map(|dt| {
                format!(
                    "[{}] ",
                    dt.format(&self.timestamp_format).unwrap_or_default()
                )
            })
            .unwrap_or_else(|_| String::from("[??:??] "))
    }

    pub fn extra_channels(&self) -> Vec<String> {
        self.channels
            .iter()
            .filter(|c| **c != SERVER_TAB)
            .cloned()
            .collect()
    }

    pub fn settings_snapshot(&self) -> Settings {
        let mut snapshot = Settings {
            nickname: self.nickname.clone(),
            server: self.server.clone(),
            password: self.password.clone(),
            irc_port: self.irc_port,
            irc_use_tls: self.irc_use_tls,
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
            matrix: self.matrix_account.clone(),
            discord: self.discord_account.clone(),
        };
        snapshot.accounts.insert(
            self.server.clone(),
            crate::config::ServerAccount {
                nick: self.nickname.clone(),
                password: self.password.clone(),
                service: self.account_service.clone(),
                auth_method: self.auth_method.clone(),
            },
        );
        snapshot
    }

    pub fn persist_settings(&self) {
        if let Err(e) = self.settings_snapshot().save() {
            eprintln!("Failed to save settings: {e}");
        }
    }

    pub fn sync_account_for_server(&mut self, server: &str) {
        self.accounts.insert(
            server.to_string(),
            crate::config::ServerAccount {
                nick: self.nickname.clone(),
                password: self.password.clone(),
                service: self.account_service.clone(),
                auth_method: self.auth_method.clone(),
            },
        );
    }

    pub fn load_account_for_server(&mut self, server: &str) {
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

    pub fn message_style(&self, user: &str, body: &str) -> chat_view::LineStyle {
        if user == "System" {
            return chat_view::LineStyle::System;
        }
        let clean = Self::normalized_nick(user);
        if clean.eq_ignore_ascii_case(&self.nickname) {
            return chat_view::LineStyle::SelfMsg;
        }
        if body.contains(&self.nickname) {
            return chat_view::LineStyle::Mention;
        }
        chat_view::LineStyle::Normal
    }

    pub fn should_notify(&self, channel: &str, user: &str, style: chat_view::LineStyle) -> bool {
        if !self.notifications_enabled || user == "System" || style == chat_view::LineStyle::SelfMsg
        {
            return false;
        }
        let hidden = !self.window.is_visible();
        let inactive = channel != self.active_channel;
        let dm = !channels::is_channel_target(channel);
        let mention = style == chat_view::LineStyle::Mention;
        hidden || inactive || dm || mention
    }

    pub fn notify_kind(&self, channel: &str, style: chat_view::LineStyle) -> NotifyKind {
        if !channels::is_channel_target(channel) {
            NotifyKind::DirectMessage
        } else if style == chat_view::LineStyle::Mention {
            NotifyKind::Mention
        } else {
            NotifyKind::Activity
        }
    }

    pub fn append_message(
        &mut self,
        channel: &str,
        user: &str,
        body: &str,
        style: chat_view::LineStyle,
    ) {
        let ts = self.timestamp_prefix();
        let history = self.chat_histories.entry(channel.to_string()).or_default();
        history.push(ChatLine {
            timestamp: ts.clone(),
            user: user.to_string(),
            body: body.to_string(),
            style,
        });
        if self.active_channel == channel {
            chat_view::append_bubble(
                &self.chat_view,
                &ts,
                user,
                body,
                style,
                self.nick_colors_enabled,
            );
        }
    }

    /// Send plain text to the active IRC channel or Matrix room (fail-closed on error).
    fn send_plain_message(&mut self, sender: &ComponentSender<Self>, text: &str) {
        if self.active_channel == SERVER_TAB {
            self.append_message(
                SERVER_TAB,
                "System",
                "Select a channel first.",
                chat_view::LineStyle::System,
            );
            return;
        }
        // Matrix room send
        if let Some(matrix_room_id) = self
            .matrix_rooms
            .find_by_display_name(&self.active_channel)
            .map(|r| r.room_id.clone())
        {
            let body = text.to_string();
            let nick = self
                .matrix_user_id
                .clone()
                .unwrap_or_else(|| self.nickname.clone());
            let client = self.matrix_client.clone();
            let rid = matrix_room_id.clone();
            let ch = self.active_channel.clone();
            let s = sender.clone();
            self.append_message(&ch, &nick, &body, chat_view::LineStyle::SelfMsg);
            runtime::spawn(async move {
                if let Some(c) = client {
                    if let Err(e) = c.send_message(&rid, &body).await {
                        s.input(AppInput::ReceiveServerMessage(format!(
                            "[Matrix send failed]: {e}"
                        )));
                    }
                } else {
                    s.input(AppInput::ReceiveServerMessage(
                        "[Matrix]: not connected.".into(),
                    ));
                }
            });
            return;
        }
        // Discord channel or DM send.
        if let Some(discord_channel_id) = self
            .discord_channels
            .find_by_display_name(&self.active_channel)
            .map(|channel| channel.channel_id.clone())
        {
            let body = text.to_string();
            let channel = self.active_channel.clone();
            let user = self
                .discord_user_id
                .clone()
                .unwrap_or_else(|| "Discord bot".to_string());
            let client = self.discord_client.clone();
            let s = sender.clone();
            self.append_message(&channel, &user, &body, chat_view::LineStyle::SelfMsg);
            runtime::spawn(async move {
                match client {
                    Some(client) => {
                        if let Err(error) = client.send_message(&discord_channel_id, &body).await {
                            s.input(AppInput::DiscordError(format!(
                                "Discord send failed: {error}"
                            )));
                        }
                    }
                    None => s.input(AppInput::DiscordError(
                        "Discord is not connected.".to_string(),
                    )),
                }
            });
            return;
        }
        // IRC send
        if let Some(tx) = self.irc_sender.clone() {
            match tx.send_privmsg(&self.active_channel, text) {
                Ok(()) => {
                    let ch = self.active_channel.clone();
                    let nick = self.nickname.clone();
                    self.append_message(&ch, &nick, text, chat_view::LineStyle::SelfMsg);
                }
                Err(e) => {
                    let ch = self.active_channel.clone();
                    self.append_message(
                        &ch,
                        "System",
                        &format!("Send failed: {e}"),
                        chat_view::LineStyle::System,
                    );
                }
            }
        } else {
            self.append_message(
                &self.active_channel.clone(),
                "System",
                "Cannot send: not connected.",
                chat_view::LineStyle::System,
            );
        }
    }

    pub fn show_channel_history(&self) {
        let lines: Vec<(String, String, String, chat_view::LineStyle)> = self
            .chat_histories
            .get(&self.active_channel)
            .map(|h| {
                h.iter()
                    .map(|l| (l.timestamp.clone(), l.user.clone(), l.body.clone(), l.style))
                    .collect()
            })
            .unwrap_or_default();
        chat_view::render_history(&self.chat_view, &lines, self.nick_colors_enabled);
    }

    pub fn refresh_channels(&self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.channel_box.first_child() {
            self.channel_box.remove(&child);
        }
        // IRC section
        let filter = self.channel_filter.to_lowercase();
        if matches!(
            self.protocol_filter,
            ProtocolFilter::All | ProtocolFilter::Irc
        ) {
            self.channel_box
                .append(&crate::ui::sidebar::section_header("IRC"));
            let mut irc_channels: Vec<&String> = self
                .channels
                .iter()
                .filter(|c| {
                    if filter.is_empty() {
                        return true;
                    }
                    c.to_lowercase().contains(&filter)
                })
                .collect();
            irc_channels.sort_by(|a, b| {
                let af = self.favorite_channels.contains(a);
                let bf = self.favorite_channels.contains(b);
                match (af, bf) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.to_lowercase().cmp(&b.to_lowercase()),
                }
            });
            for ch in irc_channels {
                let unread = self.unread_counts.get(ch).copied().unwrap_or(0);
                let is_fav = self.favorite_channels.contains(ch);
                let is_active = *ch == self.active_channel;
                let row = crate::ui::sidebar::build_room_row(
                    sender,
                    ch,
                    unread,
                    self.mention_counts.get(ch).copied().unwrap_or(0),
                    is_active,
                    Protocol::Irc,
                    is_fav,
                );
                self.channel_box.append(&row);
            }
        }
        // Matrix section
        let matrix_rooms = self.matrix_rooms.all();
        if matches!(
            self.protocol_filter,
            ProtocolFilter::All | ProtocolFilter::Matrix
        ) && !matrix_rooms.is_empty()
        {
            self.channel_box
                .append(&crate::ui::sidebar::section_header("Matrix"));
            for room in matrix_rooms.into_iter().filter(|room| {
                filter.is_empty() || room.display_name.to_lowercase().contains(&filter)
            }) {
                let is_active = room.display_name == self.active_channel;
                let mentions = self
                    .mention_counts
                    .get(&room.display_name)
                    .copied()
                    .unwrap_or(0);
                let row = crate::ui::sidebar::build_room_row(
                    sender,
                    &room.display_name,
                    room.unread_count,
                    mentions,
                    is_active,
                    Protocol::Matrix {
                        room_id: room.room_id.to_string(),
                    },
                    false,
                );
                self.channel_box.append(&row);
            }
        }
        // Discord section
        let discord_channels = self.discord_channels.all();
        if matches!(
            self.protocol_filter,
            ProtocolFilter::All | ProtocolFilter::Discord
        ) && !discord_channels.is_empty()
        {
            self.channel_box
                .append(&crate::ui::sidebar::section_header("Discord"));
            for channel in discord_channels.into_iter().filter(|channel| {
                filter.is_empty() || channel.display_name.to_lowercase().contains(&filter)
            }) {
                let is_active = channel.display_name == self.active_channel;
                let unread = self
                    .unread_counts
                    .get(&channel.display_name)
                    .copied()
                    .unwrap_or(0);
                let mentions = self
                    .mention_counts
                    .get(&channel.display_name)
                    .copied()
                    .unwrap_or(0);
                let row = crate::ui::sidebar::build_room_row(
                    sender,
                    &channel.display_name,
                    unread,
                    mentions,
                    is_active,
                    Protocol::Discord {
                        channel_id: channel.channel_id.clone(),
                    },
                    false,
                );
                self.channel_box.append(&row);
            }
        }
    }

    pub fn refresh_users(&self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.user_box.first_child() {
            self.user_box.remove(&child);
        }
        if let Some(users) = self.channel_users.get(&self.active_channel) {
            for user in users {
                let clean = Self::normalized_nick(user);
                let muted = self
                    .muted_users
                    .get(&self.active_channel)
                    .map(|v| v.contains(&clean))
                    .unwrap_or(false);
                let hbox = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(4)
                    .build();
                let color = crate::ui::chat_view::NICK_COLORS[chat_view::nick_color_index(user)];
                let btn = gtk::Button::new();
                let lbl = gtk::Label::new(None);
                lbl.set_markup(&format!("<span foreground=\"{}\">{}</span>", color, user));
                btn.set_child(Some(&lbl));
                btn.set_hexpand(true);
                btn.add_css_class("user-btn");
                if muted {
                    btn.add_css_class("muted-user");
                }
                let s1 = sender.clone();
                let u1 = clean.clone();
                btn.connect_clicked(move |_| s1.input(AppInput::JoinChannel(u1.clone())));
                let mute_btn = gtk::Button::with_label(if muted { "🔇" } else { "🔊" });
                mute_btn.add_css_class("mute-btn");
                let s2 = sender.clone();
                let c2 = self.active_channel.clone();
                let u2 = clean.clone();
                mute_btn.connect_clicked(move |_| {
                    s2.input(AppInput::ToggleMute {
                        channel: c2.clone(),
                        user: u2.clone(),
                    })
                });
                hbox.append(&btn);
                hbox.append(&mute_btn);
                let row = gtk::ListBoxRow::new();
                row.set_child(Some(&hbox));
                self.user_box.append(&row);
            }
        } else {
            let row = gtk::ListBoxRow::new();
            row.set_selectable(false);
            row.set_activatable(false);
            let box_ = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(6)
                .margin_top(14)
                .margin_bottom(14)
                .margin_start(12)
                .margin_end(12)
                .build();
            let title = gtk::Label::builder()
                .label("No member list")
                .halign(gtk::Align::Start)
                .build();
            title.add_css_class("empty-title");
            let body = gtk::Label::builder()
                .label("IRC channel names appear here after sync. Matrix and Discord member browsers are not shown in this panel yet.")
                .halign(gtk::Align::Start)
                .wrap(true)
                .build();
            body.add_css_class("empty-body");
            box_.append(&title);
            box_.append(&body);
            row.set_child(Some(&box_));
            self.user_box.append(&row);
        }
    }

    fn send_irc_join(&self, target: &str) {
        if let Some(tx) = &self.irc_sender {
            if channels::is_channel_target(target) {
                let _ = tx.send_join(target);
            }
        }
    }
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppInput;
    type Output = ();

    view! {
        gtk::Window {
            set_default_size: (1280, 760),
            set_size_request: (800, 500),
            set_resizable: true,
            set_decorated: true,
            add_css_class: "boulder-relay",
            set_titlebar: Some(&theme::build_titlebar()),

            connect_close_request[sender] => move |window| {
                sender.input(AppInput::SaveSettings);
                if model.background_on_close && (model.connection == ConnectionState::Connected
                    || model.discord_connection == ConnectionState::Connected) {
                    window.set_visible(false);
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            },

            #[wrap(Some)]
            set_child = &gtk::Paned {
                set_orientation: gtk::Orientation::Horizontal,
                set_position: 328,
                set_hexpand: true,
                set_vexpand: true,
                set_shrink_start_child: false,
                set_shrink_end_child: false,

                // ── Left navigation ─────────────────────────────────
                #[wrap(Some)]
                set_start_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_width_request: 328,
                    set_vexpand: true,
                    add_css_class: "navigation-shell",

                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_width_request: 64,
                        set_vexpand: true,
                        set_spacing: 10,
                        set_margin_top: 12,
                        set_margin_bottom: 12,
                        set_margin_start: 8,
                        set_margin_end: 8,
                        add_css_class: "space-rail",

                        gtk::Label {
                            set_label: "BX",
                            set_tooltip_text: Some("boulderX"),
                            add_css_class: "rail-logo",
                            add_css_class: "rail-button",
                        },
                        gtk::Button {
                            set_label: "IRC",
                            set_tooltip_text: Some("Show IRC rooms"),
                            add_css_class: "rail-button",
                            add_css_class: "rail-irc",
                            connect_clicked => AppInput::SetProtocolFilter(ProtocolFilter::Irc),
                        },
                        gtk::Button {
                            set_label: "MX",
                            set_tooltip_text: Some("Show Matrix rooms"),
                            add_css_class: "rail-button",
                            add_css_class: "rail-matrix",
                            connect_clicked => AppInput::SetProtocolFilter(ProtocolFilter::Matrix),
                        },
                        gtk::Button {
                            #[watch]
                            set_label: match model.discord_connection {
                                ConnectionState::Offline => "DC",
                                ConnectionState::Connecting => "DC…",
                                ConnectionState::Connected => "DC✓",
                            },
                            set_tooltip_text: Some("Show Discord channels"),
                            add_css_class: "rail-button",
                            add_css_class: "rail-discord",
                            connect_clicked => AppInput::SetProtocolFilter(ProtocolFilter::Discord),
                        },
                        gtk::Box {
                            set_vexpand: true,
                        },
                        gtk::Button {
                            set_label: "⚙",
                            set_tooltip_text: Some("Preferences"),
                            add_css_class: "rail-button",
                            connect_clicked => AppInput::OpenPreferences,
                        },
                    },

                    gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_width_request: 264,
                    set_vexpand: true,
                    add_css_class: "sidebar",

                    // App header
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,
                        set_margin_all: 12,
                        add_css_class: "sidebar-header",

                        gtk::Label {
                            set_label: "Home",
                            set_hexpand: true,
                            set_halign: gtk::Align::Start,
                            add_css_class: "app-title",
                        },
                        gtk::Label {
                            set_label: "IRC · Matrix · Discord",
                            set_halign: gtk::Align::End,
                            add_css_class: "sidebar-subtitle",
                        },
                    },

                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,
                        set_margin_start: 12,
                        set_margin_end: 12,
                        set_margin_bottom: 10,
                        add_css_class: "protocol-tabs",

                        gtk::Button {
                            set_label: "All",
                            add_css_class: "tab-button",
                            connect_clicked => AppInput::SetProtocolFilter(ProtocolFilter::All),
                        },
                        gtk::Button {
                            set_label: "IRC",
                            add_css_class: "tab-button",
                            add_css_class: "tab-irc",
                            connect_clicked => AppInput::SetProtocolFilter(ProtocolFilter::Irc),
                        },
                        gtk::Button {
                            set_label: "Matrix",
                            add_css_class: "tab-button",
                            add_css_class: "tab-matrix",
                            connect_clicked => AppInput::SetProtocolFilter(ProtocolFilter::Matrix),
                        },
                        gtk::Button {
                            set_label: "Discord",
                            add_css_class: "tab-button",
                            add_css_class: "tab-discord",
                            connect_clicked => AppInput::SetProtocolFilter(ProtocolFilter::Discord),
                        },
                    },

                    // Welcome / empty-state when offline with no rooms yet
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 8,
                        set_margin_start: 12,
                        set_margin_end: 12,
                        set_margin_bottom: 8,
                        add_css_class: "welcome-panel",
                        #[watch]
                        set_visible: model.connection == ConnectionState::Offline
                            && !model.matrix_connected
                            && model.discord_connection == ConnectionState::Offline
                            && model.channels.iter().all(|c| c == SERVER_TAB),

                        gtk::Label {
                            set_label: "Welcome to boulderX",
                            set_halign: gtk::Align::Start,
                            add_css_class: "welcome-title",
                        },
                        gtk::Label {
                            set_label: "Connect IRC, Matrix, or a Discord bot to get started.",
                            set_halign: gtk::Align::Start,
                            set_wrap: true,
                            add_css_class: "welcome-body",
                        },
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 6,
                            gtk::Button {
                                set_label: "Connect IRC",
                                add_css_class: "suggested-action",
                                connect_clicked => AppInput::OpenIrcLogin,
                            },
                            gtk::Button {
                                set_label: "Matrix sign-in",
                                add_css_class: "suggested-action",
                                connect_clicked => AppInput::OpenMatrixLogin,
                            },
                            gtk::Button {
                                set_label: "Discord bot",
                                add_css_class: "suggested-action",
                                connect_clicked => AppInput::OpenDiscordLogin,
                            },
                        },
                    },

                    // Quick-connect status pill
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 6,
                        set_margin_start: 12,
                        set_margin_end: 12,
                        set_margin_bottom: 4,

                        gtk::Label {
                            #[watch]
                            set_label: &model.status,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            set_hexpand: true,
                            add_css_class: match model.connection {
                                ConnectionState::Connected => "status-connected",
                                ConnectionState::Connecting => "status-connecting",
                                ConnectionState::Offline => "status-offline",
                            },
                        },
                        gtk::Button {
                            set_label: "IRC",
                            add_css_class: "badge-irc",
                            set_sensitive: model.connection == ConnectionState::Offline,
                            set_tooltip_text: Some("Log in / connect to IRC"),
                            connect_clicked => AppInput::OpenIrcLogin,
                        },
                        gtk::Button {
                            set_label: "MX",
                            add_css_class: "badge-matrix",
                            set_tooltip_text: Some("Sign in to Matrix"),
                            connect_clicked => AppInput::OpenMatrixLogin,
                        },
                        gtk::Button {
                            #[watch]
                            set_label: match model.discord_connection {
                                ConnectionState::Offline => "DC",
                                ConnectionState::Connecting => "DC…",
                                ConnectionState::Connected => "DC ✓",
                            },
                            add_css_class: "badge-discord",
                            set_sensitive: model.discord_connection != ConnectionState::Connecting,
                            set_tooltip_text: Some("Connect or disconnect a Discord bot"),
                            connect_clicked => AppInput::DiscordButtonClicked,
                        },
                        gtk::Button {
                            set_label: "Accounts",
                            add_css_class: "flat",
                            set_tooltip_text: Some("Manage IRC, Matrix, and Discord accounts"),
                            connect_clicked => AppInput::OpenAccountManager,
                        },
                    },
                    gtk::Label {
                        #[watch]
                        set_label: &model.discord_status,
                        set_margin_start: 12,
                        set_margin_end: 12,
                        set_margin_bottom: 4,
                        set_halign: gtk::Align::Start,
                        set_ellipsize: gtk::pango::EllipsizeMode::End,
                        add_css_class: match model.discord_connection {
                            ConnectionState::Connected => "status-connected",
                            ConnectionState::Connecting => "status-connecting",
                            ConnectionState::Offline => "status-offline",
                        },
                    },

                    // Search / filter
                    gtk::SearchEntry {
                        set_placeholder_text: Some("Filter rooms…"),
                        set_margin_start: 12,
                        set_margin_end: 12,
                        set_margin_bottom: 6,
                        connect_changed[sender] => move |e| sender.input(AppInput::UpdateChannelFilter(e.text().to_string())),
                    },

                    gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                    // Unified room list
                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        set_vscrollbar_policy: gtk::PolicyType::Automatic,
                        set_hscrollbar_policy: gtk::PolicyType::Never,
                        #[local_ref] channel_box_ref -> gtk::ListBox {
                            set_selection_mode: gtk::SelectionMode::Single,
                        }
                    },

                    gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                    // Bottom sidebar actions
                    gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_spacing: 4,
                        set_margin_all: 8,

                        gtk::Entry {
                            set_placeholder_text: Some("#channel or nick"),
                            connect_activate[sender] => move |e| {
                                let t = e.text().to_string();
                                if !t.is_empty() { e.set_text(""); sender.input(AppInput::JoinEntry(t)); }
                            },
                        },
                        gtk::Button {
                            set_label: "🔍  Browse server channels",
                            add_css_class: "flat",
                            connect_clicked => AppInput::BrowseChannels,
                        },
                        gtk::Button {
                            set_label: "Join Matrix Room…",
                            add_css_class: "flat",
                            connect_clicked => AppInput::OpenMatrixJoin,
                        },
                        gtk::Button {
                            set_label: "View Logs",
                            add_css_class: "flat",
                            connect_clicked => AppInput::OpenLogViewer,
                        },
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 4,
                            gtk::Button {
                                set_label: "Disconnect",
                                add_css_class: "destructive-action",
                                set_hexpand: true,
                                set_sensitive: model.connection == ConnectionState::Connected,
                                connect_clicked => AppInput::UserDisconnect,
                            },
                            gtk::Button {
                                set_label: "Quit",
                                add_css_class: "destructive-action",
                                connect_clicked => AppInput::Quit,
                            },
                        },
                    },
                    },
                },

                // ── Right: chat + users ──────────────────────────────
                #[wrap(Some)]
                set_end_child = &gtk::Paned {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_position: 820,
                    set_hexpand: true,
                    set_vexpand: true,
                    set_shrink_end_child: false,

                    // Chat panel
                    #[wrap(Some)]
                    set_start_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_hexpand: true,
                        set_vexpand: true,
                        add_css_class: "chat-panel",

                        // Channel header bar
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 10,
                            set_margin_start: 16,
                            set_margin_end: 16,
                            set_margin_top: 10,
                            set_margin_bottom: 6,
                            add_css_class: "channel-header",

                            gtk::Label {
                                #[watch] set_label: &model.active_channel,
                                set_halign: gtk::Align::Start,
                                add_css_class: "channel-title",
                            },
                            gtk::Label {
                                #[watch]
                                set_label: model.channel_topics.get(&model.active_channel).map(String::as_str).unwrap_or(""),
                                set_hexpand: true,
                                set_halign: gtk::Align::Start,
                                set_ellipsize: gtk::pango::EllipsizeMode::End,
                                add_css_class: "channel-topic",
                            },
                        },

                        gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                        // Message area
                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_hexpand: true,
                            set_vscrollbar_policy: gtk::PolicyType::Automatic,
                            #[local_ref] chat_view_ref -> gtk::TextView {
                                set_editable: false,
                                set_cursor_visible: false,
                                set_wrap_mode: gtk::WrapMode::WordChar,
                                set_left_margin: 16,
                                set_right_margin: 16,
                                set_top_margin: 8,
                                set_bottom_margin: 8,
                                add_css_class: "chat-view",
                            }
                        },

                        gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                        // Composer
                        gtk::Box {
                            set_orientation: gtk::Orientation::Horizontal,
                            set_spacing: 8,
                            set_margin_start: 12,
                            set_margin_end: 12,
                            set_margin_top: 6,
                            set_margin_bottom: 10,
                            add_css_class: "composer",

                            #[local_ref] composer_entry_ref -> gtk::Entry {
                                set_placeholder_text: Some("Message…  (/help for commands)"),
                                set_hexpand: true,
                                add_css_class: "composer-entry",
                                connect_activate[sender] => move |e| {
                                    let t = e.text().to_string();
                                    if !t.is_empty() { e.set_text(""); sender.input(AppInput::SendMessage(t)); }
                                },
                            },
                            gtk::Button {
                                set_label: "➤",
                                set_tooltip_text: Some("Send"),
                                add_css_class: "suggested-action",
                                add_css_class: "composer-send",
                                connect_clicked[sender] => move |_| {
                                    sender.input(AppInput::ComposerSendClicked);
                                },
                            },
                        },
                    },

                    // Users panel
                    #[wrap(Some)]
                    set_end_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical,
                        set_width_request: 180,
                        set_vexpand: true,
                        add_css_class: "users-panel",

                        gtk::Label {
                            set_label: "MEMBERS",
                            add_css_class: "sidebar-section-header",
                            set_margin_top: 12,
                            set_margin_bottom: 6,
                            set_margin_start: 10,
                            set_halign: gtk::Align::Start,
                        },
                        gtk::Separator { set_orientation: gtk::Orientation::Horizontal },
                        gtk::ScrolledWindow {
                            set_vexpand: true,
                            set_hscrollbar_policy: gtk::PolicyType::Never,
                            #[local_ref] user_box_ref -> gtk::ListBox {
                                set_selection_mode: gtk::SelectionMode::None,
                            }
                        },
                    },
                },
            },
        }
    }

    fn init(
        _init: Self::Init,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        theme::attach_window(root.upcast_ref::<gtk::Window>());
        let settings = Settings::load();
        let server_tab = String::from(SERVER_TAB);
        let mut chat_histories: HashMap<String, Vec<ChatLine>> = HashMap::new();
        let ts = "[??:??] ".to_string();
        chat_histories.insert(
            server_tab.clone(),
            vec![
                ChatLine {
                    timestamp: ts.clone(),
                    user: "System".to_string(),
                    body: "Ready. Configure server/nick and Connect.".to_string(),
                    style: chat_view::LineStyle::System,
                },
                ChatLine {
                    timestamp: ts.clone(),
                    user: "System".to_string(),
                    body: "Use /join #channel or the join box.".to_string(),
                    style: chat_view::LineStyle::System,
                },
            ],
        );
        let channel_box = gtk::ListBox::new();
        channel_box.set_selection_mode(gtk::SelectionMode::Single);
        let user_box = gtk::ListBox::new();
        user_box.set_selection_mode(gtk::SelectionMode::None);
        let chat_view = gtk::TextView::new();
        chat_view::setup_tags(&chat_view);
        let composer_entry = gtk::Entry::new();
        let mut chans = vec![server_tab.clone()];
        for extra in &settings.extra_channels {
            if !chans.contains(extra) {
                chans.push(extra.clone());
            }
        }
        let favorites = if settings.favorites.is_empty() {
            vec![server_tab.clone()]
        } else {
            settings.favorites.clone()
        };
        let active = if settings.last_channel.is_empty() || !chans.contains(&settings.last_channel)
        {
            server_tab.clone()
        } else {
            settings.last_channel.clone()
        };
        let mut model = AppModel {
            servers: Vec::new(),
            current_server: String::new(),
            senders: HashMap::new(),
            server_states: HashMap::new(),
            connection: ConnectionState::Offline,
            user_disconnected: false,
            status: String::from("Offline"),
            active_channel: active,
            channels: chans,
            favorite_channels: favorites,
            muted_users: HashMap::new(),
            ignored_users: std::collections::HashSet::new(),
            unread_counts: HashMap::new(),
            mention_counts: HashMap::new(),
            chat_histories,
            channel_users: HashMap::new(),
            irc_sender: None,
            nickname: if settings.nickname.is_empty() {
                DEFAULT_NICKNAME.to_string()
            } else {
                settings.nickname
            },
            server: if settings.server.is_empty() {
                DEFAULT_SERVER.to_string()
            } else {
                settings.server
            },
            password: settings.password,
            irc_port: if settings.irc_port == 0 {
                DEFAULT_PORT
            } else {
                settings.irc_port
            },
            irc_use_tls: settings.irc_use_tls,
            notifications_enabled: settings.notifications_enabled,
            background_on_close: settings.background_on_close,
            channel_filter: String::new(),
            protocol_filter: ProtocolFilter::All,
            channel_list_results: Vec::new(),
            channel_topics: HashMap::new(),
            nick_colors_enabled: settings.nick_colors_enabled,
            timestamp_format: settings.timestamp_format,
            account_service: if settings.account_service.is_empty() {
                DEFAULT_ACCOUNT_SERVICE.to_string()
            } else {
                settings.account_service
            },
            auth_method: if settings.auth_method.is_empty() {
                "nickserv".to_string()
            } else {
                settings.auth_method
            },
            accounts: settings.accounts,
            pending_register_email: None,
            matrix_account: settings.matrix,
            discord_account: settings.discord,
            matrix_client: None,
            matrix_user_id: None,
            matrix_rooms: RoomRegistry::new(),
            matrix_connected: false,
            discord_client: None,
            discord_user_id: None,
            discord_channels: DiscordChannelRegistry::default(),
            discord_connection: ConnectionState::Offline,
            discord_status: String::from("Discord offline"),
            channel_box: channel_box.clone(),
            user_box: user_box.clone(),
            chat_view: chat_view.clone(),
            composer_entry: composer_entry.clone(),
            window: root.clone(),
        };
        let srv_init = model.server.clone();
        model.load_account_for_server(&srv_init);
        let channel_box_ref = &model.channel_box;
        let user_box_ref = &model.user_box;
        let chat_view_ref = &model.chat_view;
        let composer_entry_ref = &model.composer_entry;
        let widgets = view_output!();
        let parts = ComponentParts { model, widgets };
        parts.model.show_channel_history();
        parts.model.refresh_channels(&sender);
        parts.model.refresh_users(&sender);

        // Keyboard shortcuts
        let win = parts.model.window.clone();
        let app = relm4::main_application();
        let s_prefs = sender.clone();
        let act_prefs = gtk::gio::SimpleAction::new("preferences", None);
        act_prefs.connect_activate(move |_, _| s_prefs.input(AppInput::OpenPreferences));
        app.add_action(&act_prefs);
        app.set_accels_for_action("app.preferences", &["<Control>comma"]);

        let s_irc = sender.clone();
        let act_irc = gtk::gio::SimpleAction::new("connect-irc", None);
        act_irc.connect_activate(move |_, _| s_irc.input(AppInput::OpenIrcLogin));
        app.add_action(&act_irc);
        app.set_accels_for_action("app.connect-irc", &["<Control>n"]);

        let s_mx = sender.clone();
        let act_mx = gtk::gio::SimpleAction::new("connect-matrix", None);
        act_mx.connect_activate(move |_, _| s_mx.input(AppInput::OpenMatrixLogin));
        app.add_action(&act_mx);
        app.set_accels_for_action("app.connect-matrix", &["<Control><Shift>n"]);

        let act_activate = gtk::gio::SimpleAction::new("activate", None);
        let win2 = win.clone();
        act_activate.connect_activate(move |_, _| {
            win2.set_visible(true);
            win2.present();
        });
        app.add_action(&act_activate);

        parts
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppInput::UpdateNickname(nick) => {
                self.nickname = nick;
                let s = self.server.clone();
                self.sync_account_for_server(&s);
            }
            AppInput::UpdateServer(srv) => {
                let c = self.server.clone();
                self.sync_account_for_server(&c);
                self.load_account_for_server(&srv);
                self.server = srv;
            }
            AppInput::UpdatePassword(pwd) => {
                self.password = pwd;
                let s = self.server.clone();
                self.sync_account_for_server(&s);
            }
            AppInput::UpdateNotificationsEnabled(v) => {
                self.notifications_enabled = v;
                self.persist_settings();
            }
            AppInput::UpdateBackgroundOnClose(v) => {
                self.background_on_close = v;
                self.persist_settings();
            }
            AppInput::ComposerSendClicked => {
                let t = self.composer_entry.text().to_string();
                if !t.is_empty() {
                    self.composer_entry.set_text("");
                    sender.input(AppInput::SendMessage(t));
                }
            }
            AppInput::UpdateChannelFilter(f) => {
                self.channel_filter = f;
                self.refresh_channels(&sender);
            }
            AppInput::SetProtocolFilter(filter) => {
                self.protocol_filter = filter;
                self.refresh_channels(&sender);
            }
            AppInput::MarkChannelRead(ch) => {
                self.unread_counts.remove(&ch);
                self.mention_counts.remove(&ch);
            }
            AppInput::IgnoreUser(nick) => {
                let clean = Self::normalized_nick(&nick);
                self.ignored_users.insert(clean.clone());
                let chan = self.active_channel.clone();
                self.append_message(
                    &chan,
                    "System",
                    &format!("Ignoring {}", clean),
                    chat_view::LineStyle::System,
                );
            }
            AppInput::UnignoreUser(nick) => {
                let clean = Self::normalized_nick(&nick);
                self.ignored_users.remove(&clean);
                let chan = self.active_channel.clone();
                self.append_message(
                    &chan,
                    "System",
                    &format!("Unignored {}", clean),
                    chat_view::LineStyle::System,
                );
            }
            AppInput::UserRenamed { old, new } => {
                for list in self.channel_users.values_mut() {
                    for u in list.iter_mut() {
                        if *u == old {
                            *u = new.clone();
                        }
                    }
                }
                self.refresh_users(&sender);
            }
            AppInput::BrowseChannels => {
                if self.connection != ConnectionState::Connected {
                    self.append_message(
                        SERVER_TAB,
                        "System",
                        "Connect first to browse channels.",
                        chat_view::LineStyle::System,
                    );
                    return;
                }
                self.channel_list_results.clear();
                if let Some(tx) = &self.irc_sender {
                    let _ = tx.send(irc::client::prelude::Message::from("LIST"));
                    self.append_message(
                        SERVER_TAB,
                        "System",
                        "Requesting channel list…",
                        chat_view::LineStyle::System,
                    );
                }
            }
            AppInput::ChannelListEntry { name, users, topic } => {
                self.channel_list_results.push((name, users, topic));
            }
            AppInput::ChannelListEnd => {
                // Reuse existing browse dialog logic inline for now
                let results = self.channel_list_results.clone();
                if results.is_empty() {
                    self.append_message(
                        SERVER_TAB,
                        "System",
                        "No channels returned.",
                        chat_view::LineStyle::System,
                    );
                    return;
                }
                let mut sorted = results;
                sorted.sort_by(|a, b| b.1.cmp(&a.1));
                let dialog = gtk::Window::builder()
                    .transient_for(&self.window)
                    .modal(true)
                    .title("Browse Channels")
                    .default_width(720)
                    .default_height(520)
                    .build();
                dialog.add_css_class("boulder-relay");
                let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
                vbox.set_margin_all(12);
                let search = gtk::SearchEntry::builder()
                    .placeholder_text("Filter…")
                    .hexpand(true)
                    .build();
                vbox.append(&search);
                let scrolled = gtk::ScrolledWindow::builder().vexpand(true).build();
                let list_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
                scrolled.set_child(Some(&list_box));
                vbox.append(&scrolled);
                let mut rows: Vec<(gtk::Box, String, String)> = Vec::new();
                for (name, users, topic) in &sorted {
                    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
                    let info = gtk::Label::builder()
                        .label(format!("{} ({} users) {}", name, users, topic))
                        .halign(gtk::Align::Start)
                        .hexpand(true)
                        .ellipsize(gtk::pango::EllipsizeMode::End)
                        .build();
                    let join_btn = gtk::Button::with_label("Join");
                    join_btn.add_css_class("suggested-action");
                    let s = sender.clone();
                    let ch = name.clone();
                    let dlg = dialog.clone();
                    join_btn.connect_clicked(move |_| {
                        s.input(AppInput::JoinChannel(ch.clone()));
                        dlg.close();
                    });
                    row.append(&info);
                    row.append(&join_btn);
                    list_box.append(&row);
                    rows.push((row, name.clone(), topic.clone()));
                }
                let rows2 = rows.clone();
                search.connect_changed(move |e| {
                    let q = e.text().to_lowercase();
                    for (row, name, topic) in &rows2 {
                        row.set_visible(
                            q.is_empty()
                                || format!("{} {}", name, topic).to_lowercase().contains(&q),
                        );
                    }
                });
                let close = gtk::Button::with_label("Close");
                let d = dialog.clone();
                close.connect_clicked(move |_| d.close());
                vbox.append(&close);
                dialog.set_child(Some(&vbox));
                dialog.present();
            }
            AppInput::ChannelTopic { channel, topic } => {
                self.channel_topics.insert(channel, topic);
            }
            AppInput::OpenPreferences => {
                let dialog = gtk::Window::builder()
                    .transient_for(&self.window)
                    .modal(true)
                    .title("Preferences")
                    .default_width(420)
                    .default_height(360)
                    .build();
                dialog.add_css_class("boulder-relay");
                let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
                vbox.set_margin_all(12);
                let nick_check = gtk::CheckButton::builder()
                    .label("Enable nickname colors")
                    .active(self.nick_colors_enabled)
                    .build();
                vbox.append(&nick_check);
                let notif_check = gtk::CheckButton::builder()
                    .label("Desktop notifications")
                    .active(self.notifications_enabled)
                    .build();
                vbox.append(&notif_check);
                let bg_check = gtk::CheckButton::builder()
                    .label("Hide window on close (keep running)")
                    .active(self.background_on_close)
                    .build();
                vbox.append(&bg_check);
                vbox.append(&gtk::Label::new(Some("Timestamp format:")));
                let ts_entry = gtk::Entry::builder().text(&self.timestamp_format).build();
                vbox.append(&ts_entry);
                let apply = gtk::Button::with_label("Apply");
                let s = sender.clone();
                let d = dialog.clone();
                let nc = nick_check.clone();
                let te = ts_entry.clone();
                let nfc = notif_check.clone();
                let bgc = bg_check.clone();
                apply.connect_clicked(move |_| {
                    s.input(AppInput::UpdateNickColorsEnabled(nc.is_active()));
                    s.input(AppInput::UpdateNotificationsEnabled(nfc.is_active()));
                    s.input(AppInput::UpdateBackgroundOnClose(bgc.is_active()));
                    s.input(AppInput::UpdateTimestampFormat(te.text().to_string()));
                    d.close();
                });
                vbox.append(&apply);
                dialog.set_child(Some(&vbox));
                dialog.present();
            }
            AppInput::UpdateNickColorsEnabled(v) => {
                self.nick_colors_enabled = v;
                self.persist_settings();
            }
            AppInput::UpdateTimestampFormat(f) => {
                self.timestamp_format = f;
                self.persist_settings();
            }
            AppInput::OpenRegisterDialog => {
                dialogs::show_register_dialog(&self.window, &sender, &self.nickname);
            }
            AppInput::SubmitRegistration {
                nick,
                password,
                email,
            } => {
                self.nickname = nick.clone();
                self.password = password.clone();
                self.pending_register_email = if email.is_empty() {
                    None
                } else {
                    Some(email.clone())
                };
                self.persist_settings();
                if self.connection == ConnectionState::Offline {
                    sender.input(AppInput::Connect);
                }
                let service = self.account_service.clone();
                if let Some(tx) = &self.irc_sender {
                    let cmd = if email.is_empty() {
                        format!("REGISTER {}", password)
                    } else {
                        format!("REGISTER {} {}", password, email)
                    };
                    let _ = tx.send_privmsg(&service, &cmd);
                    self.append_message(
                        SERVER_TAB,
                        "System",
                        &format!("Sent registration for {}.", nick),
                        chat_view::LineStyle::System,
                    );
                    self.pending_register_email = None;
                }
            }
            AppInput::SubmitVerification { nick, code } => {
                let service = self.account_service.clone();
                if let Some(tx) = &self.irc_sender {
                    let _ =
                        tx.send_privmsg(&service, &format!("VERIFY REGISTER {} {}", nick, code));
                    self.append_message(
                        SERVER_TAB,
                        "System",
                        &format!("Sent VERIFY for {}.", nick),
                        chat_view::LineStyle::System,
                    );
                }
            }
            AppInput::UpdateAccountService(s) => {
                if !s.is_empty() {
                    self.account_service = s;
                    let srv = self.server.clone();
                    self.sync_account_for_server(&srv);
                    self.persist_settings();
                }
            }
            AppInput::UpdateAuthMethod(m) => {
                if !m.is_empty() {
                    self.auth_method = m;
                    let srv = self.server.clone();
                    self.sync_account_for_server(&srv);
                    self.persist_settings();
                }
            }
            AppInput::SendRawPrivmsg { target, msg } => {
                if let Some(tx) = &self.irc_sender {
                    let _ = tx.send_privmsg(&target, &msg);
                }
            }
            AppInput::AddServer(srv) => {
                let srv = srv.trim().to_string();
                if srv.is_empty() {
                    return;
                }
                if !self.servers.contains(&srv) {
                    self.servers.push(srv.clone());
                    self.senders.insert(srv.clone(), None);
                    self.server_states
                        .insert(srv.clone(), ConnectionState::Offline);
                }
                self.current_server = srv.clone();
                self.server = srv.clone();
                self.load_account_for_server(&srv);
                self.refresh_channels(&sender);
                if self.server_states.get(&srv) != Some(&ConnectionState::Connected) {
                    sender.input(AppInput::Connect);
                }
            }
            AppInput::SwitchServer(srv) => {
                if self.servers.contains(&srv) {
                    let curr = self.current_server.clone();
                    self.sync_account_for_server(&curr);
                    self.current_server = srv.clone();
                    self.server = srv.clone();
                    self.load_account_for_server(&srv);
                    let state = self
                        .server_states
                        .get(&srv)
                        .copied()
                        .unwrap_or(ConnectionState::Offline);
                    self.connection = state;
                    self.irc_sender = self.senders.get(&srv).and_then(|s| s.clone());
                    self.refresh_channels(&sender);
                    self.show_channel_history();
                    self.refresh_users(&sender);
                }
            }
            AppInput::OpenAccountManager => {
                dialogs::show_account_manager(
                    &self.window,
                    &sender,
                    &self.server,
                    &self.nickname,
                    &self.password,
                    &self.auth_method,
                    &self.account_service,
                    &self.matrix_account.homeserver,
                    &self.matrix_account.username,
                    &self.discord_account.bot_token,
                );
            }
            AppInput::OpenIrcLogin => {
                dialogs::show_irc_login_dialog(
                    &self.window,
                    &sender,
                    &self.server,
                    &self.nickname,
                    &self.password,
                    &self.auth_method,
                );
            }
            AppInput::OpenLogViewer => {
                let dialog = gtk::Window::builder()
                    .transient_for(&self.window)
                    .modal(true)
                    .title("Log Viewer")
                    .default_width(600)
                    .default_height(500)
                    .build();
                dialog.add_css_class("boulder-relay");
                let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
                vbox.set_margin_all(12);
                let search = gtk::Entry::builder()
                    .placeholder_text("Search logs…")
                    .build();
                vbox.append(&search);
                let scrolled = gtk::ScrolledWindow::new();
                scrolled.set_vexpand(true);
                let tv = gtk::TextView::new();
                tv.set_editable(false);
                tv.set_wrap_mode(gtk::WrapMode::Word);
                scrolled.set_child(Some(&tv));
                vbox.append(&scrolled);
                let buf = tv.buffer();
                if let Some(lines) = self.chat_histories.get(&self.active_channel) {
                    for line in lines {
                        buf.insert(
                            &mut buf.end_iter(),
                            &format!("<{}> {}\n", line.user, line.body),
                        );
                    }
                }
                let tv2 = tv.clone();
                search.connect_changed(move |e| {
                    let q = e.text().to_lowercase();
                    if q.is_empty() {
                        return;
                    }
                    let b = tv2.buffer();
                    let text = b.text(&b.start_iter(), &b.end_iter(), false);
                    if let Some(pos) = text.to_lowercase().find(&q) {
                        let mut start = b.iter_at_offset(pos as i32);
                        let end = b.iter_at_offset((pos + q.len()) as i32);
                        b.select_range(&start, &end);
                        tv2.scroll_to_iter(&mut start, 0.0, false, 0.0, 0.0);
                    }
                });
                let close = gtk::Button::with_label("Close");
                let d = dialog.clone();
                close.connect_clicked(move |_| d.close());
                vbox.append(&close);
                dialog.set_child(Some(&vbox));
                dialog.present();
            }
            AppInput::Quit => {
                self.persist_settings();
                if let Some(tx) = self.irc_sender.take() {
                    let _ = tx.send_quit("boulderX signing off");
                }
                if let Some(client) = self.discord_client.take() {
                    runtime::spawn(async move {
                        client.shutdown().await;
                    });
                }
                relm4::main_application().quit();
            }
            AppInput::SaveSettings => self.persist_settings(),
            AppInput::Connect => {
                if self.connection != ConnectionState::Offline {
                    return;
                }
                self.connection = ConnectionState::Connecting;
                self.user_disconnected = false;
                self.status = String::from("Connecting…");
                self.persist_settings();
                let channels_to_join: Vec<String> = self
                    .channels
                    .iter()
                    .filter(|c| channels::is_channel_target(c))
                    .cloned()
                    .collect();
                IrcConnection::spawn(
                    sender.clone(),
                    self.nickname.clone(),
                    self.server.clone(),
                    self.password.clone(),
                    self.auth_method.clone(),
                    channels_to_join,
                    self.irc_port,
                    self.irc_use_tls,
                );
            }
            AppInput::UserDisconnect => {
                self.user_disconnected = true;
                sender.input(AppInput::Disconnect);
            }
            AppInput::Disconnect => {
                if let Some(tx) = self.irc_sender.take() {
                    let _ = tx.send_quit("boulderX signing off");
                    self.connection = ConnectionState::Offline;
                    self.status = String::from("Offline");
                    self.channel_list_results.clear();
                    self.append_message(
                        SERVER_TAB,
                        "System",
                        "Disconnected.",
                        chat_view::LineStyle::System,
                    );
                }
            }
            AppInput::NetworkStatus(s) => {
                self.status = s.clone();
                if is_terminal_irc_status(&s) {
                    let was_connected = self.connection == ConnectionState::Connected;
                    self.connection = ConnectionState::Offline;
                    self.irc_sender = None;
                    self.append_message(SERVER_TAB, "System", &s, chat_view::LineStyle::System);
                    if was_connected && !self.user_disconnected {
                        let s2 = sender.clone();
                        gtk::glib::timeout_add_seconds_local(5, move || {
                            s2.input(AppInput::Connect);
                            gtk::glib::ControlFlow::Break
                        });
                    }
                    self.user_disconnected = false;
                }
            }
            AppInput::NetworkConnected(tx) => {
                self.irc_sender = Some(tx.clone());
                self.senders.insert(self.server.clone(), Some(tx.clone()));
                self.server_states
                    .insert(self.server.clone(), ConnectionState::Connected);
                self.connection = ConnectionState::Connected;
                self.status = String::from("Connected");
                self.append_message(
                    SERVER_TAB,
                    "System",
                    &format!("Connected to {} as {}.", self.server, self.nickname),
                    chat_view::LineStyle::System,
                );
                if let Some(email) = self.pending_register_email.take() {
                    let service = self.account_service.clone();
                    let cmd = format!("REGISTER {} {}", self.password, email);
                    let _ = tx.send_privmsg(&service, &cmd);
                    self.append_message(
                        SERVER_TAB,
                        "System",
                        &format!("Sent registration to {}.", service),
                        chat_view::LineStyle::System,
                    );
                }
            }
            AppInput::SelectChannel(ch) => {
                self.active_channel = ch.clone();
                self.unread_counts.remove(&ch);
                self.mention_counts.remove(&ch);
                self.matrix_rooms.clear_unread_by_display_name(&ch);
                self.show_channel_history();
                self.refresh_users(&sender);
                self.persist_settings();
            }
            AppInput::JoinChannel(target) => {
                if !self.channels.contains(&target) {
                    self.channels.push(target.clone());
                    let ts = self.timestamp_prefix();
                    let body = format!("Tracking {}", target);
                    self.chat_histories
                        .entry(target.clone())
                        .or_insert_with(|| {
                            vec![ChatLine {
                                timestamp: ts,
                                user: "System".to_string(),
                                body,
                                style: chat_view::LineStyle::System,
                            }]
                        });
                    self.refresh_channels(&sender);
                    self.send_irc_join(&target);
                } else {
                    self.send_irc_join(&target);
                }
                self.active_channel = target;
                self.show_channel_history();
                self.refresh_users(&sender);
                self.persist_settings();
            }
            AppInput::PartChannel(ch) => {
                if ch == SERVER_TAB || !channels::is_channel_target(&ch) {
                    return;
                }
                if let Some(tx) = &self.irc_sender {
                    let _ = tx.send_part(&ch);
                }
                self.channels.retain(|c| c != &ch);
                self.chat_histories.remove(&ch);
                self.channel_users.remove(&ch);
                self.muted_users.remove(&ch);
                self.channel_topics.remove(&ch);
                if self.active_channel == ch {
                    self.active_channel = SERVER_TAB.to_string();
                    self.show_channel_history();
                }
                self.refresh_channels(&sender);
                self.refresh_users(&sender);
                self.persist_settings();
            }
            AppInput::ClearChannel(ch) => {
                self.chat_histories.insert(ch.clone(), Vec::new());
                if self.active_channel == ch {
                    self.chat_view.buffer().set_text("");
                }
            }
            AppInput::ToggleFavorite(ch) => {
                if self.favorite_channels.contains(&ch) {
                    self.favorite_channels.retain(|c| c != &ch);
                } else {
                    self.favorite_channels.push(ch.clone());
                }
                self.refresh_channels(&sender);
                self.persist_settings();
            }
            AppInput::ToggleMute { channel, user } => {
                let list = self.muted_users.entry(channel.clone()).or_default();
                if list.contains(&user) {
                    list.retain(|u| u != &user);
                    self.append_message(
                        &channel,
                        "System",
                        &format!("Unmuted {}", user),
                        chat_view::LineStyle::System,
                    );
                } else {
                    list.push(user.clone());
                    list.sort_by_key(|u| u.to_lowercase());
                    self.append_message(
                        &channel,
                        "System",
                        &format!("Muted {}", user),
                        chat_view::LineStyle::System,
                    );
                }
                if self.active_channel == channel {
                    self.refresh_users(&sender);
                }
            }
            AppInput::ReceiveMessage {
                channel,
                user,
                body,
                protocol,
            } => {
                let clean = Self::normalized_nick(&user);
                if self.ignored_users.contains(&clean) {
                    return;
                }
                if self
                    .muted_users
                    .get(&channel)
                    .map(|v| v.contains(&clean))
                    .unwrap_or(false)
                {
                    return;
                }
                if !self.channels.contains(&channel) {
                    if let Protocol::Irc = protocol {
                        self.channels.push(channel.clone());
                        self.refresh_channels(&sender);
                    }
                }
                // Ensure Matrix rooms have a history slot under the display name.
                if let Protocol::Matrix { ref room_id } = protocol {
                    self.chat_histories
                        .entry(channel.clone())
                        .or_insert_with(Vec::new);
                    if self.matrix_rooms.get(room_id).is_none() {
                        self.matrix_rooms.insert(room_id.clone(), channel.clone());
                    }
                }
                if let Protocol::Discord { ref channel_id } = protocol {
                    self.chat_histories
                        .entry(channel.clone())
                        .or_insert_with(Vec::new);
                    if self.discord_channels.get(channel_id).is_none() {
                        self.discord_channels
                            .insert(channel_id.clone(), channel.clone());
                    }
                }
                let style = self.message_style(&user, &body);
                self.append_message(&channel, &user, &body, style);
                if channel != self.active_channel {
                    *self.unread_counts.entry(channel.clone()).or_insert(0) += 1;
                    if style == chat_view::LineStyle::Mention {
                        *self.mention_counts.entry(channel.clone()).or_insert(0) += 1;
                    }
                    if let Protocol::Matrix { ref room_id } = protocol {
                        self.matrix_rooms.increment_unread(room_id);
                    }
                    self.refresh_channels(&sender);
                }
                if self.should_notify(&channel, &user, style) {
                    let kind = self.notify_kind(&channel, style);
                    notify::send_message_notification(&channel, &user, &body, kind);
                }
            }
            AppInput::ReceiveServerMessage(body) => {
                self.append_message(SERVER_TAB, "System", &body, chat_view::LineStyle::System);
            }
            AppInput::BatchAddUsers { channel, users } => {
                let list = self.channel_users.entry(channel.clone()).or_default();
                for u in users {
                    if !list.contains(&u) {
                        list.push(u);
                    }
                }
                list.sort_by_key(|a| a.to_lowercase());
                if self.active_channel == channel {
                    self.refresh_users(&sender);
                }
            }
            AppInput::UserJoined { channel, user } => {
                let list = self.channel_users.entry(channel.clone()).or_default();
                if !list.contains(&user) {
                    list.push(user);
                    list.sort_by_key(|a| a.to_lowercase());
                }
                if self.active_channel == channel {
                    self.refresh_users(&sender);
                }
            }
            AppInput::UserLeft { channel, user } => {
                if let Some(list) = self.channel_users.get_mut(&channel) {
                    list.retain(|u| u != &user);
                }
                if self.active_channel == channel {
                    self.refresh_users(&sender);
                }
            }
            AppInput::UserQuit { user } => {
                for list in self.channel_users.values_mut() {
                    list.retain(|u| u != &user);
                }
                self.refresh_users(&sender);
            }
            AppInput::JoinEntry(text) => {
                let text = text.trim();
                if text.is_empty() {
                    return;
                }
                if text.starts_with('/') {
                    sender.input(AppInput::SendMessage(text.to_string()));
                    return;
                }
                // Use parse_join_entry so multi-join preserves DMs vs channels.
                match channels::parse_join_entry(text) {
                    Some(channels::JoinTarget::Channel(ch)) => {
                        sender.input(AppInput::JoinChannel(ch))
                    }
                    Some(channels::JoinTarget::DirectMessage(nick)) => {
                        sender.input(AppInput::JoinChannel(nick))
                    }
                    Some(channels::JoinTarget::Multi(targets)) => {
                        for t in targets {
                            match t {
                                channels::JoinTarget::Channel(ch) => {
                                    sender.input(AppInput::JoinChannel(ch))
                                }
                                channels::JoinTarget::DirectMessage(nick) => {
                                    sender.input(AppInput::JoinChannel(nick))
                                }
                                channels::JoinTarget::Multi(_) => {}
                            }
                        }
                    }
                    None => {}
                }
            }
            AppInput::SendMessage(text) => {
                let text = text.trim();
                if text.is_empty() {
                    return;
                }
                match commands::parse_slash_command(text) {
                    SlashCommand::Plain(body) => {
                        if body.is_empty() {
                            return;
                        }
                        self.send_plain_message(&sender, &body);
                    }
                    SlashCommand::Join { channels: chans } => {
                        for ch in chans {
                            sender.input(AppInput::JoinChannel(ch));
                        }
                    }
                    SlashCommand::Part { target } => {
                        let target = target.unwrap_or_else(|| self.active_channel.clone());
                        sender.input(AppInput::PartChannel(target));
                    }
                    SlashCommand::Msg { target, body } => {
                        if let Some(tx) = &self.irc_sender {
                            let _ = tx.send_privmsg(&target, &body);
                            if !self.channels.contains(&target) {
                                sender.input(AppInput::JoinChannel(target.clone()));
                            }
                            let nick = self.nickname.clone();
                            self.append_message(
                                &target,
                                &nick,
                                &body,
                                chat_view::LineStyle::SelfMsg,
                            );
                            self.active_channel = target;
                            self.show_channel_history();
                        } else {
                            self.append_message(
                                SERVER_TAB,
                                "System",
                                "Cannot /msg: not connected to IRC.",
                                chat_view::LineStyle::System,
                            );
                        }
                    }
                    SlashCommand::Nick { nick } => {
                        if let Some(tx) = &self.irc_sender {
                            let _ = tx.send(irc::client::prelude::Message::from(
                                format!("NICK {}", nick).as_str(),
                            ));
                        }
                        self.nickname = nick;
                        self.persist_settings();
                    }
                    SlashCommand::Me { action } => {
                        let ch = self.active_channel.clone();
                        // Matrix active room → m.emote (same routing as plain send).
                        if let Some(matrix_room_id) = self
                            .matrix_rooms
                            .find_by_display_name(&ch)
                            .map(|r| r.room_id.clone())
                        {
                            let client = self.matrix_client.clone();
                            let rid = matrix_room_id;
                            let act = action.clone();
                            let s = sender.clone();
                            let me = format!(
                                "* {}",
                                self.matrix_user_id
                                    .clone()
                                    .unwrap_or_else(|| self.nickname.clone())
                            );
                            self.append_message(&ch, &me, &action, chat_view::LineStyle::SelfMsg);
                            runtime::spawn(async move {
                                if let Some(c) = client {
                                    if let Err(e) = c.send_emote(&rid, &act).await {
                                        s.input(AppInput::ReceiveServerMessage(format!(
                                            "[Matrix /me failed]: {e}"
                                        )));
                                    }
                                } else {
                                    s.input(AppInput::ReceiveServerMessage(
                                        "[Matrix]: not connected.".into(),
                                    ));
                                }
                            });
                        } else if let Some(tx) = &self.irc_sender {
                            let full = format!("\x01ACTION {}\x01", action);
                            match tx.send_privmsg(&ch, &full) {
                                Ok(()) => {
                                    let me = format!("* {}", self.nickname);
                                    self.append_message(
                                        &ch,
                                        &me,
                                        &action,
                                        chat_view::LineStyle::SelfMsg,
                                    );
                                }
                                Err(e) => {
                                    self.append_message(
                                        &ch,
                                        "System",
                                        &format!("/me failed: {e}"),
                                        chat_view::LineStyle::System,
                                    );
                                }
                            }
                        } else {
                            self.append_message(
                                SERVER_TAB,
                                "System",
                                "Cannot /me: not connected.",
                                chat_view::LineStyle::System,
                            );
                        }
                    }
                    SlashCommand::Whois { nick } => {
                        if let Some(tx) = &self.irc_sender {
                            let _ = tx.send(irc::client::prelude::Message::from(
                                format!("WHOIS {}", nick).as_str(),
                            ));
                        }
                    }
                    SlashCommand::Away { message } => {
                        if let Some(tx) = &self.irc_sender {
                            let _ = tx.send(irc::client::prelude::Message::from(
                                format!("AWAY :{}", message).as_str(),
                            ));
                        }
                    }
                    SlashCommand::Back => {
                        if let Some(tx) = &self.irc_sender {
                            let _ = tx.send(irc::client::prelude::Message::from("AWAY"));
                        }
                    }
                    SlashCommand::Topic { text } => {
                        let ch = self.active_channel.clone();
                        if let Some(tx) = &self.irc_sender {
                            match text {
                                None => {
                                    let _ = tx.send(irc::client::prelude::Message::from(
                                        format!("TOPIC {}", ch).as_str(),
                                    ));
                                }
                                Some(t) => {
                                    let _ = tx.send(irc::client::prelude::Message::from(
                                        format!("TOPIC {} :{}", ch, t).as_str(),
                                    ));
                                }
                            }
                        }
                    }
                    SlashCommand::Ignore { nick } => sender.input(AppInput::IgnoreUser(nick)),
                    SlashCommand::Unignore { nick } => sender.input(AppInput::UnignoreUser(nick)),
                    SlashCommand::Clear => {
                        sender.input(AppInput::ClearChannel(self.active_channel.clone()))
                    }
                    SlashCommand::List => sender.input(AppInput::BrowseChannels),
                    SlashCommand::Help => {
                        let ch = self.active_channel.clone();
                        self.append_message(&ch, "System", "Commands: /join /part /msg /me /nick /whois /away /back /topic /ignore /unignore /clear /list /help", chat_view::LineStyle::System);
                    }
                    SlashCommand::Unknown { name } => {
                        let ch = self.active_channel.clone();
                        self.append_message(
                            &ch,
                            "System",
                            &format!("Unknown command: /{name}. Type /help."),
                            chat_view::LineStyle::System,
                        );
                    }
                }
            }
            // ── Matrix handlers ──────────────────────────────────────
            AppInput::OpenMatrixLogin => {
                dialogs::show_matrix_login_dialog(
                    &self.window,
                    &sender,
                    &self.matrix_account.homeserver,
                    &self.matrix_account.username,
                );
            }
            AppInput::OpenMatrixJoin => {
                dialogs::show_matrix_join_dialog(&self.window, &sender);
            }
            AppInput::ClearMatrixAccount => {
                self.matrix_account = crate::config::MatrixAccount::default();
                self.persist_settings();
                self.append_message(
                    SERVER_TAB,
                    "System",
                    "Cleared saved Matrix account.",
                    chat_view::LineStyle::System,
                );
            }
            AppInput::MatrixLogin {
                homeserver,
                username,
                password,
                remember,
            } => {
                if remember {
                    self.matrix_account = crate::config::MatrixAccount {
                        homeserver: homeserver.clone(),
                        username: username.clone(),
                        password: password.clone(),
                    };
                    self.persist_settings();
                }
                self.append_message(
                    SERVER_TAB,
                    "System",
                    &format!("Connecting to Matrix ({})…", homeserver),
                    chat_view::LineStyle::System,
                );
                let s = sender.clone();
                runtime::spawn(async move {
                    match MatrixClient::new(&homeserver).await {
                        Ok(client) => match client.login_password(&username, &password).await {
                            Ok(()) => {
                                let user_id = client
                                    .user_id()
                                    .map(|u| u.to_string())
                                    .unwrap_or_else(|| username.clone());
                                let (tx, rx) = mpsc::unbounded_channel::<MatrixEvent>();
                                bridge_matrix_events(rx, s.clone());
                                s.input(AppInput::MatrixStoreClient(client.clone()));
                                client.start_sync(tx);
                                s.input(AppInput::MatrixConnected { user_id });
                            }
                            Err(e) => s.input(AppInput::ReceiveServerMessage(format!(
                                "[Matrix Login Error]: {e}"
                            ))),
                        },
                        Err(e) => s.input(AppInput::ReceiveServerMessage(format!(
                            "[Matrix Error]: {e}"
                        ))),
                    }
                });
            }
            AppInput::MatrixStoreClient(client) => {
                self.matrix_client = Some(client);
            }
            AppInput::MatrixConnected { user_id } => {
                self.matrix_user_id = Some(user_id.clone());
                self.matrix_connected = true;
                self.append_message(
                    SERVER_TAB,
                    "System",
                    &format!("Matrix connected as {}.", user_id),
                    chat_view::LineStyle::System,
                );
                self.refresh_channels(&sender);
            }
            AppInput::MatrixRoomJoined { room_id, room_name } => {
                self.matrix_rooms.insert(room_id, room_name.clone());
                self.chat_histories
                    .entry(room_name.clone())
                    .or_insert_with(Vec::new);
                self.refresh_channels(&sender);
            }
            AppInput::MatrixRoomLeft { room_id } => {
                // Resolve display name before remove so we can clear history / active tab.
                let display = self
                    .matrix_rooms
                    .get(&room_id)
                    .map(|r| r.display_name.clone());
                // Best-effort server leave; always drop local state.
                if let Ok(rid) = room_id.parse::<matrix_sdk::ruma::OwnedRoomId>() {
                    if let Some(c) = self.matrix_client.clone() {
                        let s = sender.clone();
                        runtime::spawn(async move {
                            if let Err(e) = c.leave_room(&rid).await {
                                s.input(AppInput::ReceiveServerMessage(format!(
                                    "[Matrix leave]: {e}"
                                )));
                            }
                        });
                    }
                }
                self.matrix_rooms.remove(&room_id);
                if let Some(name) = display {
                    self.chat_histories.remove(&name);
                    self.unread_counts.remove(&name);
                    self.mention_counts.remove(&name);
                    if self.active_channel == name {
                        self.active_channel = SERVER_TAB.to_string();
                        self.show_channel_history();
                        self.refresh_users(&sender);
                    }
                }
                self.refresh_channels(&sender);
            }
            AppInput::MatrixJoinRoom(alias) => {
                if alias.is_empty() {
                    dialogs::show_matrix_join_dialog(&self.window, &sender);
                    return;
                }
                self.append_message(
                    SERVER_TAB,
                    "System",
                    &format!("Joining Matrix room {}…", alias),
                    chat_view::LineStyle::System,
                );
                let client = self.matrix_client.clone();
                let s = sender.clone();
                runtime::spawn(async move {
                    if let Some(c) = client {
                        if let Ok(id) = alias.parse::<matrix_sdk::ruma::OwnedRoomOrAliasId>() {
                            match c.inner.join_room_by_id_or_alias(&id, &[]).await {
                                Ok(room) => {
                                    let room_id = room.room_id().to_string();
                                    let name = room.name().unwrap_or_else(|| room_id.clone());
                                    s.input(AppInput::MatrixRoomJoined {
                                        room_id,
                                        room_name: name,
                                    });
                                }
                                Err(e) => s.input(AppInput::ReceiveServerMessage(format!(
                                    "[Matrix]: Failed to join: {e}"
                                ))),
                            }
                        } else {
                            s.input(AppInput::ReceiveServerMessage(format!(
                                "[Matrix]: invalid room id or alias: {alias}"
                            )));
                        }
                    } else {
                        s.input(AppInput::ReceiveServerMessage(
                            "[Matrix]: not connected — sign in first.".into(),
                        ));
                    }
                });
            }
            AppInput::MatrixSendMessage { room_id, body } => {
                if let Ok(rid) = room_id.parse::<matrix_sdk::ruma::OwnedRoomId>() {
                    let client = self.matrix_client.clone();
                    let s = sender.clone();
                    runtime::spawn(async move {
                        if let Some(c) = client {
                            if let Err(e) = c.send_message(&rid, &body).await {
                                s.input(AppInput::ReceiveServerMessage(format!(
                                    "[Matrix send failed]: {e}"
                                )));
                            }
                        }
                    });
                }
            }
            // ── Discord bot handlers ──────────────────────────────────
            AppInput::OpenDiscordLogin => {
                dialogs::show_discord_login_dialog(
                    &self.window,
                    &sender,
                    &self.discord_account.bot_token,
                );
            }
            AppInput::DiscordButtonClicked => {
                if self.discord_connection == ConnectionState::Connected {
                    sender.input(AppInput::DisconnectDiscord);
                } else if self.discord_connection == ConnectionState::Offline {
                    sender.input(AppInput::OpenDiscordLogin);
                }
            }
            AppInput::ClearDiscordAccount => {
                self.discord_account = crate::config::DiscordAccount::default();
                self.persist_settings();
                self.append_message(
                    SERVER_TAB,
                    "System",
                    "Cleared saved Discord bot token.",
                    chat_view::LineStyle::System,
                );
            }
            AppInput::DiscordLogin {
                bot_token,
                remember,
            } => {
                if bot_token.trim().is_empty() {
                    self.discord_status = "Discord bot token required".to_string();
                    self.append_message(
                        SERVER_TAB,
                        "System",
                        "Discord connection requires a bot token.",
                        chat_view::LineStyle::System,
                    );
                    return;
                }
                if self.discord_connection != ConnectionState::Offline {
                    self.append_message(
                        SERVER_TAB,
                        "System",
                        "Discord is already connecting or connected.",
                        chat_view::LineStyle::System,
                    );
                    return;
                }
                if remember {
                    self.discord_account.bot_token = bot_token.clone();
                    self.persist_settings();
                }
                self.discord_connection = ConnectionState::Connecting;
                self.discord_status = "Connecting Discord bot…".to_string();
                self.append_message(
                    SERVER_TAB,
                    "System",
                    "Connecting Discord bot…",
                    chat_view::LineStyle::System,
                );
                let s = sender.clone();
                runtime::spawn(async move {
                    let (tx, rx) = mpsc::unbounded_channel::<DiscordEvent>();
                    match DiscordClient::connect(&bot_token, tx).await {
                        Ok(client) => {
                            bridge_discord_events(rx, s.clone());
                            s.input(AppInput::DiscordStoreClient(client));
                        }
                        Err(error) => s.input(AppInput::DiscordError(format!(
                            "Discord connection failed: {error}"
                        ))),
                    }
                });
            }
            AppInput::DiscordStoreClient(client) => {
                self.discord_client = Some(client);
            }
            AppInput::DiscordConnected { user_id } => {
                self.discord_user_id = Some(user_id.clone());
                self.discord_connection = ConnectionState::Connected;
                self.discord_status = format!("Discord connected as {user_id}");
                self.append_message(
                    SERVER_TAB,
                    "System",
                    &format!("Discord bot connected as {user_id}."),
                    chat_view::LineStyle::System,
                );
            }
            AppInput::DiscordChannelDiscovered {
                channel_id,
                display_name,
            } => {
                let is_new = self.discord_channels.get(&channel_id).is_none();
                self.discord_channels
                    .insert(channel_id, display_name.clone());
                self.chat_histories.entry(display_name).or_default();
                if is_new {
                    self.refresh_channels(&sender);
                }
            }
            AppInput::DiscordMessage {
                channel_id,
                dm_display_name,
                sender: message_sender,
                body,
            } => {
                let display_name = self
                    .discord_channels
                    .get(&channel_id)
                    .map(|channel| channel.display_name.clone())
                    .or(dm_display_name)
                    .unwrap_or_else(|| format!("Discord channel {channel_id}"));
                if self.discord_channels.get(&channel_id).is_none() {
                    self.discord_channels
                        .insert(channel_id.clone(), display_name.clone());
                    self.chat_histories.entry(display_name.clone()).or_default();
                    self.refresh_channels(&sender);
                }
                sender.input(AppInput::ReceiveMessage {
                    channel: display_name,
                    user: message_sender,
                    body,
                    protocol: Protocol::Discord { channel_id },
                });
            }
            AppInput::DiscordChannelDeleted { channel_id } => {
                if let Some(channel) = self.discord_channels.remove(&channel_id) {
                    self.chat_histories.remove(&channel.display_name);
                    self.unread_counts.remove(&channel.display_name);
                    self.mention_counts.remove(&channel.display_name);
                    if self.active_channel == channel.display_name {
                        self.active_channel = SERVER_TAB.to_string();
                        self.show_channel_history();
                    }
                    self.refresh_channels(&sender);
                }
            }
            AppInput::DiscordError(error) => {
                self.discord_status = format!("Discord error: {error}");
                if self.discord_connection == ConnectionState::Connecting {
                    self.discord_connection = ConnectionState::Offline;
                    self.discord_client = None;
                }
                self.append_message(
                    SERVER_TAB,
                    "System",
                    &format!("[Discord]: {error}"),
                    chat_view::LineStyle::System,
                );
            }
            AppInput::DisconnectDiscord => {
                if let Some(client) = self.discord_client.take() {
                    runtime::spawn(async move {
                        client.shutdown().await;
                    });
                }
                let discord_names: Vec<String> = self
                    .discord_channels
                    .all()
                    .iter()
                    .map(|channel| channel.display_name.clone())
                    .collect();
                let active_discord_channel = discord_names.contains(&self.active_channel);
                self.discord_connection = ConnectionState::Offline;
                self.discord_status = "Discord disconnected".to_string();
                self.discord_user_id = None;
                self.discord_channels.clear();
                for name in discord_names {
                    self.chat_histories.remove(&name);
                    self.unread_counts.remove(&name);
                    self.mention_counts.remove(&name);
                }
                self.append_message(
                    SERVER_TAB,
                    "System",
                    "Discord disconnected.",
                    chat_view::LineStyle::System,
                );
                if active_discord_channel {
                    self.active_channel = SERVER_TAB.to_string();
                    self.show_channel_history();
                }
                self.refresh_channels(&sender);
            }
            AppInput::DiscordDisconnected => {
                let was_active = self.discord_connection != ConnectionState::Offline;
                self.discord_connection = ConnectionState::Offline;
                self.discord_client = None;
                self.discord_user_id = None;
                self.discord_status = "Discord disconnected".to_string();
                if was_active {
                    self.append_message(
                        SERVER_TAB,
                        "System",
                        "Discord gateway disconnected.",
                        chat_view::LineStyle::System,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_status_covers_identify_and_connection_failed() {
        assert!(is_terminal_irc_status("Disconnected"));
        assert!(is_terminal_irc_status(
            "Connection failed: identify error: boom"
        ));
        assert!(is_terminal_irc_status("Connection failed: stream error: x"));
        assert!(is_terminal_irc_status("NickServ auth failed: old"));
        assert!(!is_terminal_irc_status("Connecting…"));
        assert!(!is_terminal_irc_status("Connected"));
    }
}
