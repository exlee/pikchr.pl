//! Benchmark-only entry points. This module is absent from normal builds.

use crate::{OutputType, image::RenderBackground};

pub fn render_pikchr(source: &str) -> usize {
    crate::render::render(OutputType::Pikchr, source)
        .expect("benchmark Pikchr fixture must render")
        .len()
}

pub fn render_svgbob(source: &str) -> usize {
    crate::render::render(OutputType::Svgbob, source)
        .expect("benchmark Svgbob fixture must render")
        .len()
}

pub fn raster_svg(svg: &str) -> usize {
    crate::image::render_svg_to_image(svg, 1.0, RenderBackground::Transparent)
        .expect("benchmark SVG fixture must rasterize")
        .pixels
        .len()
}

pub fn svgbob_canvas(rows: usize, columns: usize) -> usize {
    crate::svgbob_editor::perf_canvas_workload(rows, columns)
}

pub fn dependency_fanout(size: usize) -> usize {
    crate::perf_dependency_workload(size)
}

pub fn grammar_scroll(wrap_width: f32, viewports: usize) -> usize {
    crate::help::grammar::perf_layout_workload(wrap_width, viewports)
}

pub fn eval_tcl(source: &str) -> Result<usize, String> {
    if !crate::tcl::is_tcl_loadable() {
        return Err("Tcl runtime is unavailable".to_owned());
    }
    crate::tcl::eval_tcl(source).map(|output| output.len())
}

pub fn eval_ruby(source: &str) -> Result<usize, String> {
    crate::mruby::eval_mruby(source).map(|output| output.len())
}
