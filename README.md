# boulderX

GTK4 + libadwaita IRC, Matrix, and Discord bot client in Rust (relm4).

**v0.6.2** — IRC, Matrix, and Discord bot chat with protocol tabs, a modern three-pane layout, slash commands, and Sisyphus Blue accents.

## Status (honest)

| Path | Works today |
|------|-------------|
| IRC connect (TLS 6697 default) | Yes — configurable port/TLS in settings |
| IRC join / send / receive | Yes |
| Slash commands | Yes — `/join /part /msg /me /nick /whois /away /back /topic /ignore /unignore /clear /list /help`; unknown commands show an error |
| Settings persist | Yes — `~/.config/boulder-relay/settings.toml` (mode `0600`) |
| Matrix login / sync / send / join | Yes — uses a process-wide Tokio runtime (no bare spawn from GTK) |
| Matrix store | Yes — `~/.local/share/boulderX/matrix` (XDG data, not CWD) |
| Matrix unread badges | Yes — room registry unread counters |
| Matrix leave | Best-effort server leave + local remove |
| Protocol tabs | Yes — All / IRC / Matrix / Discord room filters |
| Discord bot | Guild text/news channels, visible threads, DMs, send/receive, attachments, embeds, stickers, reply context, channel cleanup, and clean disconnect |
| Multi-server concurrent IRC | **Deferred** — maps exist; no full UI switcher yet |
| SASL EXTERNAL (client cert) | **Deferred** — PLAIN/NickServ work; EXTERNAL needs cert wiring |
| SSO Matrix login UI | **Deferred** — password login only |

Not a full Element/HexChat/Discord client clone. Discord support uses authorized bot
accounts only; user tokens and selfbots are not supported.

## Quick start

```bash
# Arch Linux dependencies
sudo pacman -S --needed base-devel rust cargo gtk4 libadwaita openssl sqlite

cargo build --release
./target/release/boulderX
# or: cargo run --release
```

1. **Accounts / IRC** — nick, optional NickServ/SASL password, server.
2. **Connect** — TLS on port 6697 by default (`irc_port` / `irc_use_tls` in settings.toml).
3. Join with the join box or `/join #channel`.
4. **MX** — homeserver + user/password for Matrix.
5. **Discord** — connect an authorized bot token from the Accounts dialog. Enable
   the Message Content intent for the bot in the Discord Developer Portal. User
   tokens and selfbots are not supported.

## Build / test

```bash
cargo test                 # pure unit tests (commands, channels, config, rooms)
cargo build --release
cargo build --release --offline   # vendored tree
```

## Module layout

```
src/
  main.rs          — GTK entry; installs shared Tokio runtime
  runtime.rs       — multi-thread runtime for Matrix from the UI thread
  app.rs           — AppModel + input handlers
  irc/
    connection.rs  — IRC thread + event loop (port/TLS aware)
    commands.rs    — slash command parser (unit-tested)
  matrix/
    client.rs      — login, send, sync, leave; XDG store path
    rooms.rs       — RoomRegistry + unread
    sync.rs        — MatrixEvent → AppInput bridge
  config.rs        — TOML load/save
  channels.rs      — channel/DM join helpers
  discord/         — bot gateway, channel/DM discovery, message bridge
  ui/              — sidebar, chat view, dialogs, composer helpers
```

## Packaging

- Arch PKGBUILD: https://github.com/SisyphusAeolides/arch-pkgbuilds/tree/main/boulderx
- Desktop + AppStream under `packaging/`
- Icons in `assets/`

## License

GPL-2.0-or-later
