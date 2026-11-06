# H1: saved command history (SQLite)

Clanky 1.1 keeps every command card across restarts in one SQLite file the person can
open, search, export and edit. This milestone is the store and its command line.
The in-app viewer is H2 and starts from a mockup Marc approves first.

Gate: `scripts/gate-h1-history.sh` (pass/fail, real server, values fixed below).

## Decisions (fixed for H1)

- **Library:** rusqlite (MIT). Linux links the system `libsqlite3.so.0`; nothing is
  bundled on Linux (Fedora rule). Windows and macOS builds enable rusqlite's
  `bundled` feature through `[target.'cfg(not(target_os = "linux"))'.dependencies]`.
  SQLite itself is public domain (approved by Marc, 2026-11-04). The RPM spec gains
  `BuildRequires: sqlite-devel`.
- **File:** `$XDG_DATA_HOME/clanky/history.sqlite3`, or
  `$HOME/.local/share/clanky/history.sqlite3` when XDG_DATA_HOME is unset or empty.
  File mode 0600, directory 0700. File locations go through the platform layer, so
  Windows and macOS add a path, not a rewrite.
- **Who writes:** the server, which owns the terminals. One writer thread fed by a
  channel; the PTY and render paths never wait on SQLite. WAL mode,
  `PRAGMA secure_delete=ON`, `busy_timeout` 5 s.
- **When:** a row is written when a block is sealed (OSC 133 D, a new prompt, or EOF).
  On SIGTERM/normal exit the server seals running blocks as incomplete, writes them,
  sets `sessions.ended_at` and checkpoints the WAL before it exits.
- **Secrets never reach the disk.** `command` and `output` are stored exactly as the
  person's own copy shows them: the strict masking policy, stars of equal length.
  Raw text never goes to the writer thread.
- **Who ran it:** `actor` is `you` for commands the owner typed, or the agent's
  `client_name` (from v4 hello) when the agent's paste/send_key submitted the line.
  The rule: the last input source before the C marker wins.
- **Output cap:** the first `history_output_kib` KiB of the masked output (cut on a
  UTF-8 boundary); `output_truncated` is 1 when anything was cut, or when the
  in-memory block was already truncated.
- **Retention:** at server start and then every 24 h, rows whose `started_at` is
  older than `history_days` days are deleted. 0 keeps everything.
- **Broken file:** a file that is not a readable SQLite database is renamed to
  `history.sqlite3.broken-<unix seconds>` (bytes unchanged), a fresh file is created,
  and the person sees one notice. The server always starts.
- **Newer file:** `PRAGMA user_version` above the version this build knows means a
  newer Clanky wrote it. The server leaves it untouched (no writes, no migration) and
  shows one notice. The CLI may still read it.
- **Off switch:** `"history": false` creates no file and writes nothing.
- **Agents:** no access to saved history in H1. The CLI reads the file directly and
  needs no server.

## Config keys (docs/config.md)

| Key | Type | Default | Values |
| --- | --- | --- | --- |
| `history` | boolean | true | false: no history file is created or written |
| `history_days` | integer | 90 | 0–3650; 0 keeps everything |
| `history_output_kib` | integer | 256 | 1–65536; stored output per command |

Changing them in the file applies live, like every other key.

## Schema (version 1, `PRAGMA user_version = 1`)

The gate and outside tools rely on these table and column names. More columns,
tables and indexes are allowed.

```sql
CREATE TABLE sessions (
  id INTEGER PRIMARY KEY,
  started_at INTEGER NOT NULL,      -- Unix ms, UTC
  ended_at INTEGER,                 -- null while running or after a crash
  host TEXT NOT NULL,
  version TEXT NOT NULL             -- Clanky version
);
CREATE TABLE commands (
  id INTEGER PRIMARY KEY,
  session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  tab_id TEXT NOT NULL,
  started_at INTEGER NOT NULL,      -- Unix ms, UTC, at the C marker
  cwd TEXT,                         -- terminal_cwd at the C marker; null if unknown
  actor TEXT NOT NULL,              -- 'you' or the agent's client_name
  command TEXT,                     -- masked; null when the shell sent none
  exit_code INTEGER,                -- null when incomplete
  duration_ms INTEGER,              -- null when incomplete
  complete INTEGER NOT NULL,        -- 1 after D, 0 when interrupted
  output TEXT NOT NULL,             -- masked, capped
  output_truncated INTEGER NOT NULL
);
CREATE TABLE agent_events (
  id INTEGER PRIMARY KEY,
  session_id INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  at INTEGER NOT NULL,              -- Unix ms, UTC
  agent TEXT NOT NULL,              -- client_name
  tab_id TEXT,
  action TEXT NOT NULL,             -- the audited op name, e.g. paste, send_key, run
  detail TEXT NOT NULL              -- the masked audit arguments as JSON
);
```

Full-text search over `command` and `output` uses an FTS5 table kept in step by
triggers (system SQLite on Fedora and the bundled build both include FTS5).

## Command line

All read the file directly; no server is needed. JSON rows are objects with exactly
these keys, oldest first:
`id, session_id, started_at, cwd, actor, command, exit_code, duration_ms, complete,
output, output_truncated` (`complete` and `output_truncated` as JSON booleans,
`started_at` as Unix ms).

| Command | Effect |
| --- | --- |
| `clanky history path` | Prints the file path |
| `clanky history [list] [--limit N] [--json]` | Newest N (default 50) rows; text table or JSON array |
| `clanky history search TEXT [--limit N] [--json]` | Full-text match on command and output |
| `clanky history show ID` | One row with its whole output |
| `clanky history export [--format json\|csv\|markdown] [--output FILE]` | Every row; CSV header is the key list above |
| `clanky history delete ID...` | Deletes rows; afterwards their text is gone from the file bytes |
| `clanky history clear [--before YYYY-MM-DD]` | Deletes all rows, or rows started before that UTC date |

`delete` and `clear` end with a WAL checkpoint (TRUNCATE) so deleted text is not left
in the WAL either. Exit codes: 0 done, 1 error, 2 bad arguments.

## Not in H1

The in-app viewer (H2, mockup first), restoring old output into reopened panes,
agent access to history, import, Windows/macOS paths beyond the platform hook.
