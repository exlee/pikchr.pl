use crate::{OutputType, SPACE_MONO_NAME};

/// Render generated source into SVG using the editor's selected output
/// language.
pub fn render(output_type: OutputType, source: &str) -> Result<String, String> {
    let svg = match output_type {
        OutputType::Pikchr => {
            pikchr_pro::pikchr::render_pikchr(pikchr_pro::types::PikchrCode::new(source))
                .map(|svg| svg.into_inner())
                .map_err(|err| err.inner_string())?
        },
        OutputType::Svgbob => svgbob::to_svg_with_settings(
            source,
            &svgbob::Settings {
                font_family: SPACE_MONO_NAME.to_string(),
                ..Default::default()
            },
        ),
    };

    Ok(inject_svg_style(&svg))
}

/// Apply the application's diagram font to either renderer's SVG output.
pub fn inject_svg_style(svg: &str) -> String {
    let mut output = svg.to_owned();
    if let Some(index) = output.find('>') {
        let style = format!(
            "<style>text,path {{ font-family: '{}'; }}</style>",
            SPACE_MONO_NAME
        );
        output.insert_str(index + 1, &style);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_pikchr_and_svgbob() {
        let pikchr = render(OutputType::Pikchr, "box").unwrap();
        assert!(pikchr.starts_with("<svg"));
        assert!(pikchr.contains("Space Mono"));

        let svgbob = render(OutputType::Svgbob, "+---+\n| A |\n+---+").unwrap();
        assert!(svgbob.starts_with("<svg"));
        assert!(svgbob.contains("Space Mono"));
    }

    #[test]
    fn reports_pikchr_errors() {
        let error = render(OutputType::Pikchr, "not valid pikchr {").unwrap_err();
        assert!(!error.is_empty());
    }
}
