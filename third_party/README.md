# Third-party code

## gpui-ce

`gpui-ce/` holds the crates of [gpui-ce](https://github.com/gpui-ce/gpui-ce), a community fork of Zed's GPU-accelerated
UI framework, that Clanky's window is built on. Clanky depends on them by path, so a build needs no network access to
fetch them.

- The crates are copied unchanged from Clanky's own copy of gpui-ce, at the commit named in `gpui-ce/SOURCE`, except
  for the files listed below, which carry Clanky's changes and a line saying so (Apache-2.0, section 4(b)).
- `gpui-ce/LICENSE.md` is gpui-ce's licence (Apache-2.0). Each crate keeps its own `LICENSE-APACHE`.
- `gpui-ce/NOTICES.md` carries the notices for the MIT-licensed code inside gpui-ce.
- `gpui-ce/Cargo.toml` is gpui-ce's workspace manifest with the member list cut down to the bundled crates.
- `scripts/bundle-gpui-ce.sh <commit>` rewrites this folder from the copy. `scripts/check-gpui-ce-bundle.py` checks that
  every file still equals the copy's.

The list below is written by the script.

<!-- bundle:begin -->
<!-- bundle:end -->
