<!-- Modified by the Clanky project; see FORK.md in this crate. -->
# clanky-vte

Forked from [vte](https://crates.io/crates/vte) 0.15.0, the escape-sequence parser of the Alacritty project, by Joe
Wilm, Christian Duerr and the vte contributors. Licensed under Apache-2.0 or MIT (LICENSE-APACHE, LICENSE-MIT).

## Changes by the Clanky project

- `src/lib.rs`: cap std OSC input at base64-encoded 1 MiB plus 64 header bytes; count separators too. On overflow discard the entire OSC through BEL or ST, across chunks, without dispatching its prefix. No-std uses its configured bound with the same discard behavior.
- `src/ansi.rs`: reject malformed OSC 52 parameter counts entirely without logging clipboard values.
- `FORK.md`: record these local modifications and retain the fork notice.

- `src/lib.rs`: APC strings are handed to the new `Perform::apc_dispatch` callback (default no-op) instead of being
  discarded, so the Kitty graphics protocol works. Details in CLANKY-PATCH.md. Written so it can be offered upstream.
- Renamed the package to `clanky-vte` and the library to `clanky_vte`; `publish = false`.
- `examples/parselog.rs` and one doc example: `vte::` renamed to `clanky_vte::`.
