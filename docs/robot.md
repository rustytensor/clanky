# Clanky the robot

The first window on a machine gets a **7-second first meeting**: Clanky rolls in,
notices you, turns, lights up and waves. Later windows get a **1.2-second greeting**.
The robot appears over the first pane while the shell starts underneath.

**Any key or click skips it. That same input still reaches the shell or control.**
Skipping does not discard what you typed. The animation stops requesting frames when it ends.

## Turn it off

Disable only startup greetings in your [config file](config.md):

```json
{"startup_animation":false}
```

`{"animations":false}` turns off startup greetings and other motion. With animations off,
explicit replay shows a static final pose.

## Replay it

```sh
clanky +hello
clanky +hello --config /absolute/path/to/config.json
```

Replay opens its own window, without a shell or server. It leaves the final picture up;
a key, click or the close button closes it. Replay does not change the first-run marker.

## Where “already met” is saved

The local marker is `$XDG_STATE_HOME/clanky/first-meeting`, or
`$HOME/.local/state/clanky/first-meeting` when XDG_STATE_HOME is empty or unset.
It is shared by your windows on that machine, not stored on an attached SSH host.
Disabled startup animation does not create it.

The robot takes its paint and light colours from the theme: `robot_paint` and `agent`.
See [themes](themes.md) for theme selection's current integration limits.
