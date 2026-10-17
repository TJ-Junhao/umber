# Embedded PNG measurements

WSL, Linux x86_64, Intel Core Ultra 9 290HX Plus, Rust 1.97.1, `png` 0.18.1; 2026-10-18. Timed processes are pinned to CPU 0. The machine was not otherwise idle. Encode times include preparing the RGBA buffer and, for the selected rule, all sampling and candidate encodings. Input generation, decoding and assertions are outside the timer.

## Corpus

All scalar fields below use the real raster chart path and its exact colour mapping, not synthetic RGB approximations. `noise_*` deliberately bypasses colour mapping to exercise independent RGBA channels, including hidden RGB under zero alpha. The corpus is defined in `crates/umber-core/src/render/image_encoding_tests.rs`.

- `heatmap_1000`: the exact SplitMix64 input, theme and mapping from `heatmap_1000x1000_svg`; `heatmap_100` uses the same construction at 100 × 100.
- `mesh_100`, `mesh_1000`: `sin(x) * cos(y) + 0.25 * x`, with cell centres on evenly spaced edges from -4 to 4, through the mesh API.
- `hist2d_100`, `hist2d_500`: 1,000,000 deterministic Box-Muller normal points through the histogram API, at 100 × 100 and 500 × 500 bins. More than a third of the cells are transparent.
- `diverging_1000`: random heatmap values in [-1, 1), centred at zero.
- `noise_100`, `noise_1000`: independent random RGBA channels.
- The following heatmaps use `x = column / (n - 1)` and `y = row / (n - 1)`, `Theme::pigment()` and `Render::Raster`:
  - `sin_100`, `sin_300`, `sin_1000`: `sin(8*x) * cos(8*y)`.
  - `row_ramp_300`, `row_ramp_1000`: `y`.
  - `ij_ramp_50`: `x + y`.
  - `noisy_ramp_20`, `noisy_ramp_50`: `x + y + 0.05*N(0,1)`.
  - `radial_1000`: `hypot(x - 0.5, y - 0.5)`.
  - `ramp_noise_001_1000`: `x + y + 0.01*N(0,1)`.
  - `checker_1000`: `(row + column) % 2`.

The normal variates use Box-Muller and the same deterministic SplitMix64 stream. Formulae, dimensions, theme and seeds are part of the test input.

## Selected rule

1. At most 16 KiB of raw RGBA: encode once with **level 3 + Adaptive**. Extra probes cost too much on small images.
2. Otherwise take the central rectangle of at most **1024 columns × 4 consecutive rows**, retaining actual neighbour relationships. Encode this sample with **level 1 + NoFilter**.
   - Sample PNG at most **4% of the sample's raw RGBA bytes**: encode the full image once with **level 3 + Paeth**. Highly repetitive inputs need stronger compression but do not benefit from paying for a full level-1 attempt first.
   - Sample PNG at least as large as its raw RGBA: encode the full image once with **level 1 + NoFilter**.
3. For other images, measure the sample with **1 Sub, 3 NoFilter and 3 Sub** as well. Choose level 1 when the smaller level-1 sample is at most **5% larger** than the smaller level-3 sample; otherwise choose level 3.
4. Encode the whole image with NoFilter at that level. Also encode with Sub unless its sample is **at least 25% larger** than the NoFilter sample. Keep the smaller complete PNG, preferring NoFilter on equal lengths. The same RGBA buffer serves every full encoding.

Every decision is a function of the pixels and dimensions. There is no clock, random sampling, global state or image-name lookup. Neither colour mapping nor the final figure PNG encoder changes.

Sub is useful for the small and medium smooth fields, while NoFilter wins on the larger smooth fields; keeping the smaller full encoding avoids trusting the sample to rank these two perfectly. Paeth handles constant rows better than Sub, which leaves each row's initial pixel unpredicted. The measured candidates below include Up, Paeth and Adaptive at every level. A 10% sample tolerance left `hist2d_100` at level 1 and produced a 22.5% size increase; the 5% tolerance selects level 3. Trying both full level-1 encodings alone still leaves smooth images too large. Sampling selects the stronger level before the full encoding, instead of always paying for a second level.

**Scope of the bounds:** the 10% size bound is asserted for these 20 corpus images. The timing goals are measured on this machine: at least 3× faster for the embedded `heatmap_1000` encoder and no corpus image more than 10% slower than level 6 + Adaptive. These are not universal size or timing guarantees for arbitrary images: the sample is a heuristic and can miss structure elsewhere in an image. In particular, the 5% sample tolerance is not a full-image size guarantee.

## Selected rule: paired comparison

One warm-up per implementation, then 11 measured encodings each, alternating old/new order. Values are medians in milliseconds. The reference is always **level 6 + Adaptive**. The selected time includes every probe and every full candidate, not just the winning encoding.

| Image | Before ms | Selected ms | Speedup | Before bytes | Selected bytes | Size change | Winning setting |
|---|---:|---:|---:|---:|---:|---:|---|
| heatmap_1000 | 100.0502 | 17.0122 | 5.88× | 3,307,504 | 1,871,353 | -43.421% | 1 NoFilter |
| heatmap_100 | 0.7931 | 0.4698 | 1.69× | 33,270 | 19,690 | -40.818% | 1 NoFilter |
| mesh_100 | 1.6126 | 0.6098 | 2.64× | 8,440 | 6,443 | -23.661% | 3 Sub |
| mesh_1000 | 53.8053 | 30.9544 | 1.74× | 254,001 | 219,972 | -13.397% | 3 NoFilter |
| hist2d_100 | 0.7531 | 0.5810 | 1.30× | 7,042 | 6,868 | -2.471% | 3 NoFilter |
| hist2d_500 | 11.6032 | 1.2898 | 9.00× | 116,438 | 93,722 | -19.509% | 1 NoFilter |
| diverging_1000 | 90.0366 | 54.2899 | 1.66× | 3,239,013 | 1,823,302 | -43.708% | 3 NoFilter |
| noise_100 | 0.4484 | 0.1556 | 2.88× | 40,173 | 40,209 | +0.090% | 1 NoFilter |
| noise_1000 | 62.0908 | 12.9322 | 4.80× | 4,001,698 | 4,004,767 | +0.077% | 1 NoFilter |
| sin_100 | 1.6276 | 0.8514 | 1.91× | 9,432 | 9,748 | +3.350% | 3 Sub |
| sin_300 | 11.9512 | 5.1255 | 2.33× | 50,888 | 52,697 | +3.555% | 3 Sub |
| sin_1000 | 74.1352 | 34.5220 | 2.15× | 317,078 | 258,236 | -18.558% | 3 NoFilter |
| row_ramp_300 | 0.4792 | 0.4128 | 1.16× | 1,224 | 1,242 | +1.471% | 3 Paeth |
| row_ramp_1000 | 4.1945 | 4.0027 | 1.05× | 6,711 | 6,766 | +0.820% | 3 Paeth |
| ij_ramp_50 | 0.0517 | 0.0354 | 1.46× | 355 | 354 | -0.282% | 3 Adaptive |
| noisy_ramp_20 | 0.0269 | 0.0225 | 1.20× | 904 | 920 | +1.770% | 3 Adaptive |
| noisy_ramp_50 | 0.2222 | 0.1236 | 1.80× | 4,742 | 4,858 | +2.446% | 3 Adaptive |
| radial_1000 | 56.4594 | 26.9603 | 2.09× | 251,435 | 178,803 | -28.887% | 3 NoFilter |
| ramp_noise_001_1000 | 197.9843 | 76.6680 | 2.58× | 992,825 | 891,811 | -10.174% | 3 NoFilter |
| checker_1000 | 4.5083 | 4.0241 | 1.12× | 8,243 | 8,233 | -0.121% | 3 Paeth |

All three goals pass in this run: **5.88×** for `heatmap_1000`, a maximum size increase of **3.555%**, and speedups from **1.05×** upward. The 3× goal is for the embedded encoder, not end-to-end SVG rendering.

The regular tests assert the size bound, exact winning settings, old/new decoded-pixel identity and repeated-encoding determinism for all 20 images. Separate rectangular fixtures exercise 6000 × 1, 1 × 6000, 2000 × 3 and 3 × 2000 sampling. All **380 image/candidate combinations** in the following measurement also decode to the input RGBA; every timed repeat matches its warm-up bytes.

## All candidates

Each entry is **milliseconds / PNG bytes**: median of five encodings after one warm-up. Candidate order is fixed, so these times are not the paired measurements above. Two-candidate entries include both full encodings and keep the smaller file.

### Deflate level 1

| Image | NoFilter | Sub | Up | Paeth | Adaptive |
|---|---:|---:|---:|---:|---:|
| heatmap_1000 | 16.772 / 1,871,353 | 17.369 / 3,286,981 | 17.607 / 3,286,607 | 16.986 / 3,320,297 | 17.036 / 3,323,523 |
| heatmap_100 | 0.184 / 19,690 | 0.178 / 33,145 | 0.181 / 33,110 | 0.175 / 33,399 | 0.183 / 33,345 |
| mesh_100 | 0.132 / 12,778 | 0.143 / 10,771 | 0.155 / 12,747 | 0.163 / 12,754 | 0.186 / 12,715 |
| mesh_1000 | 7.290 / 640,551 | 6.926 / 663,592 | 5.510 / 503,303 | 5.741 / 504,299 | 6.307 / 490,869 |
| hist2d_100 | 0.102 / 8,626 | 0.127 / 9,607 | 0.129 / 9,592 | 0.132 / 9,902 | 0.138 / 9,465 |
| hist2d_500 | 1.205 / 93,722 | 1.740 / 149,153 | 1.870 / 157,395 | 2.163 / 165,905 | 1.915 / 152,985 |
| diverging_1000 | 19.252 / 2,169,721 | 18.873 / 3,306,829 | 17.732 / 3,305,473 | 16.927 / 3,363,239 | 18.268 / 3,325,993 |
| noise_100 | 0.141 / 40,209 | 0.141 / 40,211 | 0.142 / 40,212 | 0.145 / 40,209 | 0.154 / 40,211 |
| noise_1000 | 12.523 / 4,004,767 | 12.584 / 4,004,791 | 13.159 / 4,004,794 | 13.660 / 4,004,788 | 13.932 / 4,004,780 |
| sin_100 | 0.170 / 17,323 | 0.187 / 15,087 | 0.576 / 14,485 | 0.176 / 14,221 | 0.187 / 14,199 |
| sin_300 | 0.962 / 108,345 | 1.089 / 91,591 | 0.940 / 84,062 | 0.983 / 86,512 | 1.023 / 85,927 |
| sin_1000 | 7.169 / 765,156 | 8.028 / 696,419 | 6.897 / 620,641 | 6.835 / 620,187 | 7.593 / 605,206 |
| row_ramp_300 | 0.137 / 3,875 | 0.142 / 4,129 | 0.145 / 4,164 | 0.162 / 3,407 | 0.198 / 3,394 |
| row_ramp_1000 | 1.004 / 27,911 | 1.068 / 25,258 | 1.111 / 27,365 | 1.340 / 24,404 | 1.412 / 24,094 |
| ij_ramp_50 | 0.025 / 776 | 0.027 / 1,085 | 0.026 / 1,135 | 0.026 / 912 | 0.027 / 912 |
| noisy_ramp_20 | 0.018 / 1,089 | 0.017 / 1,022 | 0.017 / 1,037 | 0.016 / 960 | 0.018 / 953 |
| noisy_ramp_50 | 0.060 / 4,802 | 0.076 / 5,500 | 0.076 / 5,467 | 0.074 / 5,433 | 0.075 / 5,315 |
| radial_1000 | 5.949 / 592,356 | 6.519 / 603,360 | 6.046 / 555,547 | 6.395 / 558,910 | 6.654 / 548,124 |
| ramp_noise_001_1000 | 12.227 / 1,163,639 | 13.702 / 1,363,668 | 13.960 / 1,375,546 | 14.599 / 1,440,085 | 14.635 / 1,372,917 |
| checker_1000 | 1.012 / 30,088 | 1.043 / 30,117 | 1.063 / 30,111 | 1.335 / 30,103 | 1.843 / 30,126 |

### Deflate level 3

| Image | NoFilter | Sub | Up | Paeth | Adaptive |
|---|---:|---:|---:|---:|---:|
| heatmap_1000 | 64.187 / 1,801,139 | 89.803 / 3,171,455 | 88.943 / 3,177,807 | 92.131 / 3,262,935 | 98.424 / 3,311,677 |
| heatmap_100 | 0.532 / 18,892 | 0.719 / 32,621 | 0.712 / 32,561 | 0.744 / 33,183 | 0.752 / 33,317 |
| mesh_100 | 0.261 / 8,254 | 0.303 / 6,443 | 0.395 / 8,878 | 0.429 / 9,327 | 0.432 / 9,314 |
| mesh_1000 | 11.052 / 219,972 | 19.961 / 335,067 | 17.646 / 293,267 | 17.339 / 297,424 | 17.773 / 290,572 |
| hist2d_100 | 0.232 / 6,868 | 0.258 / 7,233 | 0.272 / 7,477 | 0.288 / 7,786 | 0.269 / 7,371 |
| hist2d_500 | 3.601 / 77,856 | 3.932 / 110,378 | 4.460 / 128,883 | 4.509 / 133,753 | 4.480 / 121,482 |
| diverging_1000 | 55.255 / 1,823,302 | 79.051 / 3,116,043 | 76.971 / 3,114,327 | 86.518 / 3,304,893 | 84.245 / 3,243,519 |
| noise_100 | 0.441 / 40,173 | 0.434 / 40,173 | 0.431 / 40,173 | 0.495 / 40,173 | 0.443 / 40,173 |
| noise_1000 | 61.501 / 4,001,698 | 60.794 / 4,001,698 | 61.670 / 4,001,698 | 62.078 / 4,001,698 | 61.279 / 4,001,698 |
| sin_100 | 0.356 / 13,066 | 0.400 / 9,748 | 0.409 / 10,254 | 0.441 / 10,361 | 0.436 / 10,357 |
| sin_300 | 2.171 / 58,056 | 2.937 / 52,697 | 2.858 / 52,274 | 3.037 / 58,583 | 3.199 / 58,118 |
| sin_1000 | 12.588 / 258,236 | 22.879 / 358,075 | 21.513 / 361,741 | 21.387 / 371,160 | 22.573 / 363,384 |
| row_ramp_300 | 0.380 / 1,982 | 0.379 / 1,987 | 0.375 / 1,361 | 0.396 / 1,242 | 0.438 / 1,241 |
| row_ramp_1000 | 3.791 / 7,459 | 3.802 / 7,383 | 3.765 / 7,885 | 4.099 / 6,766 | 3.999 / 6,744 |
| ij_ramp_50 | 0.043 / 573 | 0.039 / 616 | 0.036 / 567 | 0.034 / 354 | 0.037 / 354 |
| noisy_ramp_20 | 0.024 / 1,055 | 0.023 / 983 | 0.023 / 1,012 | 0.022 / 923 | 0.022 / 920 |
| noisy_ramp_50 | 0.109 / 4,516 | 0.120 / 4,981 | 0.122 / 5,016 | 0.120 / 4,970 | 0.122 / 4,858 |
| radial_1000 | 8.974 / 178,803 | 16.863 / 293,170 | 17.666 / 291,742 | 17.808 / 300,394 | 18.025 / 297,259 |
| ramp_noise_001_1000 | 36.206 / 891,811 | 40.950 / 986,779 | 41.563 / 1,066,836 | 42.775 / 1,104,866 | 42.641 / 1,062,573 |
| checker_1000 | 3.630 / 8,225 | 3.553 / 8,240 | 3.527 / 8,240 | 3.957 / 8,233 | 4.475 / 8,246 |

### Deflate level 6

| Image | NoFilter | Sub | Up | Paeth | Adaptive |
|---|---:|---:|---:|---:|---:|
| heatmap_1000 | 141.062 / 1,823,357 | 102.797 / 3,147,919 | 101.673 / 3,154,592 | 101.692 / 3,248,588 | 101.831 / 3,307,504 |
| heatmap_100 | 0.954 / 19,001 | 0.781 / 32,477 | 0.784 / 32,432 | 0.787 / 33,076 | 0.797 / 33,270 |
| mesh_100 | 0.409 / 8,053 | 0.783 / 5,661 | 1.329 / 8,228 | 1.617 / 8,462 | 1.619 / 8,440 |
| mesh_1000 | 19.674 / 179,189 | 71.007 / 280,046 | 56.921 / 255,740 | 54.536 / 260,801 | 56.488 / 254,001 |
| hist2d_100 | 0.485 / 6,716 | 0.683 / 6,930 | 0.726 / 7,186 | 0.789 / 7,438 | 0.785 / 7,042 |
| hist2d_500 | 11.844 / 74,179 | 10.733 / 105,813 | 12.676 / 123,104 | 11.844 / 127,719 | 11.993 / 116,438 |
| diverging_1000 | 98.257 / 1,836,269 | 87.444 / 3,108,286 | 87.361 / 3,106,422 | 92.902 / 3,302,856 | 90.593 / 3,239,013 |
| noise_100 | 0.429 / 40,173 | 0.425 / 40,173 | 0.429 / 40,173 | 0.480 / 40,173 | 0.454 / 40,173 |
| noise_1000 | 61.256 / 4,001,698 | 61.429 / 4,001,698 | 62.660 / 4,001,698 | 61.849 / 4,001,698 | 61.423 / 4,001,698 |
| sin_100 | 0.661 / 12,582 | 1.158 / 8,841 | 1.186 / 9,635 | 1.534 / 9,459 | 1.572 / 9,432 |
| sin_300 | 5.368 / 56,607 | 10.589 / 44,520 | 11.326 / 46,251 | 11.774 / 51,318 | 11.904 / 50,888 |
| sin_1000 | 24.328 / 223,911 | 82.744 / 310,912 | 80.998 / 316,838 | 75.416 / 323,423 | 73.949 / 317,078 |
| row_ramp_300 | 0.418 / 1,945 | 0.420 / 1,987 | 0.419 / 1,336 | 0.438 / 1,226 | 0.476 / 1,224 |
| row_ramp_1000 | 3.729 / 7,426 | 3.934 / 7,383 | 3.885 / 7,853 | 3.966 / 6,732 | 4.206 / 6,711 |
| ij_ramp_50 | 0.042 / 573 | 0.053 / 538 | 0.049 / 570 | 0.047 / 355 | 0.048 / 355 |
| noisy_ramp_20 | 0.026 / 1,049 | 0.026 / 971 | 0.026 / 994 | 0.025 / 910 | 0.027 / 904 |
| noisy_ramp_50 | 0.158 / 4,503 | 0.217 / 4,867 | 0.213 / 4,876 | 0.210 / 4,847 | 0.222 / 4,742 |
| radial_1000 | 14.653 / 125,878 | 53.046 / 234,436 | 56.182 / 245,746 | 54.391 / 254,480 | 56.318 / 251,435 |
| ramp_noise_001_1000 | 125.735 / 911,831 | 181.984 / 915,942 | 196.608 / 996,358 | 203.058 / 1,028,023 | 199.327 / 992,825 |
| checker_1000 | 3.787 / 8,220 | 3.640 / 8,236 | 3.632 / 8,236 | 3.890 / 8,227 | 4.725 / 8,243 |

### Combined encodings

| Image | min(1 NoFilter, 1 Adaptive) | min(1 Sub, 1 Up) | min(1 NoFilter, 3 Adaptive) | Selected rule |
|---|---:|---:|---:|---:|
| heatmap_1000 | 33.448 / 1,871,353 | 35.165 / 3,286,607 | 113.043 / 1,871,353 | 16.638 / 1,871,353 |
| heatmap_100 | 0.384 / 19,690 | 0.372 / 33,110 | 0.948 / 19,690 | 0.456 / 19,690 |
| mesh_100 | 0.312 / 12,715 | 0.300 / 10,771 | 0.595 / 9,314 | 0.609 / 6,443 |
| mesh_1000 | 13.491 / 490,869 | 13.197 / 503,303 | 25.180 / 290,572 | 31.306 / 219,972 |
| hist2d_100 | 0.250 / 8,626 | 0.257 / 9,592 | 0.383 / 7,371 | 0.561 / 6,868 |
| hist2d_500 | 2.947 / 93,722 | 3.336 / 149,153 | 5.392 / 93,722 | 1.253 / 93,722 |
| diverging_1000 | 36.105 / 2,169,721 | 36.912 / 3,305,473 | 102.458 / 2,169,721 | 53.899 / 1,823,302 |
| noise_100 | 0.299 / 40,209 | 0.280 / 40,211 | 0.587 / 40,173 | 0.153 / 40,209 |
| noise_1000 | 27.253 / 4,004,767 | 26.413 / 4,004,791 | 77.322 / 4,001,698 | 13.247 / 4,004,767 |
| sin_100 | 0.353 / 14,199 | 0.368 / 14,485 | 0.607 / 10,357 | 0.836 / 9,748 |
| sin_300 | 1.955 / 85,927 | 2.014 / 84,062 | 3.943 / 58,118 | 5.013 / 52,697 |
| sin_1000 | 15.099 / 605,206 | 15.378 / 620,641 | 29.229 / 363,384 | 34.516 / 258,236 |
| row_ramp_300 | 0.340 / 3,394 | 0.284 / 4,129 | 0.581 / 1,241 | 0.415 / 1,242 |
| row_ramp_1000 | 2.593 / 24,094 | 2.073 / 25,258 | 5.075 / 6,744 | 4.021 / 6,766 |
| ij_ramp_50 | 0.053 / 776 | 0.055 / 1,085 | 0.063 / 354 | 0.035 / 354 |
| noisy_ramp_20 | 0.036 / 953 | 0.035 / 1,022 | 0.042 / 920 | 0.021 / 920 |
| noisy_ramp_50 | 0.144 / 4,802 | 0.153 / 5,467 | 0.195 / 4,802 | 0.121 / 4,858 |
| radial_1000 | 12.934 / 548,124 | 12.967 / 555,547 | 24.395 / 297,259 | 25.733 / 178,803 |
| ramp_noise_001_1000 | 27.548 / 1,163,639 | 27.783 / 1,363,668 | 55.168 / 1,062,573 | 75.846 / 891,811 |
| checker_1000 | 2.839 / 30,088 | 2.228 / 30,111 | 5.515 / 8,246 | 3.909 / 8,233 |

## End-to-end measurements

The base is `97f7dbf`, with the same `render.rs` benchmark harness copied to it to provide the 200-category cases. It is followed by the changed implementation including the tick-label fixes from `97b7400`, in one WSL session on CPU 0, with separate target directories and an SVG-size precondition on each build. Each criterion case uses 10 samples, a 500 ms warm-up and 5 seconds requested for measurement. Figures are constructed outside the timer. Values below are criterion's estimates and confidence intervals, not individual-call medians.

| criterion case | Base estimate, ms (interval) | Changed estimate, ms (interval) |
|---|---:|---:|
| `heatmap_1000x1000_svg` | 197.0500 (195.8100–198.2600) | 84.5930 (84.2040–85.1290) |
| `heatmap_1000x1000_png` | 153.2000 (152.5900–153.7300) | 135.7600 (135.1500–136.7200) |
| `heatmap_1000x1000_scene` | 89.9710 (89.6890–90.2960) | 65.0640 (64.7250–65.3840) |
| `category_200_svg` | 5.3427 (5.3199–5.3616) | 1.7161 (1.6997–1.7380) |
| `category_200_png` | 17.8780 (17.7120–18.0100) | 15.7120 (15.1060–16.8930) |
| `category_200_scene` | 4.8154 (4.7789–4.8611) | 1.2190 (1.2060–1.2341) |

The exact benchmark SVG changes from **4,491,171 to 2,576,303 bytes**. Its final 200 dpi PNG stays at **2,029,742 bytes** and has identical SHA-256 on both sides. The category SVG and PNG also have identical hashes. The full output gate separately checks all committed scene and final PNG hashes against `origin/main`.

## Python heatmap measurements

The unmodified `scripts/bench/python_vs_matplotlib.py` runs only `heatmap_1000_png` and `heatmap_1000_svg`, using its `worker` command with 11 timed runs after one warm-up. Both umber wheels are release builds, compiled in separate target directories, with `97f7dbf` followed by the changed implementation in the same WSL session on CPU 0. The interpreter is CPython 3.12.13, with NumPy 2.5.4 and matplotlib 3.11.2. Input generation is outside the timer; figure construction and rendering are inside it. The input differs from the Rust benchmark, hence the different output sizes.

| Library / case | Base median ms | Changed median ms | Base bytes | Changed bytes |
|---|---:|---:|---:|---:|
| umber PNG | 132.577 | 109.477 | 1,169,992 | 1,169,992 |
| umber SVG | 275.042 | 87.252 | 2,518,786 | 2,169,486 |
| matplotlib PNG | 164.140 | 163.061 | 843,789 | 843,789 |
| matplotlib SVG | 261.565 | 262.107 | 2,830,115 | 2,830,115 |

matplotlib preserves the source SVG image resolution with `interpolation="none"`. This is not a visual-equivalence claim, and the machine was not otherwise idle.

## Reproduce

Run only on the Linux reference machine:

```sh
cargo test -p umber-core --release --locked --lib render::image::encoding_tests
cargo test -p umber-core --release --locked --lib cache
taskset -c 0 cargo test -p umber-core --release --locked --lib \
  measure_embedded_png_corpus -- --ignored --nocapture --test-threads=1
taskset -c 0 cargo test -p umber-core --release --locked --lib \
  measure_selected_embedded_png_corpus -- --ignored --nocapture --test-threads=1
taskset -c 0 cargo bench -p umber-core --bench render --locked -- \
  'heatmap_1000x1000_(svg|png|scene)|category_200_' --noplot
# Use the environment containing the appropriate release wheel for each comparison.
for library in umber matplotlib; do
  for format in png svg; do
    taskset -c 0 python scripts/bench/python_vs_matplotlib.py worker \
      "$library" "heatmap_1000_$format" --runs 11
  done
done
```
