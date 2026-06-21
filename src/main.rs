use relm4::{gtk, ComponentParts, ComponentSender, RelmApp, SimpleComponent, RelmWidgetExt};
use gtk::prelude::*;
use std::collections::HashMap;
use std::thread;
use irc::client::prelude::*;
use futures::prelude::*;

// --- Gruvbox Dark CSS ---
const GRUVBOX_CSS: &str = "
    window { background-color: #282828; }
    label { color: #ebdbb2; font-family: monospace; }
    
    .sidebar { background-color: #1d2021; }
    .sidebar-title { font-weight: bold; color: #fe8019; }
    .sidebar-subtitle { font-weight: bold; color: #b8bb26; font-size: 0.9em; }

    button { 
        background-color: #3c3836; color: #b8bb26; 
        border: 1px solid #504945; border-radius: 4px; 
        padding: 6px 12px; font-family: monospace;
    }
    button:hover { background-color: #504945; }
    
    entry {
        background-color: #3c3836; color: #ebdbb2;
        border: 1px solid #504945; border-radius: 4px;
        padding: 8px; font-family: monospace;
    }
    entry:focus { border: 1px solid #fe8019; }
    
    .chat-text { background-color: #282828; color: #ebdbb2; font-family: monospace; padding: 8px; }
    
    .user-btn { 
        background-color: transparent; color: #83a598; border: none; box-shadow: none;
        padding: 4px 12px; font-family: monospace; 
    }
    .user-btn:hover { background-color: #3c3836; color: #ebdbb2; }

    .fav-btn {
        background-color: transparent;
        color: #fabd2f;
        border: 1px solid transparent; 
        box-shadow: none;
        padding: 6px 8px; 
        font-family: monospace;
    }
    .fav-btn:hover { 
        background-color: #3c3836; 
        border: 1px solid #504945; 
        color: #fbf1c7; 
    }

    .mute-btn {
        background-color: transparent;
        color: #928374;
        border: 1px solid transparent; 
        box-shadow: none;
        padding: 4px 8px; 
        font-family: monospace;
    }
    .mute-btn:hover { 
        background-color: #3c3836; 
        border: 1px solid #504945; 
        color: #ebdbb2; 
    }

    .muted-user {
        color: #928374;
        text-decoration: line-through;
    }
";

#[derive(Debug, Clone)]
pub enum AppInput {
    UpdateNickname(String),
    UpdateServer(String),
    UpdatePassword(String),
    Connect,
    NetworkStatus(String),
    NetworkConnected(irc::client::Sender),
    SelectChannel(String),
    JoinChannel(String),
    ToggleFavorite(String),
    ToggleMute { channel: String, user: String },
    ReceiveMessage { channel: String, user: String, body: String },
    ReceiveServerMessage(String),
    BatchAddUsers { channel: String, users: Vec<String> },
    UserJoined { channel: String, user: String },
    UserLeft { channel: String, user: String },
    UserQuit { user: String },
    SendMessage(String),
}

struct AppModel {
    status: String,
    active_channel: String,
    channels: Vec<String>,
    favorite_channels: Vec<String>,
    muted_users: HashMap<String, Vec<String>>,
    chat_histories: HashMap<String, String>,
    channel_users: HashMap<String, Vec<String>>,
    irc_sender: Option<irc::client::Sender>,
    nickname: String,
    server: String,
    password: String,
    channel_box: gtk::Box,
    user_box: gtk::Box,
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

                // Button to initiate Private Message
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
                    // Reusing JoinChannel since our logic handles targets without '#' as DMs
                    s1.input(AppInput::JoinChannel(u1.clone()));
                });

                // Button to toggle mute state
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
            set_title: Some("Rawhide Relay"),
            set_default_size: (1200, 700),

            gtk::Paned {
                set_orientation: gtk::Orientation::Horizontal,
                set_position: 240,
                
                #[wrap(Some)]
                set_start_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical, set_spacing: 12, set_width_request: 200,
                    add_css_class: "sidebar", set_margin_all: 0,

                    gtk::Label { set_label: "RAWHIDE RELAY", add_css_class: "sidebar-title", set_margin_top: 16 },
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
                    gtk::Label { #[watch] set_label: &format!("Status: {}", model.status), set_ellipsize: gtk::pango::EllipsizeMode::End, set_margin_start: 8, set_margin_end: 8 },
                    gtk::Button { set_label: "Connect to Server", set_margin_start: 12, set_margin_end: 12, connect_clicked => AppInput::Connect },
                    
                    gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                    gtk::Label { set_label: "Channels & DMs", add_css_class: "sidebar-subtitle", set_halign: gtk::Align::Start, set_margin_start: 12 },
                    gtk::Entry {
                        set_placeholder_text: Some("Join #channel or User"), set_margin_start: 12, set_margin_end: 12,
                        connect_activate[sender] => move |entry| {
                            let text = entry.text().to_string();
                            if !text.is_empty() {
                                entry.set_text("");
                                sender.input(AppInput::JoinChannel(text));
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

                        gtk::Label { #[watch] set_label: &format!("Active Context: {}", model.active_channel), set_halign: gtk::Align::Start },

                        gtk::ScrolledWindow {
                            set_vexpand: true, set_hexpand: true, set_propagate_natural_height: true,

                            gtk::Label {
                                set_halign: gtk::Align::Start, set_valign: gtk::Align::End,
                                set_selectable: true, set_wrap: true, add_css_class: "chat-text",
                                #[watch] set_label: model.chat_histories.get(&model.active_channel).unwrap_or(&String::new()),
                            }
                        },

                        gtk::Entry {
                            set_placeholder_text: Some("Type your message here and hit Enter..."), set_hexpand: true,
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
        let server_tab = String::from("Server");
        let dev_channel = String::from("#fedora-devel");
        let rust_channel = String::from("##rust");

        let mut chat_histories = HashMap::new();
        chat_histories.insert(
            server_tab.clone(),
            String::from("[System]: Ready. Enter password if registered and connect.\n"),
        );

        let channel_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        let user_box = gtk::Box::new(gtk::Orientation::Vertical, 0);

        let model = AppModel {
            status: String::from("Offline"),
            active_channel: server_tab.clone(),
            channels: vec![server_tab.clone(), dev_channel, rust_channel],
            favorite_channels: vec![
                server_tab.clone(),
                String::from("#fedora-devel"),
                String::from("##rust"),
            ],
            muted_users: HashMap::new(),
            chat_histories,
            channel_users: HashMap::new(),
            irc_sender: None,
            nickname: String::from("SisyphusCode"),
            server: String::from("irc.libera.chat"),
            password: String::new(),
            channel_box: channel_box.clone(),
            user_box: user_box.clone(),
        };

        let channel_box_ref = &model.channel_box;
        let user_box_ref = &model.user_box;
        let widgets = view_output!();

        let parts = ComponentParts { model, widgets };
        parts.model.refresh_channels(&sender);
        parts.model.refresh_users(&sender);
        parts
    }

    fn update(&mut self, message: Self::Input, sender: ComponentSender<Self>) {
        match message {
            AppInput::UpdateNickname(nick) => { self.nickname = nick; }
            AppInput::UpdateServer(srv) => { self.server = srv; }
            AppInput::UpdatePassword(pwd) => { self.password = pwd; }

            AppInput::Connect => {
                if self.status == "Connecting..." || self.status == "Connected" {
                    return;
                }
                self.status = String::from("Connecting...");
                let sender_clone = sender.clone();

                let channels_to_join: Vec<String> = self.channels
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
                        let config = Config {
                            nickname: Some(nickname.clone()),
                            server: Some(server_addr),
                            channels: channels_to_join,
                            port: Some(6697),
                            use_tls: Some(true),
                            nick_password: if pwd.is_empty() { None } else { Some(pwd) },
                            ..Config::default()
                        };

                        let mut client = match Client::from_config(config).await {
                            Ok(c) => c,
                            Err(_) => {
                                sender_clone.input(AppInput::NetworkStatus(String::from("Connection Failed.")));
                                return;
                            }
                        };

                        if client.identify().is_err() {
                            sender_clone.input(AppInput::NetworkStatus(String::from("Auth Failed.")));
                            return;
                        }

                        sender_clone.input(AppInput::NetworkConnected(client.sender()));

                        let mut stream = match client.stream() {
                            Ok(s) => s,
                            Err(_) => return,
                        };

                        while let Some(Ok(message)) = stream.next().await {
                            let user = message.source_nickname().unwrap_or("Unknown").to_string();

                            match message.command {
                                Command::PRIVMSG(target, body) => {
                                    let display_target = if target == nickname { user.clone() } else { target };
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
                                    sender_clone.input(AppInput::ReceiveServerMessage(format!("[Notice]: {}", body)));
                                }
                                Command::Response(code, args) => {
                                    if code == Response::RPL_NAMREPLY && args.len() >= 4 {
                                        let channel = args.iter()
                                            .find(|a| a.starts_with('#'))
                                            .cloned()
                                            .unwrap_or_else(|| args[2].clone());

                                        let users: Vec<String> = args.last()
                                            .unwrap_or(&String::new())
                                            .split_whitespace()
                                            .map(|s| s.to_string())
                                            .collect();

                                        sender_clone.input(AppInput::BatchAddUsers { channel, users });
                                    } else if code == Response::RPL_ENDOFNAMES {
                                    } else if args.len() > 1 {
                                        sender_clone.input(AppInput::ReceiveServerMessage(format!(
                                            "[{:?}]: {}",
                                            code,
                                            args[1..].join(" ")
                                        )));
                                    }
                                }
                                _ => {}
                            }
                        }
                    });
                });
            }

            AppInput::NetworkStatus(new_status) => {
                self.status = new_status;
            }

            AppInput::NetworkConnected(irc_tx) => {
                self.irc_sender = Some(irc_tx);
                self.status = String::from("Connected");
            }

            AppInput::SelectChannel(channel) => {
                self.active_channel = channel;
                self.refresh_users(&sender);
            }

            AppInput::JoinChannel(target) => {
                if !self.channels.contains(&target) {
                    self.channels.push(target.clone());
                    self.chat_histories
                        .insert(target.clone(), format!("[System]: Tracking {}\n", target));

                    self.refresh_channels(&sender);

                    if let Some(irc_tx) = &self.irc_sender {
                        if target.starts_with('#') {
                            let _ = irc_tx.send_join(&target);
                        }
                    }
                }

                self.active_channel = target;
                self.refresh_users(&sender);
            }

            AppInput::ToggleFavorite(channel) => {
                if self.favorite_channels.contains(&channel) {
                    self.favorite_channels.retain(|c| c != &channel);
                } else {
                    self.favorite_channels.push(channel.clone());
                }
                self.refresh_channels(&sender);
            }

            AppInput::ToggleMute { channel, user } => {
                let list = self.muted_users.entry(channel.clone()).or_insert_with(Vec::new);

                if list.contains(&user) {
                    list.retain(|u| u != &user);
                    let log = self.chat_histories.entry(channel.clone()).or_insert_with(String::new);
                    log.push_str(&format!("[System]: Unmuted {}\n", user));
                } else {
                    list.push(user.clone());
                    list.sort_by_key(|u| u.to_lowercase());
                    let log = self.chat_histories.entry(channel.clone()).or_insert_with(String::new);
                    log.push_str(&format!("[System]: Muted {}\n", user));
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

                let log = self.chat_histories.entry(channel).or_insert_with(String::new);
                log.push_str(&format!("<{}> {}\n", user, body));
            }

            AppInput::ReceiveServerMessage(body) => {
                let log = self.chat_histories.entry(String::from("Server")).or_insert_with(String::new);
                log.push_str(&format!("{}\n", body));
            }

            AppInput::BatchAddUsers { channel, users } => {
                let list = self.channel_users.entry(channel.clone()).or_insert_with(Vec::new);
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
                let list = self.channel_users.entry(channel.clone()).or_insert_with(Vec::new);
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
                if self.active_channel == "Server" {
                    return;
                }

                if let Some(irc_tx) = &self.irc_sender {
                    if irc_tx.send_privmsg(&self.active_channel, &text).is_ok() {
                        let log = self.chat_histories.entry(self.active_channel.clone()).or_insert_with(String::new);
                        log.push_str(&format!("<{}> {}\n", self.nickname, text));
                    }
                } else {
                    let log = self.chat_histories.entry(self.active_channel.clone()).or_insert_with(String::new);
                    log.push_str("[System]: Cannot send message, not connected.\n");
                }
            }
        }
    }
}

fn main() {
    let app = RelmApp::new("org.Sisyphus.RawhideRelay");
    
    let provider = gtk::CssProvider::new();
    provider.load_from_data(GRUVBOX_CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    app.run::<AppModel>(());
}
