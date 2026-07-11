use std::{
    collections::HashMap,
    error::Error,
    fs::File,
    io::{BufWriter, Write},
    sync::{Arc, OnceLock},
};

use eframe::egui::{self, ColorImage};
use resvg::usvg::{self, fontdb};

use crate::{NOTO_SANS_BYTES, NOTO_SANS_SYMBOLS2_BYTES, SPACE_MONO_BYTES};

const RENDER_WIDTH: f32 = 512.0;
const RENDER_LIMIT: f32 = 8192.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RenderBackground {
    Transparent,
    Color(egui::Color32),
}

pub fn write_png(file: String, image: ColorImage) -> Result<(), Box<dyn Error>> {
    let width = image.width() as u32;
    let height = image.height() as u32;

    let file = File::create(file)?;
    let w = &mut BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);

    let mut writer = encoder.write_header()?;
    let pixels: &[u8] = bytemuck::cast_slice(&image.pixels);
    writer.write_image_data(pixels)?;
    Ok(())
}
pub fn write_svg(file: String, svg: String) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(file)?;
    let bytes = svg.into_bytes();
    file.write_all(&bytes)?;
    Ok(())
}

pub fn render_svg_to_image(
    svg_content: &str,
    scale: f32,
    background: RenderBackground,
) -> Option<egui::ColorImage> {
    let xml_opt = usvg::Options {
        fontdb: font_database(),
        ..Default::default()
    };
    let svg_content = sanitize_svg_for_usvg(svg_content);
    let tree: usvg::Tree = usvg::Tree::from_str(&svg_content, &xml_opt).ok()?;
    let size = tree.size();
    let (w, h) = (size.width(), size.height());

    // 1. Determine base multiplier to target RENDER_WIDTH
    let base_mult = RENDER_WIDTH / w;

    // 2. Calculate final dimensions with scale, clamped to hardware limit
    let final_width_f = (w * base_mult * scale).min(RENDER_LIMIT);
    let final_height_f = (h * base_mult * scale).min(RENDER_LIMIT);

    // 3. Recalculate effective scale to ensure transform matches clamped pixel dimensions
    let effective_scale_x = final_width_f / w;
    let effective_scale_y = final_height_f / h;

    let width = final_width_f.ceil() as u32;
    let height = final_height_f.ceil() as u32;

    if width == 0 || height == 0 {
        return None;
    }

    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    if let RenderBackground::Color(color) = background {
        let [r, g, b, a] = color.to_srgba_unmultiplied();
        pixmap.fill(resvg::tiny_skia::Color::from_rgba8(r, g, b, a));
    }

    // Use the effective scale to ensure the SVG content fills the clamped pixmap exactly
    let transform = tiny_skia::Transform::from_scale(effective_scale_x, effective_scale_y);

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Some(egui::ColorImage::from_rgba_premultiplied(
        [width as usize, height as usize],
        pixmap.data(),
    ))
}

fn font_database() -> Arc<fontdb::Database> {
    static DATABASE: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DATABASE
        .get_or_init(|| {
            let mut database = fontdb::Database::new();
            database.load_font_data(SPACE_MONO_BYTES.to_vec());
            database.load_font_data(NOTO_SANS_BYTES.to_vec());
            database.load_font_data(NOTO_SANS_SYMBOLS2_BYTES.to_vec());
            Arc::new(database)
        })
        .clone()
}

pub fn sanitize_svg_for_usvg(svg: &str) -> std::borrow::Cow<'_, str> {
    let needs_font_size_fix = svg.contains("font-size") && svg.contains("initial");
    let needs_entity_fix = svg.contains('&');
    if !needs_font_size_fix && !needs_entity_fix {
        return std::borrow::Cow::Borrowed(svg);
    }

    let mut sanitized = if needs_font_size_fix {
        svg.replace("font-size=\"initial\"", "")
            .replace("font-size:initial", "")
            .replace("font-size: initial", "")
    } else {
        svg.to_owned()
    };

    if needs_entity_fix {
        sanitized = decode_named_html_entities_for_xml(&sanitized);
    }

    if sanitized == svg {
        std::borrow::Cow::Borrowed(svg)
    } else {
        std::borrow::Cow::Owned(sanitized)
    }
}

fn decode_named_html_entities_for_xml(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        if let Some(semi) = after.find(';') {
            let entity = &rest[amp..amp + semi + 2];
            if let Some(value) = html_entities().get(entity) {
                out.push_str(xml_safe_text(value));
                rest = &after[semi + 1..];
                continue;
            }
        }

        out.push('&');
        rest = after;
    }
    out.push_str(rest);
    out
}

fn html_entities() -> &'static HashMap<String, String> {
    #[derive(serde::Deserialize)]
    struct Entity {
        characters: String,
    }

    static ENTITIES: OnceLock<HashMap<String, String>> = OnceLock::new();
    ENTITIES.get_or_init(|| {
        let entities: HashMap<String, Entity> =
            serde_json::from_str(include_str!("../assets/html_entities.json"))
                .expect("WHATWG HTML entities table must be valid JSON");
        entities
            .into_iter()
            .filter_map(|(name, entity)| name.ends_with(';').then_some((name, entity.characters)))
            .collect()
    })
}

fn xml_safe_text(text: &str) -> &str {
    match text {
        "&" => "&amp;",
        "<" => "&lt;",
        ">" => "&gt;",
        _ => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
        <rect x="5" y="5" width="1" height="1" fill="#ff0000"/>
    </svg>"##;

    #[test]
    fn rasterizer_uses_explicit_background_color() {
        let background = egui::Color32::from_rgb(12, 34, 56);
        let image = render_svg_to_image(SVG, 1.0, RenderBackground::Color(background)).unwrap();
        assert_eq!(image.pixels[0], background);
    }

    #[test]
    fn rasterizer_keeps_transparent_background() {
        let image = render_svg_to_image(SVG, 1.0, RenderBackground::Transparent).unwrap();
        assert_eq!(image.pixels[0].a(), 0);
    }

    #[test]
    fn sanitizer_removes_initial_font_size_values() {
        let svg = r#"<svg><text font-size="initial" style="font-size: initial">x</text></svg>"#;
        let sanitized = sanitize_svg_for_usvg(svg);
        assert!(!sanitized.contains("font-size=\"initial\""));
        assert!(!sanitized.contains("font-size: initial"));
    }

    #[test]
    fn sanitizer_decodes_html_entities_that_xml_does_not_define() {
        let svg = r#"<svg><text>30&deg; &DoubleRightArrow; &amp; &#9654;</text></svg>"#;
        let sanitized = sanitize_svg_for_usvg(svg);

        assert!(sanitized.contains("30\u{00B0}"));
        assert!(sanitized.contains("\u{21D2}"));
        assert!(sanitized.contains("&amp;"));
        assert!(sanitized.contains("&#9654;"));
    }

    #[test]
    fn sanitizer_keeps_decoded_xml_syntax_safe_for_xml_parser() {
        let svg = r#"<svg><text>&AMP; &LT; &GT;</text></svg>"#;
        let sanitized = sanitize_svg_for_usvg(svg);

        assert!(sanitized.contains("&amp; &lt; &gt;"));
    }

    #[test]
    fn rasterizer_accepts_svg_with_pikchr_html_entity_text() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20">
            <text x="1" y="12">30&deg;</text>
        </svg>"##;

        let image = render_svg_to_image(svg, 1.0, RenderBackground::Transparent)
            .expect("html entity text should be sanitized before usvg parses it");
        assert!(image.width() > 0);
        assert!(image.height() > 0);
    }

    #[test]
    fn rasterizer_uses_symbol_fallback_for_missing_space_mono_glyphs() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="24">
            <text x="2" y="18" font-family="Space Mono" font-size="20">λ</text>
        </svg>"##;

        let image = render_svg_to_image(svg, 1.0, RenderBackground::Transparent)
            .expect("symbol fallback should let usvg lay out lambda text");
        assert!(
            image.pixels.iter().any(|pixel| pixel.a() > 0),
            "lambda text did not draw any visible pixels"
        );
    }
}
