use adw;
use adw::prelude::*;
use relm4::gtk;

const CSS: &str = r#"
/* Element-style three-pane shell with boulderX colors */

.boulder-relay {
  background-color: #0f1117;
  color: #eef2f7;
  font-family: "Inter", "Cantarell", sans-serif;
  font-size: 14px;
}

.navigation-shell {
  background-color: #151821;
  border-right: 1px solid #293042;
}

.space-rail {
  background-color: #0d1017;
  border-right: 1px solid #293042;
}

.rail-logo {
  min-width: 42px;
  min-height: 42px;
  border-radius: 14px;
  background: linear-gradient(135deg, #18b6f6, #7c3aed);
  color: #ffffff;
  font-weight: 800;
  font-size: 13px;
}

.rail-button {
  min-width: 42px;
  min-height: 42px;
  border-radius: 14px;
  border: 1px solid transparent;
  background-color: transparent;
  color: #aab4c4;
  font-weight: 800;
  font-size: 11px;
  padding: 0;
}

.rail-button:hover {
  background-color: #202638;
  color: #ffffff;
}

.rail-irc { color: #f59e0b; }
.rail-matrix { color: #22c55e; }
.rail-discord { color: #8b9cff; }

.sidebar {
  background-color: #171b26;
  min-width: 264px;
}

.sidebar-header {
  padding: 14px 14px 10px 14px;
}

.app-title {
  font-size: 18px;
  font-weight: 800;
  color: #f8fafc;
  letter-spacing: -0.02em;
}

.sidebar-subtitle {
  font-size: 11px;
  color: #7f8ca3;
}

.protocol-tabs {
  background-color: #10131b;
  border: 1px solid #293042;
  border-radius: 14px;
  padding: 4px;
}

.tab-button {
  min-height: 28px;
  border-radius: 10px;
  background-color: transparent;
  color: #aab4c4;
  font-size: 12px;
  font-weight: 750;
  padding: 4px 9px;
}

.tab-button:hover {
  background-color: #222838;
  color: #ffffff;
}

.tab-irc:hover { color: #fbbf24; }
.tab-matrix:hover { color: #4ade80; }
.tab-discord:hover { color: #aeb8ff; }

.sidebar-section-header {
  font-size: 11px;
  font-weight: 800;
  color: #7f8ca3;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  padding: 12px 12px 5px 12px;
}

searchentry,
entry {
  min-height: 36px;
  border-radius: 12px;
  border: 1px solid #30384d;
  background-color: #10131b;
  color: #eef2f7;
}

searchentry:focus,
entry:focus {
  border-color: #18b6f6;
  box-shadow: 0 0 0 2px rgba(24, 182, 246, 0.2);
}

.room-row {
  border-radius: 12px;
  margin: 2px 8px;
  padding: 2px 4px;
  transition: background 120ms ease;
}

.room-row:hover {
  background-color: #222838;
}

.room-row-active {
  background-color: #263146;
  box-shadow: inset 3px 0 #18b6f6;
}

.room-name {
  font-size: 14px;
  font-weight: 650;
  color: #edf2f7;
}

.room-avatar {
  min-width: 34px;
  min-height: 34px;
  border-radius: 50%;
  background: linear-gradient(135deg, #263146, #34415b);
  color: #dbeafe;
  font-weight: 800;
  font-size: 13px;
  padding: 2px;
}

.protocol-badge {
  font-size: 9px;
  font-weight: 800;
  border-radius: 999px;
  padding: 1px 7px;
  letter-spacing: 0.05em;
}

.badge-irc {
  background-color: rgba(245, 158, 11, 0.16);
  color: #fbbf24;
  border: 1px solid rgba(245, 158, 11, 0.28);
}

.badge-matrix {
  background-color: rgba(34, 197, 94, 0.16);
  color: #4ade80;
  border: 1px solid rgba(34, 197, 94, 0.28);
}

.badge-discord {
  background-color: rgba(139, 156, 255, 0.16);
  color: #aeb8ff;
  border: 1px solid rgba(139, 156, 255, 0.28);
}

.unread-badge {
  background-color: #ef4444;
  color: #ffffff;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 800;
  min-width: 19px;
  padding: 1px 6px;
}

.chat-panel {
  background-color: #10131b;
}

.channel-header {
  background-color: #151821;
  border-bottom: 1px solid #293042;
  padding: 12px 18px;
}

.channel-title {
  font-size: 18px;
  font-weight: 800;
  color: #f8fafc;
}

.channel-topic {
  font-size: 13px;
  color: #94a3b8;
}

.chat-view {
  background-color: #10131b;
  color: #e5edf7;
  font-family: "Inter", "Cantarell", sans-serif;
  font-size: 14px;
  line-height: 1.55;
}

.composer {
  background-color: #151821;
  border-top: 1px solid #293042;
  padding: 10px 14px;
}

.composer-entry {
  background-color: #0f131d;
  border: 1px solid #30384d;
  border-radius: 18px;
  color: #eef2f7;
  padding: 7px 14px;
  font-size: 14px;
}

.composer-send {
  background-color: #18b6f6;
  color: #06111a;
  border-radius: 50%;
  min-width: 38px;
  min-height: 38px;
  font-size: 16px;
  font-weight: 800;
  padding: 0;
}

.composer-send:hover {
  background-color: #5fd3ff;
}

.users-panel {
  background-color: #171b26;
  border-left: 1px solid #293042;
  min-width: 180px;
}

.user-btn {
  background: transparent;
  border: none;
  color: #dbe4f0;
  font-size: 13px;
  padding: 4px 8px;
  border-radius: 8px;
}

.user-btn:hover {
  background-color: #222838;
}

.muted-user {
  opacity: 0.45;
  text-decoration: line-through;
}

.mute-btn {
  background: transparent;
  border: none;
  font-size: 13px;
  padding: 2px 5px;
  border-radius: 6px;
  color: #7f8ca3;
}

.empty-title {
  color: #e5edf7;
  font-weight: 700;
}

.empty-body {
  color: #7f8ca3;
  font-size: 12px;
}

.status-connected {
  color: #4ade80;
  font-size: 12px;
  font-weight: 700;
}

.status-connecting {
  color: #fbbf24;
  font-size: 12px;
  font-weight: 700;
}

.status-offline {
  color: #7f8ca3;
  font-size: 12px;
}

.welcome-panel {
  background-color: #10131b;
  border: 1px solid #30384d;
  border-radius: 16px;
  padding: 14px;
}

.welcome-title {
  font-size: 15px;
  font-weight: 800;
  color: #f8fafc;
}

.welcome-body {
  font-size: 12.5px;
  color: #9aa8bc;
}

.fav-btn,
.part-btn {
  background: transparent;
  border: none;
  color: #7f8ca3;
  font-size: 13px;
  padding: 1px 5px;
  border-radius: 6px;
  min-width: 0;
  opacity: 0.45;
}

.room-row:hover .fav-btn,
.room-row:hover .part-btn {
  opacity: 1;
}

.fav-btn:hover {
  color: #fbbf24;
  opacity: 1;
}

.part-btn:hover {
  color: #f87171;
  opacity: 1;
}

.suggested-action {
  background-color: #18b6f6;
  color: #06111a;
  border-radius: 10px;
  font-weight: 750;
}

.suggested-action:hover {
  background-color: #5fd3ff;
}

.destructive-action {
  background-color: #dc2626;
  color: #ffffff;
  border-radius: 10px;
  font-weight: 750;
}

.destructive-action:hover {
  background-color: #ef4444;
}

.flat {
  background: transparent;
  border: none;
  color: #aab4c4;
  font-size: 13px;
  border-radius: 10px;
  padding: 6px 9px;
}

.flat:hover {
  background-color: #222838;
  color: #ffffff;
}

.dialog-title {
  font-size: 18px;
  font-weight: 800;
  color: #f8fafc;
  margin-bottom: 8px;
}

separator {
  background-color: #293042;
}

scrollbar slider {
  background-color: #3b455d;
  border-radius: 999px;
  min-width: 6px;
  min-height: 6px;
}

scrollbar slider:hover {
  background-color: #58657d;
}

scrollbar trough {
  background-color: transparent;
}
"#;

pub fn load_css() {
    let provider = gtk::CssProvider::new();
    provider.load_from_data(CSS);
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub fn build_titlebar() -> adw::HeaderBar {
    let bar = adw::HeaderBar::new();
    bar.set_show_end_title_buttons(true);
    bar.add_css_class("flat");
    bar
}

pub fn attach_window(window: &gtk::Window) {
    window.add_css_class("boulder-relay");
}
