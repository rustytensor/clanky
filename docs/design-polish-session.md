# Session polish: notifications and layout restoration

## Behavior

`notify_after_seconds` is a nonnegative integer, default 10; 0 disables
notifications. Time each OSC 133 C..D block with a monotonic clock in the
terminal engine, including blocks that start and finish between window frames.
Only complete blocks at or above the threshold notify, once per window while it
is attached, if the block's terminal was not the focused pane at its OSC D
marker. An owner connection publishes focus changes to the server immediately;
the terminal engine records the server's current focused terminal on each D.
The owner connection's focus claim is cleared on disconnect, and another
connection cannot clear it. The UI does not decide from focus at poll time. Do not replay retained completions when attaching. A missing
OSC exit code is displayed as unknown. Use `notify-send` when available through
the platform boundary; a missing helper or session bus must not stop editing.
The notification is `Clanky` / `Command finished (exit 7)` for exit 7. Do not
put command text or output in desktop notifications.

`restore_layout` is boolean, default true. For local windows, save the current
layout, focus, each terminal's current shell working directory, and cell size
as private JSON under the config directory, atomically on relevant changes and
when directories change. Do not store commands, environment, terminal text,
tokens or history. On a new server only, validate the entire file and restore
fresh shells in the saved directories. Never execute a saved command. A running
server's workspace wins; competing windows must not restore twice. Invalid,
unavailable or unsafe state fails without overwriting an existing workspace.
Remote sessions must not silently restore local state into a remote server.
Saved directories may contain private names; use owner-only permissions and do
not follow an attacker-supplied symlink for the state file. Close or resize must
not save a partial layout over a newer one.

## Gate: `scripts/gate-polish-session.sh`

Before implementation, the fixed expectations are:

1. Start a private session `dbus-daemon` and fake freedesktop Notifications
   receiver. Exercise three real OSC 133 blocks: an unfocused 10.2-second block
   ending in 7, a focused 10.2-second block ending in 9, and an unfocused short block. At a
   threshold of 10 seconds the receiver must get exactly one notification:
   app `Clanky`, summary `Clanky`, body `Command finished (exit 7)`. A zero
   threshold produces none. All processes belong to the gate and are reaped.
2. Focus is on the first terminal at both long blocks' D markers, then changes
   to the second before polling. The first must not notify and the second must
   notify once, despite its later focus. The gate writes a layout fixture with a
   two-pane column split, focused second pane, two distinct existing temporary
   directories, and terminal sizes 24x40 and 30x60. The fixture file is 0600
   inside an owned 0755 configuration directory. Start the real clanky binary
   in an offscreen window-startup mode with no existing server; observe the new
   workspace, shell directories and sizes. Start a second offscreen window on
   the same server: no additional terminals or replaced workspace. Both use
   the same startup and restore path as a normal graphical window.
3. `CLANKY_GATE_FAULT=polish` changes the observed exit code from 7 to 8 at the
   ordinary exact notification comparison; the gate must print FAIL there. No
   expected values are changed. The clean run prints PASS.
4. Run `scripts/gate-m9.sh` once in the clean gate; it includes the M7 core gate
   once. All cargo builds are under the shared lock with at most eight jobs.

The integration helper must use the same notification and restoration routines
as the window and the real terminal engine/server, not a mock of their behavior.
No real desktop notification daemon or existing session is contacted. The gate
uses a private bus, private runtime/config directories and gate-owned shells.
