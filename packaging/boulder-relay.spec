Name:           boulder-relay
Version:        0.2.2
Release:        1%{?dist}
Summary:        GTK4 IRC client for Rocky Linux community channels on Libera.Chat

License:        GPL-2.0-or-later
URL:            https://github.com/SisyphusCode/boulder-relay
Source0:        boulder-relay-%{version}.tar.gz

# Rust binary; no C debugsource to package.
%global debug_package %{nil}

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(openssl)
BuildRequires:  desktop-file-utils
BuildRequires:  libappstream-glib

Requires:       gtk4
Requires:       openssl-libs

%description
Boulder Relay is a GTK4 IRC client built in Rust with relm4, oriented
toward Rocky Linux development and community chat on Libera.Chat. It
connects over TLS, supports NickServ authentication, persistent settings,
slash commands, and ships with #rockylinux, #rockylinux-devel, and
#rockylinux-social as default channels.

%prep
%autosetup -n boulder-relay-%{version}

%build
cargo build --release --offline

%install
install -Dm755 target/release/boulder-relay %{buildroot}%{_bindir}/boulder-relay
install -Dm644 packaging/boulder-relay.desktop %{buildroot}%{_datadir}/applications/boulder-relay.desktop
install -Dm644 assets/boulder-relay.png %{buildroot}%{_datadir}/icons/hicolor/128x128/apps/boulder-relay.png
install -Dm644 assets/boulder-relay.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/boulder-relay.svg
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
%{_datadir}/icons/hicolor/scalable/apps/boulder-relay.svg
%{_metainfodir}/org.Sisyphus.BoulderRelay.metainfo.xml

%changelog
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
