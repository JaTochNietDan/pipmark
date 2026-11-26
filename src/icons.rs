//! Normalize imported SVG artwork once; runtime redraws use the managed raster.
use resvg::{tiny_skia, usvg};
use std::path::Path;

pub struct Raster {
    pub png: Vec<u8>,
    pub template: bool,
}
pub fn rasterize(bytes: &[u8]) -> Result<Raster, String> {
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("Choose an SVG smaller than 2 MiB".into());
    }
    let mut options = usvg::Options::default();
    // Load fonts only if the parser actually encounters text (including nested
    // SVG images). The database belongs to this import, never the idle app.
    let select_font = usvg::FontResolver::default_font_selector();
    options.font_resolver.select_font = Box::new(move |font, database| {
        if database.is_empty() {
            std::sync::Arc::make_mut(database).load_system_fonts();
        }
        select_font(font, database)
    });
    // Icon imports are self-contained: never resolve local or remote image paths.
    options.image_href_resolver.resolve_string = Box::new(|_, _| None);
    let tree = usvg::Tree::from_data(bytes, &options).map_err(|e| format!("Invalid SVG: {e}"))?;
    let mut pixmap = tiny_skia::Pixmap::new(64, 64).ok_or("Cannot allocate icon raster")?;
    let scale = (64.0 / tree.size().width()).min(64.0 / tree.size().height());
    let x = (64.0 - tree.size().width() * scale) / 2.0;
    let y = (64.0 - tree.size().height() * scale) / 2.0;
    resvg::render(
        &tree,
        tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, x, y),
        &mut pixmap.as_mut(),
    );
    let mut first: Option<[u8; 3]> = None;
    let mut template = true;
    for pixel in pixmap.pixels().iter().filter(|p| p.alpha() > 32) {
        let color = pixel.demultiply();
        let rgb = [color.red(), color.green(), color.blue()];
        if let Some(reference) = first {
            if rgb.iter().zip(reference).any(|(a, b)| a.abs_diff(b) > 8) {
                template = false;
            }
        } else {
            first = Some(rgb);
        }
    }
    if first.is_none() {
        return Err("The SVG has no visible artwork".into());
    }
    let png = pixmap.encode_png().map_err(|e| e.to_string())?;
    Ok(Raster { png, template })
}
pub fn import(path: &Path, directory: &Path) -> Result<String, String> {
    if std::fs::metadata(path).map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 {
        return Err("Choose an SVG smaller than 2 MiB".into());
    }
    let raster = rasterize(&std::fs::read(path).map_err(|e| e.to_string())?)?;
    std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let id = objc2_foundation::NSUUID::UUID().UUIDString().to_string();
    let destination = directory.join(format!(
        "{}-{id}.png",
        if raster.template { "template" } else { "color" }
    ));
    std::fs::write(&destination, raster.png).map_err(|e| e.to_string())?;
    Ok(destination.to_string_lossy().into_owned())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn renders_svg_text_using_installed_fonts() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><text x="4" y="48" font-family="Helvetica" font-size="48">A</text></svg>"#;
        let raster = rasterize(svg).unwrap();
        assert!(raster.template);
        let pixels = tiny_skia::Pixmap::decode_png(&raster.png).unwrap();
        assert!(pixels.pixels().iter().filter(|p| p.alpha() > 32).count() > 100);
    }
    #[test]
    fn renders_embedded_png_with_original_colors() {
        let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32"><image width="64" height="32" href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAYAAAD0In+KAAAADklEQVR4nGP4z8AAQv8BD/kD/YURmXYAAAAASUVORK5CYII="/></svg>"#;
        let raster = rasterize(svg).unwrap();
        assert!(!raster.template);
        let pixels = tiny_skia::Pixmap::decode_png(&raster.png).unwrap();
        let left = pixels.pixel(8, 32).unwrap().demultiply();
        let right = pixels.pixel(56, 32).unwrap().demultiply();
        assert!(left.red() > 200 && left.blue() < 30);
        assert!(right.blue() > 200 && right.red() < 30);
    }
    #[test]
    fn detects_monochrome_and_preserves_multiple_colors() {
        let mono = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><circle cx="12" cy="12" r="10" fill="red"/></svg>"#;
        assert!(rasterize(mono).unwrap().template);
        let color = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect width="12" height="24" fill="red"/><rect x="12" width="12" height="24" fill="blue"/></svg>"#;
        assert!(!rasterize(color).unwrap().template);
    }
    #[test]
    fn rejects_invalid_empty_and_external_only_icons() {
        assert!(rasterize(b"not svg").is_err());
        assert!(
            rasterize(br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"/>"#)
                .is_err()
        );
        assert!(rasterize(br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><image href="/etc/passwd" width="24" height="24"/></svg>"#).is_err());
    }
    #[test]
    fn does_not_load_valid_images_from_external_paths() {
        let file = std::env::temp_dir().join(format!(
            "badgeify-icon-{}.png",
            objc2_foundation::NSUUID::UUID().UUIDString()
        ));
        let artwork = rasterize(br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect width="24" height="24"/></svg>"#).unwrap();
        std::fs::write(&file, artwork.png).unwrap();
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><image href="{}" width="64" height="64"/></svg>"#,
            file.display()
        );
        let result = rasterize(svg.as_bytes());
        std::fs::remove_file(file).unwrap();
        assert!(result.is_err());
    }
}
