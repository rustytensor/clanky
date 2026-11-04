# M9: text-mode editor

`clanky edit FILE[:LINE[:COL]] [--review FILE]` runs without a display, using
`clanky-edit` for UTF-8 text, selections, undo/redo, disk conflict checks and
hunk decisions. Only command-line dispatch changes in the existing binary; the
window, server and socket API behavior stay unchanged. An absent numeric suffix
means line 1, column 1; numeric suffixes follow the core's one-based scalar
position rules. The editor opens existing UTF-8 files only.

The optional review file is UTF-8 JSON: `{"hunks":[{"start":0,"end":5,
"replacement":"ALPHA"}]}`. Ranges are half-open UTF-8 byte offsets into the
opened document, validated by `Document::propose`; invalid files are errors,
not silently skipped. Hunks start pending. Display old and proposed text inline,
including stale state. Review controls select a hunk, accept/reject one or all;
all acts only on pending hunks. Accept edits the document in memory, reject does
not; save remains explicit. Disk changes and overlapping edits use the core's
conflict/stale rules. Do not overwrite external changes.

Use ratatui with crossterm. Ordinary arrows move; Shift+arrows select; typing
replaces selection; Ctrl+Z/Y undo/redo, Ctrl+S saves, Ctrl+F finds, Ctrl+Q asks
before discarding unsaved edits, and Ctrl+C exits safely. F5 accepts the next
pending hunk and F6 rejects the next; F7/F8 accept/reject all pending. Document
all bindings in the UI. The terminal guard restores raw mode, alternate screen
and cursor on normal exit, Ctrl+C and unwind. There are no plugins, language
servers, file tree or syntax highlighting.

## Dependencies and licence

The proposed pinned direct crates are `ratatui 0.29.0` (MIT, 543514-byte
crate) and `crossterm 0.28.1` (MIT, 132275-byte crate). A fresh metadata
resolution against v0.4.5 identifies the following additional crate archives
not already in Cargo.lock (compressed bytes, upstream declared licences; a
permitted MIT or Apache alternative is chosen where offered). Platform-specific
archives are included, although the build is Linux-first:

| Crate | Licence | Bytes |
| --- | --- | ---: |
| cassowary 0.3.0 | MIT OR Apache-2.0 | 22876 |
| castaway 0.2.4 | MIT | 12546 |
| compact_str 0.8.2 | MIT | 88384 |
| crossterm_winapi 0.9.1 | MIT | 16027 |
| darling 0.24.1 | MIT | 51607 |
| darling_core 0.24.1 | MIT | 83947 |
| darling_macro 0.24.1 | MIT | 2481 |
| either 1.19.0 | MIT OR Apache-2.0 | 22501 |
| foldhash 0.1.5 | Zlib | 21901 |
| hashbrown 0.15.5 | MIT OR Apache-2.0 | 140908 |
| heck 0.5.0 | MIT OR Apache-2.0 | 11517 |
| ident_case 1.0.1 | MIT OR Apache-2.0 | 3492 |
| indoc 2.0.8 | MIT OR Apache-2.0 | 17682 |
| instability 0.3.14 | MIT | 14825 |
| itertools 0.13.0 | MIT OR Apache-2.0 | 146261 |
| lru 0.12.5 | MIT | 16047 |
| mio 1.2.4 | MIT | 110437 |
| paste 1.0.15 | MIT OR Apache-2.0 | 18374 |
| ryu 1.0.23 | Apache-2.0 OR BSL-1.0 | 52823 |
| signal-hook 0.3.18 | MIT OR Apache-2.0 | 50862 |
| signal-hook-mio 0.2.5 | MIT OR Apache-2.0 | 14373 |
| strsim 0.11.1 | MIT | 14266 |
| strum 0.26.3 | MIT | 7237 |
| strum_macros 0.26.4 | MIT | 27531 |
| unicode-truncate 1.1.0 | MIT OR Apache-2.0 | 12529 |
| unicode-width 0.1.14 | MIT OR Apache-2.0 | 271615 |
| unicode-width 0.2.0 | MIT OR Apache-2.0 | 271509 |
| wasi 0.11.1+wasi-snapshot-preview1 | MIT OR Apache-2.0 | 28477 |
| winapi 0.3.9 | MIT OR Apache-2.0 | 1200382 |
| winapi-i686-pc-windows-gnu 0.4.0 | MIT OR Apache-2.0 | 2918815 |
| winapi-x86_64-pc-windows-gnu 0.4.0 | MIT OR Apache-2.0 | 2947998 |

The probe totals 9,296,019 bytes including both direct crates. Final workspace
resolution may reuse or change versions; check its complete licences and sizes
before adding dependencies, and list new crates and why in the commit.

## Gate fixed before implementation

`scripts/gate-m9.sh` builds the real binary under the shared build lock (at most
eight jobs), drives it in an 80x24 PTY, and runs `scripts/gate-m7-core.sh` once.
The fixture starts with `alpha\nbravo\ncharlie\ndelta\n`, opens at line 2,
column 3, types `X`, undoes with Ctrl+Z, accepts `alpha` -> `ALPHA` with F5,
rejects `charlie` -> `CHARLIE` with F6, saves with Ctrl+S, then quits with
Ctrl+Q. Expected saved bytes are exactly `ALPHA\nbravo\ncharlie\ndelta\n`.

At save, a test-only `CLANKY_TUI_CAPTURE` path asks the same renderer to render
its state through ratatui's 80x24 test backend and write exactly 24 rows of 80
characters plus a newline each. Expected rows are `Clanky editor`, `ALPHA`,
`bravo`, `charlie`, `delta`, 18 blank rows, and
`saved | Ctrl+S save | Ctrl+Q quit`, each padded with spaces to 80 columns. This
capture does not affect normal use. The PTY must return to its original termios
state and output both leave-alternate-screen and show-cursor sequences. A
30-second limit includes startup and interaction. Test-owned files and processes
are cleaned up. The planted `CLANKY_GATE_FAULT=tui` changes only the observed
saved bytes before comparison and must print FAIL specifically there. Then run
the clean gate once, including the M7 core gate, and report.
