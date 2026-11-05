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

Startup hooks are isolated in `robot_window.rs`. The app passes the same selected M12 theme used by its calm
scene, including `robot_paint`; there is no Stone fallback in startup. `Startup` exposes one absolute end deadline
to `FrameClock`. The deadline frame clears the overlay and removes that deadline. Key/click skip invalidates the
frame clock before normal event routing continues. Layout and hit rectangles are unchanged.

Geist registration belongs to clanky-skins. Robot colours derive from semantic theme colours and `robot_paint`.
The tiny `CLK-1` marking is decorative reference text, not a UI control; it appears once the torso is flat.
Curves and projection are native; no screenshot or browser is used at runtime.

## Checks and review picture

`robot::tests::robot_frames` checks 7000/1200 ms stop, skipping, the opaque Stone antenna and both mid-turn samples.
`CLANKY_ROBOT_FAULT=1` makes the stop check fail. `robot::tests::startup_marker` uses a private fixture directory and
checks first/full then later/short, without touching the real marker.

`scripts/check-robot-startup.sh` is the focused integration check (shared build lock, eight workers, software Vulkan).
Its clock-driven check expects 439 full-scene frames, 76 short-greeting frames, or 20 frames through a skip at 300 ms,
then zero. It also checks a literal `robot_paint` override in rendered pixels. `CLANKY_ROBOT_STARTUP_FAULT=clock`
plants endless redraws and must fail. The real-window portion runs under a private Xvfb display/home/server: full
then short startup, two seconds with zero frames after each, exact `x` arriving at a real PTY after skip, and a
click both skipping the robot and opening one New tab. No desktop session or real marker is touched. Optional
`CLANKY_FRAME_TRACE` output records startup/sample/end/skip clocks and frame counts, never input text.
The full M12 gate and native compositor review remain the lead's.

`cargo run -p clanky --example robot-frame -- 7000 OUTPUT.rgba` writes a 400 × 400 premultiplied RGBA frame to a new
file. The lead still compares the animated picture and 2× crops with the reference; component checks do not certify
that visual acceptance.
