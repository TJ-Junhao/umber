# Embedded PNG measurements

WSL, Linux x86_64, Intel Core Ultra 9 290HX Plus, Rust 1.97.1, `png` 0.18.1; 2026-10-18. The compression run is pinned to CPU 0. Times are medians of five encodings after one warm-up, in milliseconds, including preparation of the RGBA byte buffer. Input generation, decoding and assertions are outside the timer. The machine was not otherwise idle.

## Corpus

- `heatmap_1000`: the exact SplitMix64 input, theme and mapping from `heatmap_1000x1000_svg`.
- `heatmap_100`: the same construction at 100 × 100.
- `mesh_100`, `mesh_1000`: a smooth `sin(x) * cos(y) + 0.25 * x` field on evenly spaced edges from -4 to 4, rendered through the mesh API.
- `hist2d_100`, `hist2d_500`: 1,000,000 deterministic Box-Muller normal points through the histogram API, at 100 × 100 and 500 × 500 bins; over a third of the cells are transparent.
- `diverging_1000`: a 1,000 × 1,000 heatmap with values in [-1, 1), centred at zero.
- `noise_100`, `noise_1000`: independent random RGBA channels, including hidden RGB in fully transparent pixels, at the two sizes.

## Selected rule

Encode with **level 1 + NoFilter**. If that PNG occupies at most **one third of the raw RGBA buffer**, also encode with **level 3 + NoFilter**, reusing that buffer. Keep the shorter PNG, preferring level 1 on equal lengths. No sampling, clocks or random state enter the choice.

The selection criterion is at least 3× faster on the benchmark heatmap, no corpus image more than 10% larger than level 6 + Adaptive, and every corpus image faster to encode. The measured rule satisfies all three: 6.25× on the benchmark, a maximum size increase of 0.090%, and speedups from 2.23× to 6.25× across the corpus. These are corpus bounds, not a claim of a universal compression-size guarantee.

All 144 image/candidate combinations decode to the input RGBA bytes. Every timed result is also byte-identical to its warm-up. The normal test independently checks the selected setting, previous-vs-new decoded pixels, repeated-encoding determinism and the 10% size bound.

| Image | Before ms | Selected ms | Speedup | Before bytes | Selected bytes | Size change |
|---|---:|---:|---:|---:|---:|---:|
| heatmap_1000 | 102.823 | 16.447 | 6.25× | 3,307,504 | 1,871,353 | -43.421% |
| heatmap_100 | 0.793 | 0.186 | 4.26× | 33,270 | 19,690 | -40.818% |
| mesh_100 | 1.588 | 0.380 | 4.18× | 8,440 | 8,254 | -2.204% |
| mesh_1000 | 53.657 | 18.452 | 2.91× | 254,001 | 219,972 | -13.397% |
| hist2d_100 | 0.735 | 0.329 | 2.23× | 7,042 | 6,868 | -2.471% |
| hist2d_500 | 11.941 | 4.514 | 2.65× | 116,438 | 77,856 | -33.135% |
| diverging_1000 | 90.900 | 20.162 | 4.51× | 3,239,013 | 2,169,721 | -33.013% |
| noise_100 | 0.439 | 0.150 | 2.93× | 40,173 | 40,209 | +0.090% |
| noise_1000 | 61.609 | 13.908 | 4.43× | 4,001,698 | 4,004,767 | +0.077% |

## All candidates

Each entry is **milliseconds / PNG bytes**. `Adaptive` is the PNG encoder’s adaptive filter; the other filters are fixed for every row. Combinations run both full encodings and keep the smaller file.

### Deflate level 1

| Image | NoFilter | Sub | Up | Adaptive |
|---|---:|---:|---:|---:|
| heatmap_1000 | 18.059 / 1,871,353 | 20.629 / 3,286,981 | 20.762 / 3,286,607 | 18.531 / 3,323,523 |
| heatmap_100 | 0.193 / 19,690 | 0.184 / 33,145 | 0.181 / 33,110 | 0.181 / 33,345 |
| mesh_100 | 0.131 / 12,778 | 0.144 / 10,771 | 0.156 / 12,747 | 0.169 / 12,715 |
| mesh_1000 | 6.611 / 640,551 | 6.968 / 663,592 | 5.496 / 503,303 | 6.214 / 490,869 |
| hist2d_100 | 0.106 / 8,626 | 0.145 / 9,607 | 0.212 / 9,592 | 0.160 / 9,465 |
| hist2d_500 | 1.085 / 93,722 | 1.681 / 149,153 | 1.709 / 157,395 | 1.851 / 152,985 |
| diverging_1000 | 23.637 / 2,169,721 | 18.525 / 3,306,829 | 21.669 / 3,305,473 | 22.731 / 3,325,993 |
| noise_100 | 0.153 / 40,209 | 0.155 / 40,211 | 0.153 / 40,212 | 0.162 / 40,211 |
| noise_1000 | 14.683 / 4,004,767 | 14.190 / 4,004,791 | 14.153 / 4,004,794 | 18.144 / 4,004,780 |

### Deflate level 3

| Image | NoFilter | Sub | Up | Adaptive |
|---|---:|---:|---:|---:|
| heatmap_1000 | 62.597 / 1,801,139 | 90.089 / 3,171,455 | 95.278 / 3,177,807 | 95.987 / 3,311,677 |
| heatmap_100 | 0.525 / 18,892 | 0.693 / 32,621 | 0.711 / 32,561 | 0.733 / 33,317 |
| mesh_100 | 0.249 / 8,254 | 0.286 / 6,443 | 0.385 / 8,878 | 0.426 / 9,314 |
| mesh_1000 | 10.757 / 219,972 | 19.931 / 335,067 | 17.482 / 293,267 | 17.400 / 290,572 |
| hist2d_100 | 0.228 / 6,868 | 0.254 / 7,233 | 0.271 / 7,477 | 0.273 / 7,371 |
| hist2d_500 | 3.901 / 77,856 | 4.323 / 110,378 | 5.196 / 128,883 | 4.681 / 121,482 |
| diverging_1000 | 59.564 / 1,823,302 | 78.230 / 3,116,043 | 86.759 / 3,114,327 | 89.721 / 3,243,519 |
| noise_100 | 0.425 / 40,173 | 0.427 / 40,173 | 0.429 / 40,173 | 0.449 / 40,173 |
| noise_1000 | 60.349 / 4,001,698 | 73.713 / 4,001,698 | 62.603 / 4,001,698 | 65.681 / 4,001,698 |

### Deflate level 6

| Image | NoFilter | Sub | Up | Adaptive |
|---|---:|---:|---:|---:|
| heatmap_1000 | 144.091 / 1,823,357 | 101.284 / 3,147,919 | 103.857 / 3,154,592 | 102.823 / 3,307,504 |
| heatmap_100 | 1.234 / 19,001 | 0.858 / 32,477 | 0.813 / 32,432 | 0.793 / 33,270 |
| mesh_100 | 0.401 / 8,053 | 0.785 / 5,661 | 1.315 / 8,228 | 1.588 / 8,440 |
| mesh_1000 | 20.800 / 179,189 | 81.364 / 280,046 | 60.851 / 255,740 | 53.657 / 254,001 |
| hist2d_100 | 0.491 / 6,716 | 0.662 / 6,930 | 0.738 / 7,186 | 0.735 / 7,042 |
| hist2d_500 | 11.548 / 74,179 | 10.196 / 105,813 | 17.311 / 123,104 | 11.941 / 116,438 |
| diverging_1000 | 110.893 / 1,836,269 | 91.193 / 3,108,286 | 93.310 / 3,106,422 | 90.900 / 3,239,013 |
| noise_100 | 0.428 / 40,173 | 0.425 / 40,173 | 0.424 / 40,173 | 0.439 / 40,173 |
| noise_1000 | 65.462 / 4,001,698 | 64.940 / 4,001,698 | 63.583 / 4,001,698 | 61.609 / 4,001,698 |

### Two encodings and the selected conditional rule

| Image | min(1 NoFilter, 1 Adaptive) | min(1 Sub, 1 Up) | min(1 NoFilter, 3 Adaptive) | Selected rule |
|---|---:|---:|---:|---:|
| heatmap_1000 | 38.738 / 1,871,353 | 38.345 / 3,286,607 | 112.586 / 1,871,353 | 16.447 / 1,871,353 |
| heatmap_100 | 0.385 / 19,690 | 0.359 / 33,110 | 0.935 / 19,690 | 0.186 / 19,690 |
| mesh_100 | 0.308 / 12,715 | 0.302 / 10,771 | 0.567 / 9,314 | 0.380 / 8,254 |
| mesh_1000 | 16.773 / 490,869 | 12.554 / 503,303 | 24.050 / 290,572 | 18.452 / 219,972 |
| hist2d_100 | 0.248 / 8,626 | 0.253 / 9,592 | 0.382 / 7,371 | 0.329 / 6,868 |
| hist2d_500 | 2.955 / 93,722 | 3.361 / 149,153 | 5.393 / 93,722 | 4.514 / 77,856 |
| diverging_1000 | 37.892 / 2,169,721 | 40.994 / 3,305,473 | 108.776 / 2,169,721 | 20.162 / 2,169,721 |
| noise_100 | 0.315 / 40,209 | 0.305 / 40,211 | 0.597 / 40,173 | 0.150 / 40,209 |
| noise_1000 | 34.062 / 4,004,767 | 29.558 / 4,004,791 | 89.368 / 4,001,698 | 13.908 / 4,004,767 |

Level 1 alone makes the smooth mesh too large, regardless of these filters. Always trying two level-1 encodings still misses the size bound. The conditional second NoFilter encoding retains the benchmark’s fast path and improves compression for smooth and sparse inputs.

## End-to-end measurements

The base is `97f7dbf`, followed by the changed implementation, in the same WSL session, pinned to CPU 0. The render benchmark's own settings override the requested sample count and warm-up: it uses 10 samples and a 500 ms warm-up, with 5 seconds requested for measurement. Figures are constructed outside the timer. The intervals below are criterion's reported time estimates, not individual-call medians.

| criterion case | Base estimate, ms (interval) | Changed estimate, ms (interval) |
|---|---:|---:|
| `heatmap_1000x1000_svg` | 203.530 (199.800–207.520) | 97.659 (95.275–100.280) |
| `heatmap_1000x1000_png` | 156.860 (155.000–157.980) | 156.090 (152.420–159.560) |
| `heatmap_1000x1000_scene` | 87.853 (85.879–89.685) | 79.240 (76.843–80.903) |
| `category_200_svg` | 6.3136 (6.1036–6.5632) | 2.0358 (1.9627–2.1241) |
| `category_200_png` | 20.998 (20.315–22.073) | 17.772 (17.234–18.138) |
| `category_200_scene` | 5.7059 (5.4440–5.8645) | 1.3492 (1.3167–1.3854) |

The SVG estimate falls 52.0%; the PNG intervals overlap, so this run does not establish a PNG speedup. The exact benchmark SVG shrinks from **4,491,171 to 2,576,303 bytes** (42.64%). Its final 200 dpi PNG remains **2,029,742 bytes**, with the same SHA-256 on both sides: `0ad23200bfa8b4d6509d1b4cb7e3c58d32f3996047b5cd5821c3e2b91ed2b6d5`.

The Python measurements use the unmodified `scripts/bench/python_vs_matplotlib.py` from `v032-python-bench`, its `worker` command for only `heatmap_1000_png` and `heatmap_1000_svg`, 11 timed runs per combination after the script's warm-up, CPython 3.15.0, NumPy 2.5.3 and matplotlib 3.11.2. Both umber extensions are release builds. The script's input differs from the Rust benchmark, hence the different output sizes. matplotlib preserves the SVG source image resolution (`interpolation="none"`); this is still not a visual-equivalence assertion.

| Library / case | Base median ms | Changed median ms | Base bytes | Changed bytes |
|---|---:|---:|---:|---:|
| umber PNG | 148.361 | 130.096 | 1,169,992 | 1,169,992 |
| umber SVG | 304.071 | 104.446 | 2,518,786 | 2,169,486 |
| matplotlib PNG | 177.123 | 182.429 | 843,789 | 843,789 |
| matplotlib SVG | 330.545 | 279.886 | 2,830,115 | 2,830,115 |

The unchanged matplotlib SVG time also moves, illustrating the host's run-to-run variation; do not interpret these numbers as an isolated-machine comparison.

A separate dependency-free `rustc -O` base64 measurement uses 3,307,504 deterministic random bytes, CPU 0, two warm-ups and 11 measured runs, alternating the old and new encoders. After asserting identical text, the medians are **2.334 ms before / 1.244 ms after** (1.88×). This isolated microbenchmark is not a criterion result.

## Reproduce

```sh
taskset -c 0 cargo test -p umber-core --release --locked \
  measure_embedded_png_corpus -- --ignored --nocapture --test-threads=1
cargo test -p umber-core --release --locked render::image::encoding_tests
cargo test -p umber-core --release --locked cache
```
