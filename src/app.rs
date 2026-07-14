use relm4::prelude::*;
use gtk::prelude::*;
use relm4::{gtk, ComponentParts, ComponentSender, SimpleComponent};
use relm4::RelmWidgetExt;
use crate::matrix::client::{MatrixClient, MatrixEvent};
use crate::irc::IrcModel;
use crate::ui::dialogs::{show_matrix_login_dialog, show_matrix_join_dialog};
use crate::ui::sidebar::build_sidebar;
use crate::ui::chat_view::ChatViewModel;
use crate::ui::composer::build_composer;
use tokio::sync::mpsc;
use std::sync::Arc;

#[derive(Debug)]
pub enum AppInput {
    MatrixLogin { homeserver: String, username: String, password: String },
    MatrixJoinRoom(String),
    MatrixEvent(MatrixEvent),
    SelectRoom(String),
    SendMessage(String),
    ShowMatrixLogin,
    ShowMatrixJoinRoom,
    ConnectIrc { server: String, port: u16, nick: String, channel: String },
    IrcMessage(String),
}

pub struct AppModel {
    pub matrix_client: Option<Arc<MatrixClient>>,
    pub irc_model: IrcModel,
    pub active_room: Option<String>,
    pub matrix_tx: Option<mpsc::UnboundedSender<MatrixEvent>>,
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Input = AppInput;
    type Output = ();
    type Init = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("boulderX"),
            set_default_size: (1100, 700),

            gtk::Box {
                set_orientation: gtk::Orientation::Horizontal,

                #[name = "sidebar"]
                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_width_request: 240,
                },

                gtk::Separator {
                    set_orientation: gtk::Orientation::Vertical,
                },

                gtk::Box {
                    set_orientation: gtk::Orientation::Vertical,
                    set_hexpand: true,

                    #[name = "chat_area"]
                    gtk::ScrolledWindow {
                        set_vexpand: true,
                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,
                            set_margin_all: 12,
                        }
                    },

                    #[name = "composer_area"]
                    gtk::Box {
                        set_orientation: gtk::Orientation::Horizontal,
                        set_margin_all: 8,
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
        let model = AppModel {
            matrix_client: None,
            irc_model: IrcModel::new(),
            active_room: None,
            matrix_tx: None,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, input: Self::Input, sender: ComponentSender<Self>) {
        match input {
            AppInput::ShowMatrixLogin => {
                // Dialog shown from UI layer
            }
            AppInput::ShowMatrixJoinRoom => {
                // Dialog shown from UI layer
            }
            AppInput::MatrixLogin { homeserver, username, password } => {
                let sender2 = sender.clone();
                tokio::spawn(async move {
                    match MatrixClient::new(&homeserver).await {
                        Ok(client) => {
                            if let Err(e) = client.login_password(&username, &password).await {
                                let _ = sender2.input(AppInput::MatrixEvent(
                                    MatrixEvent::SyncError(e.to_string())
                                ));
                                return;
                            }
                            let (tx, mut rx) = mpsc::unbounded_channel::<MatrixEvent>();
                            let client_arc = Arc::new(client.clone());
                            client.start_sync(tx.clone());
                            let sender3 = sender2.clone();
                            tokio::spawn(async move {
                                while let Some(ev) = rx.recv().await {
                                    sender3.input(AppInput::MatrixEvent(ev));
                                }
                            });
                        }
                        Err(e) => {
                            let _ = sender2.input(AppInput::MatrixEvent(
                                MatrixEvent::SyncError(e.to_string())
                            ));
                        }
                    }
                });
            }
            AppInput::MatrixJoinRoom(alias) => {
                if let Some(client) = &self.matrix_client {
                    let client = Arc::clone(client);
                    tokio::spawn(async move {
                        let _ = client.inner.join_room_by_id_or_alias(
                            alias.as_str().try_into().unwrap(),
                            &[]
                        ).await;
                    });
                }
            }
            AppInput::MatrixEvent(ev) => {
                match ev {
                    MatrixEvent::Connected { user_id: _ } => {}
                    MatrixEvent::RoomMessage { .. } => {}
                    MatrixEvent::RoomJoined { .. } => {}
                    MatrixEvent::RoomLeft { .. } => {}
                    MatrixEvent::SyncError(_) => {}
                    MatrixEvent::Disconnected => {}
                }
            }
            AppInput::SelectRoom(room_id) => {
                self.active_room = Some(room_id);
            }
            AppInput::SendMessage(body) => {
                if let (Some(client), Some(room_id)) = (&self.matrix_client, &self.active_room) {
                    let client = Arc::clone(client);
                    let room_id: matrix_sdk::ruma::OwnedRoomId = room_id.as_str().try_into().unwrap();
                    tokio::spawn(async move {
                        let _ = client.send_message(&room_id, &body).await;
                    });
                }
            }
            AppInput::ConnectIrc { .. } => {}
            AppInput::IrcMessage(_) => {}
        }
    }
}
