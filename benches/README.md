# DiagramIDE interaction workloads

Run the optimized workload suite with:

```sh
cargo bench -p diagramide --bench interaction --features perf-workloads
```

`DIAGRAMIDE_BENCH_SAMPLES` changes the default 30 measured samples. The runner
performs three warm-up iterations and reports median, p95, coefficient of
variation, and a checksum for each fixture. Compare runs on the same machine,
toolchain, power mode, and checkout. A result is actionable only when it beats
the greater of 5% or twice the baseline coefficient of variation without a
material regression in another workload.

The suite covers generated-source rendering, a 120x240 Svgbob canvas edit,
dependency overlay fan-out, 120 grammar viewports, SVG rasterization, and the
locally available Tcl and Ruby evaluators. Use `--features profile` and Tracy
for frame scheduling, message queues, GPU texture installation, and multi-window
interaction; those paths require the real application event loop and are not
represented by a microbenchmark.

## Initial baseline

Captured on 2026-07-11 with macOS 26.3.1, Apple arm64, rustc/cargo 1.93.0, and
the release benchmark profile. The first run compiled the release graph before
measurement. Tcl was unavailable in the benchmark process.

| Workload | Median | p95 | CV |
| --- | ---: | ---: | ---: |
| Pikchr edit preview | 11.459 us | 15.000 us | 0.247 |
| Svgbob edit preview | 146.250 us | 168.208 us | 0.057 |
| Svgbob canvas 120x240 | 1.533 ms | 1.573 ms | 0.013 |
| Dependency fan-out 160 | 134.291 us | 144.500 us | 0.031 |
| Grammar scroll 120 | 181.459 us | 186.042 us | 0.008 |
| SVG raster 640x360 | 212.250 us | 223.500 us | 0.026 |
| Tcl edit preview | 388.916 us | 416.667 us | 0.035 |
| Ruby edit preview | 16.667 us | 26.458 us | 0.272 |
