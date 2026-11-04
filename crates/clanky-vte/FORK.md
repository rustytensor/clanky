# clanky-vte

Forked from [vte](https://crates.io/crates/vte) 0.15.0, the escape-sequence parser of the Alacritty project, by Joe
Wilm, Christian Duerr and the vte contributors. Licensed under Apache-2.0 or MIT (LICENSE-APACHE, LICENSE-MIT).

## Changes by the Clanky project

- `src/lib.rs`: APC strings are handed to the new `Perform::apc_dispatch` callback (default no-op) instead of being
  discarded, so the Kitty graphics protocol works. Details in CLANKY-PATCH.md. Written so it can be offered upstream.
- Renamed the package to `clanky-vte` and the library to `clanky_vte`; `publish = false`.
- `examples/parselog.rs` and one doc example: `vte::` renamed to `clanky_vte::`.
