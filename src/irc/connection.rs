use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use futures::prelude::*;
use irc::client::prelude::*;
use crate::channels;
use crate::app::{AppInput, DEFAULT_PORT};
use relm4::ComponentSender;
use crate::app::AppModel;

pub struct IrcConnection;

impl IrcConnection {
    pub fn spawn(
        sender: ComponentSender<AppModel>,
        nickname: String,
        server_addr: String,
        pwd: String,
        auth_method: String,
        channels_to_join: Vec<String>,
        port: u16,
        use_tls: bool,
    ) {
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().expect("Tokio runtime");
            rt.block_on(async {
                let is_sasl_plain = auth_method == "sasl_plain";
                let is_sasl_external = auth_method == "sasl_external";
                let is_sasl = is_sasl_plain || is_sasl_external;
                let needs_nickserv = !pwd.is_empty() && !is_sasl;
                let port = if port == 0 { DEFAULT_PORT } else { port };
                let config = Config {
                    nickname: Some(nickname.clone()),
                    server: Some(server_addr),
                    channels: vec![],
                    port: Some(port),
                    use_tls: Some(use_tls),
                    nick_password: if needs_nickserv { Some(pwd.clone()) } else { None },
                    ..Config::default()
                };
                let mut client = match Client::from_config(config).await {
                    Ok(c) => c,
                    Err(e) => {
                        sender.input(AppInput::NetworkStatus(format!("Connection failed: {e}")));
                        return;
                    }
                };
                if is_sasl {
                    let _ = client.send_cap_req(&[irc::proto::caps::Capability::Sasl]);
                } else if let Err(e) = client.identify() {
                    // Prefix must match Offline handling in AppInput::NetworkStatus.
                    sender.input(AppInput::NetworkStatus(format!(
                        "Connection failed: identify error: {e}"
                    )));
                    return;
                }
                let irc_tx = client.sender();
                sender.input(AppInput::NetworkConnected(irc_tx.clone()));
                let join_channels = |tx: &irc::client::Sender| {
                    for chan in &channels_to_join {
                        let _ = tx.send_join(chan);
                    }
                };
                let mut channels_joined = false;
                let mut stream = match client.stream() {
                    Ok(s) => s,
                    Err(e) => {
                        sender.input(AppInput::NetworkStatus(format!(
                            "Connection failed: stream error: {e}"
                        )));
                        return;
                    }
                };
                while let Some(result) = stream.next().await {
                    let message = match result {
                        Ok(m) => m,
                        Err(e) => {
                            sender.input(AppInput::ReceiveServerMessage(format!("[Error]: {e}")));
                            continue;
                        }
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
                            sender.input(AppInput::ReceiveMessage {
                                channel: display_target,
                                user: display_user,
                                body: display_body,
                                protocol: crate::app::Protocol::Irc,
                            });
                        }
                        Command::JOIN(channel, _, _) => {
                            sender.input(AppInput::UserJoined { channel: channel.clone(), user: user.clone() });
                            sender.input(AppInput::ReceiveMessage {
                                channel,
                                user: "System".to_string(),
                                body: format!("{} joined.", user),
                                protocol: crate::app::Protocol::Irc,
                            });
                        }
                        Command::PART(channel, _) => {
                            sender.input(AppInput::UserLeft { channel: channel.clone(), user: user.clone() });
                            sender.input(AppInput::ReceiveMessage {
                                channel,
                                user: "System".to_string(),
                                body: format!("{} left.", user),
                                protocol: crate::app::Protocol::Irc,
                            });
                        }
                        Command::NICK(new_nick) => {
                            sender.input(AppInput::UserRenamed { old: user, new: new_nick });
                        }
                        Command::QUIT(_) => {
                            sender.input(AppInput::UserQuit { user });
                        }
                        Command::NOTICE(_, body) => {
                            sender.input(AppInput::ReceiveServerMessage(format!("[Notice]: {body}")));
                            // NickServ wording varies by network — match common success phrases.
                            let identified = body.contains("You are now identified")
                                || body.contains("Password accepted")
                                || body.contains("you are now recognized")
                                || body.to_ascii_lowercase().contains("successfully identified");
                            if needs_nickserv && !channels_joined && identified {
                                channels_joined = true;
                                join_channels(&irc_tx);
                            }
                        }
                        Command::TOPIC(channel, Some(topic)) => {
                            sender.input(AppInput::ChannelTopic { channel, topic });
                        }
                        Command::CAP(_, sub, _, params) => {
                            if is_sasl && sub == irc::proto::CapSubCommand::ACK {
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
                        Command::AUTHENTICATE(data) => {
                            if is_sasl_plain && data == "+" {
                                let auth = format!("\0{}\0{}", nickname, pwd);
                                let encoded = BASE64.encode(auth.as_bytes());
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
                                    // Join after MOTD so networks with unusual NickServ wording
                                    // still get saved channels (may re-join after identify).
                                    Response::RPL_ENDOFMOTD | Response::ERR_NOMOTD => {
                                        if !channels_joined {
                                            channels_joined = true;
                                            join_channels(&irc_tx);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            if code == Response::RPL_NAMREPLY && args.len() >= 4 {
                                let channel = args.iter().find(|a| channels::is_channel_target(a)).cloned().unwrap_or_else(|| args[2].clone());
                                let users: Vec<String> = args.last().unwrap_or(&String::new()).split_whitespace().map(|s| s.to_string()).collect();
                                sender.input(AppInput::BatchAddUsers { channel, users });
                            } else if code == Response::RPL_LIST && args.len() >= 3 {
                                let name = args.get(1).cloned().unwrap_or_default();
                                let users: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                                let topic = if args.len() > 3 { args[3..].join(" ") } else { String::new() };
                                sender.input(AppInput::ChannelListEntry { name, users, topic });
                            } else if code == Response::RPL_LISTEND {
                                sender.input(AppInput::ChannelListEnd);
                            } else if code == Response::RPL_TOPIC && args.len() >= 2 {
                                let ch = args.get(1).cloned().unwrap_or_default();
                                let topic = if args.len() > 2 { args[2..].join(" ") } else { String::new() };
                                sender.input(AppInput::ChannelTopic { channel: ch, topic });
                            } else if args.len() > 1 {
                                sender.input(AppInput::ReceiveServerMessage(format!("[{code:?}]: {}", args[1..].join(" "))));
                            }
                        }
                        _ => {}
                    }
                }
                sender.input(AppInput::NetworkStatus(String::from("Disconnected")));
                sender.input(AppInput::ReceiveServerMessage(String::from("[System]: Connection closed.")));
            });
        });
    }
}
