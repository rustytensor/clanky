# M6 streaming secret masking core

## Gate first

`scripts/gate-m6-masking.sh` builds only `clanky-mask` offline under the shared
build lock and runs `masking-gate`. Build/cache paths come from CARGO_TARGET_DIR
and CARGO_HOME; CLANKY_BUILD_LOCK defaults to `/srv/devops/build/clanky/.build.lock`.
No server, terminal, display, real environment map or real key file is used.
All credentials are synthetic pseudorandom strings generated in process memory
at run time, never fixtures on disk. The PRNG is for fake inputs, not security.

Expected results, fixed before implementation:

1. Each input `before <secret> after` becomes exactly
   `before [masked:<NAME>] after`. Cases and names:
   - `sk-` + 48 alphanumeric bytes: OPENAI.
   - `sk-or-v1-` + 64 hex bytes: OPENAI (same prefix family).
   - `ghp_` + 36 alphanumeric bytes: GITHUB.
   - `github_pat_` + 82 alphanumeric/underscore bytes: GITHUB.
   - `AKIA` + 16 uppercase alphanumeric bytes: AWS.
   - `xoxb-`, `xoxp-`, `xoxa-`, `xoxr-`, `xoxs-`, `xoxc-`, `xoxe-`
     + 32 allowed bytes: SLACK.
   - `hf_` + 34 alphanumeric bytes: HUGGINGFACE.
   - `rpa_` + 48 alphanumeric bytes: RUNPOD.
   - `Authorization: Bearer ` + 40 URL-token bytes: BEARER. The entire
     header/value match is replaced, not just its credential suffix.
   - Complete PRIVATE KEY, RSA PRIVATE KEY, EC PRIVATE KEY, DSA PRIVATE KEY,
     OPENSSH PRIVATE KEY and ENCRYPTED PRIVATE KEY PEM blocks: PRIVATE_KEY.
     Each body is generated base64 text; matching BEGIN/END delimiters included.
   - A synthetic environment value of 32 alphanumeric characters under
     TEST_API_TOKEN: TEST_API_TOKEN.
   - Named extra literal of 24 random alphanumeric characters: EXTRA.
2. Feed every above secret at EVERY possible two-chunk byte boundary; concatenate
   both push results and finish. Expected exactly `[masked:<NAME>]` every time.
   Feed each as three chunks at byte 2 and length-2, and one byte at a time;
   expected the same. Compare only against the named constant, never a previous
   execution's output. A raw emitted prefix is also accumulated for leak scans.
3. Adjacent configured env/extra values become
   `[masked:TEST_API_TOKEN][masked:EXTRA]`; two adjacent AWS keys become
   `[masked:AWS][masked:AWS]`. Variable-length token runs without a separator
   are intentionally one secret, not two credentials whose boundary is guessed.
4. `sk-abcde ghp_short hf_short rpa_short AKIA123` stays exactly unchanged.
   `雪🙂 <secret> café\r\n` becomes `雪🙂 [masked:<NAME>] café\r\n`, byte for
   byte, including chunk boundaries inside UTF-8 sequences. Invalid UTF-8 plain
   bytes also pass unchanged; the API is bytes, not lossy text.
5. A non-secret environment name IGNORE_THIS and a value shorter than 12 Unicode
   scalar characters under SHORT_TOKEN are ignored. Empty extra literal and
   invalid label fail with static errors. Error Display/Debug never echo input.
   A 12-character multibyte env value is masked. Longer literals sharing a prefix
   select the longer complete match; a failed continuation exposes only safe text.
6. An unterminated recognized private-key block masks through EOF. A long token
   body and a long private-key body are discarded as they arrive: buffered bytes
   never exceed `hold_limit()`. For the default gate configuration the longest
   trigger is the 37-byte ENCRYPTED PRIVATE KEY header, so the limit is exactly
   36 bytes. A standalone ordinary byte is returned immediately.
   Finish releases only nonmatching trailing prefixes; pushing after finish fails.
7. Search every concatenated result, formatted error and captured child stdout/
   stderr for the generated secrets (and generated private-key body text).
   Comparisons and failures never print observed secrets. The parent captures
   child output in memory, scans before reporting, and prints only fixed status.
   The masker and its secret buffers have no Debug implementation. The gate's
   generator likewise avoids Debug/assertion formatting of credential data.
8. `CLANKY_GATE_FAULT=split scripts/gate-m6-masking.sh` simulates disabled
   hold-back by finishing a fresh masker at each chunk in the first boundary-2
   case. Require FAIL specifically there, with a fixed diagnostic and no secret
   in any diagnostic. Production code has no environment-dependent fault mode.
   Fault once, then clean once: PASS. Unknown fault values and build errors fail.

## API and matching rules

Independent crate `clanky-mask`, standard library only; no new dependency.
Its public API is `Masker::new(env: &[(&str, &str)], extra: &[(&str, &str)])`
returning `Result<Masker, Error>`, `push(&mut self, &[u8]) -> Result<Vec<u8>, Error>`,
`finish(&mut self) -> Result<Vec<u8>, Error>`, `held_bytes() -> usize` and
`hold_limit() -> usize`. Finish is single-use; a new command needs a fresh masker.
The caller provides names/values, never the process environment. Original input
and caller-owned configuration remain the caller's responsibility to protect.
Nothing reads files, opens sockets or logs. No server API changes or wiring.

Environment names are matched case-insensitively for KEY, TOKEN, SECRET, PASSWORD
or PASSWD; values need at least 12 Unicode scalar characters. Extras are named
exact UTF-8 literals, not a new regex language. Empty extras are errors. Retain at
most 128 effective env/extra patterns, each at most 65,536 bytes; oversized input
fails rather than silently omitting protection. Labels must be 1–64 ASCII letters,
digits or underscores, starting with a letter/underscore. Invalid labels fail;
errors carry neither the name nor value. Labels or complete output markers containing a configured secret
or a recognized built-in token are rejected, so a marker cannot echo a secret.
Duplicate exact literals use the first configured name (env before extras).

The recognizer uses conservative shapes, not issuer validation. `sk-` requires
at least 6 following ASCII alphanumeric/underscore/hyphen bytes. The GitHub,
Hugging Face and RunPod prefixes above require at least 12 such bytes. Slack
matches `xox` + any ASCII letter + `-`, then at least 12 such bytes.
AWS is exactly AKIA plus 16 uppercase letters/digits. Bearer matches the ASCII
case-insensitive prefix `Authorization: Bearer ` plus at least one byte from
ASCII alphanumeric or `-._~+/=`. No surrounding word boundary is required.
Once a variable-length run is recognized, emit its marker and suppress the rest
of that run until a nonmember byte; token length is unbounded without buffering.
Private keys emit at the complete BEGIN delimiter, then suppress through the
corresponding END delimiter (or EOF). Public-key blocks are not matched.

For matches starting at the same byte, wait for incomplete longer candidates;
then choose the longest completed trigger, with configured literals first on
ties. Otherwise choose the leftmost match. Overlapping matches are not emitted
as separate hunks. Literal adjacency and fixed-length AWS adjacency are exact;
unseparated variable token runs are conservatively coalesced as described above.
Markers are output only, never reprocessed. No guarantee is made for secrets
outside these shapes/configured values or for independently masked overlapping
windows of a stream. The server must keep one ordered masker per command/read
stream and must not send raw output while a suffix is held back.

## Memory and streaming

Secret bytes may exist only in private process memory for matching/hold-back.
No secret-bearing type derives or implements Debug. Errors are fixed variants
with static Display text; failures never include input bytes. No raw diagnostic
formatting or secret-bearing assertions. Caller data is borrowed, internal
matcher copies are owned and overwritten on drop. The hold-back uses a fixed
allocation, wipes removed bytes immediately, and wipes the entire allocation on
drop. Volatile zero writes plus a compiler fence prevent dead-store removal;
this small unsafe primitive is isolated and documented. No allocator-growth
copies of an accumulated secret buffer. No third-party zeroization dependency.
This does not promise locked pages, swap exclusion, or erasure of caller copies.

Process input incrementally, not by buffering a whole chunk. Keep only the
undecided suffix which can still begin a pattern. Maximum hold-back is the
longest literal/recognition-trigger length minus one; private-key bodies and
long token tails use state, not storage. End-marker matching stores only a
public delimiter position. At EOF incomplete nonmatches are returned unchanged;
recognized secrets remain masked. This gives bounded memory independent of
command output size; matching work grows with the number and length of patterns.

## Delivery

Branch `m6-masking` starts at the accepted M7 lock fix. Tags are
`v0.3.1-mask.N`; workspace versions match. Commit this design, report, then
implement without a review pause. Workers may own the core and independent gate;
the lead reads every line and integrates. No existing terminal/editor gate rerun.
Stop after the fault FAIL and clean PASS. The person's view remains untouched:
only later agent-read integration will consume this crate's output.
