//! Renders a clear colour to a PNG. `cargo run -p jkva-gfx --bin gfx-clear -- out.png [size]`

use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args
        .next()
        .unwrap_or_else(|| {
            eprintln!("usage: gfx-clear <out.png> [size]");
            std::process::exit(2);
        })
        .into();
    let size: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(256);

    if let Some(name) = jkva_gfx::device_name() {
        println!("device: {name}");
    }
    let image = match jkva_gfx::render_clear(size, size, [0.13, 0.55, 0.86, 1.0]) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("gfx-clear: {e}");
            std::process::exit(1);
        }
    };
    let file = std::fs::File::create(&out).expect("create output");
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer.write_image_data(&image.rgba).expect("png data");
    println!("wrote {} ({}x{})", out.display(), image.width, image.height);
}
