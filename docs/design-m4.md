# M4: sessions outlive their windows

## Gate first

Command: `scripts/gate-m4.sh`. Expected bytes and states below are fixed before
implementation. No missing-input skips or downloads. Gate transport is a local
stdio pipe through `clanky --stdio`, not live SSH: no passwordless SSH fixture is
available for this run. It uses the same framed client/bridge path as SSH attach.
No sshd is started or changed, and no TCP listener is introduced.

1. A real client process starts the real headless server on demand in a private
   temporary directory. It opens two terminal IDs, A and B, at 3 rows by 20
   columns. Save their IDs, command IDs and the actual server PID, never an owner
   credential. The client stays alive with its socket open until killed below.
2. A runs a generated shell script: OSC 133 C, `printf 'one\n'`, OSC 133 D with
   code 0, then `printf 'h0\nh1\nh2\nh3\nh4\n'`. Wait for exit 0. Its block
   output is exactly `one`; raw output is `ESC]133;C BEL one CR LF ESC]133;D;0 BEL`
   then `h0 CR LF h1 CR LF h2 CR LF h3 CR LF h4 CR LF` (no separating spaces).
   On a 3-row terminal, retained history is exactly `["one","h0","h1","h2"]`.
   Scroll to the oldest row: the visible lines become `["one","h0","h1"]`.
   Deny agent access to A before starting B's permission probe.
3. B writes its own shell PID to a private fixture file, emits OSC 133 C and
   `start\n`, then waits for a release file. After release it prints `done\n`,
   emits OSC 133 D with code 7, and exits 7. Before detach, observe the exact
   prefix `ESC]133;C BEL start CR LF` and a still-running command.
   While B is running, its child starts `clanky --stdio` and presents the REAL
   owner token. Expected: owner auth returns `permission_denied`; a v1 read of
   denied A through that bridge also returns `permission_denied`. The bridge
   remains an agent connection rather than terminating in place of the read.
   Probe results go to a private fixture file, never into B's transcript; the
   token and request carrying it are never printed. Require these exact results
   before killing the first client. B itself remains agent-allowed.
4. Save a workspace with one side-by-side split containing A then B, focus B,
   active group 0. Both remain distinct v1 tab IDs. Select A's [0,3) block bytes,
   collapse it and queue a task labelled `Persist` with payload `one`. A remains
   agent-denied; B stays allowed. These choices are server state, not client files.
5. Kill the client process by its saved PID (SIGKILL). Require it to be dead
   while the same server PID is alive. Create the release file and wait for B's
   saved shell PID to disappear BEFORE attaching another client. Thus B actually
   finishes while detached; the gate does not merely wait after reconnecting.
6. A fresh client through the local stdio bridge reattaches. Require the same
   server PID and both original tab IDs. Require the exact saved split/focus,
   A's exact raw output, history and scrolled screen, selection/collapse/task,
   and denied permission. B's raw output is exactly `ESC]133;C BEL start CR LF
   done CR LF ESC]133;D;7 BEL`; its completed block is `start\ndone`, exit 7.
   Ordinary unauthenticated v1 access to A must return `permission_denied`;
   owner access can still inspect it. Neither reconnection nor inspection runs
   a new shell or recreates the original commands.
7. Close both sessions explicitly. Require the server PID to disappear within
   five seconds. Merely closing the client connection is NOT close_tab.
8. Clean mode invokes `scripts/gate-m3.sh` once; that already includes M2 and M1.
9. Planted fault: `CLANKY_GATE_FAULT=reattach scripts/gate-m4.sh` changes only
   the observed reattached A block output from `one` to `lost`. The ordinary
   exact comparison must FAIL specifically there. Run fault once, clean once;
   clean must exit 0 with final PASS. No expected value is weakened.

Socket readiness and each finite wait have a 10-second bound, detached child
completion 10 seconds, complete helper processes 30 seconds. Poll only saved
PIDs, never name patterns. Finally blocks close clients and terminate/reap only
fixture-owned server/client processes on any failure. All fixture files are
private and removed. Tests do not touch existing sessions or a real clipboard.
Build artifacts are captured under the shared build lock so another branch cannot
replace the executables between build and gate. Scratch copies are removed after.

## Ownership and lifetime

The server alone owns Engine, PTYs, terminal grids/history, command transcripts,
blocks, image data, permissions and M3 action/task state. There is one workspace
layout per server. Windows own rendering, input, clipboard and transient prompts;
they never own or shut down the server's Engine. Closing/crashing a window only
drops its connection. A new window loads the existing workspace without creating
extra shells. If there are no sessions yet, it creates the initial shell.

`clanky` connects to a stable private socket and starts `clanky --server` on demand
if the socket does not exist. `--server` is headless; `--headless --socket PATH`
remains a supported foreground server entry point for existing clients/gates.
A private instance lock prevents competing launchers from making two servers.
An existing socket is never blindly removed or replaced. Stale/incompatible
sockets produce an error instead of deleting an unknown service's endpoint.
The server is detached from the launching terminal with standard platform code.

An empty new server has a 30-second startup grace. Once it has held any session,
it exits after the last close_tab, with a short drain grace for the final reply.
Running commands and exited-but-open tabs both keep it alive. A window close is
not a server shutdown request. Explicit server termination still shuts down PTYs.
This milestone survives CLIENT loss, not server crashes, power loss or reboot;
there is no on-disk terminal database or service installation.

Use `$XDG_RUNTIME_DIR/clanky/server.sock` by default; the platform may use an
existing private `/run/user/<uid>` runtime directory when XDG_RUNTIME_DIR is absent.
Custom `--socket` paths retain the existing private-directory checks. No changes
are made to system services, machine sleep behavior or another checkout.

## Additive owner transport

v1 and v2 request/reply shapes and run semantics remain unchanged. Their ordinary
agent connections still honor per-tab permission. New owner operations use v3.
A GUI authenticates its connection with a random 256-bit owner capability from a
0600 sidecar inside the private socket directory. It is never printed or logged.
No request may gain owner privileges merely by supplying a trusted boolean.
The credential and instance lock are platform-owned, symlink-safe regular files.
The token is necessary but NOT sufficient: tab processes run under the same UID
and can read it.

Before granting owner authority, the server takes the kernel peer PID from
SO_PEERCRED and walks its parent chain. Any match to a terminal process spawned
by this server, or its process group/session, classifies the connection as AGENT
regardless of the token. Keep terminal process/session identities beyond shell
exit while descendants may survive; PID identities include start time to avoid
mistaking PID reuse for provenance. A peer descending from the server itself is
also agent-side, covering the spawn-to-registration race. Missing, inconsistent
or unprovable ancestry fails closed. Check at accept and again at authentication;
an agent classification is sticky for the connection.

OWNER requires BOTH a client outside all of this server's terminal ancestry and
a valid capability. Desktop-launched GUI clients and sshd-launched remote bridges
can satisfy both. `clanky --stdio` launched inside a tab must instead bridge as
an agent, including when its caller explicitly sends the real token. Do not
upgrade an agent stream, proxy through an outside privileged helper, or retry a
denied owner request on a privileged connection.

Honest limit: same-UID processes outside Clanky are not sandboxed. This checks
current ancestry and process-group/session membership, not arbitrary same-UID
code execution. An agent with SSH access to a host can attach as owner of that
host's server through sshd, including a local SSH login if one is available.

The v3 auth reply grants owner authority on THAT connection. Authenticated v1/v2
calls are the same operations the local UI previously performed as the owner.
Owned streams use increasing decimal request IDs to stay bounded without the
4096-request lifetime of ordinary agent connections. Idle owner connections do
not expire while a window is minimized. Partial requests retain time limits.

New v3 operations:

- hello: version 3, server_pid and owner/session capabilities.
- auth: token; a generic success/error, never an echoed credential.
- workspace_get: `{revision,groups:[{layout,focus}],active}`. Layout uses the
  existing leaf/split tree with serde's explicit variant representation.
- workspace_set: the same groups/active plus expected revision. Validate that
  every live terminal appears exactly once, focus belongs to its group, and
  tree size/depth are bounded. Conflict returns an error, not last-writer-wins.
- snapshot: tab_id, optional known generation and known image keys. Return an
  unchanged marker for an unchanged grid, otherwise cells/cursor/image placements
  plus only missing immutable image pixels. Pixel payloads are base64, deduplicated
  by server-generated keys. Never serialize Arc addresses or duplicate the same
  pixel buffer for each placement. The client hydrates the existing renderer type.
- history: tab_id, offset and limit (at most 64 rows), oldest retained row first;
  trim trailing blank cells, not content bytes. Return total and next_offset.
- scroll: tab_id and signed row delta; the terminal's own display offset persists.
  Snapshots honor it, hide the live cursor while scrolled, and do not misplace
  live-grid images over history. No second terminal parser/history implementation.
- input: tab_id and bounded base64 bytes; resize: tab_id, rows, cols.
- permission_get / permission_set: tab_id and, for set, allow_agent boolean.

Only hello/auth are available before owner authentication. Ordinary agents keep
using v1/v2; the broader agent screen/input/event API is still M5. Input requests
remain bounded to 1 MiB lines; snapshot replies have a separate 128 MiB ceiling
covering the existing 250,000-cell/64 MiB decoded-image limits. Clients reject
oversize/malformed replies, cache immutable image pixels and use generation checks
instead of retransmitting unchanged screens. No unbounded background queue.

The core automatically adds new v1/rerun terminals to the workspace and removes
closed leaves, collapsing siblings. GUI layout changes are compare-and-set;
a conflict reloads server state and reports it rather than overwriting another
window. Permissions and terminal scroll offsets are not inferred from layout.
The frontend uses a remote Client facade, including for M3 panel actions; it has
no hidden in-process Engine to lose on exit.

## SSH and stdio

`clanky attach host` launches `ssh -T -- host clanky --stdio`. The remote command
is fixed; host is a validated argument, never shell-interpolated. SSH supplies
transport authentication/encryption and its normal host-key policy. Clanky opens
no TCP socket or port. The remote program must be installed and able to find its
private runtime directory.

`clanky --stdio [--socket PATH]` ensures the local server exists and bridges
bounded JSON-line requests on stdin/stdout. Outside terminal ancestry it may
connect as owner using the capability; inside a terminal it connects as an agent
and forwards requests without elevation. It consumes any internal handshake
without exposing the token on stdout. A caller's explicit auth request still
reaches the server's ancestry check and cannot change an agent classification. It
translates request IDs when forwarding; EOF detaches without closing sessions.
GUI SSH clients and the local pipe gate use the same pipe-backed Client transport.
Read/write failures and timeouts end that client, not the server's sessions.
No unsafe replay of a possibly executed run request after a broken connection.

## Delivery

Up to five workers: core workspace/snapshots/history; API owner transport/client;
platform server lifecycle/stdio; frontend reattachment; headless gate. Ownership
boundaries are agreed before launch; lead integrates and reads every changed line.
All builds/checks use the shared lock, at most -j8 for the lead and own-crate -j2
for workers. The editor team's checkout is never touched. No new third-party crate
is planned. Fork edits, if needed, require their notices and FORK.md entries.

The first commit also makes M10 Linux 1.0, preserving its packaging scope, and
moves Windows/macOS to After 1.0 (1.x). Patch-tagged implementation precedes the
single fault/clean gate pair; v0.4.0 records the passing result. Manual window
crash/reattach, multiple-window layout conflicts and actual remote SSH remain
separate acceptance checks. Stop at the usual first real failure, disallowed
licence, v1-breaking question or approximately 610k context tokens. M5 does not
start after this gate.
