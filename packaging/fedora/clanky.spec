Name:           clanky
Version:        0.3.3
Release:        1%{?dist}
Summary:        GPU-rendered terminal for coding agents
License:        MIT OR Apache-2.0
URL:            https://github.com/rustytensor/clanky
Source0:        %{name}-%{version}.tar.gz
BuildRequires:  cargo
BuildRequires:  rust
Provides:       bundled(crate(clanky-vt)) = 0.26.0
Provides:       bundled(crate(clanky-vte)) = 0.15.0

%description
Clanky is a GPU-rendered terminal with tabs, split panes, and a local socket
API for coding agents.

%prep
%setup -q

%build
export CARGO_NET_OFFLINE=true
export CARGO_BUILD_JOBS=8
cargo build --release --locked --offline -p clanky -j 8

%install
install -Dm755 "${CARGO_TARGET_DIR:-target}/release/clanky" %{buildroot}%{_bindir}/clanky
install -Dm644 packaging/linux/clanky.desktop %{buildroot}%{_datadir}/applications/clanky.desktop
install -Dm644 packaging/linux/io.github.rustytensor.Clanky.metainfo.xml %{buildroot}%{_metainfodir}/io.github.rustytensor.Clanky.metainfo.xml
install -Dm644 packaging/linux/io.github.rustytensor.Clanky.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/io.github.rustytensor.Clanky.svg
install -Dm644 packaging/linux/clanky.1 %{buildroot}%{_mandir}/man1/clanky.1

%files
%license LICENSE-MIT LICENSE-APACHE
%{_bindir}/clanky
%{_datadir}/applications/clanky.desktop
%{_metainfodir}/io.github.rustytensor.Clanky.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/io.github.rustytensor.Clanky.svg
%{_mandir}/man1/clanky.1*
