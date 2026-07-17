use matrix_sdk::{
    Client,
    config::SyncSettings,
    room::Room,
    ruma::{
        events::room::message::{RoomMessageEventContent, MessageType, SyncRoomMessageEvent},
        OwnedRoomId, OwnedUserId,
    },
};
use std::path::PathBuf;
use tokio::sync::mpsc;

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

/// Stable XDG data dir for the Matrix SQLite store (not process CWD).
pub fn matrix_store_dir() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|h| PathBuf::from(h).join(".local").join("share"))
                .unwrap_or_else(|_| PathBuf::from(".local/share"))
        });
    base.join("boulderX").join("matrix")
}

impl MatrixClient {
    pub async fn new(homeserver: &str) -> anyhow::Result<Self> {
        let store = matrix_store_dir();
        std::fs::create_dir_all(&store)?;
        let client = Client::builder()
            .homeserver_url(homeserver)
            .sqlite_store(store, None)
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
        let Some(room) = self.inner.get_room(room_id) else {
            anyhow::bail!("room not found in session: {room_id}");
        };
        let content = RoomMessageEventContent::text_plain(body);
        room.send(content).await?;
        Ok(())
    }

    /// Send an emote (`m.emote`) to a Matrix room.
    pub async fn send_emote(&self, room_id: &OwnedRoomId, body: &str) -> anyhow::Result<()> {
        let Some(room) = self.inner.get_room(room_id) else {
            anyhow::bail!("room not found in session: {room_id}");
        };
        let content = RoomMessageEventContent::emote_plain(body);
        room.send(content).await?;
        Ok(())
    }

    pub async fn leave_room(&self, room_id: &OwnedRoomId) -> anyhow::Result<()> {
        let Some(room) = self.inner.get_room(room_id) else {
            anyhow::bail!("room not found: {room_id}");
        };
        room.leave().await?;
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

    pub fn start_sync(&self, tx: mpsc::UnboundedSender<MatrixEvent>) {
        let inner = self.inner.clone();
        // Caller must invoke this from the shared runtime (runtime::spawn).
        tokio::spawn(async move {
            let user_id = inner.user_id().map(|u| u.to_string()).unwrap_or_default();
            let _ = tx.send(MatrixEvent::Connected { user_id });

            // Seed already-joined rooms so the sidebar populates after login.
            for room in inner.joined_rooms() {
                let room_id = room.room_id().to_string();
                let room_name = room.name().unwrap_or_else(|| room_id.clone());
                let _ = tx.send(MatrixEvent::RoomJoined { room_id, room_name });
            }

            inner.add_event_handler({
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
            if let Err(e) = inner.sync(settings).await {
                let _ = tx.send(MatrixEvent::SyncError(e.to_string()));
            }
            let _ = tx.send(MatrixEvent::Disconnected);
        });
    }
}
