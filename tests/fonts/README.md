# Font fixtures

These files are for tests only. `tests/fonts/**` is excluded from both wheels and source
distributions. Only `UmberFixtureSerif-Variable.ttf` has been modified and subset; it is built
from `SourceSerif4Variable-Roman.ttf`, as described below.

Core tests and the output corpus read these fixtures at run time from this repository
path. When `tests/fonts/` is absent outside a repository checkout (for example, in an
unpacked sdist or `cargo package` tarball), cases requiring these files are skipped with
an explicit message. A missing directory in a checkout, or a missing or unreadable file
in an existing fixture directory, is an error. Tests using the bundled fonts still run.

Regular and bold tests use `../../crates/umber-core/fonts/Inter-Regular.ttf` and
`../../crates/umber-core/fonts/Inter-SemiBold.ttf`. Their source, version and hashes are in
`../../crates/umber-core/fonts/MANIFEST.md`, and their SIL Open Font License 1.1 is in
`../../crates/umber-core/fonts/LICENSES/Inter-OFL.txt`. The SemiBold file occupies the custom
font's `bold` slot.

The CFF and variable fixtures are Adobe Source Serif 4, release **4.005R** (font version 4.005).
They come from the official desktop release archive:

<https://github.com/adobe-fonts/source-serif/releases/download/4.005R/source-serif-4.005_Desktop.zip>

Archive SHA-256: `549fdb8f9a682bd06944298621404969f6de77c2e422ff3b8244a1dcd6a0c425`.

| File | Archive member under `source-serif-4.005_Desktop/` | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| `SourceSerif4-Regular.otf` | `OTF/SourceSerif4-Regular.otf` | 241392 | `edf160d0d584deee8a3bb2c3371b2a7624ca63580fbe02c57c1f4c91e84d8787` |
| `SourceSerif4Variable-Roman.ttf` | `VAR/SourceSerif4Variable-Roman.ttf` | 1204208 | `14d360ee1b76655da9276628b229e11671bc1f5d1083636144db6677d452cf55` |
| `SourceSerif4-OFL.md` | `LICENSE.md` | 4492 | `75784a295293a8992f5a8d99210566e0064a012e6dab6731305e3787f15896c7` |

Both fonts are licensed under the SIL Open Font License 1.1, reproduced in
`SourceSerif4-OFL.md`. Together they are under 1.5 MB. The variable font retains its axes. Its
named instances at weight 400 and 600 nearest the default optical size are drawn for regular
and bold text; the regular one is the default instance. The small existing
`../../crates/umber-core/tests/fixtures/fallback-only.ttf` exercises missing digits and minus;
its provenance is also recorded in the bundled font manifest.

`UmberFixtureSerif-Variable.ttf` (103,700 bytes, SHA-256
`a965047aef0c740630803d8750e15974cabd87e12835a1f7d51c9341ad991cb4`) is a variable font whose
default instance is not the regular weight. `scripts/fonts/build_variable_fixture.py` builds it
from `SourceSerif4Variable-Roman.ttf` with fontTools 4.66.1: the optical size axis is pinned at
20, the default weight moves to 300 (Light), the Semibold instance is dropped so that the
named instances are ExtraLight 200, Light 300, Regular 400, Bold 700 and Black 900, and the font
is subset to Basic Latin, Latin-1 Supplement and a few punctuation marks. As a modified version
it does not use the Reserved Font Name: its family is renamed "Umber Fixture Serif". It keeps
Adobe's copyright notice and the SIL Open Font License 1.1, reproduced in
`SourceSerif4-OFL.md`.
