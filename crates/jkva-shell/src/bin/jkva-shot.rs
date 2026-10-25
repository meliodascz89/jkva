//! Renders an HTML file to a PNG. `cargo run -p jkva-shell --bin jkva-shot -- page.html out.png [w] [h]`

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: jkva-shot <input.html> <output.png> [width] [height]");
        std::process::exit(2);
    }
    let input = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[1]);
    let width: u32 = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(800);
    let height: u32 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(600);

    let html = match std::fs::read_to_string(&input) {
        Ok(html) => html,
        Err(e) => {
            eprintln!("jkva-shot: cannot read {}: {e}", input.display());
            std::process::exit(1);
        }
    };
    let result = match jkva_shell::render_page(&html, width, height) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("jkva-shot: {e}");
            std::process::exit(1);
        }
    };
    for warning in &result.warnings {
        eprintln!("jkva-shot: warning: {warning}");
    }
    let file = std::fs::File::create(&output).expect("create output");
    let mut encoder = png::Encoder::new(
        std::io::BufWriter::new(file),
        result.image.width,
        result.image.height,
    );
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("png header");
    writer
        .write_image_data(&result.image.rgba)
        .expect("png data");
    println!(
        "wrote {} ({}x{}, {} boxes)",
        output.display(),
        result.image.width,
        result.image.height,
        result.layout.boxes.len()
    );
}
