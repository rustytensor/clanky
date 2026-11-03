use std::{fs::File, io::Read};

fn observe() -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || args[0] != "--input" {
        return Err("usage: clanky-image-gate --input FILE".into());
    }
    const LIMIT: usize = 16 * 1024 * 1024;
    let mut bytes = Vec::new();
    File::open(&args[1])?
        .take((LIMIT + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > LIMIT {
        return Err("input exceeds 16 MiB".into());
    }
    let whole = clanky_core::snapshot_from_chunks(24, 80, &[bytes.as_slice()])?;
    let whole = clanky_render::offscreen_image_samples(&whole)?;
    let chunks: Vec<&[u8]> = bytes.chunks(2).collect();
    let chunked = clanky_core::snapshot_from_chunks(24, 80, &chunks)?;
    let chunked = clanky_render::offscreen_image_samples(&chunked)?;
    Ok(serde_json::json!({"whole": whole, "chunked": chunked}))
}

fn main() {
    match observe() {
        Ok(value) => println!("{value}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
