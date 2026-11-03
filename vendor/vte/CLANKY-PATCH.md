# vte 0.15.0 APC callback patch

Base: crates.io vte 0.15.0, unmodified package source and MIT/Apache-2.0
licences included. Registry cache markers `.cargo-ok` and
`.cargo_vcs_info.json` are omitted.

Only `src/lib.rs` differs from upstream. It adds the default-no-op
`Perform::apc_dispatch(&mut self, bytes: &[u8])`, an APC buffer, and two parser
states. ESC underscore starts APC; ESC backslash dispatches its complete payload
without delimiters. State and payload survive calls to `advance`.
CAN/SUB discard the payload and execute the cancellation control. Any other ESC
sequence discards the APC and resumes the existing escape parser. The existing
ESC-backslash dispatch still occurs, preserving the default performer's events.
SOS/PM, OSC, DCS, CSI and ground processing are unchanged.

The std buffer accepts at most 16 MiB; overflow suppresses the entire callback,
never dispatching a truncated prefix. In no-std builds the existing const-generic
OSC capacity also bounds APC (no allocation requirement is added). Overflow
remains in APC state until termination, cancellation or an aborting escape.

No byte scanner is used by Clanky. Its existing observer uses this callback;
the upstream ANSI processor and observer advance in single-byte lockstep so
placement captures the actual APC event's cursor, not a chunk's final cursor.

Functional validation is owned by the M2 integration gate; no upstream test
suite is run as part of this patch.
