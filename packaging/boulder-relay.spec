Name:           boulder-relay
Version:        0.5.0
Release:        1%{?dist}
Summary:        GTK4 IRC client for any IRC network (general purpose)

License:        GPL-2.0-or-later
URL:            https://github.com/SisyphusAeolides/boulder-relay
Source0:        boulder-relay-%{version}.tar.gz

# Rust binary; no C debugsource to package.
%global debug_package %{nil}

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(libadwaita-1)
BuildRequires:  pkgconfig(openssl)
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib

Requires:       gtk4
Requires:       libadwaita
Requires:       openssl-libs

%description
Boulder Relay is a general-purpose GTK4 IRC client built in Rust with relm4.
It works with any IRC server/network. No baked-in default channels or
distro branding. Clean interface with multi-channel support, favorites,
user lists, timestamps, slash commands, and persistent settings.

%prep
%autosetup -n boulder-relay-%{version}

%build
cargo build --release --offline

%install
install -Dm755 target/release/boulder-relay %{buildroot}%{_bindir}/boulder-relay
install -Dm644 packaging/boulder-relay.desktop %{buildroot}%{_datadir}/applications/boulder-relay.desktop
install -Dm644 assets/boulder-relay-128.png %{buildroot}%{_datadir}/icons/hicolor/128x128/apps/boulder-relay.png
install -Dm644 assets/boulder-relay-256.png %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/boulder-relay.png
install -Dm644 packaging/org.Sisyphus.BoulderRelay.metainfo.xml %{buildroot}%{_metainfodir}/org.Sisyphus.BoulderRelay.metainfo.xml

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
* Mon Jul 06 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.4.0-1
- Update version to 0.4.0 to match Cargo.toml
- Fix COPR build: Makefile now extracts version from Cargo.toml dynamically
- Fix version mismatch between Cargo.toml (0.4.0) and spec file (0.3.0)

* Sun Jul 05 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.3.0-1
- Switched default icon to Sisyphus logo (PNG at multiple sizes for hicolor).
- Added per-nickname coloring in chat messages (Gruvbox palette).
- Channel topics now displayed under active context header (from RPL_TOPIC).
- Proper multi-size icon installation (128/256 + SVG).
- Remove all hardcoded default channels and distro-specific branding (Fedora/RHEL/Rocky).
- General purpose IRC client: usable on any network, any channels (Gentoo, Sisyphus, etc.).
- Add basic /me (CTCP ACTION) support for actions.
- Updated packaging, README, metainfo, desktop for generic use.
- Added Gentoo ebuild skeleton (net-irc/boulder-relay-9999).
- Relaxed old EL9 pin notes; better for modern systems like Gentoo + Catalyst.

* Fri Jun 26 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.6-1
- Fix RHEL default channel: use #rhel instead of nonexistent #rhel-devel

* Fri Jun 26 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.5-1
- Set window/taskbar icon and fix StartupWMClass for desktop integration
- Add background mode on close with desktop notifications for IRC messages
- Add Quit button and notification/background preferences in settings

* Fri Jun 26 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.4-1
- Persist user-added channels between sessions
- Improve join box and /join parsing for arbitrary channels
- Add Fedora 44 COPR build target

* Fri Jun 26 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.3-1
- Add #fedora, #fedora-devel, and #rhel-devel default channels
- Group sidebar by community with tooltips and color accents
- Broaden branding for Fedora, RHEL, and Rocky Linux

* Wed Jun 24 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.2-1
- Fix white GNOME title bar: load CSS after GTK init and use custom WindowControls
- Override Adwaita default-decoration header styling with higher-priority Gruvbox CSS

* Wed Jun 24 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.1-1
- Fix white GNOME title bar with custom dark HeaderBar and scoped Gruvbox CSS
- Add nick highlights, /clear, /part, /help, channel leave button, last channel restore

* Wed Jun 24 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.2.0-1
- Add persistent settings, disconnect control, timestamps, and slash commands
- Improve chat view with auto-scrolling TextView
- Regenerate vendored crates and verify EL10 / EPEL 10 compatibility

* Tue Jun 23 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.1.0-9
- Fix channel joins to wait for NickServ login on +r channels

* Tue Jun 23 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.1.0-8
- Rename project from rawhide-relay to boulder-relay

* Tue Jun 23 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.1.0-7
- Retarget default channels to Rocky Linux Libera community
- Pin relm4/gtk4 0.8 for Rocky Linux 9 (GLib 2.68) compatibility
- Regenerate vendored crates, improve connection error handling
- Add Rocky Linux build script and runtime dependencies

* Sun Jun 21 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.1.0-6
- Add side-by-side UI buttons for user DM and mute toggles

* Sun Jun 21 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.1.0-5
- Swap appstream for libappstream-glib to fix validation
- Restore %%check phase for Copr builds
