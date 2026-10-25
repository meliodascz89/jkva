//! Renders the M0 triangle to a PNG. `cargo run -p jkva-gfx --bin gfx-triangle -- out.png [size]`

use std::path::PathBuf;

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args
        .next()
        .unwrap_or_else(|| {
            eprintln!("usage: gfx-triangle <out.png> [size]");
            std::process::exit(2);
        })
        .into();
    let size: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(256);

    let image = match jkva_gfx::render_triangle(size, size) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("gfx-triangle: {e}");
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
