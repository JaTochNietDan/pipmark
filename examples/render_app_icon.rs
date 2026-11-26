//! Deterministic iconset generation from the repository's vector artwork.
use std::path::Path;
fn main() {
    let directory = std::env::args().nth(1).expect("iconset output directory");
    let directory = Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let tree = resvg::usvg::Tree::from_data(
        include_bytes!("../assets/app-icon.svg"),
        &Default::default(),
    )
    .unwrap();
    for points in [16, 32, 128, 256, 512] {
        for scale in [1, 2] {
            let pixels = points * scale;
            let mut pixmap = resvg::tiny_skia::Pixmap::new(pixels, pixels).unwrap();
            resvg::render(
                &tree,
                resvg::tiny_skia::Transform::from_scale(
                    pixels as f32 / 1024.0,
                    pixels as f32 / 1024.0,
                ),
                &mut pixmap.as_mut(),
            );
            let suffix = if scale == 2 { "@2x" } else { "" };
            pixmap
                .save_png(directory.join(format!("icon_{points}x{points}{suffix}.png")))
                .unwrap();
        }
    }
}
