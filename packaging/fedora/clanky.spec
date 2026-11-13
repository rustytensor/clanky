# Rust sources may begin with #![attribute] lines; those are not shebangs.
%global __brp_mangle_shebangs_exclude_from ^%{_prefix}/src/debug/

Name:           clanky
Version:        1.2.2
Release:        1%{?dist}
Summary:        GPU-rendered terminal for coding agents
# Clanky is MIT OR Apache-2.0; its bundled fonts are OFL-1.1 (Geist, Geist Mono, JetBrains Mono, Inter) and Bitstream-Vera (DejaVu).
License:        (MIT OR Apache-2.0) AND OFL-1.1 AND Bitstream-Vera
URL:            https://github.com/rustytensor/clanky
Source0:        https://github.com/rustytensor/clanky/archive/v%{version}/clanky-%{version}.tar.gz
BuildRequires:  cargo
BuildRequires:  rust
# Saved history links the system SQLite (rusqlite without its bundled copy).
BuildRequires:  sqlite-devel
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
cargo build --release --locked --offline -p clanky --bin clanky -j 8

%check
"${CARGO_TARGET_DIR:-target}/release/clanky" --help > /dev/null

%install
install -d -m 0755 %{buildroot}%{_prefix} %{buildroot}%{_bindir} \
    %{buildroot}%{_datadir} %{buildroot}%{_datadir}/applications \
    %{buildroot}%{_datadir}/icons %{buildroot}%{_datadir}/icons/hicolor \
    %{buildroot}%{_datadir}/icons/hicolor/scalable %{buildroot}%{_datadir}/icons/hicolor/scalable/apps \
    %{buildroot}%{_metainfodir} %{buildroot}%{_mandir} %{buildroot}%{_mandir}/man1 \
    %{buildroot}%{_licensedir} %{buildroot}%{_licensedir}/%{name}
install -Dm755 "${CARGO_TARGET_DIR:-target}/release/clanky" %{buildroot}%{_bindir}/clanky
install -Dm644 packaging/linux/clanky.desktop %{buildroot}%{_datadir}/applications/clanky.desktop
install -Dm644 packaging/linux/io.github.rustytensor.Clanky.metainfo.xml %{buildroot}%{_metainfodir}/io.github.rustytensor.Clanky.metainfo.xml
install -Dm644 packaging/linux/io.github.rustytensor.Clanky.svg %{buildroot}%{_datadir}/icons/hicolor/scalable/apps/io.github.rustytensor.Clanky.svg
install -Dm644 packaging/linux/clanky.1 %{buildroot}%{_mandir}/man1/clanky.1
# A setgid build directory would otherwise pass its setgid bit to every directory here;
# only a five-digit mode clears it (GNU chmod keeps it for 0755).
find %{buildroot} -type d -exec chmod 00755 {} +

%files
%license LICENSE-MIT LICENSE-APACHE NOTICE
%license assets/fonts/OFL-JetBrainsMono.txt assets/fonts/OFL-Inter.txt assets/fonts/LICENSE-DejaVu.txt
%license assets/fonts/OFL-Geist.txt
%license assets/themes/upstream/LICENSE-Catppuccin
%{_bindir}/clanky
%{_datadir}/applications/clanky.desktop
%{_metainfodir}/io.github.rustytensor.Clanky.metainfo.xml
%{_datadir}/icons/hicolor/scalable/apps/io.github.rustytensor.Clanky.svg
%{_mandir}/man1/clanky.1*

%changelog
* Thu Nov 12 2026 Clanky contributors <noreply@github.com> - 1.2.2-1
- File opens that must not follow symlinks use the system flag names, so the guard also holds on ARM64.

* Thu Nov 12 2026 Clanky contributors <noreply@github.com> - 1.2.1-1
- Project rules brought in line with the maintainer's global rules; no change to the program.

* Thu Nov 12 2026 Clanky contributors <noreply@github.com> - 1.2.0-1
- Darker dark themes with richer terminal colours, a new near-black Ink theme, readable joke themes.
- Packages for Debian 13, Ubuntu 26.04 and Arch.

* Tue Nov 10 2026 Clanky contributors <noreply@github.com> - 1.1.0-1
- Saved command history in SQLite, with clanky history to list, search, export and delete it.

* Thu Nov 05 2026 Clanky contributors <noreply@github.com> - 1.0.0-1
- First stable release: the calm look, 13 themes, Settings, Post-AI mode and faster rendering.

* Thu Nov 05 2026 Clanky contributors <noreply@github.com> - 1.0.0~rc.1-1
- Calm look: Stone, Paper, Graphite, Catppuccin and three joke themes; Geist fonts; a new Settings panel.
- Faster typing and scrolling: the window fetches only the rows that changed.
- Post-AI mode, a start-up robot, and system-font fallback for icon glyphs.

* Wed Nov 04 2026 Clanky contributors <noreply@github.com> - 0.6.1-1
- Each theme has its own terminal colours; colours a program sets exactly are kept.

* Wed Nov 04 2026 Clanky contributors <noreply@github.com> - 0.6.0-1
- Studio look: rounded pane cards, side rails, command cards, glows and motion.
- Dark, light and follow-the-system themes; colour overrides and font size.
- Settings panel with live reload, export and import.
- Copy, search and clipboard use masked text.

* Wed Nov 04 2026 Clanky contributors <noreply@github.com> - 0.5.5-1
- Start on systems whose runtime directory allows group search.

* Wed Nov 04 2026 Clanky contributors <noreply@github.com> - 0.5.4-1
- Document every merged feature, the agent API and masking limits.

* Wed Nov 04 2026 Clanky contributors <noreply@github.com> - 0.5.3-1
- Send compose drafts larger than 64 KiB in pieces.

* Wed Nov 04 2026 Clanky contributors <noreply@github.com> - 0.5.2-1
- Add the text editor, session restore and command-finished notifications.

* Wed Nov 04 2026 Clanky contributors <noreply@github.com> - 0.5.1-1
- Keep package versions in step with Cargo and build offline source RPMs.

* Tue Nov 03 2026 Clanky contributors <noreply@github.com> - 0.5.0-1
- Package the current source version and build only the application binary.

* Tue Nov 03 2026 Clanky contributors <noreply@github.com> - 0.3.3-1
- Add Linux distribution packaging.
