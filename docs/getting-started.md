# Getting started

Start `clanky` on Linux with a graphical display. Type a command in the shell.
Click the tab bar to switch tabs and click a pane to focus it. Use
[shortcuts](shortcuts.md) to open tabs, split right or below, close a pane,
adjust zoom, and grant or revoke agent access to the focused terminal.

## Images

Clanky supports the Kitty graphics protocol's direct-transfer subset for PNG,
RGB and RGBA. Image display is clipped to the pane. File and shared-memory
transport, cropping, compression controls, animation and z-order are not
supported. See the [image design](design-m2.md).

## Command blocks and palette

A shell that emits OSC 133 markers lets the sidebar show command blocks. Select
a block and use Copy, Collapse, Rerun or Send. Copy needs `wl-copy` on Wayland
or `xclip` on X11. Rerun asks for explicit argv and an absolute working directory;
it opens a new terminal. Send queues selected bytes as a task; it does not start
an agent. Large or truncated blocks need an explicit byte range through the API.
Ctrl+Shift+P opens the palette; arrows select an action and Enter runs it.

## Reattach

Closing the window leaves the server and its sessions running. Start `clanky`
again to reconnect. Explicitly closing the last session ends the server. Sessions
survive a client closing, not a server crash or reboot. `clanky attach HOST` uses
SSH and requires Clanky and working key-based SSH on the remote host.

## Coming in 1.0

The graphical editor panel and agent-change review are not available yet. The
editing core exists but is not connected to the window. Agent output secret
masking, the compose box, and the additional M5 agent features are also not yet
integrated. **Agent reads are not protected by secret masking.**
