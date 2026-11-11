//! Complex scripts that reorder and stack (#604): Myanmar, Khmer, Devanagari. Cluster and line
//! breaking tests don't depend on installed fonts (a missing font draws .notdef boxes, but the
//! clusters and breaks are the same); shaping tests use a system font when one is installed,
//! and the Thai tests install a fixture face, so they run the same on every machine (#59).

use crate::*;
use wordcraft_doc::{Pos, StoryRef};

fn lay(doc: &Document) -> DocLayout {
    let mut c = LayoutCache::new();
    layout(doc, &mut c, &LayoutOptions::default())
}

/// Body paragraph `i`'s layout.
fn para_layout(l: &DocLayout, i: u32) -> &ParaLayout {
    for p in &l.pages {
        for it in &p.items {
            if let Placed::Lines { story: StoryRef::Body, path, para, .. } = it
                && path.0 == vec![i]
            {
                return para;
            }
        }
    }
    panic!("paragraph {i} not laid out");
}

/// "မြန်မာစာ": the syllables မြ | န် | မာ | စာ.
const MYANMAR: &str = "\u{1019}\u{103C}\u{1014}\u{103A}\u{1019}\u{102C}\u{1005}\u{102C}";

#[test]
fn myanmar_syllables_are_single_clusters() {
    crate::test_fonts();
    let l = lay(&Document::from_text(MYANMAR));
    let pl = para_layout(&l, 0);
    let starts: Vec<usize> = pl.clusters.iter().map(|c| c.start).collect();
    // Grapheme clusters would also start at the aa signs (bytes 15 and 21), splitting မာ and စာ.
    assert_eq!(starts, vec![0, 6, 12, 18]);
    // The caret can't land inside a syllable: every caret offset is a cluster start or the end.
    let offs = pl.line_offsets(0);
    assert!(!offs.contains(&15) && !offs.contains(&21), "{offs:?}");
    // Stacked consonants (virama) and Khmer subscripts (coeng) too.
    let l = lay(&Document::from_text("\u{1019}\u{1039}\u{1018}\u{102C} \u{1781}\u{17D2}\u{1798}\u{17C2}\u{179A}"));
    let pl = para_layout(&l, 0);
    let starts: Vec<usize> = pl.clusters.iter().map(|c| c.start).collect();
    assert_eq!(starts, vec![0, 12, 13, 25]);
}

#[test]
fn myanmar_without_spaces_wraps_between_syllables() {
    crate::test_fonts();
    let text = MYANMAR.repeat(80);
    let l = lay(&Document::from_text(&text));
    let pl = para_layout(&l, 0);
    assert!(pl.lines.len() > 1, "a long Myanmar paragraph wraps");
    let breaks = wordcraft_doc::cluster::myanmar_breaks(&text);
    // A line may end before each syllable: before မာ and စာ, never before န် (a final).
    for c in &pl.clusters {
        assert_eq!(c.break_after, breaks.contains(&c.end), "break after {}", c.end);
    }
    assert!(pl.clusters.iter().any(|c| c.break_after && c.end == 12) && !pl.clusters.iter().any(|c| c.break_after && c.end == 6));
    for line in pl.lines.iter().skip(1) {
        let start = pl.clusters[line.c0].start;
        assert!(breaks.contains(&start), "line starts at syllable {start}");
    }
    // Lines are full: a syllable break, not an emergency break after one cluster.
    assert!(pl.lines[0].c1 - pl.lines[0].c0 > 10);
}

/// A face with Myanmar letters (an installed system font), or `None` to skip.
fn myanmar_face() -> Option<wordcraft_fonts::FaceRef> {
    crate::test_fonts();
    wordcraft_fonts::FontDb::global().fallback_for('\u{1000}', 0).map(|f| wordcraft_fonts::FaceRef::of(&f)).filter(|f| f.has_layout_tables())
}

#[test]
fn myanmar_prevowel_is_drawn_before_its_consonant() {
    crate::test_fonts();
    let Some(face) = myanmar_face() else {
        eprintln!("skipped: no Myanmar font with OpenType tables installed");
        return;
    };
    // ကေ: in memory consonant first, on screen the vowel sign ေ first.
    let ka = face.glyph_for('\u{1000}');
    let g = wordcraft_fonts::shape(&face, "\u{1000}\u{1031}", &[], |c| c);
    assert_eq!(g.len(), 2, "{g:?}");
    assert_eq!(g[1].gid, ka, "the consonant is drawn second: {g:?}");
    assert!(g.iter().all(|x| x.cluster == 0), "one cluster: {g:?}");
    // Itemised by script: the same syllable after Latin text in the same face shapes the same
    // (shaped as one Latin run, the vowel sign would stay after the consonant).
    if face.covers('H') {
        let mixed = wordcraft_fonts::shape(&face, "Hi \u{1000}\u{1031}", &[], |c| c);
        let tail: Vec<u32> = mixed.iter().filter(|x| x.cluster >= 3).map(|x| x.gid).collect();
        assert_eq!(tail, g.iter().map(|x| x.gid).collect::<Vec<_>>());
    }
}

#[test]
fn myanmar_in_a_latin_font_falls_back_to_a_regular_myanmar_face() {
    crate::test_fonts();
    if myanmar_face().is_none() {
        eprintln!("skipped: no Myanmar font with OpenType tables installed");
        return;
    }
    let l = lay(&Document::from_text(MYANMAR));
    let pl = para_layout(&l, 0);
    let faces: Vec<_> = pl.clusters.iter().map(|c| pl.styles[c.style as usize].face).collect();
    assert!(faces.windows(2).all(|w| w[0].id() == w[1].id()), "one face for the word");
    let f = faces[0];
    assert!(f.has_layout_tables(), "{} can reorder Myanmar", f.family);
    assert!(!f.italic && (f.weight - 400.0).abs() <= 100.0, "a regular weight, not {} {}", f.family, f.style);
    assert!(pl.glyphs.iter().all(|g| g.gid != 0), "no missing glyphs");
    // The caret after the first syllable sits after its glyphs.
    let x6 = l.caret(&Pos::body(0, 6)).map(|c| c.x).unwrap_or(0.0);
    let x0 = l.caret(&Pos::body(0, 0)).map(|c| c.x).unwrap_or(0.0);
    assert!(x6 > x0);
}

/// The issue's run (#1549): Thai letters and digits, Latin letters, digits and the baht sign.
const THAI_RUN: &str = "[D] น้ำ ค่ำ ปั้น ฝั่ง ปู่ ๑๒๓๔๕๖๗๘๙๐ ฿1,250.50";

/// Tests that install the Thai fixture into the process-wide font database hold this lock: the
/// install is idempotent, but two at once would race (the dictionary tests lock their shared
/// state the same way).
static THAI_FONT_TESTS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The Thai fixture face's family: the bundled Source Sans 3 renamed, with the Thai block's
/// letters, digits and signs (U+0E01..=U+0E5B) mapped onto a spare cmap group. Word's Thai
/// substitutes (Cordia New, Leelawadee UI, Tahoma…) carry Latin letters and digits beside the
/// Thai ones, and so does the fixture: it covers [`THAI_RUN`].
const THAI_FIXTURE: &str = "Thai Fixture1";

/// The fixture font file, built the way the fonts crate's system-font fixtures are built: the
/// family names patched in place (the replacement is as long as "Source Sans 3"), and the
/// format-12 cmap group mapping U+1F916 (a codepoint nothing else uses) remapped to the Thai
/// block, with the groups sorted again around it so lookups still find them.
fn thai_fixture_font() -> Vec<u8> {
    let mut data = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/fonts/SourceSans3-Regular.ttf")).unwrap();
    assert_eq!(THAI_FIXTURE.len(), "Source Sans 3".len(), "the names patch in place");
    let utf16 = |s: &str| s.encode_utf16().flat_map(u16::to_be_bytes).collect::<Vec<u8>>();
    for (from, to) in [(utf16("Source Sans 3"), utf16(THAI_FIXTURE)), (b"Source Sans 3".to_vec(), THAI_FIXTURE.as_bytes().to_vec())] {
        let mut i = 0;
        while let Some(at) = data[i..].windows(from.len()).position(|w| w == from) {
            data[i + at..i + at + to.len()].copy_from_slice(&to);
            i += at + to.len();
        }
    }
    let tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    let mut cmap = 0;
    for r in 0..tables {
        let at = 12 + r * 16;
        if &data[at..at + 4] == b"cmap" {
            cmap = u32::from_be_bytes(data[at + 8..at + 12].try_into().unwrap()) as usize;
        }
    }
    let records = u16::from_be_bytes([data[cmap + 2], data[cmap + 3]]) as usize;
    let mut seen: Vec<usize> = Vec::new();
    for r in 0..records {
        let at = cmap + 4 + r * 8;
        let off = u32::from_be_bytes(data[at + 4..at + 8].try_into().unwrap()) as usize + cmap;
        // Several encoding records can point at one subtable; patch each subtable once.
        if u16::from_be_bytes([data[off], data[off + 1]]) != 12 || seen.contains(&off) {
            continue;
        }
        seen.push(off);
        let n = u32::from_be_bytes(data[off + 12..off + 16].try_into().unwrap()) as usize;
        let mut groups = Vec::with_capacity(n);
        for g in 0..n {
            let o = off + 16 + g * 12;
            groups.push((
                u32::from_be_bytes(data[o..o + 4].try_into().unwrap()),
                u32::from_be_bytes(data[o + 4..o + 8].try_into().unwrap()),
                u32::from_be_bytes(data[o + 8..o + 12].try_into().unwrap()),
            ));
        }
        let mut patched = 0;
        for g in &mut groups {
            if g.0 == 0x1F916 {
                *g = (0x0E01, 0x0E5B, g.2);
                patched += 1;
            }
        }
        assert!(patched > 0, "the spare group is where the fonts crate's fixtures patch it");
        groups.sort();
        for (g, (start, end, gid)) in groups.iter().enumerate() {
            let o = off + 16 + g * 12;
            data[o..o + 4].copy_from_slice(&start.to_be_bytes());
            data[o + 4..o + 8].copy_from_slice(&end.to_be_bytes());
            data[o + 8..o + 12].copy_from_slice(&gid.to_be_bytes());
        }
    }
    data
}

/// Install the Thai fixture as a user font of the process-wide database, so the Thai tests
/// below run the same on every machine: which Thai fonts the system happens to have installed
/// (a CI runner with none, or one whose only Thai font isn't in Word's substitute lists, the
/// red CI of #59) no longer decides whether they run or pass. Callers hold [`THAI_FONT_TESTS`].
fn install_thai_fixture() {
    let db = wordcraft_fonts::FontDb::global();
    if db.has_family(THAI_FIXTURE) {
        return;
    }
    let installed = db.install_font("TestThaiFont.ttf", &thai_fixture_font());
    assert!(installed.is_ok(), "the Thai fixture installs: {installed:?}");
}

/// Body paragraph 0 of `text`, its run set in `font` at 16 pt (`None` keeps the document's
/// default font), and the face of every cluster: (cluster start, face).
fn cluster_faces(text: &str, font: Option<&str>) -> Vec<(usize, wordcraft_fonts::FaceRef)> {
    let mut d = Document::from_text(text);
    let p = d.para_mut(StoryRef::Body, &Path::top(0)).unwrap();
    p.runs[0].props.font = font.map(Into::into);
    p.runs[0].props.size = Some(16.0);
    let l = lay(&d);
    let pl = para_layout(&l, 0);
    pl.clusters.iter().map(|c| (c.start, pl.styles[c.style as usize].face)).collect()
}

#[test]
fn a_missing_thai_font_draws_the_whole_run_in_one_thai_face() {
    crate::test_fonts();
    let _lock = THAI_FONT_TESTS.lock().unwrap();
    install_thai_fixture();
    // Word substitutes one font (Cordia New) for a missing Thai font, so the Latin letters, the
    // digits and the baht sign keep the Thai letters' face and size. A Thai name the lists don't
    // spell out ("TH Niramit AS") is matched by its "TH " prefix and substituted the same way.
    // The substitute covers the run where a listed Thai font is installed, and the run moves to
    // the fixture face where none is, so the test no longer depends on the machine's fonts.
    let db = wordcraft_fonts::FontDb::global();
    for font in ["TH Sarabun New", "Angsana New", "TH Niramit AS"] {
        if db.has_family(font) {
            continue;
        }
        let faces = cluster_faces(THAI_RUN, Some(font));
        let first = faces[0].1;
        let names: Vec<&str> = faces.iter().map(|(_, f)| f.family.as_str()).collect();
        assert!(faces.iter().all(|(_, f)| f.id() == first.id()), "{font}: one face for the run, not {names:?}");
        assert!(first.covers('\u{0E19}') && first.covers('D') && first.covers('1') && first.covers('\u{0E3F}'), "{font} → {}", first.family);
    }
}

#[test]
fn a_thai_substitute_without_latin_hands_the_run_to_one_face_covering_it() {
    crate::test_fonts();
    let _lock = THAI_FONT_TESTS.lock().unwrap();
    install_thai_fixture();
    // When the substitute lacks the run's Thai letters (no listed Thai font is installed, so the
    // Latin fallback family stands in) or its Latin ones, the run moves to one face covering all.
    let latin = wordcraft_fonts::word::resolve("Source Sans 3", false, false).face;
    let face = crate::para::thai_run_face("TH Sarabun New", true, latin, THAI_RUN, false, false).expect("one face covers the run");
    assert_ne!(face.id(), latin.id());
    for c in THAI_RUN.chars().filter(|c| !c.is_whitespace()) {
        assert!(face.covers(c), "{} covers {c}", face.family);
    }
    // Only for a Thai family that was substituted.
    assert!(crate::para::thai_run_face("Calibri", true, latin, THAI_RUN, false, false).is_none());
    assert!(crate::para::thai_run_face("TH Sarabun New", false, latin, THAI_RUN, false, false).is_none());
    // A substitute that already covers the run keeps it.
    assert!(crate::para::thai_run_face("TH Sarabun New", true, face, THAI_RUN, false, false).is_none());
    // Text that is neither Thai nor Latin doesn't make the run move.
    assert!(crate::para::thai_run_face("TH Sarabun New", true, latin, "abc \u{2603}", false, false).is_none());
}

#[test]
fn latin_text_in_a_missing_latin_font_stays_in_its_substitute_beside_thai() {
    crate::test_fonts();
    let _lock = THAI_FONT_TESTS.lock().unwrap();
    install_thai_fixture();
    // Word keeps Latin text in the Latin font's substitute; only the Thai (or Myanmar) letters
    // fall back. A missing Latin family (and the default body font, Aptos, wherever it is
    // missing) must not move "Hello" into a Thai face.
    let myanmar = myanmar_face().is_some();
    for font in [Some("No Such Latin Font 1549"), None] {
        let requested = font.unwrap_or(wordcraft_doc::styles::BODY_FONT);
        let latin = wordcraft_fonts::word::resolve(requested, false, false).face;
        for text in ["Hello \u{0E2A}\u{0E27}\u{0E31}\u{0E2A}\u{0E14}\u{0E35}", "Hello \u{1019}\u{103C}\u{1014}\u{103A}\u{1019}\u{102C}"] {
            if text.contains('\u{1019}') && !myanmar {
                continue;
            }
            let faces = cluster_faces(text, font);
            let hello: Vec<_> = faces.iter().filter(|(at, _)| *at < 5).collect();
            assert_eq!(hello.len(), 5, "{faces:?}");
            assert!(
                hello.iter().all(|(_, f)| f.id() == latin.id()),
                "{requested}: Hello in {}, not {:?}",
                latin.family,
                hello.iter().map(|(_, f)| f.family.as_str()).collect::<Vec<_>>()
            );
            assert!(faces.iter().any(|(at, f)| *at >= 6 && f.id() != latin.id()), "the other script falls back");
        }
    }
}

#[test]
fn thai_in_an_installed_latin_font_keeps_the_latin_face_for_latin() {
    crate::test_fonts();
    let _lock = THAI_FONT_TESTS.lock().unwrap();
    install_thai_fixture();
    // The family exists (Source Sans 3 is bundled), so nothing is substituted: Latin stays in it
    // and only the Thai letters fall back, character by character, as before.
    let faces = cluster_faces(THAI_RUN, Some("Source Sans 3"));
    let latin = wordcraft_fonts::word::resolve("Source Sans 3", false, false).face;
    assert!(faces.iter().any(|(_, f)| f.id() == latin.id()), "Latin clusters stay in Source Sans 3");
    assert!(faces.iter().any(|(_, f)| f.id() != latin.id() && f.covers('\u{0E19}')), "Thai clusters fall back to a Thai face");
    assert_eq!(faces[0].1.id(), latin.id(), "[D] opens the run in Source Sans 3");
}

#[test]
fn a_missing_latin_font_with_a_thai_complex_script_font_keeps_both() {
    crate::test_fonts();
    let _lock = THAI_FONT_TESTS.lock().unwrap();
    install_thai_fixture();
    // A run with its own complex-script font (`w:cs`) draws the Thai letters in that font's
    // substitute and the Latin letters in the Latin font's substitute, as Word does. Where the
    // machine has none of Word's Thai substitutes (the red CI of #59: a Thai font installed
    // that the lists don't name), the Thai letters move to the one face that covers them
    // instead — never into the Latin substitute.
    let mut d = Document::from_text(THAI_RUN);
    let p = d.para_mut(StoryRef::Body, &Path::top(0)).unwrap();
    p.runs[0].props.font = Some("No Such Latin Font 1549".into());
    p.runs[0].props.font_cs = Some("TH Sarabun New".into());
    let l = lay(&d);
    let pl = para_layout(&l, 0);
    let faces: Vec<_> = pl.clusters.iter().map(|c| pl.styles[c.style as usize].face).collect();
    let latin = wordcraft_fonts::word::resolve("No Such Latin Font 1549", false, false).face;
    let thai = wordcraft_fonts::word::resolve("TH Sarabun New", false, false).face;
    assert_eq!(faces[0].id(), latin.id(), "[D] in the Latin substitute");
    if thai.covers('\u{0E19}') {
        assert!(faces.iter().any(|f| f.id() == thai.id()), "Thai letters in the Thai substitute {}", thai.family);
    } else {
        assert!(
            faces.iter().any(|f| f.id() != latin.id() && f.covers('\u{0E19}')),
            "Thai letters in a face that covers them, not the Latin substitute"
        );
    }
}

/// Regression for #59's red CI: which Thai fonts the machine happens to have installed no
/// longer decides the Thai tests' outcome. Even where no listed Thai substitute is installed
/// (the run's font missing, the substitute without Thai letters), the run draws in one face
/// that covers it — the fixture stands in for the installed font on every machine.
#[test]
fn a_thai_run_with_no_listed_substitute_still_draws_in_one_face() {
    let _lock = THAI_FONT_TESTS.lock().unwrap();
    install_thai_fixture();
    let faces = cluster_faces(THAI_RUN, Some("TH Niramit AS"));
    let first = faces[0].1;
    let names: Vec<&str> = faces.iter().map(|(_, f)| f.family.as_str()).collect();
    assert!(faces.iter().all(|(_, f)| f.id() == first.id()), "one face for the run, not {names:?}");
    for c in THAI_RUN.chars().filter(|c| !c.is_whitespace()) {
        assert!(first.covers(c), "{} covers {c}", first.family);
    }
}

#[test]
fn a_missing_roman_thai_font_is_not_drawn_in_cambria() {
    crate::test_fonts();
    // Word draws a missing font the font table classes as roman in Cambria, but a missing Thai
    // font keeps its Thai substitute: the same face whether or not the table calls it roman.
    let db = wordcraft_fonts::FontDb::global();
    if db.has_family("TH Sarabun New") {
        eprintln!("skipped: TH Sarabun New is installed");
        return;
    }
    let face = |roman: bool| {
        let mut d = Document::from_text("\u{0E19}\u{0E49}\u{0E33} Thai");
        if roman {
            d.settings.roman_fonts = vec!["TH Sarabun New".into()];
        }
        let p = d.para_mut(StoryRef::Body, &Path::top(0)).unwrap();
        p.runs[0].props.font = Some("TH Sarabun New".into());
        let l = lay(&d);
        let pl = para_layout(&l, 0);
        pl.styles[pl.clusters.last().expect("a cluster").style as usize].face
    };
    let cambria = wordcraft_fonts::word::resolve("Cambria", false, false).face;
    assert_ne!(face(true).id(), cambria.id());
    assert_eq!(face(true).id(), face(false).id());
}

/// `w14:ligatures` (Word writes `historicalDiscretional` on whole Khmer documents) turns `liga`
/// and `clig` off, which breaks Khmer OS syllables: Khmer text is shaped the same either way.
#[test]
fn khmer_ignores_the_ligature_setting() {
    use wordcraft_doc::props::{CharProps, Ligatures};
    let text = "\u{179F}\u{17CA}\u{17BE}\u{1794}\u{17A2}\u{1784}\u{17D2}\u{1780}\u{17C1}\u{178F} \u{1795}\u{17D2}\u{178F}\u{1793}\u{17D2}\u{1791}\u{17B6}\u{1791}\u{17C4}\u{179F}";
    let gids = |ligatures: Option<Ligatures>| {
        let chr = CharProps { font: Some("Khmer OS Siemreap".into()), size: Some(12.0), ligatures, ..Default::default() };
        let mut d = Document::new();
        d.body = vec![wordcraft_doc::para_block(wordcraft_doc::Paragraph::with_text(text, chr))];
        let l = lay(&d);
        para_layout(&l, 0).glyphs.iter().map(|g| g.gid).collect::<Vec<_>>()
    };
    let hd = Ligatures { historical: true, discretional: true, ..Default::default() };
    assert_eq!(gids(Some(hd)), gids(None));
    assert_eq!(gids(Ligatures::from_ooxml("none")), gids(None));
}
