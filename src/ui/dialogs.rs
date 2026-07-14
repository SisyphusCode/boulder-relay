use gtk::prelude::*;
use relm4::{gtk, ComponentSender};
use relm4::RelmWidgetExt;
use crate::app::{AppInput, AppModel};

pub fn show_matrix_login_dialog(parent: &gtk::Window, sender: &ComponentSender<AppModel>) {
    let dialog = gtk::Window::builder()
        .transient_for(parent).modal(true).title("Sign in to Matrix")
        .default_width(420).default_height(320).build();
    dialog.add_css_class("boulder-relay");
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 12);
    vbox.set_margin_all(20);
    let title = gtk::Label::builder().label("Matrix Login").halign(gtk::Align::Start).build();
    title.add_css_class("dialog-title");
    vbox.append(&title);
    let hs_entry = gtk::Entry::builder()
        .placeholder_text("Homeserver (e.g. https://matrix.org)").text("https://matrix.org").build();
    vbox.append(&gtk::Label::new(Some("Homeserver:")));
    vbox.append(&hs_entry);
    let user_entry = gtk::Entry::builder().placeholder_text("@user:matrix.org").build();
    vbox.append(&gtk::Label::new(Some("Username:")));
    vbox.append(&user_entry);
    let pass_entry = gtk::Entry::builder().placeholder_text("Password").visibility(false).build();
    vbox.append(&gtk::Label::new(Some("Password:")));
    vbox.append(&pass_entry);
    let status = gtk::Label::builder().label("").halign(gtk::Align::Start).wrap(true).build();
    status.add_css_class("status-connecting");
    vbox.append(&status);
    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk::Align::End);
    btn_box.set_margin_top(8);
    let cancel = gtk::Button::with_label("Cancel");
    let d1 = dialog.clone();
    cancel.connect_clicked(move |_| d1.close());
    btn_box.append(&cancel);
    let login_btn = gtk::Button::with_label("Sign In");
    login_btn.add_css_class("suggested-action");
    let s = sender.clone(); let d2 = dialog.clone();
    let hs = hs_entry.clone(); let usr = user_entry.clone();
    let pwd = pass_entry.clone(); let st = status.clone();
    login_btn.connect_clicked(move |_| {
        let homeserver = hs.text().to_string().trim().to_string();
        let username = usr.text().to_string().trim().to_string();
        let password = pwd.text().to_string();
        if homeserver.is_empty() || username.is_empty() || password.is_empty() {
            st.set_label("All fields required."); return;
        }
        st.set_label("Connecting\u{2026}");
        s.input(AppInput::MatrixLogin { homeserver, username, password });
        d2.close();
    });
    btn_box.append(&login_btn);
    vbox.append(&btn_box);
    dialog.set_child(Some(&vbox));
    dialog.present();
}

pub fn show_matrix_join_dialog(parent: &gtk::Window, sender: &ComponentSender<AppModel>) {
    let dialog = gtk::Window::builder()
        .transient_for(parent).modal(true).title("Join Matrix Room")
        .default_width(380).default_height(180).build();
    dialog.add_css_class("boulder-relay");
    let vbox = gtk::Box::new(gtk::Orientation::Vertical, 10);
    vbox.set_margin_all(16);
    let entry = gtk::Entry::builder().placeholder_text("#room:matrix.org or !roomid:server").build();
    vbox.append(&gtk::Label::new(Some("Room alias or ID:")));
    vbox.append(&entry);
    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_box.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let d1 = dialog.clone();
    cancel.connect_clicked(move |_| d1.close());
    btn_box.append(&cancel);
    let join_btn = gtk::Button::with_label("Join");
    join_btn.add_css_class("suggested-action");
    let s = sender.clone(); let d2 = dialog.clone(); let e = entry.clone();
    join_btn.connect_clicked(move |_| {
        let alias = e.text().to_string().trim().to_string();
        if !alias.is_empty() { s.input(AppInput::MatrixJoinRoom(alias)); d2.close(); }
    });
    btn_box.append(&join_btn);
    vbox.append(&btn_box);
    dialog.set_child(Some(&vbox));
    dialog.present();
}
