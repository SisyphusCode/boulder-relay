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

const NICK_COLORS: [&str; 8] = [
    "#fabd2f",
    "#b8bb26",
    "#83a598",
    "#d3869b",
    "#fe8019",
    "#8ec07c",
    "#fb4934",
    "#d79921",
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
  /list, /clear, /nick name, /whois nick, /away [msg], /back, /topic [text], /ignore nick, /unignore nick, /help\n\
Join box: #channel (or nick for DM). Comma-separate for multi-join: #foo,#bar\n\
Sidebar filter searches your joined list.\n\
\"Register new account\u{2026}\" for NickServ registration + email verification.\n";

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
    /// Intentional user-initiated disconnect â suppresses auto-reconnect.
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
    servers: Vec<String>,
    current_server: String,
    senders: HashMap<String, Option<irc::client::Sender>>,
    server_states: HashMap<String, ConnectionState>,
    connection: ConnectionState,
    /// True when the user explicitly clicked Disconnect â suppresses auto-reconnect.
    user_disconnected: bool,
    status: String,
    active_channel: String,
    channels: Vec<String>,
    favorite_channels: Vec<String>,
    muted_users: HashMap<String, Vec<String>>,
    ignored_users: std::collections::HashSet<String>,
    unread_counts: HashMap<String, u32>,
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
    window: gtk::Window,
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
        self.channels.iter().filter(|c| **c != SERVER_TAB).cloned().collect()
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
        snapshot.accounts.insert(self.server.clone(), config::ServerAccount {
            nick: self.nickname.clone(),
            password: self.password.clone(),
            service: self.account_service.clone(),
            auth_method: self.auth_method.clone(),
        });
        snapshot
    }

    fn sync_account_for_server(&mut self, server: &str) {
        self.accounts.insert(server.to_string(), config::ServerAccount {
            nick: self.nickname.clone(),
            password: self.password.clone(),
            service: self.account_service.clone(),
            auth_method: self.auth_method.clone(),
        });
    }

    fn load_account_for_server(&mut self, server: &str) {
        if let Some(acc) = self.accounts.get(server) {
            if !acc.nick.is_empty() { self.nickname = acc.nick.clone(); }
            if !acc.password.is_empty() { self.password = acc.password.clone(); }
            if !acc.service.is_empty() { self.account_service = acc.service.clone(); }
            if !acc.auth_method.is_empty() { self.auth_method = acc.auth_method.clone(); }
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
            if let Some(bg) = bg { tag.set_background(Some(bg)); }
            table.add(&tag);
        }
        for (i, &color) in NICK_COLORS.iter().enumerate() {
            let tag_name = format!("nick-{}", i);
            let tag = gtk::TextTag::new(Some(&tag_name));
            tag.set_foreground(Some(color));
            tag.set_weight(600);
            table.add(&tag);
        }
    }

    fn message_style(&self, user: &str, body: &str) -> LineStyle {
        if user == "System" { return LineStyle::System; }
        let clean = Self::normalized_nick(user);
        if clean.eq_ignore_ascii_case(&self.nickname) { return LineStyle::SelfMsg; }
        if body.contains(&self.nickname) { return LineStyle::Mention; }
        LineStyle::Normal
    }

    fn append_line(&mut self, channel: &str, timestamp: String, user: Option<String>, body: String, style: LineStyle) {
        let history = self.chat_histories.entry(channel.to_string()).or_insert_with(Vec::new);
        history.push(ChatLine { timestamp: timestamp.clone(), user: user.clone(), body: body.clone(), style });
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

    fn append_rich_chat_line(&self, ts: &str, user: &str, body: &str, style: LineStyle) {
        let buffer = self.chat_view.buffer();
        let mut end = buffer.end_iter();
        let ts_tag = if style == LineStyle::System { "system" } else { "normal" };
        buffer.insert_with_tags_by_name(&mut end, ts, &[ts_tag]);
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
        buffer.insert_with_tags_by_name(&mut end, &format!("{}\n", body), &[Self::style_tag(style)]);
        let mark = buffer.create_mark(None, &buffer.end_iter(), false);
        self.chat_view.scroll_to_mark(&mark, 0.0, false, 0.0, 0.0);
    }

    fn append_message(&mut self, channel: &str, user: &str, body: &str, style: LineStyle) {
        let ts = self.timestamp_prefix();
        // Store history
        let history = self.chat_histories.entry(channel.to_string()).or_insert_with(Vec::new);
        history.push(ChatLine { timestamp: ts.clone(), user: Some(user.to_string()), body: body.to_string(), style });
        // Render only if the channel is active
        if self.active_channel == channel {
            self.append_rich_chat_line(&ts, user, body, style);
        }
    }

    fn persist_settings(&self) {
        if let Err(e) = self.settings_snapshot().save() {
            eprintln!("Failed to save settings: {e}");
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
                    self.append_styled_to_chat_view(&format!("{}[System]: {}\n", line.timestamp, line.body), line.style);
                }
            }
        }
    }

    fn append_section_header(&self, label: &str) {
        let header = gtk::Label::builder()
            .label(label)
            .halign(gtk::Align::Start)
            .margin_start(8).margin_top(8).margin_bottom(2)
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
        let content = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(4).build();
        let select_btn = gtk::Button::with_label(channel);
        select_btn.set_hexpand(true);
        select_btn.set_halign(gtk::Align::Fill);
        select_btn.set_tooltip_text(Some("Click to switch to this context"));
        let s1 = sender.clone();
        let ch1 = channel.to_string();
        select_btn.connect_clicked(move |_| { s1.input(AppInput::SelectChannel(ch1.clone())); });
        let fav_btn = gtk::Button::with_label(if is_favorite { "â" } else { "â" });
        fav_btn.add_css_class("fav-btn");
        fav_btn.set_tooltip_text(Some(if is_favorite { "Remove from favorites" } else { "Add to favorites" }));
        let s2 = sender.clone();
        let ch2 = channel.to_string();
        fav_btn.connect_clicked(move |_| { s2.input(AppInput::ToggleFavorite(ch2.clone())); });
        content.append(&select_btn);
        content.append(&fav_btn);
        // Part button only for channels, not DMs or Server tab
        if channels::is_channel_target(channel) {
            let part_btn = gtk::Button::with_label("Ã");
            part_btn.add_css_class("part-btn");
            part_btn.set_tooltip_text(Some("Leave channel"));
            let s3 = sender.clone();
            let ch3 = channel.to_string();
            part_btn.connect_clicked(move |_| { s3.input(AppInput::PartChannel(ch3.clone())); });
            content.append(&part_btn);
        }
        let list_row = gtk::ListBoxRow::new();
        list_row.set_child(Some(&content));
        let s4 = sender.clone();
        let ch4 = channel.to_string();
        list_row.connect_activate(move |_| { s4.input(AppInput::SelectChannel(ch4.clone())); });
        self.channel_box.append(&list_row);
    }

    fn refresh_channels(&self, sender: &ComponentSender<Self>) {
        while let Some(child) = self.channel_box.first_child() {
            self.channel_box.remove(&child);
        }
        let filter = self.channel_filter.to_lowercase();
        let matches_filter = |name: &str| filter.is_empty() || name.to_lowercase().contains(&filter);
        let mut favorites = Vec::new();
        let mut others = Vec::new();
        for channel in &self.channels {
            if !matches_filter(channel) { continue; }
            if self.favorite_channels.contains(channel) {
                favorites.push(channel.clone());
            } else {
                others.push(channel.clone());
            }
        }
        if !favorites.is_empty() {
            self.append_section_header("â Favorites");
            for channel in &favorites { self.append_channel_row(sender, channel); }
        }
        if !others.is_empty() {
            others.sort_by_key(|name| name.to_lowercase());
            self.append_section_header("Channels & DMs");
            for channel in &others { self.append_channel_row(sender, channel); }
        } else if !filter.is_empty() && favorites.is_empty() {
            let hint = gtk::Label::builder()
                .label(format!("No matches for \u{201c}{}\u{201d}", self.channel_filter))
                .halign(gtk::Align::Start).margin_start(12).margin_top(4)
                .css_classes(["channel-section"]).build();
            let row = gtk::ListBoxRow::new();
            row.set_activatable(false);
            row.set_child(Some(&hint));
            self.channel_box.append(&row);
        }
    }

    fn show_channel_list_dialog(&self, results: Vec<(String, u32, String)>, sender: &ComponentSender<Self>) {
        if results.is_empty() {
            sender.input(AppInput::ReceiveServerMessage("No channels returned by server.".to_string()));
            return;
        }
        let mut sorted = results;
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.to_lowercase().cmp(&b.0.to_lowercase())));
        let dialog = gtk::Window::builder()
            .transient_for(self.window.upcast_ref::<gtk::Window>())
            .modal(true).title("Browse Channels")
            .default_width(720).default_height(520).build();
        dialog.add_css_class("boulder-relay");
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_all(12);
        let header = gtk::Label::builder()
            .label(format!("{} channels â type to filter", sorted.len()))
            .halign(gtk::Align::Start).css_classes(["sidebar-subtitle"]).build();
        vbox.append(&header);
        let search = gtk::SearchEntry::builder().placeholder_text("Filter by name or topicâ¦").hexpand(true).build();
        vbox.append(&search);
        let scrolled = gtk::ScrolledWindow::builder().vexpand(true).build();
        let list_container = gtk::Box::new(gtk::Orientation::Vertical, 4);
        scrolled.set_child(Some(&list_container));
        vbox.append(&scrolled);
        let mut all_rows: Vec<(gtk::Box, String, String)> = Vec::new();
        for (name, users, topic) in &sorted {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let display_topic = if topic.is_empty() { "<no topic>" } else { topic.as_str() };
            let info = gtk::Label::builder()
                .label(format!("{}  ({} users)  {}", name, users, display_topic))
                .halign(gtk::Align::Start).hexpand(true)
                .ellipsize(gtk::pango::EllipsizeMode::End).build();
            let join_btn = gtk::Button::with_label("Join");
            join_btn.add_css_class("suggested-action");
            let s = sender.clone();
            let ch = name.clone();
            let dlg = dialog.clone();
            join_btn.connect_clicked(move |_| { s.input(AppInput::JoinChannel(ch.clone())); dlg.close(); });
            row.append(&info);
            row.append(&join_btn);
            list_container.append(&row);
            all_rows.push((row, name.clone(), topic.clone()));
        }
        let rows_for_filter = all_rows.clone();
        search.connect_changed(move |entry| {
            let q = entry.text().to_lowercase();
            for (row, name, topic) in &rows_for_filter {
                let hay = format!("{} {}", name, topic).to_lowercase();
                row.set_visible(q.is_empty() || hay.contains(&q));
            }
        });
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
            .modal(true).title("Preferences")
            .default_width(400).default_height(300).build();
        dialog.add_css_class("boulder-relay");
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
        vbox.set_margin_all(12);
        let nick_check = gtk::CheckButton::builder().label("Enable nickname colors").active(self.nick_colors_enabled).build();
        vbox.append(&nick_check);
        vbox.append(&gtk::Label::new(Some("Timestamp format (strftime):")));
        let ts_entry = gtk::Entry::builder().text(&self.timestamp_format).placeholder_text("%H:%M or %I:%M %p").build();
        vbox.append(&ts_entry);
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
            .modal(true).title("Account Registration")
            .default_width(450).default_height(440).build();
        dialog.add_css_class("boulder-relay");
        let main_vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
        main_vbox.set_margin_all(12);
        let status_label = gtk::Label::builder().label("").halign(gtk::Align::Start).wrap(true).build();
        main_vbox.append(&status_label);
        main_vbox.append(&gtk::Label::new(Some("Account Service:")));
        let service_entry = gtk::Entry::builder().text(&self.account_service).placeholder_text("NickServ").build();
        main_vbox.append(&service_entry);
        let nick_entry = gtk::Entry::builder().text(&self.nickname).placeholder_text("Desired nickname").build();
        main_vbox.append(&gtk::Label::new(Some("Nickname:")));
        main_vbox.append(&nick_entry);
        let pass_entry = gtk::Entry::builder().visibility(false).placeholder_text("Password").build();
        main_vbox.append(&gtk::Label::new(Some("Password:")));
        main_vbox.append(&pass_entry);
        let confirm_entry = gtk::Entry::builder().visibility(false).placeholder_text("Confirm password").build();
        main_vbox.append(&gtk::Label::new(Some("Confirm Password:")));
        main_vbox.append(&confirm_entry);
        let no_email_check = gtk::CheckButton::builder().label("Register without email").active(false).build();
        main_vbox.append(&no_email_check);
        let email_label = gtk::Label::new(Some("Email:"));
        let email_entry = gtk::Entry::builder().placeholder_text("your@email.com").build();
        main_vbox.append(&email_label);
        main_vbox.append(&email_entry);
        let email_e_clone = email_entry.clone();
        let email_l_clone = email_label.clone();
        no_email_check.connect_toggled(move |check| {
            email_e_clone.set_visible(!check.is_active());
            email_l_clone.set_visible(!check.is_active());
        });
        // Verify section
        let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
        sep.set_margin_top(8);
        main_vbox.append(&sep);
        main_vbox.append(&gtk::Label::builder().label("Verify: enter code from email").halign(gtk::Align::Start).build());
        let verify_nick = gtk::Entry::builder().text(&self.nickname).placeholder_text("Your nickname").build();
        main_vbox.append(&gtk::Label::new(Some("Nickname:")));
        main_vbox.append(&verify_nick);
        let code_entry = gtk::Entry::builder().placeholder_text("Code from email").build();
        main_vbox.append(&gtk::Label::new(Some("Verification Code:")));
        main_vbox.append(&code_entry);
        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        btn_box.set_halign(gtk::Align::End);
        btn_box.set_margin_top(8);
        let cancel = gtk::Button::with_label("Close");
        let d_close = dialog.clone();
        cancel.connect_clicked(move |_| { d_close.close(); });
        btn_box.append(&cancel);
        let reg_btn = gtk::Button::with_label("Register");
        {
            let s = sender.clone();
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
                if service.is_empty() { status.set_label("Account Service cannot be empty."); return; }
                if nick.is_empty() { status.set_label("Nickname is required."); return; }
                if pass.is_empty() { status.set_label("Password is required."); return; }
                if pass != conf { status.set_label("Passwords do not match."); return; }
                if !no_email.is_active() && email.is_empty() { status.set_label("Email required."); return; }
                s.input(AppInput::UpdateAccountService(service));
                s.input(AppInput::SubmitRegistration { nick, password: pass, email });
                status.set_label("Registration sent. Check Server tab and your email.");
            });
        }
        btn_box.append(&reg_btn);
        let verify_btn = gtk::Button::with_label("Verify");
        {
            let s2 = sender.clone();
            let status2 = status_label.clone();
            let v_nick = verify_nick.clone();
            let code_e = code_entry.clone();
            verify_btn.connect_clicked(move |_| {
                let nick = v_nick.text().to_string().trim().to_string();
                let code = code_e.text().to_string().trim().to_string();
                if nick.is_empty() || code.is_empty() { status2.set_label("Nickname and code required."); return; }
                s2.input(AppInput::SubmitVerification { nick, code });
                status2.set_label("Verification sent. Check Server tab.");
            });
        }
        btn_box.append(&verify_btn);
        main_vbox.append(&btn_box);
        dialog.set_child(Some(&main_vbox));
        dialog.present();
    }

    fn show_account_manager(&self, sender: &ComponentSender<Self>) {
        let dialog = gtk::Window::builder()
            .transient_for(self.window.upcast_ref::<gtk::Window>())
            .modal(true).title("Account Manager")
            .default_width(500).default_height(400).build();
        dialog.add_css_class("boulder-relay");
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_all(12);
        vbox.append(&gtk::Label::new(Some("Manage accounts per server.")));
        for (srv, acc) in &self.accounts {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let label = gtk::Label::new(Some(&format!("{}: {} (service: {})", srv, acc.nick, acc.service)));
            row.append(&label);
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
            .modal(true).title("Log Viewer")
            .default_width(600).default_height(500).build();
        dialog.add_css_class("boulder-relay");
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 8);
        vbox.set_margin_all(12);
        let search = gtk::Entry::builder().placeholder_text("Search logsâ¦").build();
        vbox.append(&search);
        let scrolled = gtk::ScrolledWindow::new();
        scrolled.set_vexpand(true);
        let textview = gtk::TextView::new();
        textview.set_editable(false);
        textview.set_wrap_mode(gtk::WrapMode::Word);
        scrolled.set_child(Some(&textview));
        vbox.append(&scrolled);
        let buffer = textview.buffer();
        if let Some(lines) = self.chat_histories.get(&self.active_channel) {
            for line in lines {
                let prefix = if let Some(u) = &line.user { format!("<{}> ", u) } else { "[System] ".to_string() };
                buffer.insert(&mut buffer.end_iter(), &format!("{}{}\n", prefix, line.body));
            }
        }
        // Bug #4 fixed: removed unused `search_clone` binding
        let tv_clone = textview.clone();
        search.connect_changed(move |e| {
            let query = e.text().to_lowercase();
            if query.is_empty() { return; }
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
                let content = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(4).build();
                let color = NICK_COLORS[Self::nick_color_index(user)];
                let dm_btn = gtk::Button::new();
                let dm_label = gtk::Label::new(None);
                dm_label.set_markup(&format!("<span foreground=\"{}\">{}</span>", color, user));
                dm_btn.set_child(Some(&dm_label));
                dm_btn.set_hexpand(true);
                dm_btn.set_halign(gtk::Align::Fill);
                dm_btn.add_css_class("user-btn");
                if muted { dm_btn.add_css_class("muted-user"); }
                let s1 = sender.clone();
                let u1 = clean_user.clone();
                dm_btn.connect_clicked(move |_| { s1.input(AppInput::JoinChannel(u1.clone())); });
                let mute_btn = gtk::Button::with_label(if muted { "ð" } else { "ð" });
                mute_btn.add_css_class("mute-btn");
                let s2 = sender.clone();
                let c2 = self.active_channel.clone();
                let u2 = clean_user.clone();
                mute_btn.connect_clicked(move |_| { s2.input(AppInput::ToggleMute { channel: c2.clone(), user: u2.clone() }); });
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
        gtk::Window {
            set_default_size: (1200, 700),
            set_size_request: (800, 500),
            set_resizable: true,
            set_decorated: true,
            set_hexpand: true,
            set_vexpand: true,
            add_css_class: "boulder-relay",
            set_titlebar: Some(&theme::build_titlebar()),

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
            set_child = &gtk::Paned {
                    set_hexpand: true, set_vexpand: true,
                    set_orientation: gtk::Orientation::Horizontal,
                    set_position: 240,
                    set_shrink_start_child: false,
                    set_shrink_end_child: false,

                    #[wrap(Some)]
                    set_start_child = &gtk::Box {
                    set_orientation: gtk::Orientation::Vertical, set_spacing: 12, set_width_request: 200, set_hexpand: true, set_vexpand: true,
                    add_css_class: "sidebar", set_margin_all: 0,

                    gtk::Label { set_label: "BOULDER RELAY", add_css_class: "sidebar-title", set_margin_top: 16 },
                    gtk::Label { set_label: "GTK4 IRC Client â any network, any channel", set_margin_start: 12, set_margin_end: 12 },
                    gtk::Separator { set_orientation: gtk::Orientation::Horizontal },

                    gtk::Label { set_label: "Network Configuration", add_css_class: "sidebar-subtitle", set_halign: gtk::Align::Start, set_margin_start: 12 },
                    gtk::Entry {
                        set_text: &model.nickname, set_placeholder_text: Some("Nickname"), set_margin_start: 12, set_margin_end: 12,
                        connect_changed[sender] => move |entry| { sender.input(AppInput::UpdateNickname(entry.text().to_string())); }
                    },
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal, set_spacing: 4, set_margin_start: 12, set_margin_end: 12,
                        gtk::Entry {
                            set_text: &model.password, set_placeholder_text: Some("Account Password"), set_hexpand: true, set_visibility: false,
                            connect_changed[sender] => move |entry| { sender.input(AppInput::UpdatePassword(entry.text().to_string())); }
                        },
                        gtk::Button {
                            set_label: "ð", set_tooltip_text: Some("Show or hide password"),
                            connect_clicked => move |button| {
                                if let Some(entry) = button.prev_sibling().and_downcast::<gtk::Entry>() {
                                    let visible = entry.property::<bool>("visibility");
                                    entry.set_visibility(!visible);
                                }
                            }
                        },
                    },
                    gtk::Button { set_label: "Register new accountâ¦", set_margin_start: 12, set_margin_end: 12, connect_clicked => AppInput::OpenRegisterDialog },
                    gtk::Button { set_label: "Manage Accounts", set_margin_start: 12, set_margin_end: 12, connect_clicked => AppInput::OpenAccountManager },
                    gtk::Entry {
                        set_text: &model.server, set_placeholder_text: Some("Server address"), set_margin_start: 12, set_margin_end: 12,
                        connect_changed[sender] => move |entry| { sender.input(AppInput::UpdateServer(entry.text().to_string())); }
                    },
                    gtk::Entry {
                        set_text: &model.account_service, set_placeholder_text: Some("Account Service (e.g. NickServ)"), set_margin_start: 12, set_margin_end: 12,
                        connect_changed[sender] => move |entry| { sender.input(AppInput::UpdateAccountService(entry.text().to_string())); }
                    },
                    gtk::Entry {
                        set_text: &model.auth_method, set_placeholder_text: Some("Auth Method (nickserv / sasl_plain)"), set_margin_start: 12, set_margin_end: 12,
                        connect_changed[sender] => move |entry| { sender.input(AppInput::UpdateAuthMethod(entry.text().to_string())); }
                    },
                    gtk::Entry {
                        set_placeholder_text: Some("Add server (e.g. irc.oftc.net)"), set_margin_start: 12, set_margin_end: 12,
                        connect_activate[sender] => move |entry| {
                            let s = entry.text().to_string();
                            if !s.is_empty() { entry.set_text(""); sender.input(AppInput::AddServer(s)); }
                        }
                    },
                    gtk::Button { set_label: "View Logs", set_margin_start: 12, set_margin_end: 12, connect_clicked => AppInput::OpenLogViewer },
                    gtk::Label {
                        #[watch] set_label: &format!("Status: {}", model.status),
                        set_ellipsize: gtk::pango::EllipsizeMode::End, set_margin_start: 8, set_margin_end: 8,
                        add_css_class: match model.connection {
                            ConnectionState::Connected => "status-connected",
                            ConnectionState::Connecting => "status-connecting",
                            ConnectionState::Offline => "status-offline",
                        },
                    },
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal, set_spacing: 8, set_margin_start: 12, set_margin_end: 12,
                        gtk::Button { set_label: "Connect", set_sensitive: model.connection == ConnectionState::Offline, connect_clicked => AppInput::Connect },
                        gtk::Button { set_label: "Disconnect", add_css_class: "destructive", set_sensitive: model.connection == ConnectionState::Connected, connect_clicked => AppInput::UserDisconnect },
                        gtk::Button { set_label: "Quit", add_css_class: "destructive", connect_clicked => AppInput::Quit },
                    },
                    gtk::CheckButton {
                        set_label: Some("Run in background when closed"), set_active: model.background_on_close,
                        set_margin_start: 12, set_margin_end: 12,
                        connect_toggled[sender] => move |check| { sender.input(AppInput::UpdateBackgroundOnClose(check.is_active())); }
                    },
                    gtk::CheckButton {
                        set_label: Some("Desktop notifications"), set_active: model.notifications_enabled,
                        set_margin_start: 12, set_margin_end: 12,
                        connect_toggled[sender] => move |check| { sender.input(AppInput::UpdateNotificationsEnabled(check.is_active())); }
                    },
                    gtk::Button { set_label: "Preferencesâ¦", set_margin_start: 12, set_margin_end: 12, connect_clicked => AppInput::OpenPreferences },
                    gtk::Separator { set_orientation: gtk::Orientation::Horizontal },
                    gtk::Label { set_label: "Channels & DMs", add_css_class: "sidebar-subtitle", set_halign: gtk::Align::Start, set_margin_start: 12 },
                    gtk::Entry {
                        set_placeholder_text: Some("#channel or nick for DM, comma for multi"),
                        set_margin_start: 12, set_margin_end: 12,
                        connect_activate[sender] => move |entry| {
                            let text = entry.text().to_string();
                            if !text.is_empty() { entry.set_text(""); sender.input(AppInput::JoinEntry(text)); }
                        }
                    },
                    gtk::Button { set_label: "ð Browse server channelsâ¦", set_margin_start: 12, set_margin_end: 12, connect_clicked => AppInput::BrowseChannels },
                    gtk::SearchEntry {
                        set_placeholder_text: Some("Filter your joined channelsâ¦"), set_margin_start: 12, set_margin_end: 12,
                        connect_changed[sender] => move |entry| { sender.input(AppInput::UpdateChannelFilter(entry.text().to_string())); }
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
                    set_hexpand: true, set_vexpand: true,
                    set_shrink_start_child: false,
                    set_shrink_end_child: false,

                    #[wrap(Some)]
                    set_start_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical, set_spacing: 12, set_margin_all: 16, set_width_request: 300, set_hexpand: true, set_vexpand: true,
                        add_css_class: "chat-panel",
                        gtk::Label { #[watch] set_label: &format!("Active: {}", model.active_channel), set_halign: gtk::Align::Start },
                        gtk::Label {
                            #[watch]
                            set_label: model.channel_topics.get(&model.active_channel).map(String::as_str).unwrap_or(""),
                            set_halign: gtk::Align::Start, set_ellipsize: gtk::pango::EllipsizeMode::End,
                            set_css_classes: &["channel-topic"], set_margin_start: 4,
                        },
                        gtk::ScrolledWindow {
                            set_vexpand: true, set_hexpand: true, set_propagate_natural_height: true,
                            #[local_ref] chat_view_ref -> gtk::TextView {
                                set_editable: false, set_cursor_visible: true,
                                set_wrap_mode: gtk::WrapMode::Word, set_vexpand: true,
                            }
                        },
                        gtk::Entry {
                            set_placeholder_text: Some("Message, /join #chan, /msg nick text, /helpâ¦"), set_hexpand: true,
                            connect_activate[sender] => move |entry| {
                                let text = entry.text().to_string();
                                if !text.is_empty() { entry.set_text(""); sender.input(AppInput::SendMessage(text)); }
                            }
                        }
                    },

                    #[wrap(Some)]
                    set_end_child = &gtk::Box {
                        set_orientation: gtk::Orientation::Vertical, set_spacing: 12, set_width_request: 200, set_hexpand: true, set_vexpand: true,
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

    fn init(_init: Self::Init, root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        theme::attach_window(root.upcast_ref::<gtk::Window>());
        let settings = Settings::load();
        let server_tab = String::from(SERVER_TAB);
        let mut chat_histories = HashMap::new();
        let ts = "[??:??] ".to_string();
        chat_histories.insert(server_tab.clone(), vec![
            ChatLine { timestamp: ts.clone(), user: None, body: "Ready. Configure server/nick and Connect.".to_string(), style: LineStyle::System },
            ChatLine { timestamp: ts.clone(), user: None, body: "Use /join #channel or the join box. Any IRC network supported.".to_string(), style: LineStyle::System },
            ChatLine { timestamp: ts.clone(), user: None, body: "Settings saved to ~/.config/boulder-relay/settings.toml".to_string(), style: LineStyle::System },
        ]);
        let channel_box = gtk::ListBox::new();
        channel_box.set_selection_mode(gtk::SelectionMode::Single);
        let user_box = gtk::ListBox::new();
        user_box.set_selection_mode(gtk::SelectionMode::None);
        let chat_view = gtk::TextView::new();
        Self::setup_chat_tags(&chat_view);
        let mut channels = vec![server_tab.clone()];
        for extra in &settings.extra_channels {
            if !channels.contains(extra) { channels.push(extra.clone()); }
        }
        let favorites = if settings.favorites.is_empty() { vec![server_tab.clone()] } else { settings.favorites.clone() };
        let active_channel = if settings.last_channel.is_empty() || !channels.contains(&settings.last_channel) {
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
            user_disconnected: false,
            status: String::from("Offline"),
            active_channel,
            channels,
            favorite_channels: favorites,
            muted_users: HashMap::new(),
            ignored_users: std::collections::HashSet::new(),
            unread_counts: HashMap::new(),
            mention_counts: HashMap::new(),
            chat_histories,
            channel_users: HashMap::new(),
            irc_sender: None,
            nickname: if settings.nickname.is_empty() { String::from(DEFAULT_NICKNAME) } else { settings.nickname },
            server: if settings.server.is_empty() { String::from(DEFAULT_SERVER) } else { settings.server },
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
            accounts: settings.accounts,
            pending_register_email: None,
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
            AppInput::UpdateNickname(nick) => { self.nickname = nick; let srv = self.server.clone(); self.sync_account_for_server(&srv); }
            AppInput::UpdateServer(srv) => { let curr = self.server.clone(); self.sync_account_for_server(&curr); self.load_account_for_server(&srv); self.server = srv; }
            AppInput::UpdatePassword(pwd) => { self.password = pwd; let srv = self.server.clone(); self.sync_account_for_server(&srv); }
            AppInput::UpdateNotificationsEnabled(enabled) => { self.notifications_enabled = enabled; self.persist_settings(); }
            AppInput::UpdateBackgroundOnClose(enabled) => { self.background_on_close = enabled; self.persist_settings(); }
            AppInput::UpdateChannelFilter(filter) => { self.channel_filter = filter; self.refresh_channels(&sender); }
            AppInput::MarkChannelRead(channel) => {
                self.unread_counts.remove(&channel);
                self.mention_counts.remove(&channel);
            }
            AppInput::IgnoreUser(nick) => {
                let clean = Self::normalized_nick(&nick);
                self.ignored_users.insert(clean.clone());
                let chan = self.active_channel.clone();
                let ts = self.timestamp_prefix();
                self.append_line(&chan, ts, None, format!("Ignoring {}", clean), LineStyle::System);
            }
            AppInput::UnignoreUser(nick) => {
                let clean = Self::normalized_nick(&nick);
                self.ignored_users.remove(&clean);
                let chan = self.active_channel.clone();
                let ts = self.timestamp_prefix();
                self.append_line(&chan, ts, None, format!("Unignored {}", clean), LineStyle::System);
            }
            AppInput::UserRenamed { old, new } => {
                for list in self.channel_users.values_mut() {
                    for u in list.iter_mut() {
                        if *u == old { *u = new.clone(); }
                    }
                }
                self.refresh_users(&sender);
            }

            AppInput::BrowseChannels => {
                if self.connection != ConnectionState::Connected {
                    let ts = self.timestamp_prefix();
                    self.append_line(SERVER_TAB, ts, None, "Connect first to browse channels.".to_string(), LineStyle::System);
                    return;
                }
                self.channel_list_results.clear();
                if let Some(irc_tx) = &self.irc_sender {
                    let _ = irc_tx.send(Message::from("LIST"));
                    let ts = self.timestamp_prefix();
                    self.append_line(SERVER_TAB, ts, None, "Requesting channel listâ¦".to_string(), LineStyle::System);
                }
            }

            AppInput::ChannelListEntry { name, users, topic } => { self.channel_list_results.push((name, users, topic)); }
            AppInput::ChannelListEnd => { let results = self.channel_list_results.clone(); self.show_channel_list_dialog(results, &sender); }
            AppInput::ChannelTopic { channel, topic } => { self.channel_topics.insert(channel, topic); }
            AppInput::OpenPreferences => { self.show_preferences_dialog(&sender); }
            AppInput::UpdateNickColorsEnabled(enabled) => { self.nick_colors_enabled = enabled; self.persist_settings(); }
            AppInput::UpdateTimestampFormat(fmt) => { self.timestamp_format = fmt; self.persist_settings(); }
            AppInput::OpenRegisterDialog => { self.show_register_dialog(&sender); }

            AppInput::SubmitRegistration { nick, password, email } => {
                self.nickname = nick.clone();
                self.password = password.clone();
                self.pending_register_email = if email.is_empty() { None } else { Some(email.clone()) };
                self.persist_settings();
                if self.connection == ConnectionState::Offline { sender.input(AppInput::Connect); }
                let service = self.account_service.clone();
                if let Some(irc_tx) = &self.irc_sender {
                    let cmd = if email.is_empty() { format!("REGISTER {}", password) } else { format!("REGISTER {} {}", password, email) };
                    let _ = irc_tx.send_privmsg(&service, &cmd);
                    let ts = self.timestamp_prefix();
                    self.append_line(SERVER_TAB, ts, None, format!("Sent registration for {} to {}.", nick, service), LineStyle::System);
                    self.pending_register_email = None;
                } else {
                    let ts = self.timestamp_prefix();
                    self.append_line(SERVER_TAB, ts, None, format!("Saved. Will REGISTER with {} after connect.", service), LineStyle::System);
                }
            }

            AppInput::SubmitVerification { nick, code } => {
                let service = self.account_service.clone();
                if let Some(irc_tx) = &self.irc_sender {
                    let _ = irc_tx.send_privmsg(&service, &format!("VERIFY REGISTER {} {}", nick, code));
                    let ts = self.timestamp_prefix();
                    self.append_line(SERVER_TAB, ts, None, format!("Sent VERIFY for {} to {}.", nick, service), LineStyle::System);
                } else {
                    let ts = self.timestamp_prefix();
                    self.append_line(SERVER_TAB, ts, None, "Connect first to verify.".to_string(), LineStyle::System);
                }
            }

            AppInput::UpdateAccountService(service) => {
                if !service.is_empty() { self.account_service = service; let srv = self.server.clone(); self.sync_account_for_server(&srv); self.persist_settings(); }
            }
            AppInput::UpdateAuthMethod(method) => {
                if !method.is_empty() { self.auth_method = method; let srv = self.server.clone(); self.sync_account_for_server(&srv); self.persist_settings(); }
            }
            AppInput::SendRawPrivmsg { target, msg } => {
                if let Some(irc_tx) = &self.irc_sender { let _ = irc_tx.send_privmsg(&target, &msg); }
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
                    // Bug #8 fixed: swap active irc_sender from senders map
                    self.senders.insert(curr, Some(self.irc_sender.clone().unwrap_or_else(|| self.irc_sender.clone().unwrap())));
                    self.irc_sender = self.senders.get(&srv).and_then(|s| s.clone());
                    self.current_server = srv.clone();
                    self.server = srv.clone();
                    self.load_account_for_server(&srv);
                    let state = self.server_states.get(&srv).copied().unwrap_or(ConnectionState::Offline);
                    self.connection = state;
                    self.refresh_channels(&sender);
                    self.show_channel_history();
                    self.refresh_users(&sender);
                }
            }

            AppInput::OpenAccountManager => { self.show_account_manager(&sender); }
            AppInput::OpenLogViewer => { self.show_log_viewer(&sender); }

            AppInput::Quit => {
                self.persist_settings();
                if let Some(irc_tx) = self.irc_sender.take() { let _ = irc_tx.send_quit("Boulder Relay signing off"); }
                relm4::main_application().quit();
            }

            AppInput::SaveSettings => self.persist_settings(),

            AppInput::Connect => {
                if self.connection != ConnectionState::Offline { return; }
                self.connection = ConnectionState::Connecting;
                self.user_disconnected = false;
                self.status = String::from("Connectingâ¦");
                self.persist_settings();
                let sender_clone = sender.clone();
                let channels_to_join: Vec<String> = self.channels.iter().filter(|c| channels::is_channel_target(c)).cloned().collect();
                let nickname = self.nickname.clone();
                let server_addr = self.server.clone();
                let pwd = self.password.clone();
                let auth_method = self.auth_method.clone();
                thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().expect("Tokio runtime");
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
                            nick_password: if needs_nickserv { Some(pwd.clone()) } else { None },
                            ..Config::default()
                        };
                        let mut client = match Client::from_config(config).await {
                            Ok(c) => c,
                            Err(e) => { sender_clone.input(AppInput::NetworkStatus(format!("Connection failed: {e}"))); return; }
                        };
                        if is_sasl {
                            let _ = client.send_cap_req(&[irc::proto::caps::Capability::Sasl]);
                        } else if let Err(e) = client.identify() {
                            sender_clone.input(AppInput::NetworkStatus(format!("NickServ auth failed: {e}")));
                            return;
                        }
                        let irc_tx = client.sender();
                        sender_clone.input(AppInput::NetworkConnected(irc_tx.clone()));
                        let join_channels = |tx: &irc::client::Sender| {
                            for chan in &channels_to_join { let _ = tx.send_join(chan); }
                        };
                        let mut channels_joined = false;
                        let mut stream = match client.stream() {
                            Ok(s) => s,
                            Err(_) => return,
                        };
                        while let Some(result) = stream.next().await {
                            let message = match result {
                                Ok(m) => m,
                                Err(e) => { sender_clone.input(AppInput::ReceiveServerMessage(format!("[Error]: {e}"))); continue; }
                            };
                            let user = message.source_nickname().unwrap_or("Unknown").to_string();
                            match message.command {
                                Command::PRIVMSG(target, body) => {
                                    let display_target = if target == nickname { user.clone() } else { target };
                                    let (display_user, display_body) = if body.starts_with("\x01ACTION ") && body.ends_with('\x01') {
                                        let act = body.trim_start_matches("\x01ACTION ").trim_end_matches('\x01');
                                        (format!("* {}", user), act.to_string())
                                    } else {
                                        (user, body)
                                    };
                                    sender_clone.input(AppInput::ReceiveMessage { channel: display_target, user: display_user, body: display_body });
                                }
                                Command::JOIN(channel, _, _) => {
                                    sender_clone.input(AppInput::UserJoined { channel: channel.clone(), user: user.clone() });
                                    sender_clone.input(AppInput::ReceiveMessage { channel, user: "System".to_string(), body: format!("{} joined.", user) });
                                }
                                Command::PART(channel, _) => {
                                    sender_clone.input(AppInput::UserLeft { channel: channel.clone(), user: user.clone() });
                                    sender_clone.input(AppInput::ReceiveMessage { channel, user: "System".to_string(), body: format!("{} left.", user) });
                                }
                                Command::NICK(new_nick) => {
                                    sender_clone.input(AppInput::UserRenamed { old: user, new: new_nick });
                                }
                                Command::QUIT(_) => {
                                    sender_clone.input(AppInput::UserQuit { user });
                                }
                                Command::NOTICE(_, body) => {
                                    sender_clone.input(AppInput::ReceiveServerMessage(format!("[Notice]: {body}")));
                                    if needs_nickserv && !channels_joined && body.contains("You are now identified") {
                                        channels_joined = true;
                                        join_channels(&irc_tx);
                                    }
                                    if body.contains("has been successfully registered") || body.contains("account has been verified") || body.contains("Account registered") {
                                        sender_clone.input(AppInput::ReceiveServerMessage(format!("[Auth Success]: {}", body)));
                                    }
                                }
                                Command::TOPIC(channel, Some(topic)) => {
                                    sender_clone.input(AppInput::ChannelTopic { channel, topic });
                                }
                                Command::CAP(_, sub, _, params) => {
                                    if is_sasl && sub == irc::proto::CapSubCommand::ACK {
                                        if let Some(exts) = params {
                                            if exts.contains("sasl") {
                                                if is_sasl_plain { let _ = irc_tx.send_sasl_plain(); }
                                                else if is_sasl_external { let _ = irc_tx.send_sasl_external(); }
                                            }
                                        }
                                    }
                                }
                                Command::AUTHENTICATE(data) => {
                                    if is_sasl_plain && data == "+" {
                                        let auth = format!("\0{}\0{}", nickname, pwd);
                                        // Bug #6 fixed: use base64 0.22 Engine API
                                        let encoded = BASE64.encode(auth.as_bytes());
                                        let _ = irc_tx.send_sasl(encoded);
                                    } else if is_sasl_external && data == "+" {
                                        let _ = irc_tx.send_sasl_external();
                                    }
                                }
                                Command::Response(code, args) => {
                                    if !channels_joined {
                                        match code {
                                            Response::RPL_LOGGEDIN => { channels_joined = true; join_channels(&irc_tx); }
                                            Response::RPL_ENDOFMOTD | Response::ERR_NOMOTD if !needs_nickserv => { channels_joined = true; join_channels(&irc_tx); }
                                            _ => {}
                                        }
                                    }
                                    if code == Response::RPL_NAMREPLY && args.len() >= 4 {
                                        // Bug #10 fixed: use is_channel_target instead of hardcoded '#'
                                        let channel = args.iter().find(|a| channels::is_channel_target(a)).cloned().unwrap_or_else(|| args[2].clone());
                                        let users: Vec<String> = args.last().unwrap_or(&String::new()).split_whitespace().map(|s| s.to_string()).collect();
                                        sender_clone.input(AppInput::BatchAddUsers { channel, users });
                                    } else if code == Response::RPL_LIST && args.len() >= 3 {
                                        let name = args.get(1).cloned().unwrap_or_default();
                                        let users: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                                        let topic = if args.len() > 3 { args[3..].join(" ") } else { String::new() };
                                        sender_clone.input(AppInput::ChannelListEntry { name, users, topic });
                                    } else if code == Response::RPL_LISTEND {
                                        sender_clone.input(AppInput::ChannelListEnd);
                                    } else if code == Response::RPL_TOPIC && args.len() >= 2 {
                                        let ch = args.get(1).cloned().unwrap_or_default();
                                        let topic = if args.len() > 2 { args[2..].join(" ") } else { String::new() };
                                        sender_clone.input(AppInput::ChannelTopic { channel: ch, topic });
                                    } else if args.len() > 1 {
                                        sender_clone.input(AppInput::ReceiveServerMessage(format!("[{code:?}]: {}", args[1..].join(" "))));
                                    }
                                }
                                _ => {}
                            }
                        }
                        sender_clone.input(AppInput::NetworkStatus(String::from("Disconnected")));
                        sender_clone.input(AppInput::ReceiveServerMessage(String::from("[System]: Connection closed.")));
                    });
                });
            }

            // Bug #5 fixed: UserDisconnect sets user_disconnected = true, suppressing auto-reconnect
            AppInput::UserDisconnect => {
                self.user_disconnected = true;
                sender.input(AppInput::Disconnect);
            }

            AppInput::Disconnect => {
                if let Some(irc_tx) = self.irc_sender.take() {
                    let _ = irc_tx.send_quit("Boulder Relay signing off");
                    self.connection = ConnectionState::Offline;
                    self.status = String::from("Offline");
                    self.channel_list_results.clear();
                    let ts = self.timestamp_prefix();
                    self.append_line(SERVER_TAB, ts, None, "Disconnected.".to_string(), LineStyle::System);
                }
            }

            AppInput::NetworkStatus(new_status) => {
                self.status = new_status.clone();
                if new_status == "Disconnected" || new_status.starts_with("Connection failed") {
                    let was_connected = self.connection == ConnectionState::Connected;
                    self.connection = ConnectionState::Offline;
                    self.irc_sender = None;
                    // Bug #5 fixed: only auto-reconnect if the user did NOT explicitly disconnect
                    if was_connected && !self.user_disconnected {
                        let s = sender.clone();
                        gtk::glib::timeout_add_seconds_local(5, move || {
                            s.input(AppInput::Connect);
                            gtk::glib::ControlFlow::Break
                        });
                    }
                    self.user_disconnected = false;
                }
            }

            AppInput::NetworkConnected(irc_tx) => {
                self.irc_sender = Some(irc_tx.clone());
                // Also store in senders map for multi-server
                self.senders.insert(self.server.clone(), Some(irc_tx.clone()));
                self.server_states.insert(self.server.clone(), ConnectionState::Connected);
                self.connection = ConnectionState::Connected;
                self.status = String::from("Connected");
                let ts = self.timestamp_prefix();
                self.append_line(SERVER_TAB, ts, None, format!("Connected to {} as {}.", self.server, self.nickname), LineStyle::System);
                let service = self.account_service.clone();
                if let Some(email) = self.pending_register_email.take() {
                    let cmd = format!("REGISTER {} {}", self.password, email);
                    let _ = irc_tx.send_privmsg(&service, &cmd);
                    let ts2 = self.timestamp_prefix();
                    self.append_line(SERVER_TAB, ts2, None, format!("Sent registration to {}. Check email.", service), LineStyle::System);
                }
            }

            AppInput::SelectChannel(channel) => {
                self.active_channel = channel.clone();
                self.unread_counts.remove(&channel);
                self.mention_counts.remove(&channel);
                self.show_channel_history();
                self.refresh_users(&sender);
                self.persist_settings();
            }

            AppInput::JoinChannel(target) => {
                if !self.channels.contains(&target) {
                    self.channels.push(target.clone());
                    let ts = self.timestamp_prefix();
                    self.chat_histories.insert(target.clone(), vec![ChatLine { timestamp: ts, user: None, body: format!("Tracking {}", target), style: LineStyle::System }]);
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
                if channel == SERVER_TAB || !channels::is_channel_target(&channel) { return; }
                if let Some(irc_tx) = &self.irc_sender { let _ = irc_tx.send_part(&channel); }
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
                if self.active_channel == channel { self.chat_view.buffer().set_text(""); }
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
                let ts = self.timestamp_prefix();
                let list = self.muted_users.entry(channel.clone()).or_insert_with(Vec::new);
                if list.contains(&user) {
                    list.retain(|u| u != &user);
                    self.append_line(&channel, ts, None, format!("Unmuted {}", user), LineStyle::System);
                } else {
                    list.push(user.clone());
                    list.sort_by_key(|u| u.to_lowercase());
                    self.append_line(&channel, ts, None, format!("Muted {}", user), LineStyle::System);
                }
                if self.active_channel == channel { self.refresh_users(&sender); }
            }

            AppInput::ReceiveMessage { channel, user, body } => {
                let clean = Self::normalized_nick(&user);
                if self.ignored_users.contains(&clean) { return; }
                if self.is_muted(&channel, &user) { return; }
                if !self.channels.contains(&channel) && !channels::is_channel_target(&channel) {
                    self.channels.push(channel.clone());
                    self.refresh_channels(&sender);
                }
                let style = self.message_style(&user, &body);
                self.append_message(&channel, &user, &body, style);
                // Track unread/mention counts for inactive channels
                if channel != self.active_channel {
                    *self.unread_counts.entry(channel.clone()).or_insert(0) += 1;
                    if style == LineStyle::Mention {
                        *self.mention_counts.entry(channel.clone()).or_insert(0) += 1;
                    }
                }
                if self.should_notify(&channel, &user, style) {
                    let kind = self.notify_kind(&channel, style);
                    notify::send_message_notification(&channel, &user, &body, kind);
                }
            }

            AppInput::ReceiveServerMessage(body) => {
                let ts = self.timestamp_prefix();
                self.append_line(SERVER_TAB, ts, None, body, LineStyle::System);
            }

            AppInput::BatchAddUsers { channel, users } => {
                let list = self.channel_users.entry(channel.clone()).or_insert_with(Vec::new);
                for u in users { if !list.contains(&u) { list.push(u); } }
                list.sort_by_key(|a| a.to_lowercase());
                if self.active_channel == channel { self.refresh_users(&sender); }
            }

            AppInput::UserJoined { channel, user } => {
                let list = self.channel_users.entry(channel.clone()).or_insert_with(Vec::new);
                if !list.contains(&user) { list.push(user); list.sort_by_key(|a| a.to_lowercase()); }
                if self.active_channel == channel { self.refresh_users(&sender); }
            }

            AppInput::UserLeft { channel, user } => {
                if let Some(list) = self.channel_users.get_mut(&channel) { list.retain(|u| u != &user); }
                if self.active_channel == channel { self.refresh_users(&sender); }
            }

            AppInput::UserQuit { user } => {
                for list in self.channel_users.values_mut() { list.retain(|u| u != &user); }
                self.refresh_users(&sender);
            }

            // Bug #9 fixed: JoinEntry only routes slash-commands to SendMessage;
            // raw text always goes to JoinChannel to avoid dropping /msg body
            AppInput::JoinEntry(text) => {
                let text = text.trim();
                if text.is_empty() { return; }
                if text.starts_with('/') {
                    sender.input(AppInput::SendMessage(text.to_string()));
                    return;
                }
                // Support comma-separated multi-join from the join box
                if text.contains(',') {
                    for target in channels::parse_join_command_multi(text) {
                        sender.input(AppInput::JoinChannel(target));
                    }
                    return;
                }
                match channels::parse_join_entry(text) {
                    Some(channels::JoinTarget::Channel(ch)) => sender.input(AppInput::JoinChannel(ch)),
                    Some(channels::JoinTarget::DirectMessage(nick)) => sender.input(AppInput::JoinChannel(nick)),
                    Some(channels::JoinTarget::Multi(_)) => {}
                    None => {}
                }
            }

            AppInput::SendMessage(text) => {
                let text = text.trim();
                if text.is_empty() { return; }
                if text.starts_with('/') {
                    let mut parts = text.splitn(3, ' ');
                    let command = parts.next().unwrap_or("");
                    match command {
                        "/join" | "/j" => {
                            if let Some(raw) = parts.next() {
                                for ch in channels::parse_join_command_multi(raw) {
                                    sender.input(AppInput::JoinChannel(ch));
                                }
                            }
                            return;
                        }
                        "/msg" | "/query" => {
                            // Bug #9 fixed: splitn(3, ' ') so the full message body is captured
                            if let Some(target) = parts.next() {
                                let body = parts.next().unwrap_or("");
                                if !body.is_empty() {
                                    if let Some(irc_tx) = &self.irc_sender {
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
                                if let Some(irc_tx) = &self.irc_sender { let _ = irc_tx.send(Message::from(format!("NICK {}", nick).as_str())); }
                                self.nickname = nick.to_string();
                                self.persist_settings();
                            }
                            return;
                        }
                        "/part" => {
                            let target = parts.next().map(str::to_string).unwrap_or_else(|| self.active_channel.clone());
                            sender.input(AppInput::PartChannel(target));
                            return;
                        }
                        "/clear" => { sender.input(AppInput::ClearChannel(self.active_channel.clone())); return; }
                        "/help" => {
                            let channel = self.active_channel.clone();
                            let ts = self.timestamp_prefix();
                            self.append_line(&channel, ts, None, HELP_TEXT.to_string(), LineStyle::System);
                            return;
                        }
                        "/me" => {
                            let action = parts.next().unwrap_or("");
                            if !action.is_empty() {
                                let full = format!("\x01ACTION {}\x01", action);
                                if let Some(irc_tx) = &self.irc_sender { let _ = irc_tx.send_privmsg(&self.active_channel, &full); }
                                let me_user = format!("* {}", self.nickname);
                                let chan = self.active_channel.clone();
                                self.append_message(&chan, &me_user, action, LineStyle::SelfMsg);
                            }
                            return;
                        }
                        "/list" => { sender.input(AppInput::BrowseChannels); return; }
                        "/whois" => {
                            if let Some(target) = parts.next() {
                                if let Some(irc_tx) = &self.irc_sender { let _ = irc_tx.send(Message::from(format!("WHOIS {}", target).as_str())); }
                            }
                            return;
                        }
                        "/away" => {
                            let msg = parts.next().unwrap_or("Away");
                            if let Some(irc_tx) = &self.irc_sender { let _ = irc_tx.send(Message::from(format!("AWAY :{}", msg).as_str())); }
                            return;
                        }
                        "/back" => {
                            if let Some(irc_tx) = &self.irc_sender { let _ = irc_tx.send(Message::from("AWAY")); }
                            return;
                        }
                        "/topic" => {
                            let new_topic = parts.next().unwrap_or("");
                            let chan = self.active_channel.clone();
                            if let Some(irc_tx) = &self.irc_sender {
                                if new_topic.is_empty() { let _ = irc_tx.send(Message::from(format!("TOPIC {}", chan).as_str())); }
                                else { let _ = irc_tx.send(Message::from(format!("TOPIC {} :{}", chan, new_topic).as_str())); }
                            }
                            return;
                        }
                        "/ignore" => {
                            if let Some(target) = parts.next() {
                                sender.input(AppInput::IgnoreUser(target.to_string()));
                            }
                            return;
                        }
                        "/unignore" => {
                            if let Some(target) = parts.next() {
                                sender.input(AppInput::UnignoreUser(target.to_string()));
                            }
                            return;
                        }
                        _ => {}
                    }
                }

                if self.active_channel == SERVER_TAB {
                    let ts = self.timestamp_prefix();
                    self.append_line(SERVER_TAB, ts, None, "Select a channel or DM before sending.".to_string(), LineStyle::System);
                    return;
                }

                if let Some(irc_tx) = self.irc_sender.clone() {
                    if irc_tx.send_privmsg(&self.active_channel, text).is_ok() {
                        let channel = self.active_channel.clone();
                        let my_nick = self.nickname.clone();
                        self.append_message(&channel, &my_nick, text, LineStyle::SelfMsg);
                    }
                } else {
                    let ts = self.timestamp_prefix();
                    let channel = self.active_channel.clone();
                    self.append_line(&channel, ts, None, "Cannot send: not connected.".to_string(), LineStyle::System);
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

