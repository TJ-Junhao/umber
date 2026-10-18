# Embedded PNG measurements

WSL, Linux x86_64, Intel Core Ultra 9 290HX Plus, Rust 1.97.1, `png` 0.18.1; 2026-10-18. Timed processes are pinned to CPU 0. The machine was not otherwise idle. Input construction, decoding and assertions are outside the timer. Each encoder's timer includes preparing its RGBA buffer and all complete candidate encodings, including discarded candidates.

## Rule

1. Encode every image twice at deflate **level 1**, with **NoFilter** and **Adaptive**, using the same RGBA buffer. Keep the smaller complete PNG; ties keep NoFilter.
2. If the raw RGBA buffer is **larger than 32 KiB** and the smaller PNG is **at most one third of the raw buffer**, encode one more full-image candidate at **level 3**. Use Adaptive if the complete level-1 Adaptive PNG is smaller than half the complete level-1 NoFilter PNG; otherwise use NoFilter. In code, the comparison is `adaptive.len() < unfiltered.len() / 2`, with integer division.
3. Keep the third encoding only if it is strictly smaller. Equal lengths retain the level-1 result.

There is no sampling, clock-dependent selection, global state, image-name lookup or fourth candidate. The extra candidate's filter is selected from complete encoded lengths, not a region of the image. Colour mapping and the final figure PNG encoder are unchanged.

The two level-1 encodings alone failed the size bound on 11 of the 35 images. Examples: `mesh_1000` was 490,869 bytes against a 279,401-byte bound, `sin_300` was 85,927 against 59,080, and `rgb_gradient_1500` was 187,724 against 79,445. The conditional level-3 encoding reduces repetitive-image sizes without adding a third pass to the benchmark heatmap or independent RGBA noise. Choosing Adaptive only when prediction more than halves the complete level-1 size handles independent smooth RGB channels and narrow ramps; NoFilter retains repeated colormap colours on the other compressible fields.

**Trade-off.** All 35 images meet the size bound and preserve decoded pixels and deterministic bytes, and the large noisy or smooth fields that dominate heatmap encoding time are 1.3–3.1× faster (`heatmap_1000` 3.05×). Very repetitive images, which already compress to a few kilobytes, can take longer, because every image now takes at least two complete encodings: the slowest measured cases are the 1 × 20,000 ramp (+10 ms), the 1,000 × 1,000 category heatmap (+3 ms) and the synthetic 1,500 × 1,500 RGB gradient (+15 ms). The largest absolute slowdown is about a fifth of the time saved on `heatmap_1000`.

## Bounds

The reference is always **deflate level 6 + Adaptive** on the same RGBA image.

- For every image: `new_bytes <= max(1.10 * old_bytes, old_bytes + 8192)`. The table shows the integer floor of that maximum, since PNG lengths are integers. The regular size test asserts the equivalent integer inequality without rounding a floating-point percentage.
- For `heatmap_1000`: `new_ms <= old_ms / 2`.
- Decoding must reproduce every input RGBA byte, including hidden RGB at zero alpha. Repeated encodings must be byte-identical.

These are corpus bounds, not universal size or timing guarantees. Slower images are listed in the table below rather than excluded.

## Corpus

The 35 images are constructed in `crates/umber-core/src/render/image_encoding_tests.rs`. The original 20 remain, with 15 additional cases. Scalar fields use actual raster images extracted from `Figure::to_scene`, not synthetic RGB approximations. All figures use `Figure::new(6.0, 4.0, Theme::pigment())`.

- `heatmap_1000` and `heatmap_100`: the render benchmark's exact SplitMix64 values, names, theme and heatmap mapping. `diverging_1000` uses values in [-1, 1), centred at zero.
- `mesh_100` and `mesh_1000`: `sin(x) * cos(y) + 0.25*x`, evaluated at cell centres on evenly spaced edges from -4 to 4, through the mesh API.
- `hist2d_100` and `hist2d_500`: 1,000,000 deterministic Box-Muller normal points, with 100² or 500² bins. More than a third of the cells are transparent.
- `noise_100` and `noise_1000`: synthetic independent random RGBA channels, including hidden RGB under zero alpha.
- Square heatmap fields use `x = column/(n-1)`, `y = row/(n-1)` and `Render::Raster`:
  - `sin_100`, `sin_300`, `sin_1000`: `sin(8*x) * cos(8*y)`.
  - `row_ramp_100`, `row_ramp_300`, `row_ramp_1000`: `y`.
  - `ij_ramp_50`, `ij_ramp_100`: `x + y`.
  - `noisy_ramp_20`, `noisy_ramp_50`, `noisy_ramp_100`: `x + y + 0.05*N(0,1)`.
  - `radial_1000`: `hypot(x - 0.5, y - 0.5)`.
  - `ramp_noise_001_1000`: `x + y + 0.01*N(0,1)`.
  - `checker_50`, `checker_100`, `checker_1000`: `(row + column) % 2`.
- `fig-category-labels-heatmap-1000`: exactly the figure in `tests/support/corpus/category_labels.rs`, including default heatmap options: `((row + column) % 100)` on 1,000 × 1,000 cells.
- The following rectangular fields use the mesh API with integer-spaced edges and `Render::Raster`. Dimensions in their names are width × height; coordinates are `x = column/max(width-1,1)`, `y = row/max(height-1,1)`:
  - `transparent_blocks_1000x1000`: `x + y`, with NaN in alternating 100 × 100 blocks; exactly 50% of the cells are transparent.
  - `noisy_center_band_1000x1000`: `x + y` except rows 498–501, which contain SplitMix64 uniform values.
  - `flat_center_band_1000x1000`: `x + y` except rows 498–501, which contain 0.5.
  - `center_flat_edge_ramp_2000x600`: 0.5 in columns 500–1499, and `x + y` elsewhere.
  - `ramp_8x3000`, `ramp_1x20000`, `ramp_3000x8`: `x + y`.
  - `smooth_3x4000`: `sin(8*x) * cos(8*y)`.
- `rgb_gradient_1500`: synthetic 1,500 × 1,500 RGBA, with `r = floor(255*x)`, `g = floor(255*y)`, `b = floor(127.5*(x+y))`, `a = 255`; `x = column/1499`, `y = row/1499`.

The normal variates use Box-Muller and the same deterministic SplitMix64 stream. The test fixes all formulas, dimensions and seeds. Separate synthetic random-RGBA fixtures retain pixel identity and determinism coverage at 6,000 × 1, 1 × 6,000, 2,000 × 3 and 3 × 2,000.

## Full paired comparison

One warm-up per implementation, then 11 measured encodings each, alternating old/new order. Times are medians in milliseconds. Speedup is old/new. Every row passes its size limit.

| Image | Old ms | New ms | Speedup | Old bytes | New bytes | Size limit |
|---|---:|---:|---:|---:|---:|---:|
| heatmap_1000 | 102.5060 | 33.5655 | 3.05× | 3,307,504 | 1,871,353 | 3,638,254 |
| heatmap_100 | 0.8062 | 0.3985 | 2.02× | 33,270 | 19,690 | 41,462 |
| mesh_100 | 1.5950 | 0.5770 | 2.76× | 8,440 | 8,254 | 16,632 |
| mesh_1000 | 53.5965 | 23.8911 | 2.24× | 254,001 | 219,972 | 279,401 |
| hist2d_100 | 0.7544 | 0.5001 | 1.51× | 7,042 | 6,868 | 15,234 |
| hist2d_500 | 11.9730 | 6.5567 | 1.83× | 116,438 | 77,856 | 128,081 |
| diverging_1000 | 90.6256 | 36.3772 | 2.49× | 3,239,013 | 2,169,721 | 3,562,914 |
| noise_100 | 0.4507 | 0.3005 | 1.50× | 40,173 | 40,209 | 48,365 |
| noise_1000 | 62.3628 | 26.1478 | 2.39× | 4,001,698 | 4,004,767 | 4,401,867 |
| sin_100 | 1.5317 | 0.3759 | 4.07× | 9,432 | 14,199 | 17,624 |
| sin_300 | 11.6444 | 4.1140 | 2.83× | 50,888 | 58,056 | 59,080 |
| sin_1000 | 72.6054 | 27.2469 | 2.66× | 317,078 | 258,236 | 348,785 |
| row_ramp_300 | 0.4730 | 0.7218 | 0.66× | 1,224 | 1,982 | 9,416 |
| row_ramp_1000 | 4.0879 | 5.7404 | 0.71× | 6,711 | 7,459 | 14,903 |
| ij_ramp_50 | 0.0558 | 0.0567 | 0.98× | 355 | 776 | 8,547 |
| noisy_ramp_20 | 0.0357 | 0.0462 | 0.77× | 904 | 953 | 9,096 |
| noisy_ramp_50 | 0.2250 | 0.1441 | 1.56× | 4,742 | 4,802 | 12,934 |
| radial_1000 | 55.3125 | 21.9565 | 2.52× | 251,435 | 178,803 | 276,578 |
| ramp_noise_001_1000 | 198.5141 | 64.4325 | 3.08× | 992,825 | 891,811 | 1,092,107 |
| checker_1000 | 4.7541 | 6.3921 | 0.74× | 8,243 | 8,225 | 16,435 |
| row_ramp_100 | 0.0903 | 0.1662 | 0.54× | 409 | 670 | 8,601 |
| checker_100 | 0.0875 | 0.1538 | 0.57× | 277 | 351 | 8,469 |
| ij_ramp_100 | 0.1568 | 0.2468 | 0.64× | 732 | 1,189 | 8,924 |
| noisy_ramp_100 | 1.4560 | 0.4674 | 3.12× | 17,387 | 16,535 | 25,579 |
| checker_50 | 0.0345 | 0.0486 | 0.71× | 164 | 204 | 8,356 |
| fig-category-labels-heatmap-1000 | 5.6326 | 8.8990 | 0.63× | 19,591 | 19,830 | 27,783 |
| transparent_blocks_1000x1000 | 13.6408 | 10.4328 | 1.31× | 76,928 | 54,012 | 85,120 |
| noisy_center_band_1000x1000 | 16.9191 | 11.8245 | 1.43× | 144,005 | 38,476 | 158,405 |
| flat_center_band_1000x1000 | 18.1595 | 12.0731 | 1.50× | 129,399 | 30,267 | 142,338 |
| center_flat_edge_ramp_2000x600 | 23.1871 | 12.8884 | 1.80× | 124,207 | 68,739 | 136,627 |
| ramp_8x3000 | 1.6954 | 2.8534 | 0.59× | 4,547 | 5,647 | 12,739 |
| smooth_3x4000 | 1.7144 | 3.2846 | 0.52× | 3,057 | 5,289 | 11,249 |
| ramp_1x20000 | 4.7845 | 14.8683 | 0.32× | 1,351 | 1,453 | 9,543 |
| ramp_3000x8 | 0.4097 | 0.3382 | 1.21× | 2,829 | 3,384 | 11,021 |
| rgb_gradient_1500 | 14.9460 | 30.0704 | 0.50× | 71,253 | 69,314 | 79,445 |

## End-to-end SVG comparison

Base `97f7dbf`, then the changed implementation, in one WSL session on CPU 0. Builds use separate target directories and check the expected SVG and PNG sizes before timing. The benchmark constructs the figure outside the timer, takes 10 samples after a 500 ms warm-up, and requests 5 seconds of measurement. Criterion extended the changed measurement to about 6 seconds and reported one high outlier.

| Case | Old estimate, ms (interval) | New estimate, ms (interval) |
|---|---:|---:|
| `heatmap_1000x1000_svg` | 196.47 (195.63–197.35) | 105.03 (102.78–108.37) |

The end-to-end estimate is 1.87× faster. This comparison includes the branch's label-cache and base64 changes, not just the embedded encoder. The exact SVG changes from **4,491,171 to 2,576,303 bytes**. The final 200 dpi PNG is **2,029,742 bytes** on both sides and is byte-identical. The 200-category figure's SVG and PNG also remain byte-identical; their performance was not retimed in this comparison.

## Reproduce

Run only on the Linux reference machine. The first command checks the whole corpus's size bound, decoded-pixel identity, selected encoding and determinism, plus base64 equality and the narrow/wide fixtures. Timings are run explicitly in release mode, separate from ordinary tests so that debug instrumentation and parallel test execution do not set the timing result.

```sh
cargo test -p umber-core --release --locked --lib render::image::encoding_tests
taskset -c 0 cargo test -p umber-core --release --locked --lib \
  measure_selected_embedded_png_corpus -- --ignored --nocapture --test-threads=1
taskset -c 0 cargo bench -p umber-core --bench render --locked -- \
  heatmap_1000x1000_svg --noplot
```
