# Clanky the robot

Reference: `mockups/robot6.html`, design commit `4cada783`.

- `clanky::robot::frame` builds a deterministic frame from a theme, pixel rectangle, greeting kind and clock.
  `frame_with_measure` takes the window renderer's text measurer for configured font families.
- First meeting lasts 7000 ms; later greetings last 1200 ms. Three projected boxes use 700 px perspective.
  Panels flatten before/after their turns. The rest uses lyon paths and the existing scene text renderer.
- `Animation::sample` emits its last logical sample once. `skip` stops it without owning or consuming input.
- The window claims `<state directory>/clanky/first-meeting` with an atomic create-new operation. Later windows
  greet briefly. Disabled animation does not create the marker. The platform layer selects the state directory.
- `startup_animation: false` or `animations: false` disables startup playback. Keys, clicks and committed IME
  text dismiss the overlay before continuing through their normal handlers. The shell starts independently.
- `clanky +hello [--config PATH]` opens a separate replay window without a server or marker write. It retains the
  final frame, requests no more animation redraws, and closes on a key/click. With animations disabled it is static.

## Integration

Startup hooks are isolated in `robot_window.rs`. The current M11 integration base can choose Stone by appearance;
the M12 app must pass its selected theme to `robot::frame_with_measure` instead of that compatibility selection.
Its idle scheduler must include `robot_deadline()` only while an overlay exists. This branch still has the older
window scheduler; the app team's `m11-idle` base supplies the idle fix. No claim is made here that the old scheduler
passes the full M12 idle check.

Geist registration belongs to clanky-skins. Robot colours derive from semantic theme colours and `robot_paint`.
The tiny `CLK-1` marking is decorative reference text, not a UI control; it appears once the torso is flat.
Curves and projection are native; no screenshot or browser is used at runtime.

## Checks and review picture

`robot::tests::robot_frames` checks 7000/1200 ms stop, skipping, the opaque Stone antenna and both mid-turn samples.
`CLANKY_ROBOT_FAULT=1` makes the stop check fail. `robot::tests::startup_marker` uses a private fixture directory and
checks first/full then later/short, without touching the real marker. The final startup/PTY-input check belongs to
the app integration gate; no real desktop session was driven here.

`cargo run -p clanky --example robot-frame -- 7000 OUTPUT.rgba` writes a 400 × 400 premultiplied RGBA frame to a new
file. The lead still compares the animated picture and 2× crops with the reference; component checks do not certify
that visual acceptance.
