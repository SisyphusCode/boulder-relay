mod config;
mod theme;

use config::Settings;
use futures::prelude::*;
use gtk::glib::{self, DateTime};
use gtk::prelude::*;
use irc::client::prelude::*;
use relm4::{gtk, ComponentParts, ComponentSender, RelmApp, RelmWidgetExt, SimpleComponent};
use std::collections::HashMap;
use std::thread;

const DEFAULT_SERVER: &str = "irc.libera.chat";
const DEFAULT_NICKNAME: &str = "SisyphusCode";
const DEFAULT_PORT: u16 = 6697;
const SERVER_TAB: &str = "Server";

/// Rocky Linux community channels on Libera.Chat (see wiki.rockylinux.org/irc).
const DEFAULT_CHANNELS: &[&str] = &[
    "#rockylinux",
    "#rockylinux-devel",
    "#rockylinux-social",
];

#[derive(Copy, Clone)]
enum LineStyle {
    Normal,
    SelfMsg,
    System,
    Mention,
}

#[derive(Clone)]
struct ChatLine {
    text: String,
    style: LineStyle,
}

const HELP_TEXT: &str = "Commands: /join #chan, /part [#chan], /msg nick text, /clear, /nick name, /help\n";

#[derive(Debug, Clone, PartialEq, Eq)]
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
    NetworkStatus(String),
    NetworkConnected(irc::client::Sender),
    SelectChannel(String),
    JoinChannel(String),
    PartChannel(String),
    ClearChannel(String),
    ToggleFavorite(String),
    ToggleMute { channel: String, user: String },
    ReceiveMessage { channel: String, user: String, body: String },
    ReceiveServerMessage(String),
    BatchAddUsers { channel: String, users: Vec<String> },
    UserJoined { channel: String, user: String },
    UserLeft { channel: String, user: String },
    UserQuit { user: String },
    SendMessage(String),
    SaveSettings,
}

struct AppModel {
    connection: ConnectionState,
    status: String,
    active_channel: String,
    channels: Vec<String>,
    favorite_channels: Vec<String>,
    muted_users: HashMap<String, Vec<String>>,
    chat_histories: HashMap<String, Vec<ChatLine>>,
    channel_users: HashMap<String, Vec<String>>,
    irc_sender: Option<irc::client::Sender>,
    nickname: String,
    server: String,
    password: String,
    channel_box: gtk::Box,
    user_box: gtk::Box,
    chat_view: gtk::TextView,
}

impl AppModel {
    fn normalized_nick(user: &str) -> String {
        user.trim_start_matches(&['@', '+', '%', '~', '&'][..]).to_string()
    }

    fn is_muted(&self, channel: &str, user: &str) -> bool {
        let clean = Self::normalized_nick(user);
        self.muted_users
            .get(channel)
            .map(|users| users.iter().any(|u| u == &clean))
            .unwrap_or(false)
    }

    fn timestamp_prefix() -> String {
        DateTime::now_local()
            .map(|dt| format!("[{}] ", dt.format("%H:%M").unwrap_or_default()))
            .unwrap_or_else(|_| String::from("[??:??] "))
    }

    fn settings_snapshot(&self) -> Settings {
        Settings {
            nickname: self.nickname.clone(),
            server: self.server.clone(),
            password: self.password.clone(),
            favorites: self.favorite_channels.clone(),
            last_channel: self.active_channel.clone(),
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
    }

    fn message_style(&self, user: &str, body: &str) -> LineStyle {
        if user == "System" {
            return LineStyle::System;
        }
        let clean = Self::normalized_nick(user);
        if clean.eq_ignore_ascii_case(&self.nickname) {
            return LineStyle::SelfMsg;
        }
        if body.contains(&self.nickname) {
            return LineStyle::Mention;
        }
        LineStyle::Normal
    }

    fn append_line(&mut self, channel: &str, line: &str, style: LineStyle) {
        let history = self
            .chat_histories
            .entry(channel.to_string())
            .or_insert_with(Vec::new);
        history.push(ChatLine {
            text: line.to_string(),
            style,
        });

        if self.active_channel == channel {
            self.append_styled_to_chat_view(line, style);
        }
    }

    fn append_styled_to_chat_view(&self, line: &str, style: LineStyle) {
        let buffer = self.chat_view.buffer();
        let mut end = buffer.end_iter();
        buffer.insert_with_tags_by_name(&mut end, line, &[Self::style_tag(style)]);

        let mark = buffer.create_mark(None, &buffer.end_iter(), false);
        self.chat_view.scroll_to_mark(&mark, 0.0, false, 0.0, 0.0);
    }

    fn append_to_chat_view(&self, line: &str) {
        self.append_styled_to_chat_view(line, LineStyle::Normal);
    }

    fn persist_settings(&self) {
        if let Err(error) = self.settings_snapshot().save() {
            eprintln!("Failed to save Boulder Relay settings: {error}");
        }
    }

    fn show_channel_history(&self) {
        let buffer = self.chat_view.buffer();
        buffer.set_text("");

        if let Some(lines) = self.chat_histories.get(&self.active_channel) {
            for line in lines {
                self.append_styled_to_chat_view(&line.text, line.style);
            }
        }
    }

    fn refresh_channels(&self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.channel_box.first_child() {
            self.channel_box.remove(&child);
        }

        let mut favorites = Vec::new();
        let mut others = Vec::new();

        for channel in &self.channels {
            if self.favorite_channels.contains(channel) {
                favorites.push(channel.clone());
            } else {
                others.push(channel.clone());
            }
        }

        for channel in favorites.into_iter().chain(others.into_iter()) {
            let is_favorite = self.favorite_channels.contains(&channel);

            let row = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(4)
                .build();

            let select_btn = gtk::Button::with_label(&channel);
            select_btn.set_hexpand(true);
            select_btn.set_halign(gtk::Align::Fill);

            let s1 = sender.clone();
            let ch1 = channel.clone();
            select_btn.connect_clicked(move |_| {
                s1.input(AppInput::SelectChannel(ch1.clone()));
            });

            let fav_icon = if is_favorite { "★" } else { "☆" };
            let fav_btn = gtk::Button::with_label(fav_icon);
            fav_btn.add_css_class("fav-btn");

            let s2 = sender.clone();
            let ch2 = channel.clone();
            fav_btn.connect_clicked(move |_| {
                s2.input(AppInput::ToggleFavorite(ch2.clone()));
            });

            row.append(&select_btn);
            row.append(&fav_btn);

            if channel.starts_with('#') {
                let part_btn = gtk::Button::with_label("×");
                part_btn.add_css_class("part-btn");
                part_btn.set_tooltip_text(Some("Leave channel"));

                let s3 = sender.clone();
                let ch3 = channel.clone();
                part_btn.connect_clicked(move |_| {
                    s3.input(AppInput::PartChannel(ch3.clone()));
                });

                row.append(&part_btn);
            }

            self.channel_box.append(&row);
        }
    }

    fn refresh_users(&self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.user_box.first_child() {
            self.user_box.remove(&child);
        }

        if let Some(users) = self.channel_users.get(&self.active_channel) {
            for user in users {
                let clean_user = Self::normalized_nick(user);
                let muted = self.is_muted(&self.active_channel, user);

                let row = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(4)
                    .build();

                let dm_btn = gtk::Button::with_label(user);
                dm_btn.set_hexpand(true);
                dm_btn.set_halign(gtk::Align::Fill);
                dm_btn.add_css_class("user-btn");
                if muted {
                    dm_btn.add_css_class("muted-user");
                }

                let s1 = sender.clone();
                let u1 = clean_user.clone();
                dm_btn.connect_clicked(move |_| {
                    s1.input(AppInput::JoinChannel(u1.clone()));
                });

                let mute_icon = if muted { "🔇" } else { "🔊" };
                let mute_btn = gtk::Button::with_label(mute_icon);
                mute_btn.add_css_class("mute-btn");

                let s2 = sender.clone();
                let c2 = self.active_channel.clone();
                let u2 = clean_user.clone();
                mute_btn.connect_clicked(move |_| {
                    s2.input(AppInput::ToggleMute {
                        channel: c2.clone(),
                        user: u2.clone(),
                    });
                });

                row.append(&dm_btn);
                row.append(&mute_btn);
                self.user_box.append(&row);
            }
        }
    }
}

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppInput;
    type Output = ();

    view! {
        gtk::Window {
            set_default_size: (1200, 700),
            add_css_class: "boulder-relay",

            connect_close_request[sender] => move |_| {
                sender.input(AppInput::SaveSettings);
                glib::Propagation::Proceed
            },

            gtk::Paned {
                set_orientation: gtk::Orientation::Horizontal,
                set_position: 240,

                #[wrap(Some)]
                set_start_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical, set_spacing: 12, set_width_request: 200,
                    add_css_class: "sidebar", set_margin_all: 0,

                    gtk::Label { set_label: "BOULDER RELAY", add_css_class: "sidebar-title", set_margin_top: 16 },
                    gtk::Label { set_label: "Rocky Linux on Libera", set_margin_start: 12, set_margin_end: 12 },
                    gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                    gtk::Label { set_label: "Network Configuration", add_css_class: "sidebar-subtitle", set_halign: gtk::Align::Start, set_margin_start: 12 },
                    gtk::Entry {
                        set_text: &model.nickname, set_placeholder_text: Some("Nickname"), set_margin_start: 12, set_margin_end: 12,
                        connect_changed[sender] => move |entry| { sender.input(AppInput::UpdateNickname(entry.text().to_string())); }
                    },
                    gtk::Entry {
                        set_text: &model.password, set_placeholder_text: Some("NickServ Password (Opt)"), set_margin_start: 12, set_margin_end: 12,
                        set_visibility: false,
                        connect_changed[sender] => move |entry| { sender.input(AppInput::UpdatePassword(entry.text().to_string())); }
                    },
                    gtk::Entry {
                        set_text: &model.server, set_placeholder_text: Some("Server address"), set_margin_start: 12, set_margin_end: 12,
                        connect_changed[sender] => move |entry| { sender.input(AppInput::UpdateServer(entry.text().to_string())); }
                    },
                    gtk::Label {
                        #[watch]
                        set_label: &format!("Status: {}", model.status),
                        set_ellipsize: gtk::pango::EllipsizeMode::End,
                        set_margin_start: 8,
                        set_margin_end: 8,
                        add_css_class: match model.connection {
                            ConnectionState::Connected => "status-connected",
                            ConnectionState::Connecting => "status-connecting",
                            ConnectionState::Offline => "status-offline",
                        },
                    },
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 8,
                        set_margin_start: 12,
                        set_margin_end: 12,
                        gtk::Button {
                            set_label: "Connect",
                            set_sensitive: model.connection == ConnectionState::Offline,
                            connect_clicked => AppInput::Connect,
                        },
                        gtk::Button {
                            set_label: "Disconnect",
                            add_css_class: "destructive",
                            set_sensitive: model.connection == ConnectionState::Connected,
                            connect_clicked => AppInput::Disconnect,
                        },
                    },

                    gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                    gtk::Label { set_label: "Channels & DMs", add_css_class: "sidebar-subtitle", set_halign: gtk::Align::Start, set_margin_start: 12 },
                    gtk::Entry {
                        set_placeholder_text: Some("Join #channel, user, or /join …"), set_margin_start: 12, set_margin_end: 12,
                        connect_activate[sender] => move |entry| {
                            let text = entry.text().to_string();
                            if !text.is_empty() {
                                entry.set_text("");
                                sender.input(AppInput::SendMessage(text));
                            }
                        }
                    },

                    gtk::ScrolledWindow {
                        set_vexpand: true, set_hexpand: true,
                        #[local_ref] channel_box_ref -> gtk::Box {}
                    }
                },

                #[wrap(Some)]
                set_end_child = &gtk::Paned {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_position: 680,

                    #[wrap(Some)]
                    set_start_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical, set_spacing: 12, set_margin_all: 16, set_width_request: 300,
                        add_css_class: "chat-panel",

                        gtk::Label { #[watch] set_label: &format!("Active Context: {}", model.active_channel), set_halign: gtk::Align::Start },

                        gtk::ScrolledWindow {
                            set_vexpand: true, set_hexpand: true, set_propagate_natural_height: true,
                            #[local_ref] chat_view_ref -> gtk::TextView {
                                set_editable: false,
                                set_cursor_visible: false,
                                set_wrap_mode: gtk::WrapMode::Word,
                                set_vexpand: true,
                            }
                        },

                        gtk::Entry {
                            set_placeholder_text: Some("Message, /join #chan, or /msg nick text…"), set_hexpand: true,
                            connect_activate[sender] => move |entry| {
                                let text = entry.text().to_string();
                                if !text.is_empty() {
                                    entry.set_text("");
                                    sender.input(AppInput::SendMessage(text));
                                }
                            }
                        }
                    },

                    #[wrap(Some)]
                    set_end_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical, set_spacing: 12, set_width_request: 200,
                        add_css_class: "sidebar", set_margin_all: 0,

                        gtk::Label { set_label: "USERS IN CHANNEL", add_css_class: "sidebar-title", set_margin_top: 16, set_margin_bottom: 8 },
                        gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                        gtk::ScrolledWindow {
                            set_vexpand: true, set_hexpand: true,
                            #[local_ref] user_box_ref -> gtk::Box {}
                        }
                    }
                }
            }
        }
    }

    fn init(_init: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        theme::attach_window(&root);

        let settings = Settings::load();
        let server_tab = String::from(SERVER_TAB);
        let rocky_channels: Vec<String> = DEFAULT_CHANNELS.iter().map(|c| c.to_string()).collect();

        let mut chat_histories = HashMap::new();
        chat_histories.insert(
            server_tab.clone(),
            vec![
                ChatLine {
                    text: String::from(
                        "[System]: Ready for Libera.Chat. Register with NickServ and connect.\n",
                    ),
                    style: LineStyle::System,
                },
                ChatLine {
                    text: String::from(
                        "[System]: Rocky channels require a registered nick (wiki.rockylinux.org/irc).\n",
                    ),
                    style: LineStyle::System,
                },
                ChatLine {
                    text: String::from(
                        "[System]: Settings are saved to ~/.config/boulder-relay/settings.conf\n",
                    ),
                    style: LineStyle::System,
                },
            ],
        );

        let channel_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let user_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let chat_view = gtk::TextView::new();
        Self::setup_chat_tags(&chat_view);

        let mut channels = vec![server_tab.clone()];
        channels.extend(rocky_channels.clone());

        let favorites = if settings.favorites.is_empty() {
            vec![server_tab.clone(), rocky_channels[0].clone(), rocky_channels[1].clone()]
        } else {
            settings.favorites.clone()
        };

        let active_channel = if settings.last_channel.is_empty()
            || !channels.contains(&settings.last_channel)
        {
            server_tab.clone()
        } else {
            settings.last_channel
        };

        let model = AppModel {
            connection: ConnectionState::Offline,
            status: String::from("Offline"),
            active_channel,
            channels,
            favorite_channels: favorites,
            muted_users: HashMap::new(),
            chat_histories,
            channel_users: HashMap::new(),
            irc_sender: None,
            nickname: if settings.nickname.is_empty() {
                String::from(DEFAULT_NICKNAME)
            } else {
                settings.nickname
            },
            server: if settings.server.is_empty() {
                String::from(DEFAULT_SERVER)
            } else {
                settings.server
            },
            password: settings.password,
            channel_box: channel_box.clone(),
            user_box: user_box.clone(),
            chat_view: chat_view.clone(),
        };

        let channel_box_ref = &model.channel_box;
        let user_box_ref = &model.user_box;
        let chat_view_ref = &model.chat_view;
        let widgets = view_output!();

        let mut parts = ComponentParts { model, widgets };
        parts.model.show_channel_history();
        parts.model.refresh_channels(&sender);
        parts.model.refresh_users(&sender);
        parts
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppInput::UpdateNickname(nick) => self.nickname = nick,
            AppInput::UpdateServer(srv) => self.server = srv,
            AppInput::UpdatePassword(pwd) => self.password = pwd,

            AppInput::SaveSettings => self.persist_settings(),

            AppInput::Connect => {
                if self.connection != ConnectionState::Offline {
                    return;
                }
                self.connection = ConnectionState::Connecting;
                self.status = String::from("Connecting...");
                self.persist_settings();

                let sender_clone = sender.clone();
                let channels_to_join: Vec<String> = self
                    .channels
                    .iter()
                    .filter(|c| c.starts_with('#'))
                    .cloned()
                    .collect();

                let nickname = self.nickname.clone();
                let server_addr = self.server.clone();
                let pwd = self.password.clone();

                thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().expect("Failed to build Tokio core");
                    rt.block_on(async {
                        let needs_nickserv = !pwd.is_empty();
                        let config = Config {
                            nickname: Some(nickname.clone()),
                            server: Some(server_addr),
                            channels: vec![],
                            port: Some(DEFAULT_PORT),
                            use_tls: Some(true),
                            nick_password: if needs_nickserv {
                                Some(pwd.clone())
                            } else {
                                None
                            },
                            ..Config::default()
                        };

                        let mut client = match Client::from_config(config).await {
                            Ok(c) => c,
                            Err(e) => {
                                sender_clone.input(AppInput::NetworkStatus(format!(
                                    "Connection failed: {e}"
                                )));
                                return;
                            }
                        };

                        if let Err(e) = client.identify() {
                            sender_clone.input(AppInput::NetworkStatus(format!(
                                "NickServ auth failed: {e}"
                            )));
                            return;
                        }

                        let irc_tx = client.sender();
                        sender_clone.input(AppInput::NetworkConnected(irc_tx.clone()));

                        let join_channels = |tx: &irc::client::Sender| {
                            for chan in &channels_to_join {
                                let _ = tx.send_join(chan);
                            }
                            if !channels_to_join.is_empty() {
                                sender_clone.input(AppInput::ReceiveServerMessage(format!(
                                    "[System]: Joining {} channel(s).",
                                    channels_to_join.len()
                                )));
                            }
                        };

                        let mut channels_joined = false;
                        let mut stream = match client.stream() {
                            Ok(s) => s,
                            Err(_) => return,
                        };

                        while let Some(result) = stream.next().await {
                            let message = match result {
                                Ok(m) => m,
                                Err(e) => {
                                    sender_clone.input(AppInput::ReceiveServerMessage(format!(
                                        "[Error]: {e}"
                                    )));
                                    continue;
                                }
                            };
                            let user = message
                                .source_nickname()
                                .unwrap_or("Unknown")
                                .to_string();

                            match message.command {
                                Command::PRIVMSG(target, body) => {
                                    let display_target = if target == nickname {
                                        user.clone()
                                    } else {
                                        target
                                    };
                                    sender_clone.input(AppInput::ReceiveMessage {
                                        channel: display_target,
                                        user,
                                        body,
                                    });
                                }
                                Command::JOIN(channel, _, _) => {
                                    sender_clone.input(AppInput::UserJoined {
                                        channel: channel.clone(),
                                        user: user.clone(),
                                    });
                                    sender_clone.input(AppInput::ReceiveMessage {
                                        channel,
                                        user: "System".to_string(),
                                        body: format!("{} joined.", user),
                                    });
                                }
                                Command::PART(channel, _) => {
                                    sender_clone.input(AppInput::UserLeft {
                                        channel: channel.clone(),
                                        user: user.clone(),
                                    });
                                    sender_clone.input(AppInput::ReceiveMessage {
                                        channel,
                                        user: "System".to_string(),
                                        body: format!("{} left.", user),
                                    });
                                }
                                Command::QUIT(_) => {
                                    sender_clone.input(AppInput::UserQuit { user });
                                }
                                Command::NOTICE(_, body) => {
                                    sender_clone.input(AppInput::ReceiveServerMessage(format!(
                                        "[Notice]: {body}"
                                    )));
                                    if needs_nickserv
                                        && !channels_joined
                                        && body.contains("You are now identified")
                                    {
                                        channels_joined = true;
                                        join_channels(&irc_tx);
                                    }
                                }
                                Command::Response(code, args) => {
                                    if !channels_joined {
                                        match code {
                                            Response::RPL_LOGGEDIN => {
                                                channels_joined = true;
                                                join_channels(&irc_tx);
                                            }
                                            Response::RPL_ENDOFMOTD | Response::ERR_NOMOTD
                                                if !needs_nickserv =>
                                            {
                                                channels_joined = true;
                                                join_channels(&irc_tx);
                                            }
                                            _ => {}
                                        }
                                    }

                                    if code == Response::RPL_NAMREPLY && args.len() >= 4 {
                                        let channel = args
                                            .iter()
                                            .find(|a| a.starts_with('#'))
                                            .cloned()
                                            .unwrap_or_else(|| args[2].clone());

                                        let users: Vec<String> = args
                                            .last()
                                            .unwrap_or(&String::new())
                                            .split_whitespace()
                                            .map(|s| s.to_string())
                                            .collect();

                                        sender_clone.input(AppInput::BatchAddUsers {
                                            channel,
                                            users,
                                        });
                                    } else if args.len() > 1 {
                                        sender_clone.input(AppInput::ReceiveServerMessage(
                                            format!("[{code:?}]: {}", args[1..].join(" ")),
                                        ));
                                    }
                                }
                                _ => {}
                            }
                        }

                        sender_clone.input(AppInput::NetworkStatus(String::from("Disconnected")));
                        sender_clone.input(AppInput::ReceiveServerMessage(
                            String::from("[System]: Connection closed."),
                        ));
                    });
                });
            }

            AppInput::Disconnect => {
                if let Some(irc_tx) = self.irc_sender.take() {
                    let _ = irc_tx.send_quit("Boulder Relay signing off");
                    self.connection = ConnectionState::Offline;
                    self.status = String::from("Offline");
                    self.append_line(
                        SERVER_TAB,
                        &format!(
                            "{}[System]: Disconnected by user.\n",
                            Self::timestamp_prefix()
                        ),
                        LineStyle::System,
                    );
                }
            }

            AppInput::NetworkStatus(new_status) => {
                self.status = new_status.clone();
                if new_status == "Disconnected" || new_status.starts_with("Connection failed") {
                    self.connection = ConnectionState::Offline;
                    self.irc_sender = None;
                }
            }

            AppInput::NetworkConnected(irc_tx) => {
                self.irc_sender = Some(irc_tx);
                self.connection = ConnectionState::Connected;
                self.status = String::from("Connected");
                self.append_line(
                    SERVER_TAB,
                    &format!(
                        "{}[System]: Connected to {} as {}.\n",
                        Self::timestamp_prefix(),
                        self.server,
                        self.nickname
                    ),
                    LineStyle::System,
                );
            }

            AppInput::SelectChannel(channel) => {
                self.active_channel = channel;
                self.show_channel_history();
                self.refresh_users(&sender);
                self.persist_settings();
            }

            AppInput::JoinChannel(target) => {
                if !self.channels.contains(&target) {
                    self.channels.push(target.clone());
                    self.chat_histories.insert(
                        target.clone(),
                        vec![ChatLine {
                            text: format!("[System]: Tracking {target}\n"),
                            style: LineStyle::System,
                        }],
                    );
                    self.refresh_channels(&sender);

                    if let Some(irc_tx) = &self.irc_sender {
                        if target.starts_with('#') {
                            let _ = irc_tx.send_join(&target);
                        }
                    }
                }

                self.active_channel = target;
                self.show_channel_history();
                self.refresh_users(&sender);
                self.persist_settings();
            }

            AppInput::PartChannel(channel) => {
                if channel == SERVER_TAB || !channel.starts_with('#') {
                    return;
                }

                if let Some(irc_tx) = &self.irc_sender {
                    let _ = irc_tx.send_part(&channel);
                }

                self.channels.retain(|c| c != &channel);
                self.chat_histories.remove(&channel);
                self.channel_users.remove(&channel);
                self.muted_users.remove(&channel);

                if self.active_channel == channel {
                    self.active_channel = String::from(SERVER_TAB);
                    self.show_channel_history();
                }

                self.refresh_channels(&sender);
                self.refresh_users(&sender);
                self.persist_settings();
            }

            AppInput::ClearChannel(channel) => {
                self.chat_histories.insert(channel.clone(), Vec::new());
                if self.active_channel == channel {
                    self.chat_view.buffer().set_text("");
                }
            }

            AppInput::ToggleFavorite(channel) => {
                if self.favorite_channels.contains(&channel) {
                    self.favorite_channels.retain(|c| c != &channel);
                } else {
                    self.favorite_channels.push(channel.clone());
                }
                self.refresh_channels(&sender);
                self.persist_settings();
            }

            AppInput::ToggleMute { channel, user } => {
                let list = self
                    .muted_users
                    .entry(channel.clone())
                    .or_insert_with(Vec::new);

                if list.contains(&user) {
                    list.retain(|u| u != &user);
                    self.append_line(
                        &channel,
                        &format!(
                            "{}[System]: Unmuted {}\n",
                            Self::timestamp_prefix(),
                            user
                        ),
                        LineStyle::System,
                    );
                } else {
                    list.push(user.clone());
                    list.sort_by_key(|u| u.to_lowercase());
                    self.append_line(
                        &channel,
                        &format!(
                            "{}[System]: Muted {}\n",
                            Self::timestamp_prefix(),
                            user
                        ),
                        LineStyle::System,
                    );
                }

                if self.active_channel == channel {
                    self.refresh_users(&sender);
                }
            }

            AppInput::ReceiveMessage { channel, user, body } => {
                if self.is_muted(&channel, &user) {
                    return;
                }

                if !self.channels.contains(&channel) && !channel.starts_with('#') {
                    self.channels.push(channel.clone());
                    self.refresh_channels(&sender);
                }

                let style = self.message_style(&user, &body);
                let line = format!(
                    "{}<{}> {}\n",
                    Self::timestamp_prefix(),
                    user,
                    body
                );
                self.append_line(&channel, &line, style);
            }

            AppInput::ReceiveServerMessage(body) => {
                let line = format!("{}{}\n", Self::timestamp_prefix(), body);
                self.append_line(SERVER_TAB, &line, LineStyle::System);
            }

            AppInput::BatchAddUsers { channel, users } => {
                let list = self
                    .channel_users
                    .entry(channel.clone())
                    .or_insert_with(Vec::new);
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
                let list = self
                    .channel_users
                    .entry(channel.clone())
                    .or_insert_with(Vec::new);
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

            AppInput::SendMessage(text) => {
                let text = text.trim();
                if text.is_empty() {
                    return;
                }

                if text.starts_with('/') {
                    let mut parts = text.splitn(3, ' ');
                    let command = parts.next().unwrap_or("");
                    match command {
                        "/join" => {
                            if let Some(channel) = parts.next() {
                                sender.input(AppInput::JoinChannel(channel.to_string()));
                            }
                            return;
                        }
                        "/msg" | "/query" => {
                            if let Some(target) = parts.next() {
                                let body = parts.next().unwrap_or("");
                                if !body.is_empty() {
                                    if let Some(irc_tx) = &self.irc_sender {
                                        let _ = irc_tx.send_privmsg(target, body);
                                        let line = format!(
                                            "{}<{}> {}\n",
                                            Self::timestamp_prefix(),
                                            self.nickname,
                                            body
                                        );
                                        self.append_line(target, &line, LineStyle::SelfMsg);
                                    }
                                } else {
                                    sender.input(AppInput::JoinChannel(target.to_string()));
                                }
                            }
                            return;
                        }
                        "/nick" => {
                            if let Some(nick) = parts.next() {
                                self.nickname = nick.to_string();
                                self.persist_settings();
                                self.append_line(
                                    SERVER_TAB,
                                    &format!(
                                        "{}[System]: Nickname updated locally to {}. Reconnect to apply.\n",
                                        Self::timestamp_prefix(),
                                        self.nickname
                                    ),
                                    LineStyle::System,
                                );
                            }
                            return;
                        }
                        "/part" => {
                            let target = parts
                                .next()
                                .map(str::to_string)
                                .unwrap_or_else(|| self.active_channel.clone());
                            sender.input(AppInput::PartChannel(target));
                            return;
                        }
                        "/clear" => {
                            sender.input(AppInput::ClearChannel(self.active_channel.clone()));
                            return;
                        }
                        "/help" => {
                            let channel = self.active_channel.clone();
                            self.append_line(&channel, HELP_TEXT, LineStyle::System);
                            return;
                        }
                        _ => {}
                    }
                }

                if self.active_channel == SERVER_TAB {
                    self.append_line(
                        SERVER_TAB,
                        &format!(
                            "{}[System]: Select a channel or DM before sending.\n",
                            Self::timestamp_prefix()
                        ),
                        LineStyle::System,
                    );
                    return;
                }

                if let Some(irc_tx) = &self.irc_sender {
                    if irc_tx.send_privmsg(&self.active_channel, text).is_ok() {
                        let channel = self.active_channel.clone();
                        let line = format!(
                            "{}<{}> {}\n",
                            Self::timestamp_prefix(),
                            self.nickname,
                            text
                        );
                        self.append_line(&channel, &line, LineStyle::SelfMsg);
                    }
                } else {
                    let channel = self.active_channel.clone();
                    self.append_line(
                        &channel,
                        &format!(
                            "{}[System]: Cannot send message, not connected.\n",
                            Self::timestamp_prefix()
                        ),
                        LineStyle::System,
                    );
                }
            }
        }
    }
}

fn main() {
    let app = RelmApp::new("org.Sisyphus.BoulderRelay");
    theme::load_css();
    app.run::<AppModel>(());
}
