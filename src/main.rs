mod app;
mod channels;
mod config;
mod irc;
mod matrix;
mod notify;
mod theme;
mod ui;

use relm4::RelmApp;
use relm4::gtk;
use adw;
use gtk4::prelude::ApplicationExt;

use app::AppModel;

fn main() {
    gtk::init().expect("Failed to initialize GTK");
    let application = adw::Application::new(Some(notify::APP_ID), Default::default());
    application.connect_startup(|_| {
        theme::load_css();
        notify::setup_application_icon();
    });
    let relm_app = RelmApp::from_app(application);
    relm_app.run::<AppModel>(());
}
