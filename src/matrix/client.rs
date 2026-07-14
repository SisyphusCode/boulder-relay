use matrix_sdk::{
    Client,
    config::SyncSettings,
    room::Room,
    ruma::{
        events::room::message::{RoomMessageEventContent, MessageType, SyncRoomMessageEvent},
        OwnedRoomId, OwnedUserId,
    },
};
use tokio::sync::mpsc;
use crate::app::AppInput;

#[derive(Debug, Clone)]
pub enum MatrixEvent {
    Connected { user_id: String },
    RoomMessage { room_id: String, room_name: String, sender: String, body: String },
    RoomJoined { room_id: String, room_name: String },
    RoomLeft { room_id: String },
    SyncError(String),
    Disconnected,
}

#[derive(Clone)]
pub struct MatrixClient {
    pub inner: Client,
}

impl MatrixClient {
    pub async fn new(homeserver: &str) -> anyhow::Result<Self> {
        let client = Client::builder()
            .homeserver_url(homeserver)
            .sqlite_store("boulderX-matrix", None)
            .build()
            .await?;
        Ok(Self { inner: client })
    }

    pub async fn login_password(&self, username: &str, password: &str) -> anyhow::Result<()> {
        self.inner
            .matrix_auth()
            .login_username(username, password)
            .initial_device_display_name("boulderX")
            .await?;
        Ok(())
    }

    pub async fn login_token(&self, token: &str) -> anyhow::Result<()> {
        self.inner
            .matrix_auth()
            .login_token(token)
            .await?;
        Ok(())
    }

    pub fn user_id(&self) -> Option<OwnedUserId> {
        self.inner.user_id().map(|u| u.to_owned())
    }

    pub async fn send_message(&self, room_id: &OwnedRoomId, body: &str) -> anyhow::Result<()> {
        if let Some(room) = self.inner.get_room(room_id) {
            let content = RoomMessageEventContent::text_plain(body);
            room.send(content).await?;
        }
        Ok(())
    }

    pub async fn joined_rooms(&self) -> Vec<(OwnedRoomId, String)> {
        self.inner
            .joined_rooms()
            .into_iter()
            .map(|r| {
                let id = r.room_id().to_owned();
                let name = r.name().unwrap_or_else(|| id.to_string());
                (id, name)
            })
            .collect()
    }

    pub fn start_sync(self, tx: mpsc::UnboundedSender<MatrixEvent>) {
        tokio::spawn(async move {
            let user_id = self.inner.user_id().map(|u| u.to_string()).unwrap_or_default();
            let _ = tx.send(MatrixEvent::Connected { user_id });

            self.inner.add_event_handler({
                let tx2 = tx.clone();
                move |ev: SyncRoomMessageEvent, room: Room| {
                    let tx3 = tx2.clone();
                    async move {
                        if let SyncRoomMessageEvent::Original(orig) = ev {
                            let sender = orig.sender.to_string();
                            let room_id = room.room_id().to_string();
                            let room_name = room.name().unwrap_or_else(|| room_id.clone());
                            let body = match orig.content.msgtype {
                                MessageType::Text(t) => t.body,
                                _ => return,
                            };
                            let _ = tx3.send(MatrixEvent::RoomMessage {
                                room_id,
                                room_name,
                                sender,
                                body,
                            });
                        }
                    }
                }
            });

            let settings = SyncSettings::default();
            if let Err(e) = self.inner.sync(settings).await {
                let _ = tx.send(MatrixEvent::SyncError(e.to_string()));
            }
            let _ = tx.send(MatrixEvent::Disconnected);
        });
    }
}
