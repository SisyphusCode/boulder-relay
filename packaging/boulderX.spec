Name:           boulderX
Version:        0.6.1
Release:        1%{?dist}
Summary:        GTK4 + libadwaita IRC/Matrix client in Rust — Element X-style UI

License:        GPL-2.0-or-later
URL:            https://github.com/SisyphusAeolides/boulderX
Source0:        boulderX-%{version}.tar.gz

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
boulderX is a GTK4 + libadwaita IRC/Matrix client written in Rust
using relm4. Features an Element X-inspired UI with a unified sidebar,
bubble chat view, per-nickname coloring, multi-server IRC (SASL, NickServ),
Matrix login (password/SSO, E2E encryption), channel discovery, log viewer,
desktop notifications, and persistent TOML settings.

%prep
%autosetup -n boulderX-%{version}

%build
cargo build --release --offline

%install
install -Dm755 target/release/boulderX                %{buildroot}%{_bindir}/boulderX
install -Dm644 packaging/boulderX.desktop              %{buildroot}%{_datadir}/applications/boulderX.desktop
install -Dm644 assets/boulder-relay-128.png            %{buildroot}%{_datadir}/icons/hicolor/128x128/apps/boulderX.png
install -Dm644 assets/boulder-relay-256.png            %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/boulderX.png
install -Dm644 packaging/org.Sisyphus.BoulderX.metainfo.xml \
    %{buildroot}%{_metainfodir}/org.Sisyphus.BoulderX.metainfo.xml

%check
desktop-file-validate packaging/boulderX.desktop
appstream-util validate-relax --nonet packaging/org.Sisyphus.BoulderX.metainfo.xml

%files
%license LICENSE
%doc README.md
%{_bindir}/boulderX
%{_datadir}/applications/boulderX.desktop
%{_datadir}/icons/hicolor/128x128/apps/boulderX.png
%{_datadir}/icons/hicolor/256x256/apps/boulderX.png
%{_metainfodir}/org.Sisyphus.BoulderX.metainfo.xml

%changelog
* Tue Jul 14 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.6.1-1
- IRC login dialog and Accounts manager for IRC + Matrix
- Persist Matrix credentials optionally; Register/Verify NickServ flows
- Fix Join Matrix Room sidebar action

* Tue Jul 14 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.6.0-2
- Restore full ApplicationWindow UI (sidebar, chat, composer, header)
- Fix blank window caused by skeletal AppModel view rewrite
- Drop invalid GTK CSS text-align property

* Tue Jul 14 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.6.0-1
- Rename project from boulder-relay to boulderX
- Add Matrix protocol support (matrix-sdk 0.10, E2E, SQLite store)
- Element X-inspired UI: unified sidebar, room avatars, protocol badges, bubble chat view
- Refactor: extract irc/, matrix/, ui/ modules
- Unified AppModel with Protocol enum, RoomRegistry, MatrixClient
- Matrix login + room join dialogs
- Full Gruvbox dark + Sisyphus Blue Element X CSS theme
- Convert all packaging to Fedora/RPM/COPR
- Add sqlite3 build/runtime dependency

* Mon Jul 06 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.4.0-1
- Update version to 0.4.0 to match Cargo.toml

* Sun Jul 05 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.3.0-1
- Switched default icon to Sisyphus logo
- Added per-nickname coloring (Gruvbox palette)
- General purpose IRC client

* Tue Jun 23 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.1.0-8
- Initial packaging as boulder-relay (now boulderX)
