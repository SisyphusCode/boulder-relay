use gtk::prelude::*;
use gtk::{gdk, CssProvider, Settings, STYLE_PROVIDER_PRIORITY_USER, WindowControls};

pub const GRUVBOX_CSS: &str = r#"
window.boulder-relay {
    background-color: #282828;
    color: #ebdbb2;
}

headerbar.boulder-header {
    background-color: #1d2021;
    background-image: none;
    color: #ebdbb2;
    border-bottom: 1px solid #504945;
    box-shadow: none;
    min-height: 46px;
}

headerbar.boulder-header label,
headerbar.boulder-header .title {
    color: #10B981;
    font-family: monospace;
    font-weight: bold;
}

headerbar.boulder-header windowcontrols button,
headerbar.boulder-header button {
    color: #ebdbb2;
    background-color: transparent;
    background-image: none;
    border: none;
    box-shadow: none;
}

headerbar.boulder-header windowcontrols button:hover,
headerbar.boulder-header button:hover {
    background-color: #3c3836;
}

.boulder-relay {
    background-color: #282828;
    color: #ebdbb2;
}

.boulder-relay .sidebar {
    background-color: #1d2021;
}

.boulder-relay .chat-panel {
    background-color: #282828;
}

.boulder-relay label {
    color: #ebdbb2;
    font-family: monospace;
}

.boulder-relay .sidebar-title {
    font-weight: bold;
    color: #10B981;
}

/* Sisyphus branding accents */
.boulder-relay {
    --sisyphus-accent: #3B82F6;
}
.boulder-relay .sidebar-title {
    color: var(--sisyphus-accent, #10B981);
}

.boulder-relay .sidebar-subtitle {
    font-weight: bold;
    color: #b8bb26;
    font-size: 0.9em;
}

.boulder-relay .channel-section {
    font-weight: bold;
    color: #928374;
    font-size: 0.85em;
    letter-spacing: 0.04em;
}

.boulder-relay .channel-topic {
    color: #928374;
    font-size: 0.82em;
    font-style: italic;
}

.boulder-relay button.channel-rocky {
    color: #10B981;
}

.boulder-relay button.channel-fedora {
    color: #3C6EB4;
}

.boulder-relay .status-connected { color: #b8bb26; }
.boulder-relay .status-connecting { color: #fabd2f; }
.boulder-relay .status-offline { color: #928374; }

.boulder-relay button {
    background-color: #3c3836;
    color: #b8bb26;
    border: 1px solid #504945;
    border-radius: 4px;
    padding: 6px 12px;
    font-family: monospace;
}

.boulder-relay button:hover { background-color: #504945; }
.boulder-relay button.destructive { color: #fb4934; }
.boulder-relay button.part-btn {
    color: #928374;
    padding: 4px 8px;
    min-width: 0;
}

.boulder-relay button.part-btn:hover {
    color: #fb4934;
    background-color: #3c3836;
}

.boulder-relay entry {
    background-color: #3c3836;
    color: #ebdbb2;
    border: 1px solid #504945;
    border-radius: 4px;
    padding: 8px;
    font-family: monospace;
}

.boulder-relay entry:focus { border: 1px solid #fe8019; }

.boulder-relay scrolledwindow,
.boulder-relay scrolledwindow viewport,
.boulder-relay paned,
.boulder-relay paned > separator {
    background-color: #282828;
}

.boulder-relay paned separator {
    background-color: #504945;
    min-width: 2px;
    min-height: 2px;
}

.boulder-relay separator {
    background-color: #504945;
    color: #504945;
}

.boulder-relay textview {
    background-color: #282828;
    color: #ebdbb2;
    font-family: "Monospace", monospace;
    font-size: 0.95em;
    padding: 6px;
}

.boulder-relay entry {
    font-family: "Monospace", monospace;
}

.boulder-relay textview text {
    background-color: #282828;
    color: #ebdbb2;
}

.boulder-relay .user-btn {
    background-color: transparent;
    color: #83a598;
    border: none;
    box-shadow: none;
    padding: 4px 12px;
    font-family: monospace;
}

.boulder-relay .user-btn:hover {
    background-color: #3c3836;
    color: #ebdbb2;
}

.boulder-relay .fav-btn {
    background-color: transparent;
    color: #fabd2f;
    border: 1px solid transparent;
    box-shadow: none;
    padding: 6px 8px;
    font-family: monospace;
}

.boulder-relay .fav-btn:hover {
    background-color: #3c3836;
    border: 1px solid #504945;
    color: #fbf1c7;
}

.boulder-relay .mute-btn {
    background-color: transparent;
    color: #928374;
    border: 1px solid transparent;
    box-shadow: none;
    padding: 4px 8px;
    font-family: monospace;
}

.boulder-relay .mute-btn:hover {
    background-color: #3c3836;
    border: 1px solid #504945;
    color: #ebdbb2;
}

.boulder-relay .muted-user {
    color: #928374;
    text-decoration: line-through;
}

/* Better density and fonts */
.boulder-relay .sidebar {
    padding: 4px;
}

.boulder-relay button, .boulder-relay entry {
    padding: 4px 8px;
}

/* Stub for light theme or system */
@media (prefers-color-scheme: light) {
    /* Extend for light Gruvbox variant if desired */
}
"#;

pub fn apply_gtk_settings() {
    if let Some(settings) = Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(true);
    }
}

pub fn load_css() {
    apply_gtk_settings();

    let provider = CssProvider::new();
    provider.load_from_data(GRUVBOX_CSS);

    let display = gdk::Display::default().expect("GTK display must be initialized before loading CSS");
    gtk::style_context_add_provider_for_display(&display, &provider, STYLE_PROVIDER_PRIORITY_USER);
}

pub fn build_titlebar() -> gtk::HeaderBar {
    let header = gtk::HeaderBar::new();
    header.add_css_class("boulder-header");
    // Avoid Adwaita's light `default-decoration` styling on GNOME.
    header.set_show_title_buttons(false);

    let controls = WindowControls::builder()
        .side(gtk::PackType::End)
        .build();
    header.pack_end(&controls);

    let title = gtk::Label::builder()
        .label("Boulder Relay — Sisyphus")
        .css_classes(["title"])
        .build();
    header.set_title_widget(Some(&title));

    header
}

pub fn attach_window(window: &gtk::Window) {
    window.add_css_class("boulder-relay");
    window.set_titlebar(Some(&build_titlebar()));
    window.set_title(Some(""));
    window.set_icon_name(Some("boulder-relay"));
}

pub fn attach_adw_window(window: &adw::Window) {
    window.add_css_class("boulder-relay");
    // For adw, we can use AdwHeaderBar but keep custom for Gruvbox
    window.set_titlebar(Some(&build_titlebar()));
    window.set_title("");
    window.set_icon_name("boulder-relay");
}
