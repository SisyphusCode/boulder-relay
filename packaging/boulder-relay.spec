Name:           boulder-relay
Version:        0.6.0
Release:        1%{?dist}
Summary:        GTK4 + libadwaita IRC/Matrix client in Rust — Element X-style UI

License:        GPL-2.0-or-later
URL:            https://github.com/SisyphusAeolides/boulder-relay
Source0:        boulder-relay-%{version}.tar.gz

# Rust binary — no C debugsource
%global debug_package %{nil}

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(libadwaita-1)
BuildRequires:  pkgconfig(openssl)
BuildRequires:  pkgconfig(sqlite3)
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib

Requires:       gtk4
Requires:       libadwaita
Requires:       openssl-libs
Requires:       sqlite-libs

%description
Boulder Relay is a GTK4 + libadwaita IRC/Matrix client written in Rust
using relm4. Features an Element X-inspired UI with a unified sidebar,
bubble chat view, per-nickname coloring, multi-server IRC (SASL, NickServ),
Matrix login (password/SSO, E2E encryption), channel discovery, log viewer,
desktop notifications, and persistent TOML settings.

%prep
%autosetup -n boulder-relay-%{version}

%build
cargo build --release --offline

%install
install -Dm755 target/release/boulder-relay          %{buildroot}%{_bindir}/boulder-relay
install -Dm644 packaging/boulder-relay.desktop        %{buildroot}%{_datadir}/applications/boulder-relay.desktop
install -Dm644 assets/boulder-relay-128.png           %{buildroot}%{_datadir}/icons/hicolor/128x128/apps/boulder-relay.png
install -Dm644 assets/boulder-relay-256.png           %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/boulder-relay.png
install -Dm644 packaging/org.Sisyphus.BoulderRelay.metainfo.xml \
    %{buildroot}%{_metainfodir}/org.Sisyphus.BoulderRelay.metainfo.xml

%check
desktop-file-validate packaging/boulder-relay.desktop
appstream-util validate-relax --nonet packaging/org.Sisyphus.BoulderRelay.metainfo.xml

%files
%license LICENSE
%doc README.md
%{_bindir}/boulder-relay
%{_datadir}/applications/boulder-relay.desktop
%{_datadir}/icons/hicolor/128x128/apps/boulder-relay.png
%{_datadir}/icons/hicolor/256x256/apps/boulder-relay.png
%{_metainfodir}/org.Sisyphus.BoulderRelay.metainfo.xml

%changelog
* Tue Jul 14 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.6.0-1
- Add Matrix protocol support (matrix-sdk 0.10, E2E, SQLite store)
- Element X-inspired UI: unified sidebar, room avatars, protocol badges, bubble chat view
- Refactor: extract irc/ and matrix/ modules from monolithic main.rs
- New ui/ module: sidebar, chat_view, composer, dialogs
- Unified AppModel with Protocol enum (IRC/Matrix), RoomRegistry, MatrixClient
- Matrix login dialog, Matrix room join dialog
- Full Gruvbox dark + Sisyphus Blue Element X CSS theme
- Bump version to 0.6.0
- Add sqlite3 build/runtime dependency for matrix-sdk SQLite store

* Mon Jul 14 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.5.0-1
- Convert from Arch-based to Fedora/RPM packaging
- Remove PKGBUILD; all packaging now RPM/COPR only
- Fix Copr build: update spec files and dependencies

* Mon Jul 06 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.4.0-1
- Update version to 0.4.0 to match Cargo.toml
- Fix COPR build: Makefile now extracts version from Cargo.toml dynamically

* Sun Jul 05 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.3.0-1
- Switched default icon to Sisyphus logo (PNG at multiple sizes for hicolor)
- Added per-nickname coloring in chat messages (Gruvbox palette)
- Channel topics now displayed under active context header (from RPL_TOPIC)
- General purpose IRC client: usable on any network
- Add basic /me (CTCP ACTION) support

* Fri Jun 26 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.6-1
- Fix RHEL default channel: use #rhel instead of nonexistent #rhel-devel

* Fri Jun 26 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.5-1
- Set window/taskbar icon and fix StartupWMClass for desktop integration
- Add background mode on close with desktop notifications
- Add Quit button and notification/background preferences in settings

* Fri Jun 26 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.4-1
- Persist user-added channels between sessions
- Improve join box and /join parsing for arbitrary channels
- Add Fedora 44 COPR build target

* Fri Jun 26 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.3-1
- Add #fedora, #fedora-devel default channels
- Broaden branding for Fedora, RHEL, and Rocky Linux

* Wed Jun 24 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.2-1
- Fix white GNOME title bar: load CSS after GTK init

* Wed Jun 24 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.1-1
- Fix white GNOME title bar with custom dark HeaderBar
- Add nick highlights, /clear, /part, /help, channel leave button

* Wed Jun 24 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.0-1
- Add persistent settings, disconnect control, timestamps, slash commands

* Tue Jun 23 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.1.0-9
- Fix channel joins to wait for NickServ login on +r channels

* Tue Jun 23 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.1.0-8
- Rename project from rawhide-relay to boulder-relay
