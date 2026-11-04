# M7 editor core results

`v0.3.1-edit.3`: `scripts/gate-m7-core.sh` passed, exit 0.
The planted `accepted` fault failed at item 2's exact text comparison, exit 1:
observed `WRONG`, expected `one\ntwo\nTHREE\n`. The clean runtime gate ran once.

Verified: UTF-8 boundaries, mixed line endings, selection restoration, undo/redo
across save, pending hunk acceptance/rejection, shifted and stale hunks, external
file changes, line/scalar navigation, a 16,777,222-byte file, atomic replacement,
permissions, symlink preservation and old hard-link contents. Fixture cleanup
completed. No new dependencies; existing terminal gates were not rerun.

Before runtime testing, the locked build caught five dependency versions altered
by the branch version bump. Cargo restored their original registry versions;
no expected test result changed. The only requirement change was the approved
symlink behavior, committed before implementation.

The crate is independent of the app. GUI, socket and TUI integration remain
future work. Saves use optimistic conflict detection; an external write racing
the final check and rename is not excluded. The first buffer uses a String and
span history, not a rope. No latency benchmark or non-Linux acceptance is claimed.
