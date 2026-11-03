fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() && args != ["--case", "text"] {
        eprintln!("usage: clanky-text-gate [--case text]");
        std::process::exit(2);
    }
    match clanky_render::offscreen_text_samples() {
        Ok((red, black)) => println!(
            "{{\"red\":[{},{},{},{}],\"black\":[{},{},{},{}]}}",
            red[0], red[1], red[2], red[3], black[0], black[1], black[2], black[3]
        ),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
