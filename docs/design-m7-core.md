# M7 editor core

## Gate first

Run `scripts/gate-m7-core.sh`. It builds only `clanky-edit` offline, under the
shared build lock, and runs a Rust executable against fresh private fixture files.
No display, terminal engine, network, or existing user file is involved.
The following expected bytes are fixed before implementation:

1. Open UTF-8 `alpha\r\nβeta\nlast` (17 bytes). Select byte range 7..9 (β),
   replace with `B`: `alpha\r\nBeta\nlast`. Undo restores the original bytes and
   selection 7..9; redo restores the replacement and collapsed selection 8..8.
   An edit at byte 8 of the original splits β and must fail without mutation.
   Save and reopen: exact replacement bytes, including mixed endings and no EOF
   newline. A second undo after saving restores original text, not the save state.
2. On fresh `one\ntwo\nthree\n`, propose replacements 0..3 -> `ONE` and
   8..13 -> `THREE`. Pending proposals change neither buffer nor disk. Accept
   the second and reject the first: buffer `one\ntwo\nTHREE\n`, disk still
   original until explicit save. Undo/redo of acceptance restores/reapplies
   exact bytes; review decisions remain accepted/rejected, never resurrected.
3. On fresh `one\ntwo\nthree\n`, propose 0..3 -> `FIRST` and 8..13 -> `THIRD`.
   Accept in that order: `FIRST\ntwo\nTHIRD\n` (the second range moves by 2).
   Overlapping proposals: after accepting 0..3 -> `FIRST`, another 1..3 -> `x`
   is stale and accepting it fails without mutation. Undo does not revive it.
4. On fresh `one\ntwo\n`, propose 0..3 -> `ONE`, then externally write
   `external\n`. Refresh/accept marks pending hunks stale; accept and save fail;
   disk remains exactly `external\n`. A same-length external replacement
   `ONE\ntwo\n` is also detected. Deletion and pathname replacement are conflicts.
5. Parse `src/a:b.rs:2:2`: path `src/a:b.rs`, line 2, column 2. On
   `alpha\r\nβeta\nlast`, this resolves to byte 9 (Unicode scalar columns,
   not UTF-8 byte columns). `a.rs:3` defaults column 1; zero, overflow, missing
   numeric suffix and out-of-range positions fail. CRLF is one line ending;
   EOF after a newline is an empty final line. Standalone CR is ordinary text.
6. Make a 16 MiB source file from `let x = 1;\n` repeated 1,525,202 times
   (16,777,222 bytes). Replace the last `1` with `2`, undo, redo, save/reopen;
   compare all bytes with independently constructed expected bytes. History
   stores changed spans, not a full copy per keystroke. No timing threshold.
7. Opening invalid UTF-8 or a symlink fails. Saving preserves Unix permission
   bits. A hard link to the old inode keeps the old bytes after save, while the
   edited pathname gets the complete new bytes (atomic replacement). The gate
   leaves no temporary sibling files after success or detected conflict.
8. `CLANKY_GATE_FAULT=accepted scripts/gate-m7-core.sh` changes the observed
   accepted text to `WRONG` immediately before comparison in item 2. It must
   print FAIL at that exact comparison and exit nonzero. Run fault once, then
   clean once: final PASS. Unknown fault names fail. Build errors also fail.

## Public contract

New independent workspace crate `clanky-edit`, no third-party dependencies.
Rust standard library only (same MIT OR Apache-2.0 project licence). It contains
no GPU, window, terminal, socket or shell code. Existing APIs are unchanged.
Future transports must add versioned operations, not expose disk access by default.

`Document::open(path) -> Result<Document>` opens an existing regular UTF-8 file.
`text() -> &str`, `selection() -> Selection`, `select(Selection)`,
`edit(Range<usize>, &str)`, `undo() -> Result<bool>`, `redo() -> Result<bool>`,
`save()`, and `refresh() -> Result<bool>` form the local editor API. Selection
has public `anchor` and `cursor` byte offsets; backwards selections are valid.
All offsets must be UTF-8 boundaries. Each edit is one undo group; selection
before/after travels with history. Edits collapse the caret after inserted text.
No newline normalization. Empty/no-op edits do not create history. A real new
edit clears redo. Save never clears history. Read-only data is borrowed.

`propose(range, replacement) -> Result<u64>` snapshots a hunk against current
text and disk baseline. `hunks() -> &[Hunk]`, `accept(id)`, `reject(id)` expose
review. Hunk fields: `id`, `range`, `replacement`, `state`; states Pending,
Accepted, Rejected, Stale. IDs are document-local and never reused. Accept is
an ordinary undoable edit, not a disk write. Reject changes metadata only.
Only Pending hunks can be accepted or rejected. Missing IDs and repeated
operations fail. Refresh compares disk with the open/last-save baseline; any
external change conservatively stales ALL pending hunks, even disjoint ones.
IO errors also fail closed. Conflict stays latched until reopen; no force-save.
Accept and propose refresh disk first. Local edits shift disjoint pending ranges
and stale intersecting ones; insertions at either endpoint conservatively stale
a hunk. Undo/redo follows the same invalidation rule, never silently rebases a
stale hunk or reverses review decisions. No agent proposal writes disk.

`Location::parse(&str)` parses rightmost numeric `:line[:column]`, retaining
colons in paths. Positions are one-based Unicode scalar coordinates, excluding
CRLF/LF terminators; the position just past a line's content is allowed.
`Document::offset(line, column)` returns a checked UTF-8 byte offset.
The caller resolves relative paths against an explicit working directory.

Errors distinguish IO, invalid UTF-8, invalid range/position/location, conflict,
unknown hunk, non-pending hunk and exhausted ID space. No panics on caller ranges.

## Storage and limits

A contiguous UTF-8 String and span-based undo records keep the first core small.
Edits move suffix bytes (O(file size)); position lookups scan text. This supports
the concrete large-file gate without claiming rope-like scaling. Undo memory is
proportional to changed text, retained hunks and edit count, with no automatic
history eviction in this version. Rendering and async IO belong to later layers.

A disk module owns opening, baseline checks and saves. Linux-specific identity
and permission handling stays in `platform.rs`. Reject symlinks and nonregular
files. Baseline comparison checks identity, metadata and exact bytes (not mtime
alone). Save creates a unique same-directory temporary file with create_new,
writes and syncs complete bytes, preserves permissions, checks baseline again,
then atomically renames and syncs the parent directory. Cleanup removes owned
temporary files on failure. Refresh baseline after success; report sync errors.
No cross-filesystem fallback and no in-place truncation.

This is optimistic conflict detection, not a filesystem transaction: an
uncooperative process can modify the destination between the final check and
rename. Portable atomic compare-and-replace is unavailable. Callers must not
claim protection against hostile concurrent filesystem writers. The gate checks
changes completed before save/accept and atomic replacement, not that race.

## Delivery

Design is committed first, then implementation proceeds without a review pause.
Bounded workers may implement disk storage and the gate; the lead owns public
API, buffer/review logic, integration and reading all changes. No dependency adds.
Each cargo invocation uses the shared lock; build at most eight jobs. Only this
milestone gate runs: existing terminal crates are not changed or retested.
Branch tags use `v0.3.1-edit.N`; workspace package versions match each tag.
Stop after the planted failure and clean PASS, or the first unexpected real
check failure. GUI panel, socket integration, syntax highlighting, file creation,
save-as, multi-caret editing and TUI rendering are outside this core milestone.
