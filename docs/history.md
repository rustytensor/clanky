# Saved history

Clanky 1.1 keeps every command across restarts in one SQLite file: the command line,
where and when it ran, who ran it, its exit code and duration, and its output.
The design and its fixed decisions are in [design-h1-history.md](design-h1-history.md).

## The file

`$XDG_DATA_HOME/clanky/history.sqlite3`, or `$HOME/.local/share/clanky/history.sqlite3`
when XDG_DATA_HOME is unset or empty. The file is owner-only (0600) in an owner-only
directory (0700). `clanky history path` prints it.

The session server is the only writer. It saves a command when its block ends (the
shell's end marker, a new prompt, or the shell exiting). When the server stops, running
commands are saved as interrupted (no exit code, no duration).

- **Secrets never reach the disk.** Commands and output are stored as the strict
  masking policy shows them: each secret becomes stars of the same length.
- **Who ran it:** `you` for lines you typed, or the agent's `client_name` (from its v4
  hello) when the agent's paste or key submitted the line.
- **Agent actions:** the table `agent_events` keeps each acting agent request (paste,
  send_key, run and the like, not reads) with its masked arguments. Deleting a command
  also deletes the agent actions that led to it.
- **Output** is capped at `history_output_kib` KiB per command; `output_truncated`
  says when anything was cut.
- **Retention:** at server start and then daily, commands older than `history_days`
  days are deleted. 0 keeps everything.
- **Off:** `"history": false` creates no file and writes nothing. All three keys apply
  live; see [Configuration](config.md).
- **A broken file** (not a readable SQLite database) is renamed to
  `history.sqlite3.broken-<unix seconds>` unchanged, a fresh file is started, and Recent
  shows one notice. **A newer file** (written by a later Clanky) is left untouched for
  that run, with one notice.

Agents have no access to saved history.

## Command line

The commands read the file directly; no server is needed.

| Command | Effect |
| --- | --- |
| `clanky history path` | Prints the file path |
| `clanky history [list] [--limit N] [--json]` | Newest N (default 50) commands, oldest first |
| `clanky history search TEXT [--limit N] [--json]` | Commands whose command line or output contains TEXT as a phrase |
| `clanky history show ID` | One command with its whole stored output |
| `clanky history export [--format json\|csv\|markdown] [--output FILE]` | Every command; an output file is owner-only |
| `clanky history delete ID...` | Deletes commands; their text is overwritten in the file |
| `clanky history clear [--before YYYY-MM-DD]` | Deletes everything, or what started before that UTC date |

Exit codes: 0 done, 1 error, 2 bad arguments.

JSON rows (and the CSV header) have exactly these keys:
`id, session_id, started_at, cwd, actor, command, exit_code, duration_ms, complete,
output, output_truncated`. `started_at` is Unix milliseconds (UTC); `complete` and
`output_truncated` are booleans.

`delete` and `clear` rewrite the search index and checkpoint the write-ahead log, with
SQLite's `secure_delete` on, so deleted text is left neither in the database nor in its log.

## Other tools

Any SQLite tool can read the file: `sqlite3 "$(clanky history path)"`. The tables are
`sessions`, `commands` and `agent_events`; `commands_fts` is an FTS5 index kept in step
by triggers. `PRAGMA user_version` is the schema version (1).
