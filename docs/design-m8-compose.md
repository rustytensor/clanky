# M8 multi-line compose box

## Gate first

Run `scripts/gate-m8-compose.sh`. Generate fixtures under the build tree, never
read a real personal dictionary, and remove fixtures/owned processes afterward.
Build offline under the shared build lock with at most eight jobs. The gate uses
the same controller and send method as the GUI, plus real PTYs and offscreen
software rendering. The following expectations precede implementation.

1. System en_US reference text:
   `Please review this sentnce and teh changes.\nRun --quik src/mispeld.rs foo_barr std::vec https://exampl.invalid \`wrng wrod\`.\n`
   Exact flagged words, in order: `sentnce`, `teh`, ranges 19..26 and 31..34.
   No code-looking token or backtick content is flagged. The dictionary lists
   please, review, this, sentence, and, the, change, run; it does not list the two
   misspellings. Independent `hunspell -d en_US -l` confirms those two on the prose
   line; `hunspell -a` offers sentence/the respectively. Suggestions must contain
   `sentence` for sentnce and `the` for teh; ordering is not an expected value.
2. Add `sentnce` to a fresh fixture personal list: exact file `sentnce\n`. Reload
   a checker using that list: only `teh` remains flagged. No fixture reads/writes
   the person's real list. Bad dictionary/missing dictionary is a visible spell
   error, not a crash or a claim that all words are correct; typing/send still work.
3. Open compose for terminal A, insert `first line\nsecond β\n` with the shared
   editing core. Escape closes; reopen A: exactly the same bytes and selection.
   Open B: empty independent draft. Returning to A preserves its draft. Undo/redo
   of an inserted `X` restores/reapplies exactly one edit, independently of B.
4. Ctrl+Enter uses one send call, pinned to A even if terminal focus changes.
   A real PTY receiver uses raw mode and records every received byte, including
   any trailing input after the closing marker. Default exact bytes:
   `ESC[200~first line LF second SP β LF ESC[201~` (33 UTF-8 bytes).
   With compose_send_enter=true, the same 33 bytes plus a single CR, total 34.
   No implicit Enter, newline rewriting, per-line send, or shell interpolation.
   Failure keeps the draft and box; success closes the box and retains the draft.
   Embedded ESC/control bytes other than TAB/CR/LF are refused before send.
5. Render the actual flagged prose with client-local underline decorations.
   Misspelled spans have RGB (255,96,96) on the bottom two pixels of their cells;
   sample a cell inside sentnce at its bottom pixel. The adjacent blank cell at
   column 18 has ordinary background (24,24,28), not an underline. Use the same
   font/cell pipeline and software renderer as the editor/terminal, not a new shaper.
6. `CLANKY_GATE_FAULT=compose-send scripts/gate-m8-compose.sh` removes the observed
   opening bracketed-paste marker immediately before the PTY comparison. Require
   FAIL there. Run fault once, clean once. Clean invokes M7 panel once (including
   M4/M3/M2/M1), M7 core once, M6 masking once, and the narrow cwd gate once. M5
   belongs to the other branch and is not available here. No duplicate chain runs.

Dictionary reference: en_US.aff 3,225 bytes, SHA-256
`9355d1788c8196b9fc5a3a15905008aa2cf5a697b5da562e158e48e7b9980a00`;
en_US.dic 508,624 bytes, SHA-256
`36efe178f2cc05648a3c2746c1b202d2d09d3f7fa52dba124be92b97ca21b704`.
These identify the independent expectation source, not bundled dictionary files.

## Dependency and licence decision

- **spellbook 0.4.2**, MPL-2.0, compressed crate 691,342 bytes: Hunspell-compatible
  checking/suggestions. The approved exception is **MPL-2.0 (spellbook only)**.
- **hashbrown 0.17.1**, MIT OR Apache-2.0, compressed crate 155,512 bytes: spellbook's
  table storage; this exact version is already in the workspace lock/cache.
- Total compressed size of that dependency closure: 846,854 bytes. Only spellbook
  is a new package. Disable spellbook default features and use std RandomState;
  no foldhash or dev/test dependencies are added. No dictionary download/vendor.

The approved download has been resolved in a scratch manifest, not by changing
application behavior. Production dependency is pinned to `=0.4.2` in Cargo.lock.
The design commit adds the exact approved exception to AGENTS.md's licence list.

## Behavior and integration

Ctrl+Shift+Enter or the palette's Compose action opens an overlay over the focused
terminal, capturing its stable terminal ID. Keyboard/IME/mouse go to compose while
open, never the terminal. Enter inserts a newline; Ctrl+Enter sends; Escape closes
and retains the terminal's draft. Suppress repeated send key events. A terminal
that closes yields an error, never a retargeted send. Drafts are memory-only for
the window lifetime; no prompt is written to a temporary or config file.

Use clanky-edit with a new in-memory Document constructor; file-backed open/save
behavior is unchanged. Reuse EditorPane input/selection/visible-line drawing with
an in-memory factory. A memory document cannot save without a path and returns a
specific error. Compose is not a file editor, so Ctrl+S does not create a file.
Drafts are limited to 256 KiB UTF-8 (checked before edits), safely below the PTY
input budget. Ordinary editor undo/redo, selection, navigation and find remain.

Load spellbook once per compose controller, using system `.aff`/`.dic` files.
Recheck after text edits or personal-word changes, not every render frame. Spell
errors remain visible and do not prevent composing/sending. F7 requests suggestions
for the misspelled word at/after the caret (wrap to first); Up/Down selects and
Enter replaces through the editor core, so correction is undoable. Escape first
closes a suggestion popup; a further Escape closes compose. Ctrl+Alt+A adds that
word to the personal dictionary. Clearly display these controls.

Tokenization excludes complete whitespace-delimited code-looking tokens: paths
(`/` or backslash, or an internal dot), flags beginning `-`, identifiers containing
`_`, `::`, or digits, URLs, and all backtick-delimited spans (including multiline
and an unclosed span through EOF). Split remaining prose at punctuation, retaining
internal apostrophes. This is a conservative prose filter, not a code parser.

Config adds `compose_dictionary` (default en_US; safe locale name),
`compose_dictionary_dir` (absolute path, default /usr/share/hunspell), and
`compose_send_enter` (boolean, default false). Personal words live in
`compose-words.txt` beside the selected config file, otherwise in the normal
Clanky config directory. Store one validated word per line; create only on explicit
Add, preserve other entries, and report IO errors. No available config directory
means personal-word persistence is disabled with a visible explanation.

Underlines are a renderer-local `Underline` list, not fields on serialized Cell or
TerminalSnapshot. Existing drawing delegates to an empty list; compose supplies
its clipped word spans. The server/API and person's terminal state are unchanged.
No new transport or competing M5 operation. SSH compose uses the existing Client
input operation and captured remote terminal ID, with no local file guessing.

## Delivery

Design/gate first, commit/report, then implementation without a review pause.
Tags `v0.4.4-compose.N`; workspace versions match. Workers may own memory editing,
spelling/compose controller, GUI/config integration, and renderer/gate. The lead
reads every changed line and runs the milestone gate. Secret scan before commits,
push both approved remotes after each commit. Stop after fault FAIL and clean PASS.
Manual acceptance remains real desktop IME, mouse selection, suggestion interaction
and application-specific bracketed-paste behavior; the PTY gate proves exact bytes.
