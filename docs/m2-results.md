# M2 results

The automated M2 gate passed on implementation commit `ab67005` (`v0.1.3`).
Release `v0.2.0` changes only version metadata and documentation after that run.

| Check | Result |
| --- | --- |
| Planted image fault | Exit 1: red quadrant expected (255,0,0), observed (0,0,0) |
| Clean M2 gate | Exit 0, final PASS |
| Images | Real APC/parser/PNG/GPU path produced red, green, blue and white; whole input and two-byte chunks |
| Split geometry | 39/40-column panes, then 11/12-row right panes; divider misses and bottom-right hit matched |
| Independent PTYs | Exact A/B/C, exit 0 each; closing one pane preserved the other IDs and expanded its sibling |
| Config and metrics | 12 pt gave 16 px and 10x20 cells; zoom gave 11x22; reset 10x20; scale 1.5 gave 24 px and 15x30; zero rejected |
| M1 regression | Socket/process, OSC blocks, original 16x32 text fixture and independent tabs passed unchanged |

Commands, once in this order:

```sh
CLANKY_GATE_FAULT=image scripts/gate-m2.sh
scripts/gate-m2.sh
```

The full offline workspace/binary build passed with eight workers. A later
app-only build passed after zero-size suspension was corrected. Dependency
licence choices are recorded in [the inventory](deps-m2.md). The vendored vte
patch changes only `src/lib.rs`; upstream licence notices are retained.

Not certified by this gate: monitor-to-monitor scaling, minimize/restore,
interactive split resizing, IME composition, keyboard zoom, or `kitten icat`.
Those remain manual checks. Kitty support is the documented direct-transfer
subset, not every protocol extension. No performance or full upstream suite
was run. M3 has not started.
