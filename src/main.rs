mod channels;
mod config;
mod notify;
mod theme;

use config::Settings;
use futures::prelude::*;
use adw;
use gtk::glib::{self, DateTime};
use gtk::prelude::*;
use adw::prelude::*;
use irc::client::prelude::*;
use relm4::{gtk, ComponentParts, ComponentSender, RelmApp, RelmWidgetExt, SimpleComponent};
use std::collections::HashMap;
use std::thread;

const DEFAULT_SERVER: &str = "irc.libera.chat";
const DEFAULT_NICKNAME: &str = "SisyphusCode";
const DEFAULT_PORT: u16 = 6697;
const SERVER_TAB: &str = "Server";
// Default, but now overridable via settings.account_service
const DEFAULT_ACCOUNT_SERVICE: &str = "NickServ";

/// Gruvbox-inspired palette for per-nickname colors (for readability in chat).
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

const HELP_TEXT: &str = "Commands: /join chan, /j chan, /part [#chan], /msg nick text, /me text, /list, /clear, /nick name, /help\n\
/list or Browse button: search channels available on the server.\n\
Join box: type #channel (or nick for DM). Sidebar filter searches your joined list.\n\
Use the \"Register new account…\" button (or sidebar) for full nick registration + verification with configurable service.\n\
For Sisyphus Linux: see https://example-sisyphus-docs or #sisyphus on Libera.\n";

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
    Quit,
    SaveSettings,
}

struct AppModel {
    // Multi-server support
    servers: Vec<String>,
    current_server: String,
    senders: std::collections::HashMap<String, Option<irc::client::Sender>>,
    server_states: std::collections::HashMap<String, ConnectionState>,
    // Legacy single for compatibility during transition (will be removed)
    connection: ConnectionState,
    status: String,
    active_channel: String,
    channels: Vec<String>,
    favorite_channels: Vec<String>,
    muted_users: HashMap<String, Vec<String>>,
    chat_histories: HashMap<String, Vec<ChatLine>>, // key now "server:channel"
    channel_users: HashMap<String, Vec<String>>,
    irc_sender: Option<irc::client::Sender>, // current
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
    accounts: std::collections::HashMap<String, config::ServerAccount>,
    pending_register_email: Option<String>,
    ignored_users: std::collections::HashSet<String>,
}

impl AppModel {
    fn normalized_nick(user: &str) -> String {
        user.trim_start_matches(&['@', '+', '%', '~', '&'][..]).to_string()
    }

    fn nick_color_index(nick: &str) -> usize {
        let clean = Self::normalized_nick(nick);
        let hash = clean.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32));
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
            .map(|dt| format!("[{}] ", dt.format(&self.timestamp_format).unwrap_or_default()))
            .unwrap_or_else(|_| String::from("[??:??] "))
    }

    fn extra_channels(&self) -> Vec<String> {
        self.channels
            .iter()
            .filter(|channel| **channel != SERVER_TAB)
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
        // ensure current is in accounts
        let server = self.server.clone();
        snapshot.accounts.insert(server.clone(), config::ServerAccount {
            nick: self.nickname.clone(),
            password: self.password.clone(),
            service: self.account_service.clone(),
            auth_method: self.auth_method.clone(),
        });
        snapshot
    }

    fn sync_account_for_server(&mut self, server: &str) {
        let acc = config::ServerAccount {
            nick: self.nickname.clone(),
            password: self.password.clone(),
            service: self.account_service.clone(),
            auth_method: self.auth_method.clone(),
        };
        self.accounts.insert(server.to_string(), acc);
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

    fn send_irc_join(&self, target: &str) {
        if let Some(irc_tx) = &self.irc_sender {
            if channels::is_channel_target(target) {
                let _ = irc_tx.send_join(target);
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

        // Nickname color tags (layered on top of message styles)
        for (i, &color) in NICK_COLORS.iter().enumerate() {
            let tag_name = format!("nick-{}", i);
            let tag = gtk::TextTag::new(Some(&tag_name));
            tag.set_foreground(Some(color));
            tag.set_weight(600); // semi-bold for nicks
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

    fn append_line(&mut self, channel: &str, timestamp: String, user: Option<String>, body: String, style: LineStyle) {
        let history = self
            .chat_histories
            .entry(channel.to_string())
            .or_insert_with(Vec::new);
        history.push(ChatLine {
            timestamp: timestamp.clone(),
            user: user.clone(),
            body: body.clone(),
            style,
        });

        if self.active_channel == channel {
            if let Some(u) = &user {
                self.append_rich_chat_line(&timestamp, u, &body, style);
            } else {
                self.append_styled_to_chat_view(&format!("{}[System]: {}\n", timestamp, body), style);
            }
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

    /// Rich append for user messages with per-nick coloring.
    fn append_rich_chat_line(&self, ts: &str, user: &str, body: &str, style: LineStyle) {
        let buffer = self.chat_view.buffer();
        let mut end = buffer.end_iter();

        // Timestamp
        let ts_tag = match style {
            LineStyle::System => "system",
            _ => "normal",
        };
        buffer.insert_with_tags_by_name(&mut end, ts, &[ts_tag]);

        // Nick part (colored + weight) - skip for System messages
        let nick_part = if user == "System" {
            format!("[{}] ", user)
        } else if user.starts_with('*') {
            format!("{} ", user)
        } else {
            format!("<{}> ", user)
        };
        if user != "System" && self.nick_colors_enabled {
            let nick_tag = Self::nick_color_tag(user);
            buffer.insert_with_tags_by_name(&mut end, &nick_part, &[nick_tag.as_str(), Self::style_tag(style)]);
        } else {
            buffer.insert_with_tags_by_name(&mut end, &nick_part, &[Self::style_tag(style)]);
        }

        // Body with message style
        buffer.insert_with_tags_by_name(&mut end, &format!("{}\n", body), &[Self::style_tag(style)]);

        let mark = buffer.create_mark(None, &buffer.end_iter(), false);
        self.chat_view.scroll_to_mark(&mark, 0.0, false, 0.0, 0.0);
    }

    /// Store + rich display for chat messages (user or action).
    fn append_message(&mut self, channel: &str, user: &str, body: &str, style: LineStyle) {
        let ts = self.timestamp_prefix();
        let line = if user.starts_with('*') {
            format!("{}{} {}\n", ts, user, body)
        } else {
            format!("{}<{}> {}\n", ts, user, body)
        };

        // Store for history (plain text)
        let history = self
            .chat_histories
            .entry(channel.to_string())
            .or_insert_with(Vec::new);
        history.push(ChatLine {
            timestamp: ts.clone(),
            user: Some(user.to_string()),
            body: body.to_string(),
            style,
        });

        if self.active_channel == channel {
            self.append_rich_chat_line(&ts, user, body, style);
        }
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
                if let Some(u) = &line.user {
                    self.append_rich_chat_line(&line.timestamp, u, &line.body, line.style);
                } else {
                    let sys_line = format!("{}[System]: {}\n", line.timestamp, line.body);
                    self.append_styled_to_chat_view(&sys_line, line.style);
                }
            }
        }
    }

    fn append_section_header(&self, label: &str) {
        let header = gtk::Label::builder()
            .label(label)
            .halign(gtk::Align::Start)
            .margin_start(8)
            .margin_top(8)
            .margin_bottom(2)
            .build();
        header.add_css_class("channel-section");
        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);
        row.set_child(Some(&header));
        self.channel_box.append(&row);
    }

    fn append_channel_row(&self, sender: &ComponentSender<Self>, channel: &str) {
        let is_favorite = self.favorite_channels.iter().any(|c| c == channel);

        let content = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .build();

        let select_btn = gtk::Button::with_label(channel);
        select_btn.set_hexpand(true);
        select_btn.set_halign(gtk::Align::Fill);
        select_btn.set_tooltip_text(Some("Click to switch to this context"));

        let s1 = sender.clone();
        let ch1 = channel.to_string();
        select_btn.connect_clicked(move |_| {
            s1.input(AppInput::SelectChannel(ch1.clone()));
        });

        let fav_icon = if is_favorite { "★" } else { "☆" };
        let fav_btn = gtk::Button::with_label(fav_icon);
        fav_btn.add_css_class("fav-btn");
        fav_btn.set_tooltip_text(Some(if is_favorite {
            "Remove from favorites"
        } else {
            "Add to favorites"
        }));

        let s2 = sender.clone();
        let ch2 = channel.to_string();
        fav_btn.connect_clicked(move |_| {
            s2.input(AppInput::ToggleFavorite(ch2.clone()));
        });

        content.append(&select_btn);
        content.append(&fav_btn);

        if channel.starts_with('#') {
            let part_btn = gtk::Button::with_label("×");
            part_btn.add_css_class("part-btn");
            part_btn.set_tooltip_text(Some("Leave channel"));

            let s3 = sender.clone();
            let ch3 = channel.to_string();
            part_btn.connect_clicked(move |_| {
                s3.input(AppInput::PartChannel(ch3.clone()));
            });

            content.append(&part_btn);
        }

        let list_row = gtk::ListBoxRow::new();
        list_row.set_child(Some(&content));
        // Activate on row for keyboard support
        let s4 = sender.clone();
        let ch4 = channel.to_string();
        list_row.connect_activate(move |_| {
            s4.input(AppInput::SelectChannel(ch4.clone()));
        });

        self.channel_box.append(&list_row);
    }

    fn refresh_channels(&self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.channel_box.first_child() {
            self.channel_box.remove(&child);
        }

        let filter = self.channel_filter.to_lowercase();
        let matches_filter = |name: &str| -> bool {
            if filter.is_empty() {
                true
            } else {
                name.to_lowercase().contains(&filter)
            }
        };

        let mut favorites = Vec::new();
        let mut others = Vec::new();

        for channel in &self.channels {
            if !matches_filter(channel) {
                continue;
            }
            if self.favorite_channels.contains(channel) {
                favorites.push(channel.clone());
                continue;
            }
            others.push(channel.clone());
        }

        if !favorites.is_empty() {
            self.append_section_header("★ Favorites");
            for channel in &favorites {
                self.append_channel_row(sender, channel);
            }
        }

        if !others.is_empty() {
            others.sort_by_key(|name| name.to_lowercase());
            self.append_section_header("Channels & DMs");
            for channel in &others {
                self.append_channel_row(sender, channel);
            }
        } else if !filter.is_empty() && favorites.is_empty() {
            let hint = gtk::Label::builder()
                .label(format!("No matches for “{}”", self.channel_filter))
                .halign(gtk::Align::Start)
                .margin_start(12)
                .margin_top(4)
                .css_classes(["channel-section"])
                .build();
            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);
            row.set_child(Some(&hint));
            self.channel_box.append(&row);
        }
    }

    fn show_channel_list_dialog(&self, results: Vec<(String, u32, String)>, sender: &ComponentSender<Self>) {
        if results.is_empty() {
            sender.input(AppInput::ReceiveServerMessage(
                "No channels returned by server (or LIST not supported).".to_string()
            ));
            return;
        }

        let mut sorted = results;
        sorted.sort_by(|a, b| {
            // Prefer channels with more users, then name
            b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase()))
        });

        let dialog = gtk::Window::builder()
            .transient_for(self.window.upcast_ref::<gtk::Window>())
            .modal(true)
            .title("Browse Channels")
            .default_width(720)
            .default_height(520)
            .build();
        dialog.add_css_class("boulder-relay");

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_all(12);

        let header = gtk::Label::builder()
            .label(format!("{} channels found — type to filter", sorted.len()))
            .halign(gtk::Align::Start)
            .css_classes(["sidebar-subtitle"])
            .build();
        vbox.append(&header);

        let search = gtk::SearchEntry::builder()
            .placeholder_text("Filter by name or topic…")
            .hexpand(true)
            .build();
        vbox.append(&search);

        let scrolled = gtk::ScrolledWindow::builder()
            .vexpand(true)
            .build();

        let list_container = gtk::Box::new(gtk::Orientation::Vertical, 4);
        scrolled.set_child(Some(&list_container));
        vbox.append(&scrolled);

        // Pre-build all row widgets so we can show/hide on filter (efficient)
        let mut all_rows: Vec<(gtk::Box, String, String)> = Vec::new(); // (row_widget, name, topic)

        for (name, users, topic) in &sorted {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);

            let display_topic = if topic.is_empty() { "<no topic>" } else { topic.as_str() };
            let info = gtk::Label::builder()
                .label(format!("{}  ({} users)  {}", name, users, display_topic))
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

            list_container.append(&row);
            all_rows.push((row, name.clone(), topic.clone()));
        }

        // Live filter: show/hide rows
        let rows_for_filter = all_rows.clone();
        search.connect_changed(move |entry| {
            let q = entry.text().to_lowercase();
            for (row, name, topic) in &rows_for_filter {
                let hay = format!("{} {}", name, topic).to_lowercase();
                let visible = q.is_empty() || hay.contains(&q);
                row.set_visible(visible);
            }
        });

        // Close button row
        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        btn_box.set_halign(gtk::Align::End);
        let close_btn = gtk::Button::with_label("Close");
        let d2 = dialog.clone();
        close_btn.connect_clicked(move |_| { d2.close(); });
        btn_box.append(&close_btn);
        vbox.append(&btn_box);

        dialog.set_child(Some(&vbox));
        dialog.present();
    }

    fn show_preferences_dialog(&self, sender: &ComponentSender<Self>) {
        let dialog = gtk::Window::builder()
            .transient_for(self.window.upcast_ref::<gtk::Window>())
            .modal(true)
            .title("Preferences")
            .default_width(400)
            .default_height(300)
            .build();
        dialog.add_css_class("boulder-relay");

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
        vbox.set_margin_all(12);

        let nick_check = gtk::CheckButton::builder()
            .label("Enable nickname colors")
            .active(self.nick_colors_enabled)
            .build();
        vbox.append(&nick_check);

        let ts_label = gtk::Label::new(Some("Timestamp format (strftime):"));
        vbox.append(&ts_label);
        let ts_entry = gtk::Entry::builder()
            .text(&self.timestamp_format)
            .placeholder_text("%H:%M or %I:%M %p")
            .build();
        vbox.append(&ts_entry);

        // Theme picker
        let theme_label = gtk::Label::new(Some("Theme:"));
        vbox.append(&theme_label);
        let theme_box = gtk::Box::new(gtk::Orientation::Horizontal, 4);
        let gruv = gtk::Button::with_label("Gruvbox (default)");
        let sisy = gtk::Button::with_label("Sisyphus Blue");
        let def = gtk::Button::with_label("Adwaita");
        // For demo, they can trigger CSS reload (placeholder)
        theme_box.append(&gruv);
        theme_box.append(&sisy);
        theme_box.append(&def);
        vbox.append(&theme_box);

        let note = gtk::Label::builder()
            .label("Changes apply after reconnect or new messages. Use 'Preferences' to tune appearance. Full theme picker in future.")
            .wrap(true)
            .build();
        vbox.append(&note);

        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        btn_box.set_halign(gtk::Align::End);

        let cancel = gtk::Button::with_label("Cancel");
        let d1 = dialog.clone();
        cancel.connect_clicked(move |_| { d1.close(); });
        btn_box.append(&cancel);

        let apply = gtk::Button::with_label("Apply");
        let s = sender.clone();
        let d2 = dialog.clone();
        let nick_c = nick_check.clone();
        let ts_e = ts_entry.clone();
        apply.connect_clicked(move |_| {
            s.input(AppInput::UpdateNickColorsEnabled(nick_c.is_active()));
            s.input(AppInput::UpdateTimestampFormat(ts_e.text().to_string()));
            d2.close();
        });
        btn_box.append(&apply);

        vbox.append(&btn_box);
        dialog.set_child(Some(&vbox));
        dialog.present();
    }

    fn show_register_dialog(&self, sender: &ComponentSender<Self>) {
        let dialog = gtk::Window::builder()
            .transient_for(self.window.upcast_ref::<gtk::Window>())
            .modal(true)
            .title("Account Registration")
            .default_width(450)
            .default_height(380)
            .build();
        dialog.add_css_class("boulder-relay");

        let main_vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
        main_vbox.set_margin_all(12);

        // === Status / Error label ===
        let status_label = gtk::Label::builder()
            .label("")
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["error"])
            .build();
        main_vbox.append(&status_label);

        // === Service Name (configurable) ===
        let service_label = gtk::Label::new(Some("Account Service:"));
        main_vbox.append(&service_label);
        let service_entry = gtk::Entry::builder()
            .text(&self.account_service)
            .placeholder_text("NickServ")
            .tooltip_text("Usually NickServ. Change for other networks (e.g. AuthServ, Q).")
            .build();
        main_vbox.append(&service_entry);

        // === Registration Section ===
        let reg_box = gtk::Box::new(gtk::Orientation::Vertical, 6);

        let nick_entry = gtk::Entry::builder()
            .text(&self.nickname)
            .placeholder_text("Desired nickname")
            .build();
        reg_box.append(&gtk::Label::new(Some("Nickname:")));
        reg_box.append(&nick_entry);

        let pass_entry = gtk::Entry::builder()
            .visibility(false)
            .placeholder_text("Password")
            .build();
        reg_box.append(&gtk::Label::new(Some("Password:")));
        reg_box.append(&pass_entry);

        let confirm_entry = gtk::Entry::builder()
            .visibility(false)
            .placeholder_text("Confirm password")
            .build();
        reg_box.append(&gtk::Label::new(Some("Confirm Password:")));
        reg_box.append(&confirm_entry);

        // Email-less checkbox
        let no_email_check = gtk::CheckButton::builder()
            .label("Register without email (if the network allows it)")
            .active(false)
            .build();
        reg_box.append(&no_email_check);

        let email_entry = gtk::Entry::builder()
            .placeholder_text("your@email.com (required on most networks)")
            .build();
        let email_label = gtk::Label::new(Some("Email:"));
        reg_box.append(&email_label);
        reg_box.append(&email_entry);

        // Toggle email field visibility
        let email_e_clone = email_entry.clone();
        let email_l_clone = email_label.clone();
        no_email_check.connect_toggled(move |check| {
            let visible = !check.is_active();
            email_e_clone.set_visible(visible);
            email_l_clone.set_visible(visible);
        });

        main_vbox.append(&reg_box);

        // === Verify Section (separate) ===
        let verify_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        verify_box.set_margin_top(12);

        let verify_label = gtk::Label::builder()
            .label("After receiving the email, enter the verification code here:")
            .wrap(true)
            .halign(gtk::Align::Start)
            .build();
        verify_box.append(&verify_label);

        let verify_nick = gtk::Entry::builder()
            .text(&self.nickname)
            .placeholder_text("Your nickname")
            .build();
        verify_box.append(&gtk::Label::new(Some("Nickname:")));
        verify_box.append(&verify_nick);

        let code_entry = gtk::Entry::builder()
            .placeholder_text("Code from email")
            .build();
        verify_box.append(&gtk::Label::new(Some("Verification Code:")));
        verify_box.append(&code_entry);

        main_vbox.append(&verify_box);

        // === Buttons ===
        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        btn_box.set_halign(gtk::Align::End);
        btn_box.set_margin_top(12);

        let cancel = gtk::Button::with_label("Close");
        let d_close = dialog.clone();
        cancel.connect_clicked(move |_| { d_close.close(); });
        btn_box.append(&cancel);

        let reg_btn = gtk::Button::with_label("Register Account");
        let s = sender.clone();
        let d2 = dialog.clone();
        let status = status_label.clone();
        let serv_e = service_entry.clone();
        let nick_e = nick_entry.clone();
        let p_e = pass_entry.clone();
        let c_e = confirm_entry.clone();
        let e_e = email_entry.clone();
        let no_email = no_email_check.clone();
        reg_btn.connect_clicked(move |_| {
            let service = serv_e.text().to_string().trim().to_string();
            let nick = nick_e.text().to_string().trim().to_string();
            let pass = p_e.text().to_string();
            let conf = c_e.text().to_string();
            let email = if no_email.is_active() { String::new() } else { e_e.text().to_string().trim().to_string() };

            // Validation
            if service.is_empty() {
                status.set_label("Account Service cannot be empty.");
                return;
            }
            if nick.is_empty() {
                status.set_label("Nickname is required.");
                return;
            }
            if pass.is_empty() {
                status.set_label("Password is required.");
                return;
            }
            if pass != conf {
                status.set_label("Passwords do not match.");
                return;
            }
            if !no_email.is_active() && email.is_empty() {
                status.set_label("Email is required (or enable email-less registration).");
                return;
            }

            // Persist service name
            s.input(AppInput::UpdateAccountService(service.clone()));

            s.input(AppInput::SubmitRegistration { nick, password: pass, email });
            // Keep dialog open so user can verify next
            status.set_label("Registration command sent. Check the Server tab and your email.");
        });
        btn_box.append(&reg_btn);

        let verify_btn = gtk::Button::with_label("Verify Registration");
        let s2 = sender.clone();
        let d3 = dialog.clone();
        let status2 = status_label.clone();
        let v_nick = verify_nick.clone();
        let code_e = code_entry.clone();
        verify_btn.connect_clicked(move |_| {
            let nick = v_nick.text().to_string().trim().to_string();
            let code = code_e.text().to_string().trim().to_string();

            if nick.is_empty() || code.is_empty() {
                status2.set_label("Nickname and code are required for verification.");
                return;
            }
            s2.input(AppInput::SubmitVerification { nick, code });
            status2.set_label("Verification command sent. Check Server tab for result.");
        });
        btn_box.append(&verify_btn);

        // Forgot password helper
        let forgot_btn = gtk::Button::with_label("Forgot password? (Send recovery)");
        let s3 = sender.clone();
        let status3 = status_label.clone();
        let e_e2 = email_entry.clone();
        let nick_e2 = nick_entry.clone();
        forgot_btn.connect_clicked(move |_| {
            let email_str = e_e2.text().to_string();
            let email = email_str.trim();
            let nick_str = nick_e2.text().to_string();
            let nick = nick_str.trim();
            if email.is_empty() && nick.is_empty() {
                status3.set_label("Enter email or nick for recovery.");
                return;
            }
            let service = service_entry.text().to_string();
            let cmd = if !email.is_empty() {
                format!("SENDPASS {}", email)
            } else {
                format!("RESETPASS {}", nick)
            };
            s3.input(AppInput::SendRawPrivmsg { target: service, msg: cmd });
            status3.set_label("Recovery request sent. Check email or server messages.");
        });
        btn_box.append(&forgot_btn);

        main_vbox.append(&btn_box);

        dialog.set_child(Some(&main_vbox));
        dialog.present();
    }

    fn show_account_manager(&self, sender: &ComponentSender<Self>) {
        let dialog = gtk::Window::builder()
            .transient_for(self.window.upcast_ref::<gtk::Window>())
            .modal(true)
            .title("Account Manager")
            .default_width(500)
            .default_height(400)
            .build();
        dialog.add_css_class("boulder-relay");

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_all(12);

        let info = gtk::Label::new(Some("Manage accounts per server. Use the registration dialog for new accounts."));
        vbox.append(&info);

        for (srv, acc) in &self.accounts {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let label = gtk::Label::new(Some(&format!("{}: {} (service: {})", srv, acc.nick, acc.service)));
            row.append(&label);

            let change_btn = gtk::Button::with_label("Change Pass");
            let s = sender.clone();
            let service_c = acc.service.clone();
            change_btn.connect_clicked(move |_| {
                // Simple: open prompt via input or note
                s.input(AppInput::SendRawPrivmsg { target: service_c.clone(), msg: format!("SET PASSWORD <old> <new> (use /msg for now)") });
            });
            row.append(&change_btn);

            let ghost_btn = gtk::Button::with_label("Ghost");
            let s2 = sender.clone();
            let nick_c = acc.nick.clone();
            let service_c2 = acc.service.clone();
            ghost_btn.connect_clicked(move |_| {
                s2.input(AppInput::SendRawPrivmsg { target: service_c2.clone(), msg: format!("GHOST {} <pass>", nick_c) });
            });
            row.append(&ghost_btn);

            vbox.append(&row);
        }

        let close = gtk::Button::with_label("Close");
        let d = dialog.clone();
        close.connect_clicked(move |_| { d.close(); });
        vbox.append(&close);

        dialog.set_child(Some(&vbox));
        dialog.present();
    }

    fn show_log_viewer(&self, _sender: &ComponentSender<Self>) {
        let dialog = gtk::Window::builder()
            .transient_for(self.window.upcast_ref::<gtk::Window>())
            .modal(true)
            .title("Log Viewer & Search")
            .default_width(600)
            .default_height(500)
            .build();
        dialog.add_css_class("boulder-relay");

        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_all(12);

        let search = gtk::Entry::builder()
            .placeholder_text("Search logs...")
            .build();
        vbox.append(&search);

        let scrolled = gtk::ScrolledWindow::new();
        let textview = gtk::TextView::new();
        textview.set_editable(false);
        textview.set_wrap_mode(gtk::WrapMode::Word);
        scrolled.set_child(Some(&textview));
        vbox.append(&scrolled);

        // Load current history as "log"
        let buffer = textview.buffer();
        if let Some(lines) = self.chat_histories.get(&self.active_channel) {
            for line in lines {
                let prefix = if let Some(u) = &line.user {
                    format!("<{}> ", u)
                } else {
                    "[System] ".to_string()
                };
                buffer.insert(&mut buffer.end_iter(), &format!("{}{}\n", prefix, line.body));
            }
        }

        let search_clone = search.clone();
        let tv_clone = textview.clone();
        search.connect_changed(move |e| {
            let query = e.text().to_lowercase();
            if query.is_empty() { return; }
            // Simple search: select first match (basic impl)
            let buf = tv_clone.buffer();
            let text = buf.text(&buf.start_iter(), &buf.end_iter(), false);
            if let Some(pos) = text.to_lowercase().find(&query) {
                let mut start = buf.iter_at_offset(pos as i32);
                let end = buf.iter_at_offset((pos + query.len()) as i32);
                buf.select_range(&start, &end);
                tv_clone.scroll_to_iter(&mut start, 0.0, false, 0.0, 0.0);
            }
        });

        let close = gtk::Button::with_label("Close");
        let d = dialog.clone();
        close.connect_clicked(move |_| { d.close(); });
        vbox.append(&close);

        dialog.set_child(Some(&vbox));
        dialog.present();
    }

    fn refresh_users(&self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.user_box.first_child() {
            self.user_box.remove(&child);
        }

        if let Some(users) = self.channel_users.get(&self.active_channel) {
            for user in users {
                let clean_user = Self::normalized_nick(user);
                let muted = self.is_muted(&self.active_channel, user);

                let content = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(4)
                    .build();

                let color = NICK_COLORS[Self::nick_color_index(user)];
                let dm_btn = gtk::Button::new();
                let dm_label = gtk::Label::new(None);
                dm_label.set_markup(&format!("<span foreground=\"{}\">{}</span>", color, user));
                dm_btn.set_child(Some(&dm_label));
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

                content.append(&dm_btn);
                content.append(&mute_btn);

                let list_row = gtk::ListBoxRow::new();
                list_row.set_child(Some(&content));
                self.user_box.append(&list_row);
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
        adw::Window {
            set_default_size: (1200, 700),
            add_css_class: "boulder-relay",

            connect_close_request[sender] => move |window| {
                sender.input(AppInput::SaveSettings);
                if model.background_on_close && model.connection == ConnectionState::Connected {
                    window.set_visible(false);
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            },

            #[wrap(Some)]
            set_content = &adw::ToolbarView {
                add_top_bar: &theme::build_titlebar(),

                #[wrap(Some)]
                set_content = &gtk::Paned {
                    set_orientation: gtk::Orientation::Horizontal,
                    set_position: 240,

                    #[wrap(Some)]
                    set_start_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical, set_spacing: 12, set_width_request: 200,
                    add_css_class: "sidebar", set_margin_all: 0,

                    gtk::Label { set_label: "BOULDER RELAY", add_css_class: "sidebar-title", set_margin_top: 16 },
                    gtk::Label { set_label: "GTK4 IRC Client — any network, any channel", set_margin_start: 12, set_margin_end: 12 },
                    gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                    gtk::Label { set_label: "Network Configuration", add_css_class: "sidebar-subtitle", set_halign: gtk::Align::Start, set_margin_start: 12 },
                    gtk::Entry {
                        set_text: &model.nickname, set_placeholder_text: Some("Nickname"), set_margin_start: 12, set_margin_end: 12,
                        connect_changed[sender] => move |entry| { sender.input(AppInput::UpdateNickname(entry.text().to_string())); }
                    },
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_spacing: 4,
                        set_margin_start: 12,
                        set_margin_end: 12,
                        gtk::Entry {
                            set_text: &model.password,
                            set_placeholder_text: Some("Account Password"),
                            set_hexpand: true,
                            set_visibility: false,
                            connect_changed[sender] => move |entry| { sender.input(AppInput::UpdatePassword(entry.text().to_string())); }
                        },
                        gtk::Button {
                            set_label: "👁",
                            set_tooltip_text: Some("Show or hide password"),
                            connect_clicked => move |button| {
                                if let Some(entry) = button.prev_sibling().and_downcast::<gtk::Entry>() {
                                    let visible = entry.property::<bool>("visibility");
                                    entry.set_visibility(!visible);
                                }
                            }
                        },
                    },
                    gtk::Button {
                        set_label: "Register new account…",
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_clicked => AppInput::OpenRegisterDialog,
                    },
                    gtk::Button {
                        set_label: "Manage Accounts",
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_clicked => AppInput::OpenAccountManager,
                    },
                    gtk::Entry {
                        set_text: &model.server, set_placeholder_text: Some("Server address"), set_margin_start: 12, set_margin_end: 12,
                        connect_changed[sender] => move |entry| { sender.input(AppInput::UpdateServer(entry.text().to_string())); }
                    },
                    gtk::Entry {
                        set_text: &model.account_service,
                        set_placeholder_text: Some("Account Service (e.g. NickServ)"),
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_changed[sender] => move |entry| {
                            sender.input(AppInput::UpdateAccountService(entry.text().to_string()));
                        }
                    },
                    gtk::Entry {
                        set_text: &model.auth_method,
                        set_placeholder_text: Some("Auth Method (nickserv / sasl_plain)"),
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_changed[sender] => move |entry| {
                            sender.input(AppInput::UpdateAuthMethod(entry.text().to_string()));
                        }
                    },
                    gtk::Entry {
                        set_placeholder_text: Some("Add server (e.g. irc.oftc.net)"),
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_activate[sender] => move |entry| {
                            let s = entry.text().to_string();
                            if !s.is_empty() {
                                entry.set_text("");
                                sender.input(AppInput::AddServer(s));
                            }
                        }
                    },
                    gtk::Button {
                        set_label: "View Logs",
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_clicked => AppInput::OpenLogViewer,
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
                        gtk::Button {
                            set_label: "Quit",
                            add_css_class: "destructive",
                            connect_clicked => AppInput::Quit,
                        },
                    },
                    gtk::CheckButton {
                        set_label: Some("Run in background when closed"),
                        set_active: model.background_on_close,
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_toggled[sender] => move |check| {
                            sender.input(AppInput::UpdateBackgroundOnClose(check.is_active()));
                        }
                    },
                    gtk::CheckButton {
                        set_label: Some("Desktop notifications"),
                        set_active: model.notifications_enabled,
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_toggled[sender] => move |check| {
                            sender.input(AppInput::UpdateNotificationsEnabled(check.is_active()));
                        }
                    },

                    gtk::Button {
                        set_label: "Preferences…",
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_clicked => AppInput::OpenPreferences,
                    },

                    gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                    gtk::Label { set_label: "Channels & DMs", add_css_class: "sidebar-subtitle", set_halign: gtk::Align::Start, set_margin_start: 12 },
                    gtk::Entry {
                        set_placeholder_text: Some("#channel to join, nick for DM, or /join …"),
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_activate[sender] => move |entry| {
                            let text = entry.text().to_string();
                            if !text.is_empty() {
                                entry.set_text("");
                                sender.input(AppInput::JoinEntry(text));
                            }
                        }
                    },

                    gtk::Button {
                        set_label: "🔍 Browse server channels…",
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_clicked => AppInput::BrowseChannels,
                    },

                    gtk::SearchEntry {
                        set_placeholder_text: Some("Filter your joined channels…"),
                        set_margin_start: 12,
                        set_margin_end: 12,
                        connect_changed[sender] => move |entry| {
                            sender.input(AppInput::UpdateChannelFilter(entry.text().to_string()));
                        }
                    },

                    gtk::ScrolledWindow {
                        set_vexpand: true, set_hexpand: true,
                        #[local_ref] channel_box_ref -> gtk::ListBox {}
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

                        gtk::Label { #[watch] set_label: &format!("Active: {}", model.active_channel), set_halign: gtk::Align::Start },
                        gtk::Label {
                            #[watch]
                            set_label: model.channel_topics.get(&model.active_channel).map(String::as_str).unwrap_or(""),
                            set_halign: gtk::Align::Start,
                            set_ellipsize: gtk::pango::EllipsizeMode::End,
                            set_css_classes: &["channel-topic"],
                            set_margin_start: 4,
                        },

                        gtk::Entry {
                            set_placeholder_text: Some("Search chat (Ctrl+F to focus)"),
                            set_margin_start: 4,
                            set_margin_end: 4,
                        },

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
                            #[local_ref] user_box_ref -> gtk::ListBox {}
                        }
                    }
                }
                }
            }
        }
    }

    fn init(_init: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        theme::attach_window(root.upcast_ref::<gtk::Window>());

        let settings = Settings::load();
        let server_tab = String::from(SERVER_TAB);

        let mut chat_histories = HashMap::new();
        let ts = "[??:??] ".to_string();
        chat_histories.insert(
            server_tab.clone(),
            vec![
                ChatLine {
                    timestamp: ts.clone(),
                    user: None,
                    body: "Ready. Configure server/nick and Connect. No default channels.".to_string(),
                    style: LineStyle::System,
                },
                ChatLine {
                    timestamp: ts.clone(),
                    user: None,
                    body: "Use the join box or /join #channel (or nick for DM). Any IRC network supported.".to_string(),
                    style: LineStyle::System,
                },
                ChatLine {
                    timestamp: ts.clone(),
                    user: None,
                    body: "Join any channel via sidebar or commands. Joined channels + favorites are saved.".to_string(),
                    style: LineStyle::System,
                },
                ChatLine {
                    timestamp: ts.clone(),
                    user: None,
                    body: "Settings saved to ~/.config/boulder-relay/settings.conf".to_string(),
                    style: LineStyle::System,
                },
            ],
        );

        let channel_box = gtk::ListBox::new();
        channel_box.set_selection_mode(gtk::SelectionMode::Single);
        let user_box = gtk::ListBox::new();
        user_box.set_selection_mode(gtk::SelectionMode::None);
        let chat_view = gtk::TextView::new();
        Self::setup_chat_tags(&chat_view);

        let mut channels = vec![server_tab.clone()];
        for extra in &settings.extra_channels {
            if !channels.contains(extra) {
                channels.push(extra.clone());
            }
        }

        let favorites = if settings.favorites.is_empty() {
            vec![server_tab.clone()]
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

        let mut model = AppModel {
            servers: Vec::new(),
            current_server: String::new(),
            senders: HashMap::new(),
            server_states: HashMap::new(),
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
            window: root.clone(),
            notifications_enabled: settings.notifications_enabled,
            background_on_close: settings.background_on_close,
            channel_filter: String::new(),
            channel_list_results: Vec::new(),
            channel_topics: HashMap::new(),
            nick_colors_enabled: settings.nick_colors_enabled,
            timestamp_format: settings.timestamp_format,
            account_service: if settings.account_service.is_empty() { "NickServ".to_string() } else { settings.account_service },
            auth_method: if settings.auth_method.is_empty() { "nickserv".to_string() } else { settings.auth_method },
            accounts: settings.accounts.clone(),
            pending_register_email: None,
            ignored_users: std::collections::HashSet::new(),
        };
        let srv_init = model.server.clone();
        model.load_account_for_server(&srv_init);

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
            AppInput::UpdateNickname(nick) => {
                self.nickname = nick;
                let srv = self.server.clone();
                self.sync_account_for_server(&srv);
            },
            AppInput::UpdateServer(srv) => {
                let current_srv = self.server.clone();
                self.sync_account_for_server(&current_srv);
                self.load_account_for_server(&srv);
                self.server = srv;
            },
            AppInput::UpdatePassword(pwd) => {
                self.password = pwd;
                let srv = self.server.clone();
                self.sync_account_for_server(&srv);
            },
            AppInput::UpdateNotificationsEnabled(enabled) => {
                self.notifications_enabled = enabled;
                self.persist_settings();
            }
            AppInput::UpdateBackgroundOnClose(enabled) => {
                self.background_on_close = enabled;
                self.persist_settings();
            }

            AppInput::UpdateChannelFilter(filter) => {
                self.channel_filter = filter;
                self.refresh_channels(&sender);
            }

            AppInput::BrowseChannels => {
                if self.connection != ConnectionState::Connected {
                    self.append_line(
                        SERVER_TAB,
                        self.timestamp_prefix(),
                        None,
                        "Connect first to browse channels.".to_string(),
                        LineStyle::System,
                    );
                    return;
                }
                self.channel_list_results.clear();
                if let Some(irc_tx) = &self.irc_sender {
                    let _ = irc_tx.send("LIST");
                    self.append_line(
                        SERVER_TAB,
                        self.timestamp_prefix(),
                        None,
                        "Requesting channel list from server...".to_string(),
                        LineStyle::System,
                    );
                }
            }

            AppInput::ChannelListEntry { name, users, topic } => {
                self.channel_list_results.push((name, users, topic));
            }

            AppInput::ChannelListEnd => {
                let results = self.channel_list_results.clone();
                self.show_channel_list_dialog(results, &sender);
            }

            AppInput::ChannelTopic { channel, topic } => {
                self.channel_topics.insert(channel, topic);
                // The #[watch] label in the header will update on next view refresh
            }

            AppInput::OpenPreferences => {
                self.show_preferences_dialog(&sender);
            }

            AppInput::UpdateNickColorsEnabled(enabled) => {
                self.nick_colors_enabled = enabled;
                self.persist_settings();
            }

            AppInput::UpdateTimestampFormat(fmt) => {
                self.timestamp_format = fmt;
                self.persist_settings();
            }

            AppInput::OpenRegisterDialog => {
                self.show_register_dialog(&sender);
            }

            AppInput::SubmitRegistration { nick, password, email } => {
                self.nickname = nick.clone();
                self.password = password.clone();
                self.pending_register_email = if email.is_empty() { None } else { Some(email.clone()) };
                self.persist_settings();

                if self.connection == ConnectionState::Offline {
                    sender.input(AppInput::Connect);
                }

                let service = self.account_service.clone();
                if let Some(irc_tx) = &self.irc_sender {
                    let cmd = if email.is_empty() {
                        format!("REGISTER {}", password)
                    } else {
                        format!("REGISTER {} {}", password, email)
                    };
                    let _ = irc_tx.send_privmsg(&service, &cmd);
                    let msg = if email.is_empty() {
                        format!("Sent email-less registration for {} to {}.", nick, service)
                    } else {
                        format!("Sent registration for {} to {}. Check email for code.", nick, service)
                    };
                    self.append_line(SERVER_TAB, self.timestamp_prefix(), None, msg, LineStyle::System);
                    self.pending_register_email = None;
                } else {
                    self.append_line(
                        SERVER_TAB,
                        self.timestamp_prefix(),
                        None,
                        format!("Saved. Will REGISTER with {} after connect.", service),
                        LineStyle::System,
                    );
                }
            }

            AppInput::SubmitVerification { nick, code } => {
                let service = self.account_service.clone();
                if let Some(irc_tx) = &self.irc_sender {
                    let cmd = format!("VERIFY REGISTER {} {}", nick, code);
                    let _ = irc_tx.send_privmsg(&service, &cmd);
                    self.append_line(
                        SERVER_TAB,
                        self.timestamp_prefix(),
                        None,
                        format!("Sent VERIFY for {} to {}.", nick, service),
                        LineStyle::System,
                    );
                } else {
                    self.append_line(
                        SERVER_TAB,
                        self.timestamp_prefix(),
                        None,
                        "Connect first to verify registration.".to_string(),
                        LineStyle::System,
                    );
                }
            }

            AppInput::UpdateAccountService(service) => {
                if !service.is_empty() {
                    self.account_service = service;
                let srv = self.server.clone();
                self.sync_account_for_server(&srv);
                    self.persist_settings();
                }
            }

            AppInput::UpdateAuthMethod(method) => {
                if !method.is_empty() {
                    self.auth_method = method;
                let srv = self.server.clone();
                self.sync_account_for_server(&srv);
                    self.persist_settings();
                }
            }

            AppInput::SendRawPrivmsg { target, msg } => {
                if let Some(irc_tx) = &self.irc_sender {
                    let _ = irc_tx.send_privmsg(&target, &msg);
                }
            }

            AppInput::AddServer(srv) => {
                let srv = srv.trim().to_string();
                if srv.is_empty() { return; }
                if !self.servers.contains(&srv) {
                    self.servers.push(srv.clone());
                    self.senders.insert(srv.clone(), None);
                    self.server_states.insert(srv.clone(), ConnectionState::Offline);
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
                    self.refresh_channels(&sender);
                    self.show_channel_history();
                    self.refresh_users(&sender);
                }
            }

            AppInput::OpenAccountManager => {
                self.show_account_manager(&sender);
            }

            AppInput::OpenLogViewer => {
                self.show_log_viewer(&sender);
            }

            AppInput::Quit => {
                self.persist_settings();
                if let Some(irc_tx) = self.irc_sender.take() {
                    let _ = irc_tx.send_quit("Boulder Relay signing off");
                }
                relm4::main_application().quit();
            }

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
                    .filter(|c| channels::is_channel_target(c))
                    .cloned()
                    .collect();

                let nickname = self.nickname.clone();
                let server_addr = self.server.clone();
                let pwd = self.password.clone();
                let auth_method = self.auth_method.clone();

                thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().expect("Failed to build Tokio core");
                    rt.block_on(async {
                        let is_sasl_plain = auth_method == "sasl_plain";
                        let is_sasl_external = auth_method == "sasl_external";
                        let is_sasl = is_sasl_plain || is_sasl_external;
                        let needs_nickserv = !pwd.is_empty() && !is_sasl;
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

                        if is_sasl {
                            let _ = client.send_cap_req(&[irc::proto::caps::Capability::Sasl]);
                        } else if let Err(e) = client.identify() {
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
                                    // Basic CTCP ACTION (/me) support
                                    let (display_user, display_body) = if body.starts_with("\x01ACTION ") && body.ends_with("\x01") {
                                        let act = body.trim_start_matches("\x01ACTION ").trim_end_matches('\x01');
                                        (format!("* {}", user), act.to_string())
                                    } else {
                                        (user, body)
                                    };
                                    sender_clone.input(AppInput::ReceiveMessage {
                                        channel: display_target,
                                        user: display_user,
                                        body: display_body,
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
                                    // Auto-detect registration success for better UX
                                    if body.contains("has been successfully registered") 
                                        || body.contains("account has been verified")
                                        || body.contains("Account registered") {
                                        sender_clone.input(AppInput::ReceiveServerMessage(
                                            format!("[Auth Success]: {}", body)
                                        ));
                                    }
                                }
                                Command::CAP(_, sub, _, params) => {
                                    if is_sasl {
                                        if sub == irc::proto::CapSubCommand::ACK {
                                            if let Some(exts) = params {
                                                if exts.contains("sasl") {
                                                    if is_sasl_plain {
                                                        let _ = irc_tx.send_sasl_plain();
                                                    } else if is_sasl_external {
                                                        let _ = irc_tx.send_sasl_external();
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                Command::AUTHENTICATE(data) => {
                                    if is_sasl_plain && data == "+" {
                                        let auth = format!("\0{}\0{}", nickname, pwd);
                                        let encoded = base64::encode(auth.as_bytes());
                                        let _ = irc_tx.send_sasl(encoded);
                                    } else if is_sasl_external && data == "+" {
                                        let _ = irc_tx.send_sasl_external();
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
                                    } else if code == Response::RPL_LIST && args.len() >= 3 {
                                        // RPL_LIST: channel, user_count, topic (topic may be in later args or joined)
                                        let name = args.get(1).cloned().unwrap_or_default();
                                        let users: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                                        let topic = if args.len() > 3 {
                                            args[3..].join(" ")
                                        } else {
                                            String::new()
                                        };
                                        sender_clone.input(AppInput::ChannelListEntry { name, users, topic });
                                    } else if code == Response::RPL_LISTEND {
                                        sender_clone.input(AppInput::ChannelListEnd);
                                    } else if code == Response::RPL_TOPIC && args.len() >= 2 {
                                        let ch = args.get(1).cloned().unwrap_or_default();
                                        let topic = if args.len() > 2 {
                                            args[2..].join(" ")
                                        } else {
                                            String::new()
                                        };
                                        sender_clone.input(AppInput::ChannelTopic { channel: ch, topic });
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
                    self.channel_list_results.clear();
                    self.channel_topics.clear();
                    self.append_line(
                        SERVER_TAB,
                        self.timestamp_prefix(),
                        None,
                        "Disconnected by user.".to_string(),
                        LineStyle::System,
                    );
                }
            }

            AppInput::NetworkStatus(new_status) => {
                self.status = new_status.clone();
                if new_status == "Disconnected" || new_status.starts_with("Connection failed") {
                    let was_connected = self.connection == ConnectionState::Connected;
                    self.connection = ConnectionState::Offline;
                    self.irc_sender = None;
                    if was_connected {
                        // Auto-reconnect after 5 seconds for best UX
                        let s = sender.clone();
                        gtk::glib::timeout_add_seconds_local(5, move || {
                            s.input(AppInput::Connect);
                            gtk::glib::ControlFlow::Break
                        });
                    }
                }
            }

            AppInput::NetworkConnected(irc_tx) => {
                self.irc_sender = Some(irc_tx.clone());
                self.connection = ConnectionState::Connected;
                self.status = String::from("Connected");
                self.append_line(
                    SERVER_TAB,
                    self.timestamp_prefix(),
                    None,
                    format!("Connected to {} as {}.", self.server, self.nickname),
                    LineStyle::System,
                );

                // Handle pending registration from the Register dialog
                let service = self.account_service.clone();
                if let Some(email) = self.pending_register_email.take() {
                    let cmd = format!("REGISTER {} {}", self.password, email);
                    let _ = irc_tx.send_privmsg(&service, &cmd);
                    self.append_line(
                        SERVER_TAB,
                        self.timestamp_prefix(),
                        None,
                        format!("Sent registration request to {}. Check your email for the confirmation code and use /msg {} VERIFY REGISTER <nick> <code>.", service, service),
                        LineStyle::System,
                    );
                }
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
                            timestamp: self.timestamp_prefix(),
                            user: None,
                            body: format!("Tracking {}", target),
                            style: LineStyle::System,
                        }],
                    );
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
                self.channel_topics.remove(&channel);

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
                        self.timestamp_prefix(),
                        None,
                        format!("Unmuted {}", user),
                        LineStyle::System,
                    );
                } else {
                    list.push(user.clone());
                    list.sort_by_key(|u| u.to_lowercase());
                    self.append_line(
                        &channel,
                        self.timestamp_prefix(),
                        None,
                        format!("Muted {}", user),
                        LineStyle::System,
                    );
                }

                if self.active_channel == channel {
                    self.refresh_users(&sender);
                }
            }

            AppInput::ReceiveMessage { channel, user, body } => {
                let clean = Self::normalized_nick(&user);
                if self.ignored_users.contains(&clean) {
                    return;
                }
                if self.is_muted(&channel, &user) {
                    return;
                }

                if !self.channels.contains(&channel) && !channel.starts_with('#') {
                    self.channels.push(channel.clone());
                    self.refresh_channels(&sender);
                }

                let style = self.message_style(&user, &body);
                self.append_message(&channel, &user, &body, style);

                if self.should_notify(&channel, &user, style) {
                    notify::send_message_notification(&channel, &user, &body);
                }
            }

            AppInput::ReceiveServerMessage(body) => {
                self.append_line(
                    SERVER_TAB,
                    self.timestamp_prefix(),
                    None,
                    body,
                    LineStyle::System,
                );
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

            AppInput::JoinEntry(text) => {
                let text = text.trim();
                if text.is_empty() {
                    return;
                }

                if text.starts_with('/') {
                    sender.input(AppInput::SendMessage(text.to_string()));
                    return;
                }

                match channels::parse_join_entry(text) {
                    Some(channels::JoinTarget::Channel(channel)) => {
                        sender.input(AppInput::JoinChannel(channel));
                    }
                    Some(channels::JoinTarget::DirectMessage(nick)) => {
                        sender.input(AppInput::JoinChannel(nick));
                    }
                    None => {}
                }
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
                        "/join" | "/j" => {
                            if let Some(channel) = parts.next() {
                                if let Some(channel) = channels::parse_join_command(channel) {
                                    sender.input(AppInput::JoinChannel(channel));
                                }
                            }
                            return;
                        }
                        "/msg" | "/query" => {
                            if let Some(target) = parts.next() {
                                let body = parts.next().unwrap_or("");
                                if !body.is_empty() {
                                    let tx_opt = self.irc_sender.clone();
                                    if let Some(irc_tx) = tx_opt {
                                        let _ = irc_tx.send_privmsg(target, body);
                                        let my_nick = self.nickname.clone();
                                        self.append_message(target, &my_nick, body, LineStyle::SelfMsg);
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
                                    self.timestamp_prefix(),
                                    None,
                                    format!("Nickname updated locally to {}. Reconnect to apply.", self.nickname),
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
                            self.append_line(&channel, self.timestamp_prefix(), None, HELP_TEXT.to_string(), LineStyle::System);
                            return;
                        }
                        "/me" => {
                            let action = parts.next().unwrap_or("").to_string();
                            if !action.is_empty() {
                                let full = format!("\x01ACTION {}\x01", action);
                                if let Some(irc_tx) = &self.irc_sender {
                                    let _ = irc_tx.send_privmsg(&self.active_channel, &full);
                                }
                                let me_user = format!("* {}", self.nickname);
                                let chan = self.active_channel.clone();
                                self.append_message(&chan, &me_user, &action, LineStyle::SelfMsg);
                            }
                            return;
                        }
                        "/list" => {
                            sender.input(AppInput::BrowseChannels);
                            return;
                        }
                        "/ignore" => {
                            if let Some(target) = parts.next() {
                                let clean = Self::normalized_nick(target);
                                self.ignored_users.insert(clean.clone());
                                let chan = self.active_channel.clone();
                                self.append_message(&chan, "System", &format!("Ignoring {}", clean), LineStyle::System);
                            }
                            return;
                        }
                        "/unignore" => {
                            if let Some(target) = parts.next() {
                                let clean = Self::normalized_nick(target);
                                self.ignored_users.remove(&clean);
                                let chan = self.active_channel.clone();
                                self.append_message(&chan, "System", &format!("Unignored {}", clean), LineStyle::System);
                            }
                            return;
                        }
                        _ => {}
                    }
                }

                if self.active_channel == SERVER_TAB {
                    self.append_line(
                        SERVER_TAB,
                        self.timestamp_prefix(),
                        None,
                        "Select a channel or DM before sending.".to_string(),
                        LineStyle::System,
                    );
                    return;
                }

                let tx_opt = self.irc_sender.clone();
                if let Some(irc_tx) = tx_opt {
                    if irc_tx.send_privmsg(&self.active_channel, text).is_ok() {
                        let channel = self.active_channel.clone();
                        let my_nick = self.nickname.clone();
                        self.append_message(&channel, &my_nick, text, LineStyle::SelfMsg);
                    }
                } else {
                    let channel = self.active_channel.clone();
                    self.append_line(
                        &channel,
                        self.timestamp_prefix(),
                        None,
                        "Cannot send message, not connected.".to_string(),
                        LineStyle::System,
                    );
                }
            }
        }
    }
}

fn main() {
    gtk::init().expect("Failed to initialize GTK");
    let app = adw::Application::new(Some(notify::APP_ID), Default::default());
    app.connect_startup(|_| {
        theme::load_css();
        notify::setup_application_icon();
    });
    let relm_app = relm4::RelmApp::from_app(app);
    relm_app.run::<AppModel>(());
}
