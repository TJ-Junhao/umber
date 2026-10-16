//! Colormaps: the built-in tables, Oklab interpolation, `Cmap::from_colors` and the theme defaults.

#![allow(clippy::unwrap_used)]

use umber_core::{Cmap, Error, Rgba, Theme};

/// The published 256-entry `viridis` table, as `RRGGBB` digits.
const VIRIDIS: &str = concat!(
    "44015444025645045745055946075A46085C460A5D460B5E470D60470E61471063471164471365481467481668",
    "48176948186A481A6C481B6D481C6E481D6F481F70482071482173482374482475482576482677482878482979",
    "472A7A472C7A472D7B472E7C472F7D46307E46327E46337F463480453581453781453882443983443A83443B84",
    "433D84433E85423F854240864241864142874144874045884046883F47883F48893E49893E4A893E4C8A3D4D8A",
    "3D4E8A3C4F8A3C508B3B518B3B528B3A538B3A548C39558C39568C38588C38598C375A8C375B8D365C8D365D8D",
    "355E8D355F8D34608D34618D33628D33638D32648E32658E31668E31678E31688E30698E306A8E2F6B8E2F6C8E",
    "2E6D8E2E6E8E2E6F8E2D708E2D718E2C718E2C728E2C738E2B748E2B758E2A768E2A778E2A788E29798E297A8E",
    "297B8E287C8E287D8E277E8E277F8E27808E26818E26828E26828E25838E25848E25858E24868E24878E23888E",
    "23898E238A8D228B8D228C8D228D8D218E8D218F8D21908D21918C20928C20928C20938C1F948C1F958B1F968B",
    "1F978B1F988B1F998A1F9A8A1E9B8A1E9C891E9D891F9E891F9F881FA0881FA1881FA1871FA28720A38620A486",
    "21A58521A68522A78522A88423A98324AA8325AB8225AC8226AD8127AD8128AE8029AF7F2AB07F2CB17E2DB27D",
    "2EB37C2FB47C31B57B32B67A34B67935B77937B87838B9773ABA763BBB753DBC743FBC7340BD7242BE7144BF70",
    "46C06F48C16E4AC16D4CC26C4EC36B50C46A52C56954C56856C66758C7655AC8645CC8635EC96260CA6063CB5F",
    "65CB5E67CC5C69CD5B6CCD5A6ECE5870CF5773D05675D05477D1537AD1517CD2507FD34E81D34D84D44B86D549",
    "89D5488BD6468ED64590D74393D74195D84098D83E9BD93C9DD93BA0DA39A2DA37A5DB36A8DB34AADC32ADDC30",
    "B0DD2FB2DD2DB5DE2BB8DE29BADE28BDDF26C0DF25C2DF23C5E021C8E020CAE11FCDE11DD0E11CD2E21BD5E21A",
    "D8E219DAE319DDE318DFE318E2E418E5E419E7E419EAE51AECE51BEFE51CF1E51DF4E61EF6E620F8E621FBE723",
    "FDE725",
);

/// The published 256-entry `magma` table, as `RRGGBB` digits.
const MAGMA: &str = concat!(
    "00000401000501010601010802010902020B02020D03030F03031204041405041606051806051A07061C08071E",
    "0907200A08220B09240C09260D0A290E0B2B100B2D110C2F120D31130D34140E36150E38160F3B180F3D19103F",
    "1A10421C10441D11471E114920114B21114E22115024125325125527125829115A2A115C2C115F2D11612F1163",
    "31116533106734106936106B38106C390F6E3B0F703D0F713F0F72400F74420F75440F76451077471078491078",
    "4A10794C117A4E117B4F127B51127C52137C54137D56147D57157E59157E5A167E5C167F5D177F5F187F601880",
    "621980641A80651A80671B80681C816A1C816B1D816D1D816E1E81701F81721F81732081752181762181782281",
    "7922827B23827C23827E24828025828125818326818426818627818827818928818B29818C29818E2A81902A81",
    "912B81932B80942C80962C80982D80992D809B2E7F9C2E7F9E2F7FA02F7FA1307EA3307EA5317EA6317DA8327D",
    "AA337DAB337CAD347CAE347BB0357BB2357BB3367AB5367AB73779B83779BA3878BC3978BD3977BF3A77C03A76",
    "C23B75C43C75C53C74C73D73C83E73CA3E72CC3F71CD4071CF4070D0416FD2426FD3436ED5446DD6456CD8456C",
    "D9466BDB476ADC4869DE4968DF4A68E04C67E24D66E34E65E44F64E55064E75263E85362E95462EA5661EB5760",
    "EC5860ED5A5FEE5B5EEF5D5EF05F5EF1605DF2625DF2645CF3655CF4675CF4695CF56B5CF66C5CF66E5CF7705C",
    "F7725CF8745CF8765CF9785DF9795DF97B5DFA7D5EFA7F5EFA815FFB835FFB8560FB8761FC8961FC8A62FC8C63",
    "FC8E64FC9065FD9266FD9467FD9668FD9869FD9A6AFD9B6BFE9D6CFE9F6DFEA16EFEA36FFEA571FEA772FEA973",
    "FEAA74FEAC76FEAE77FEB078FEB27AFEB47BFEB67CFEB77EFEB97FFEBB81FEBD82FEBF84FEC185FEC287FEC488",
    "FEC68AFEC88CFECA8DFECC8FFECD90FECF92FED194FED395FED597FED799FED89AFDDA9CFDDC9EFDDEA0FDE0A1",
    "FDE2A3FDE3A5FDE5A7FDE7A9FDE9AAFDEBACFCECAEFCEEB0FCF0B2FCF2B4FCF4B6FCF6B8FCF7B9FCF9BBFCFBBD",
    "FCFDBF",
);

/// The published 256-entry `cividis` table, as `RRGGBB` digits.
const CIVIDIS: &str = concat!(
    "00224E00234F00245100255300255400265600275800285900285B00295D002A5F002A61002B62002C64002C66",
    "002D68002E6A002E6C002F6D00306F0030700031700031710132710533710833700C34700F3570123570143670",
    "16377018376F1A386F1C396F1E3A6F203A6F213B6E233C6E243C6E263D6E273E6E293F6E2A3F6D2B406D2D416D",
    "2E416D2F426D31436D32436D33446D34456C35456C36466C38476C39486C3A486C3B496C3C4A6C3D4A6C3E4B6C",
    "3F4C6C404C6C414D6C424E6C434E6C444F6C45506C46516C47516C48526C49536C4A536C4B546C4C556C4D556C",
    "4E566C4F576C50576C51586D52596D535A6D545A6D555B6D555C6D565C6D575D6D585E6D595E6E5A5F6E5B606E",
    "5C616E5D616E5E626E5E636F5F636F60646F61656F62656F636670646770656870656870666970676A71686A71",
    "696B716A6C716B6D726C6D726C6E726D6F726E6F736F7073707173717274727274727374737475747475757575",
    "7676767777767777777878777979777A7A787B7A787C7B787D7C787E7C787E7D787F7E78807F78817F78828079",
    "8381798482798582798683798784788885788985788A86788B87788C88788D88788E89788F8A78908B78918B78",
    "928C78928D78938E78948E77958F779690779791779892779992779A93769B94769C95769D95769E96769F9775",
    "A09875A19975A29975A39A74A49B74A59C74A69C74A79D73A89E73A99F73AAA073ABA072ACA172ADA272AEA371",
    "AFA471B0A571B1A570B3A670B4A76FB5A86FB6A96FB7A96EB8AA6EB9AB6DBAAC6DBBAD6DBCAE6CBDAE6CBEAF6B",
    "BFB06BC0B16AC1B26AC2B369C3B369C4B468C5B568C6B667C7B767C8B866C9B965CBB965CCBA64CDBB63CEBC63",
    "CFBD62D0BE62D1BF61D2C060D3C05FD4C15FD5C25ED6C35DD7C45CD9C55CDAC65BDBC75ADCC859DDC858DEC958",
    "DFCA57E0CB56E1CC55E2CD54E4CE53E5CF52E6D051E7D150E8D24FE9D34EEAD34CEBD44BEDD54AEED649EFD748",
    "F0D846F1D945F2DA44F3DB42F5DC41F6DD3FF7DE3EF8DF3CF9E03AFBE138FCE236FDE334FEE434FEE535FEE636",
    "FEE838",
);

fn hex(code: &str) -> Rgba {
    Rgba::hex(code).unwrap()
}

fn colors(codes: &[&str]) -> Vec<Rgba> {
    codes.iter().map(|code| hex(code)).collect()
}

fn custom(codes: &[&str]) -> Cmap {
    Cmap::from_colors(&colors(codes)).unwrap()
}

/// The stop `index` of a table of `RRGGBB` digits.
fn stop(table: &str, index: usize) -> Rgba {
    hex(&format!("#{}", &table[index * 6..index * 6 + 6]))
}

fn channels(color: Rgba) -> [u8; 4] {
    [color.r, color.g, color.b, color.a]
}

/// Stops copied from matplotlib's published `_cm_listed` data, one in every 17 entries: the
/// stop index and its color.
const PUBLISHED_VIRIDIS: [(usize, &str); 16] = [
    (0, "#440154"),
    (17, "#481A6C"),
    (34, "#472F7D"),
    (51, "#414487"),
    (68, "#39568C"),
    (85, "#31688E"),
    (102, "#2A788E"),
    (119, "#23888E"),
    (136, "#1F988B"),
    (153, "#22A884"),
    (170, "#35B779"),
    (187, "#54C568"),
    (204, "#7AD151"),
    (221, "#A5DB36"),
    (238, "#D2E21B"),
    (255, "#FDE725"),
];

/// See [`PUBLISHED_VIRIDIS`].
const PUBLISHED_MAGMA: [(usize, &str); 16] = [
    (0, "#000004"),
    (17, "#0B0924"),
    (34, "#20114B"),
    (51, "#3B0F70"),
    (68, "#57157E"),
    (85, "#721F81"),
    (102, "#8C2981"),
    (119, "#A8327D"),
    (136, "#C43C75"),
    (153, "#DE4968"),
    (170, "#F1605D"),
    (187, "#FA7F5E"),
    (204, "#FE9F6D"),
    (221, "#FEBF84"),
    (238, "#FDDEA0"),
    (255, "#FCFDBF"),
];

/// See [`PUBLISHED_VIRIDIS`].
const PUBLISHED_CIVIDIS: [(usize, &str); 16] = [
    (0, "#00224E"),
    (17, "#002E6C"),
    (34, "#1E3A6F"),
    (51, "#35456C"),
    (68, "#47516C"),
    (85, "#575D6D"),
    (102, "#666970"),
    (119, "#757575"),
    (136, "#848279"),
    (153, "#948E77"),
    (170, "#A59C74"),
    (187, "#B7A96E"),
    (204, "#C8B866"),
    (221, "#DBC75A"),
    (238, "#EED649"),
    (255, "#FEE838"),
];

#[test]
fn builtin_stops_match_published_values() {
    for (name, cmap, published) in [
        ("viridis", Cmap::viridis(), PUBLISHED_VIRIDIS),
        ("magma", Cmap::magma(), PUBLISHED_MAGMA),
        ("cividis", Cmap::cividis(), PUBLISHED_CIVIDIS),
    ] {
        for (index, code) in published {
            let got = channels(cmap.at(index as f64 / 255.0));
            let want = channels(hex(code));
            for (g, w) in got.into_iter().zip(want) {
                assert!(
                    g.abs_diff(w) <= 1,
                    "{name} stop {index}: {got:?} vs {want:?}"
                );
            }
        }
    }
}

#[test]
fn builtin_tables_match_reference() {
    for (name, cmap, table) in [
        ("viridis", Cmap::viridis(), VIRIDIS),
        ("magma", Cmap::magma(), MAGMA),
        ("cividis", Cmap::cividis(), CIVIDIS),
    ] {
        assert_eq!(table.len(), 256 * 6, "{name}");
        for index in 0..256 {
            let got = channels(cmap.at(index as f64 / 255.0));
            let want = channels(stop(table, index));
            for (g, w) in got.into_iter().zip(want) {
                assert!(
                    g.abs_diff(w) <= 1,
                    "{name} stop {index}: {got:?} vs {want:?}"
                );
            }
        }
    }
}

#[test]
fn builtin_endpoints_are_the_published_colors() {
    for (cmap, first, last) in [
        (Cmap::viridis(), "#440154", "#FDE725"),
        (Cmap::magma(), "#000004", "#FCFDBF"),
        (Cmap::cividis(), "#00224E", "#FEE838"),
        (Cmap::blue_red(), "#2B5C8A", "#B23A2E"),
    ] {
        assert_eq!(cmap.at(0.0), hex(first));
        assert_eq!(cmap.at(1.0), hex(last));
    }
}

#[test]
fn blue_red_passes_through_near_white() {
    assert_eq!(Cmap::blue_red().at(0.5), hex("#F4F1EC"));
}

#[test]
fn interpolation_happens_in_oklab() {
    // Oklab lightness is perceptual, so the middle of black and white is darker than the
    // sRGB average #808080.
    assert_eq!(custom(&["#000000", "#FFFFFF"]).at(0.5), hex("#636363"));
    assert_eq!(Cmap::blue_red().at(0.25), hex("#8EA5BC"));
    assert_eq!(Cmap::blue_red().at(0.75), hex("#D9988B"));
}

#[test]
fn oklab_round_trip() {
    // A colormap whose two stops are one color returns it exactly, for colors across the gamut.
    for r in (0..=255).step_by(5) {
        for g in (0..=255).step_by(5) {
            for b in (0..=255).step_by(5) {
                let color = Rgba::from_ints(r, g, b).unwrap();
                let flat = Cmap::from_colors(&[color, color]).unwrap();
                assert_eq!(flat.at(0.37), color, "({r}, {g}, {b})");
            }
        }
    }
    for color in [
        Rgba::BLACK,
        Rgba::WHITE,
        Rgba::opaque(0, 0, 0),
        Rgba::opaque(255, 255, 254),
    ] {
        let flat = Cmap::from_colors(&[color, color]).unwrap();
        assert_eq!(flat.at(0.5), color);
    }
}

#[test]
fn reversed_twice_is_identity() {
    let own = custom(&["#EDE3C8", "#4F8F86", "#3B2A55"]);
    for cmap in [
        Cmap::viridis(),
        Cmap::magma(),
        Cmap::cividis(),
        Cmap::blue_red(),
        own,
    ] {
        assert_eq!(cmap.reversed().reversed(), cmap);
        assert_ne!(cmap.reversed(), cmap);
    }
}

#[test]
fn reversed_reads_backwards() {
    let cmap = Cmap::viridis();
    let backwards = cmap.reversed();
    for index in 0..=20 {
        let t = f64::from(index) / 20.0;
        assert_eq!(backwards.at(t), cmap.at(1.0 - t), "t = {t}");
    }
    let two = custom(&["#112233", "#445566"]);
    assert_eq!(two.reversed(), custom(&["#445566", "#112233"]));
}

#[test]
fn from_colors_endpoints_exact() {
    let palette = [
        "#C4553A", "#2E6B70", "#E2A33B", "#4F6FA3", "#86A37A", "#8B5279", "#6E4C38", "#A69F94",
        "#233F86", "#E4572E", "#F2A900", "#17A398", "#9B5DE5", "#5BA4E6", "#D9658F", "#A3A3A3",
    ];
    for count in 2..=16 {
        let stops = colors(&palette[..count]);
        let cmap = Cmap::from_colors(&stops).unwrap();
        assert_eq!(cmap.at(0.0), stops[0], "{count} colors");
        assert_eq!(cmap.at(1.0), stops[count - 1], "{count} colors");
        // The colors are spaced evenly: stop `i` sits at `i / (count - 1)`.
        for (index, want) in stops.iter().enumerate() {
            assert_eq!(
                cmap.at(index as f64 / (count - 1) as f64),
                *want,
                "{count} colors"
            );
        }
    }
}

#[test]
fn at_clamps_and_returns_opaque_colors() {
    let cmap = Cmap::viridis();
    assert_eq!(cmap.at(-3.0), cmap.at(0.0));
    assert_eq!(cmap.at(7.5), cmap.at(1.0));
    assert_eq!(cmap.at(f64::NEG_INFINITY), cmap.at(0.0));
    assert_eq!(cmap.at(f64::INFINITY), cmap.at(1.0));
    assert_eq!(cmap.at(f64::NAN), cmap.at(0.0));
    let cmap = custom(&["#112233", "#FFFFFF"]);
    assert!((0..=10).all(|i| cmap.at(f64::from(i) / 10.0).a == 255));
}

#[test]
fn colors_lists_the_stops() {
    let codes = ["#EDE3C8", "#4F8F86", "#3B2A55"];
    assert_eq!(custom(&codes).colors(), colors(&codes));
    assert_eq!(
        custom(&codes).reversed().colors(),
        colors(&["#3B2A55", "#4F8F86", "#EDE3C8"])
    );
    assert_eq!(Cmap::viridis().colors().len(), 256);
    assert_eq!(Cmap::viridis().colors()[255], hex("#FDE725"));
}

#[test]
fn from_colors_rejects_transparent_colors() {
    let colors = [Rgba::RED, Rgba::BLUE, Rgba::hex("#11223380").unwrap()];
    let err = Cmap::from_colors(&colors).unwrap_err();
    assert!(matches!(err, Error::Style { .. }), "{err:?}");
    assert_eq!(
        err.to_string(),
        "Cmap.from_colors(): colors[2] has alpha 0.5; colormap colors must be opaque."
    );
    assert_eq!(err.parameter(), Some("colors"));
    assert_eq!(
        err.hint(),
        Some("use opaque colors; set transparency on the mark instead.")
    );
    assert!(Cmap::from_colors(&[Rgba::hex("#112233FE").unwrap(), Rgba::RED]).is_err());
    assert!(Cmap::from_colors(&[Rgba::hex("#112233FF").unwrap(), Rgba::RED]).is_ok());
}

#[test]
fn from_colors_rejects_other_counts() {
    for (count, noun) in [
        (0, "0 colors"),
        (1, "1 color"),
        (17, "17 colors"),
        (40, "40 colors"),
    ] {
        let err = Cmap::from_colors(&vec![Rgba::RED; count]).unwrap_err();
        assert!(matches!(err, Error::Style { .. }), "{err:?}");
        assert_eq!(
            err.to_string(),
            format!("Cmap.from_colors(): colors has {noun}; a colormap needs 2 to 16 colors.")
        );
        assert_eq!(err.parameter(), Some("colors"));
        assert!(err.hint().is_some());
    }
    assert!(Cmap::from_colors(&[Rgba::RED; 2]).is_ok());
    assert!(Cmap::from_colors(&[Rgba::RED; 16]).is_ok());
}

#[test]
fn theme_defaults_per_preset() {
    for (theme, sequential, diverging) in [
        (
            Theme::pigment(),
            &["#EDE3C8", "#A9C08E", "#4F8F86", "#3E5B8C", "#3B2A55"][..],
            &["#2E6B70", "#F3EDE4", "#C4553A"][..],
        ),
        (
            Theme::graphite(),
            &["#D8E5F6", "#C3D7F2", "#5BA4E6", "#233F86", "#101D45"][..],
            &["#233F86", "#F5F5F5", "#E4572E"][..],
        ),
        (
            Theme::midnight(),
            &["#263A52", "#1F4E66", "#3ACED6", "#96E7A7", "#F0F7C8"][..],
            &["#75AEF5", "#2A2F3A", "#DF7F78"][..],
        ),
    ] {
        assert_eq!(theme.cmap(), &custom(sequential));
        assert_eq!(theme.diverging_cmap(), &custom(diverging));
    }
    assert_eq!(Theme::default().cmap(), Theme::pigment().cmap());
}

#[test]
fn with_cmap_replaces_one_default_and_keeps_the_original() {
    let base = Theme::graphite();
    let viridis = base.with_cmap(Cmap::viridis());
    assert_eq!(viridis.cmap(), &Cmap::viridis());
    assert_eq!(viridis.diverging_cmap(), base.diverging_cmap());
    assert_eq!(base.cmap(), Theme::graphite().cmap());
    assert_ne!(viridis, base);

    let magma = base.with_diverging_cmap(Cmap::magma());
    assert_eq!(magma.diverging_cmap(), &Cmap::magma());
    assert_eq!(magma.cmap(), base.cmap());
    assert_ne!(magma, base);
    assert_eq!(base.with_cmap(base.cmap().clone()), base);
}
