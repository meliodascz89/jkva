//! Renders a small display list to a PNG. `cargo run -p jkva-gfx --bin gfx-dl -- out.png`

use jkva_paint::{DisplayItem, DisplayList, Point, Rect};
use std::path::PathBuf;

fn main() {
    let out: PathBuf = std::env::args()
        .nth(1)
        .unwrap_or_else(|| {
            eprintln!("usage: gfx-dl <out.png>");
            std::process::exit(2);
        })
        .into();

    let mut list = DisplayList::new();
    // A fake page: header, sidebar, content card and an overlap that proves order.
    list.push(DisplayItem::Rect {
        rect: Rect::new(0.0, 0.0, 640.0, 64.0),
        color: [24, 44, 86, 255],
    });
    list.push(DisplayItem::Rect {
        rect: Rect::new(0.0, 64.0, 160.0, 416.0),
        color: [232, 236, 242, 255],
    });
    list.push(DisplayItem::Rect {
        rect: Rect::new(192.0, 96.0, 416.0, 160.0),
        color: [242, 102, 38, 255],
    });
    list.push(DisplayItem::Rect {
        rect: Rect::new(320.0, 160.0, 160.0, 96.0),
        color: [33, 140, 219, 255],
    });
    let _ = Point::ZERO;

    let image = match jkva_gfx::render_display_list(640, 480, &list) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("gfx-dl: {e}");
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
