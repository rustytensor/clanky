//! Render one deterministic 400x400 robot frame as raw premultiplied RGBA.
fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let time = args
        .next()
        .ok_or("usage: robot-frame MILLISECONDS OUTPUT.rgba")?
        .parse::<u64>()
        .map_err(|_| "invalid milliseconds")?;
    let path = args.next().ok_or("missing output path")?;
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let theme = clanky::themes::builtin("stone-dark")?;
    let frame = clanky::robot::frame(
        &theme,
        [0., 0., 400., 400.],
        clanky::robot::Kind::FirstMeeting,
        time,
    );
    let scene = clanky_render::Scene {
        clear: theme.colors.sidebar,
        layers: frame.layers,
    };
    let pixels = clanky_render::offscreen_scene(&scene, 400, 400)?;
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .and_then(|mut file| file.write_all(&pixels))
        .map_err(|e| e.to_string())?;
    println!(
        "400x400 RGBA; antenna {:.3},{:.3}",
        frame.antenna[0], frame.antenna[1]
    );
    Ok(())
}
