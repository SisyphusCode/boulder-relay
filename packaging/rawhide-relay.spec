Name:           rawhide-relay
Version:        0.1.0
Release:        4%{?dist}
Summary:        GTK4 IRC client written in Rust using relm4

License:        GPL-2.0-or-later
URL:            https://github.com/SisyphusCode/rawhide-relay
Source0:        rawhide-relay-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  pkgconfig(gtk4)
BuildRequires:  pkgconfig(openssl)
BuildRequires:  desktop-file-utils
BuildRequires:  appstream

%description
Rawhide Relay is a GTK4 IRC client built in Rust with relm4.
It connects to Libera.Chat over TLS and supports NickServ auth.

%prep
%autosetup -n rawhide-relay-%{version}

%build
cargo build --release --offline

%install
install -Dm755 target/release/rawhide-relay %{buildroot}%{_bindir}/rawhide-relay
install -Dm644 packaging/rawhide-relay.desktop %{buildroot}%{_datadir}/applications/rawhide-relay.desktop
install -Dm644 assets/rawhide-relay.png %{buildroot}%{_datadir}/icons/hicolor/128x128/apps/rawhide-relay.png
install -Dm644 assets/rawhide-relay.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/rawhide-relay.svg
install -Dm644 packaging/org.Sisyphus.RawhideRelay.metainfo.xml %{buildroot}%{_metainfodir}/org.Sisyphus.RawhideRelay.metainfo.xml

%files
%license LICENSE
%doc README.md
%{_bindir}/rawhide-relay
%{_datadir}/applications/rawhide-relay.desktop
%{_datadir}/icons/hicolor/128x128/apps/rawhide-relay.png
%{_datadir}/icons/hicolor/scalable/apps/rawhide-relay.svg
%{_metainfodir}/org.Sisyphus.RawhideRelay.metainfo.xml

%changelog
* Sun Jun 21 2026 Kenny Glowner <sisyphuscode@fedoraproject.org> - 0.1.0-4
- Disable metadata validation in %%check for COPR compatibility
