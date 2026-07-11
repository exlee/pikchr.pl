use std::{
    hint::black_box,
    time::{Duration, Instant},
};

const PIKCHR: &str = "box \"Source\"; arrow; box \"Preview\"; arrow; circle \"Done\"";
const SVGBOB: &str = "+----------+     +-----------+\n|  Source  | --> |  Preview  |\n+----------+     +-----------+";
const SVG: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="360"><rect width="640" height="360" fill="white"/><path d="M20 180 H620" stroke="black"/><text x="220" y="160" font-family="Space Mono" font-size="24">DiagramIDE</text></svg>"#;

#[derive(Clone, Copy)]
struct Stats {
    median: Duration,
    p95: Duration,
    coefficient_of_variation: f64,
}

fn measure(mut workload: impl FnMut() -> usize, samples: usize) -> (Stats, usize) {
    for _ in 0..3 {
        black_box(workload());
    }
    let mut elapsed = Vec::with_capacity(samples);
    let mut checksum = 0usize;
    for _ in 0..samples {
        let started = Instant::now();
        checksum = checksum.wrapping_add(black_box(workload()));
        elapsed.push(started.elapsed());
    }
    elapsed.sort_unstable();
    let nanos = elapsed.iter().map(Duration::as_nanos).map(|n| n as f64);
    let mean = nanos.clone().sum::<f64>() / samples as f64;
    let variance = nanos.map(|n| (n - mean).powi(2)).sum::<f64>() / samples as f64;
    let p95_index = ((samples as f64 * 0.95).ceil() as usize)
        .saturating_sub(1)
        .min(samples - 1);
    (
        Stats {
            median: elapsed[samples / 2],
            p95: elapsed[p95_index],
            coefficient_of_variation: variance.sqrt() / mean,
        },
        checksum,
    )
}

fn report(name: &str, samples: usize, workload: impl FnMut() -> usize) {
    let (stats, checksum) = measure(workload, samples);
    println!(
        "{name:24} median={:?} p95={:?} cv={:.3} checksum={checksum}",
        stats.median, stats.p95, stats.coefficient_of_variation
    );
}

fn main() {
    let samples = std::env::var("DIAGRAMIDE_BENCH_SAMPLES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(30usize)
        .max(5);
    println!(
        "DiagramIDE interaction workloads: samples={samples}, profile={}, arch={}, os={}",
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::env::consts::ARCH,
        std::env::consts::OS,
    );
    report("pikchr_edit_preview", samples, || {
        diagramide::perf_support::render_pikchr(black_box(PIKCHR))
    });
    report("svgbob_edit_preview", samples, || {
        diagramide::perf_support::render_svgbob(black_box(SVGBOB))
    });
    report("svgbob_canvas_120x240", samples, || {
        diagramide::perf_support::svgbob_canvas(120, 240)
    });
    report("dependency_fanout_160", samples, || {
        diagramide::perf_support::dependency_fanout(160)
    });
    report("grammar_scroll_120", samples, || {
        diagramide::perf_support::grammar_scroll(900.0, 120)
    });
    report("svg_raster_640x360", samples, || {
        diagramide::perf_support::raster_svg(black_box(SVG))
    });

    match diagramide::perf_support::eval_tcl("return {box; arrow; box}") {
        Ok(_) => report("tcl_edit_preview", samples, || {
            diagramide::perf_support::eval_tcl("return {box; arrow; box}").unwrap_or_default()
        }),
        Err(error) => println!("tcl_edit_preview         skipped={error}"),
    }
    match diagramide::perf_support::eval_ruby("print 'box; arrow; box'") {
        Ok(_) => report("ruby_edit_preview", samples, || {
            diagramide::perf_support::eval_ruby("print 'box; arrow; box'").unwrap_or_default()
        }),
        Err(error) => println!("ruby_edit_preview        skipped={error}"),
    }
}
