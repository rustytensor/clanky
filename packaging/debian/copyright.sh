#!/bin/sh
# Print /usr/share/doc/clanky/copyright in Debian's machine-readable format (DEP-5).
# Run from the top of the source tree. The licences match the RPM spec's License field.
set -eu
text() { sed -e 's/[[:space:]]*$//' -e 's/^$/./' -e 's/^/ /' "$1"; }
cat <<'HEAD'
Format: https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/
Upstream-Name: clanky
Upstream-Contact: https://github.com/rustytensor/clanky/issues
Source: https://github.com/rustytensor/clanky

Files: *
Copyright: 2026 Marc Mironescu
License: GPL-3.0+
Comment: The binary statically links Rust crates under MIT, Apache-2.0 and similar licences;
 see NOTICE in the source.

Files: crates/clanky-vt/*
Copyright: Christian Duerr, Joe Wilm and the Alacritty contributors
           2026 Marc Mironescu
License: Apache-2.0
Comment: Forked from alacritty_terminal 0.26.0.

Files: crates/clanky-vte/*
Copyright: Christian Duerr, Joe Wilm and the Alacritty contributors
           2026 Marc Mironescu
License: MIT or Apache-2.0
Comment: Forked from vte 0.15.0.

Files: assets/themes/upstream/*
Copyright: 2021 Catppuccin
License: MIT

Files: assets/fonts/Geist-* assets/fonts/GeistMono-*
Copyright: 2026 The Geist Project Authors (https://github.com/vercel/geist-font)
License: OFL-1.1

Files: assets/fonts/JetBrainsMono-*
Copyright: 2020 The JetBrains Mono Project Authors (https://github.com/JetBrains/JetBrainsMono)
License: OFL-1.1

Files: assets/fonts/Inter-*
Copyright: 2016 The Inter Project Authors (https://github.com/rsms/inter)
License: OFL-1.1

Files: assets/fonts/DejaVuSansMono.ttf
Copyright: 2003 Bitstream, Inc.
           2006 Tavmjong Bah
           DejaVu changes are in the public domain
License: Bitstream-Vera

License: MIT
HEAD
text LICENSE-MIT | sed -n '/^ Permission/,$p'
cat <<'MID'

License: Apache-2.0
 On Debian systems, the full text of the Apache License, Version 2.0 is in
 /usr/share/common-licenses/Apache-2.0.

License: GPL-3.0+
 This program is free software: you can redistribute it and/or modify it under
 the terms of the GNU General Public License as published by the Free Software
 Foundation, either version 3 of the License, or (at your option) any later version.
 .
 On Debian systems, the full text of the GNU General Public License version 3 is in
 /usr/share/common-licenses/GPL-3.

License: OFL-1.1
MID
text assets/fonts/OFL-Inter.txt
printf '\nLicense: Bitstream-Vera\n'
text assets/fonts/LICENSE-DejaVu.txt
