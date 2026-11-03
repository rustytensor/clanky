# M1 automated gate: PASS

The implementation at `v0.0.10` built with
`cargo build --workspace --bins --offline -j 8`.

The gate was run once per mode, in this order:

1. `CLANKY_GATE_FAULT=exitcode scripts/gate-m1.sh`: exit 1,
   `FAIL: exit code expected 3 / observed 0`. The intended assertion caught the fault.
2. `scripts/gate-m1.sh`: exit 0, final `PASS`.

The clean run verified four real outputs: process exit 3 with bytes `hello`,
OSC 133 blocks `one` and `two`, red/black cell-centre pixels within tolerance,
and independent tabs with outputs `A` and `B` present in the tab list.

`v0.1.0` records this result. Only version metadata and documentation changed
since the passing build; the gate was not repeated. `v0.1.1` corrects the
workspace version entries left stale in Cargo.lock by the metadata-only refresh;
it also contains no functional code changes.

Manual acceptance remains pending: open a real window, run `vttest` menus 1 and 2,
and run two commands in fish to confirm two blocks. Automated results do not
establish window/input correctness. Images and splits are outside M1.
