//! `MultiParagraph`, a text split into paragraphs by its paragraph styles (D2047): Compose's
//! `MultiParagraphTest` (finding a paragraph) and `MultiParagraphIntegrationTest`, laid out by
//! the fixed measurer with Compose's test font metrics (`FixedFontMetrics::SAMPLE_FONT`: one em
//! wide, lines one em high, the baseline at 0.8 em), as Compose's tests lay out with its
//! `sample_font`. Compose's tests with negative offsets have no counterpart (offsets are `usize`).

use std::rc::Rc;

use compose_ui::platform::{
    FixedFontMetrics, FixedTextMeasurer, ParagraphInfo, ParagraphLayout, ParagraphSize,
    SharedTextMeasurer, find_paragraph_by_index, find_paragraph_by_line_index, find_paragraph_by_y,
};
use compose_ui::{
    AnnotatedRange, AnnotatedString, BaselineShift, Constraints, Density, FontFamilyResolver,
    LocaleList, MultiParagraph, MultiParagraphIntrinsics, Offset, Paragraph, ParagraphStyle, Path,
    PathCommand, Placeholder, PlaceholderVerticalAlign, Rect, ResolvedTextDirection, SpanStyle,
    TextAlign, TextDirection, TextIndent, TextOverflow, TextRange, TextStyle, TextUnit, UnitExt,
    create_font_family_resolver,
};

const HEBREW: char = '\u{05D0}';

fn resolver() -> FontFamilyResolver {
    create_font_family_resolver(&SharedTextMeasurer::new(Box::new(
        FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT),
    )))
}

/// Compose's `TEST_LOCALE_LIST`.
fn test_locale_list() -> LocaleList {
    LocaleList::from_language_tags("en")
}

/// Compose's `createAnnotatedString`: each string a paragraph of its own.
fn paragraphs(texts: &[&str]) -> AnnotatedString {
    AnnotatedString::build(|builder| {
        for text in texts {
            builder.push_paragraph_style(ParagraphStyle::DEFAULT);
            builder.append(text);
            builder.pop();
        }
    })
}

/// What Compose's `simpleMultiParagraph` takes.
struct Setup {
    style: TextStyle,
    font_size: TextUnit,
    max_lines: u32,
    /// Compose's `width`: unbounded when `None` (its `Float.MAX_VALUE`).
    width: Option<f32>,
    locale_list: Option<LocaleList>,
    placeholders: Vec<AnnotatedRange<Placeholder>>,
    overflow: TextOverflow,
}

impl Default for Setup {
    fn default() -> Self {
        Setup {
            style: TextStyle::DEFAULT,
            font_size: TextUnit::UNSPECIFIED,
            max_lines: u32::MAX,
            width: None,
            locale_list: None,
            placeholders: Vec::new(),
            overflow: TextOverflow::Clip,
        }
    }
}

fn style_of(setup: &Setup) -> TextStyle {
    TextStyle {
        font_size: setup.font_size,
        locale_list: setup.locale_list.clone(),
        ..TextStyle::DEFAULT
    }
    .merge(&setup.style)
}

/// Compose's `simpleMultiParagraphIntrinsics`.
fn intrinsics(text: AnnotatedString, setup: &Setup) -> MultiParagraphIntrinsics {
    MultiParagraphIntrinsics::new(
        text,
        &style_of(setup),
        &setup.placeholders,
        Density::new(1.0),
        resolver(),
        true,
        test_locale_list(),
    )
}

/// Compose's `simpleMultiParagraph`.
fn multi(text: AnnotatedString, setup: Setup) -> MultiParagraph {
    let max_width = setup.width.map_or(Constraints::INFINITY, |width| width.ceil() as i32);
    MultiParagraph::new(
        Rc::new(intrinsics(text, &setup)),
        Constraints::new(0, max_width, 0, Constraints::INFINITY),
        setup.max_lines,
        setup.overflow,
    )
}

/// `text` as one paragraph.
fn simple(text: &str, setup: Setup) -> MultiParagraph {
    multi(paragraphs(&[text]), setup)
}

fn sized(font_size: f32) -> Setup {
    Setup { font_size: font_size.sp(), ..Setup::default() }
}

/// A paragraph that's never asked anything: Compose's `mock()`.
fn mock() -> Rc<dyn Paragraph> {
    let size = ParagraphSize {
        width: 0.0,
        height: 0.0,
        min_intrinsic_width: 0.0,
        max_intrinsic_width: 0.0,
        did_exceed_max_lines: false,
        ellipsized: false,
    };
    Rc::new(ParagraphLayout::from_lines(String::new(), size, Vec::new()))
}

// Compose: compose/ui/ui-text/src/androidHostTest/kotlin/androidx/compose/ui/text/MultiParagraphTest.kt, findParagraphByIndex
#[test]
fn the_paragraph_of_an_index_covers_it() {
    let infos: Vec<ParagraphInfo> =
        (0..5).map(|index| ParagraphInfo::new(mock(), index * 10, index * 10 + 10)).collect();
    for index in 0..50 {
        assert_eq!(find_paragraph_by_index(&infos, index), index / 10, "{index}");
    }
}

// Compose: compose/ui/ui-text/src/androidHostTest/kotlin/androidx/compose/ui/text/MultiParagraphTest.kt, findParagraphByLineIndex
#[test]
fn the_paragraph_of_a_line_holds_it() {
    let infos: Vec<ParagraphInfo> = (0..5)
        .map(|index| ParagraphInfo {
            start_line_index: index * 10,
            end_line_index: index * 10 + 10,
            ..ParagraphInfo::new(mock(), 0, 0)
        })
        .collect();
    for line in 0..50 {
        assert_eq!(find_paragraph_by_line_index(&infos, line), line / 10, "{line}");
    }
}

// Compose: compose/ui/ui-text/src/androidHostTest/kotlin/androidx/compose/ui/text/MultiParagraphTest.kt, findParagraphByYPosition
#[test]
fn the_paragraph_at_a_height_spans_it() {
    let infos: Vec<ParagraphInfo> = (0..5)
        .map(|index| ParagraphInfo {
            top: index as f32 * 10.0,
            bottom: index as f32 * 10.0 + 10.0,
            ..ParagraphInfo::new(mock(), 0, 0)
        })
        .collect();
    for y in 0..50 {
        assert_eq!(find_paragraph_by_y(&infos, y as f32), y / 10, "{y}");
    }
}

// Compose: compose/ui/ui-text/src/androidHostTest/kotlin/androidx/compose/ui/text/MultiParagraphTest.kt, paragraphInfo_toLocalLineIndex
#[test]
fn a_line_of_the_text_is_a_line_of_its_paragraph() {
    let info = ParagraphInfo {
        start_line_index: 10,
        end_line_index: 20,
        ..ParagraphInfo::new(mock(), 0, 0)
    };
    assert_eq!(info.to_local_line_index(10), 0);
    assert_eq!(info.to_local_line_index(15), 5);
    assert_eq!(info.to_local_line_index(19), 9);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, didExceedMaxLines_withLineFeed
#[test]
fn lines_past_the_maximum_across_paragraphs_exceed_it() {
    // Three lines: "a", "b", "c".
    let text = paragraphs(&["a\nb", "c"]);
    for max_lines in 1..=2 {
        let paragraph = multi(text.clone(), Setup { max_lines, ..Setup::default() });
        assert!(paragraph.did_exceed_max_lines(), "max_lines = {max_lines}");
    }
    for max_lines in 3..=5 {
        let paragraph = multi(text.clone(), Setup { max_lines, ..Setup::default() });
        assert!(!paragraph.did_exceed_max_lines(), "max_lines = {max_lines}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, didExceedMaxLines_withLineWrap
#[test]
fn wrapped_lines_past_the_maximum_exceed_it() {
    // One character a line: "a", "b", "c".
    let text = paragraphs(&["ab", "c"]);
    for max_lines in 1..=2 {
        let setup = Setup { max_lines, width: Some(50.0), ..sized(50.0) };
        assert!(multi(text.clone(), setup).did_exceed_max_lines(), "max_lines = {max_lines}");
    }
    for max_lines in 3..=5 {
        let setup = Setup { max_lines, ..sized(50.0) };
        assert!(!multi(text.clone(), setup).did_exceed_max_lines(), "max_lines = {max_lines}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textOverflow_exceedMaxLines_singleParagraph
#[test]
fn a_single_paragraph_past_the_maximum_exceeds_it() {
    let paragraph = multi(paragraphs(&["a\nb"]), Setup { max_lines: 1, ..Setup::default() });
    assert!(paragraph.paragraph_info_list()[0].paragraph.did_exceed_max_lines());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textOverflow_exceedMaxLinesInMiddle_multiParagraph
#[test]
fn the_paragraph_reaching_the_maximum_exceeds_it() {
    let paragraph =
        multi(paragraphs(&["a\nb", "a\nb"]), Setup { max_lines: 3, ..Setup::default() });
    assert!(paragraph.paragraph_info_list()[1].paragraph.did_exceed_max_lines());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textOverflow_exceedMaxLinesInGap_multiParagraph
#[test]
fn a_paragraph_after_the_maximum_is_not_laid_out() {
    let paragraph = multi(paragraphs(&["a\nb", "a"]), Setup { max_lines: 2, ..Setup::default() });
    assert_eq!(paragraph.paragraph_info_list().len(), 1);
    assert!(paragraph.did_exceed_max_lines());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getPathForRange
#[test]
fn a_range_s_path_covers_each_paragraph_s_part() {
    let size = 20.0;
    let paragraph = multi(paragraphs(&["ab", "c", "de"]), sized(size));
    // "bcd": Compose compares the areas with a path operation, which needs a platform here.
    let actual = path_rects(&paragraph.get_path_for_range(1, 4));
    assert_eq!(
        actual,
        [
            Rect::new(size, 0.0, size * 2.0, size),
            Rect::new(0.0, size, size, size * 2.0),
            Rect::new(0.0, size * 2.0, size, size * 3.0),
        ]
    );
}

/// The bounds of each contour of `path`: the rectangles it was made of.
fn path_rects(path: &Path) -> Vec<Rect> {
    let mut rects: Vec<Rect> = Vec::new();
    let mut current: Option<Rect> = None;
    for command in path.data().commands() {
        let points: Vec<Offset> = match *command {
            PathCommand::MoveTo(point) => {
                rects.extend(current.take());
                vec![point]
            }
            PathCommand::LineTo(point) => vec![point],
            PathCommand::Close => continue,
            _ => panic!("a selection is made of rectangles: {command:?}"),
        };
        for point in points {
            current = Some(match current {
                Some(rect) => Rect::new(
                    rect.left.min(point.x),
                    rect.top.min(point.y),
                    rect.right.max(point.x),
                    rect.bottom.max(point.y),
                ),
                None => Rect::new(point.x, point.y, point.x, point.y),
            });
        }
    }
    rects.extend(current);
    rects
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getPathForRange_throws_exception_if_start_larger_than_end
#[test]
#[should_panic(expected = "out of range")]
fn a_path_for_a_reversed_range_panics() {
    simple("abc", Setup::default()).get_path_for_range(3, 0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getPathForRange_throws_exception_if_end_is_larger_than_text_length
#[test]
#[should_panic(expected = "out of range")]
fn a_path_past_the_text_panics() {
    simple("abc", Setup::default()).get_path_for_range(0, 4);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getPathForRange_throws_exception_if_end_is_larger_than_last_line_end
#[test]
#[should_panic(expected = "should be less or equal than last line's end")]
fn a_path_in_a_paragraph_not_laid_out_panics() {
    multi(paragraphs(&["ab", "cd"]), Setup { max_lines: 1, ..Setup::default() })
        .get_path_for_range(3, 4);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getOffsetForPosition
#[test]
fn the_offset_at_a_position_is_in_the_paragraph_there() {
    let size = 50.0;
    let text = paragraphs(&["aa", "aa", "aa"]);
    let length = text.len();
    let paragraph = multi(text, Setup { width: Some(2.0 * size), ..sized(size) });
    for index in 0..length {
        let (row, column) = ((index / 2) as f32, (index % 2) as f32);
        let position = Offset::new(size * column, size / 2.0 + size * row);
        assert_eq!(paragraph.get_offset_for_position(position), index, "{position:?}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getOffsetForPosition_emptyLine
#[test]
fn the_offset_on_an_empty_paragraph_is_its_start() {
    let size = 50.0;
    let paragraph =
        multi(paragraphs(&["aa", ""]), Setup { width: Some(2.0 * size), ..sized(size) });
    assert_eq!(paragraph.get_offset_for_position(Offset::new(size, size * 1.5)), 2);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getOffsetForPosition_emptyLines
#[test]
fn the_offset_on_one_of_empty_paragraphs_is_its_start() {
    let size = 50.0;
    let setup = Setup { width: Some(2.0 * size), ..sized(size) };
    let paragraph = multi(paragraphs(&["aa", "", ""]), setup);
    assert_eq!(paragraph.get_offset_for_position(Offset::new(size, size * 1.5)), 2);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getOffsetForPosition_longSecondParagraph_maxLines2_beforeAPI26
#[test]
fn below_a_cut_paragraph_the_offset_is_on_its_last_line() {
    // As Android before API 26: the last line shown ends where it's cut. API 26 and later
    // answer the paragraph's end (100,003).
    let size = 50.0;
    let long = "A".repeat(100_000);
    let setup = Setup { width: Some(100.0 * size), max_lines: 2, ..sized(size) };
    let paragraph = multi(paragraphs(&["AAA", &long]), setup);
    assert_eq!(paragraph.get_offset_for_position(Offset::new(size / 2.0, size * 2.5)), 3);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getOffsetForPosition_longFirstParagraph_maxLines2_beforeAPI26
#[test]
fn below_a_cut_first_paragraph_the_offset_is_on_its_last_line() {
    let size = 50.0;
    let long = "A".repeat(100_000);
    let setup = Setup { width: Some(100.0 * size), max_lines: 2, ..sized(size) };
    let paragraph = multi(paragraphs(&[&long, "AAA"]), setup);
    assert_eq!(paragraph.get_offset_for_position(Offset::new(size / 2.0, size * 2.5)), 100);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineForVerticalPosition
#[test]
fn the_line_at_a_height_is_in_the_paragraph_there() {
    let size = 50.0;
    let paragraph =
        multi(paragraphs(&["aaaa", "aaaa"]), Setup { width: Some(4.0 * size), ..sized(size) });
    for step in -1..=6 {
        let lines = step as f32 * 0.5;
        let expected = (lines as i32).clamp(0, 1) as usize;
        assert_eq!(paragraph.get_line_for_vertical_position(size * lines), expected, "{lines}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineForVerticalPosition_emptyLine
#[test]
fn the_line_of_an_empty_paragraph_is_its_first() {
    let size = 50.0;
    let paragraph =
        multi(paragraphs(&["aaaa", ""]), Setup { width: Some(4.0 * size), ..sized(size) });
    assert_eq!(paragraph.get_line_for_vertical_position(size * 1.5), 1);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineForVerticalPosition_emptyLines
#[test]
fn the_line_of_one_of_empty_paragraphs_is_its_first() {
    let size = 50.0;
    let paragraph =
        multi(paragraphs(&["aaaa", "", ""]), Setup { width: Some(4.0 * size), ..sized(size) });
    assert_eq!(paragraph.get_line_for_vertical_position(size * 1.5), 1);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineForVerticalPosition_longSecondParagraph_maxLines2
#[test]
fn below_a_cut_paragraph_the_line_is_the_last_shown() {
    let size = 50.0;
    let long = "A".repeat(100_000);
    let setup = Setup { width: Some(100.0 * size), max_lines: 2, ..sized(size) };
    let paragraph = multi(paragraphs(&["AAA", &long]), setup);
    assert_eq!(paragraph.get_line_for_vertical_position(size * 2.5), 1);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineForVerticalPosition_longFirstParagraph_maxLines2
#[test]
fn below_a_cut_first_paragraph_the_line_is_the_last_shown() {
    let size = 50.0;
    let long = "A".repeat(100_000);
    let setup = Setup { width: Some(100.0 * size), max_lines: 2, ..sized(size) };
    let paragraph = multi(paragraphs(&[&long, "AAA"]), setup);
    assert_eq!(paragraph.get_line_for_vertical_position(size * 2.5), 1);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getBoundingBox
#[test]
fn a_character_s_box_is_in_its_paragraph() {
    let size = 50.0;
    let text = paragraphs(&["aa", "aa", "aa"]);
    let length = text.len();
    let paragraph = multi(text, Setup { width: Some(length as f32 * size), ..sized(size) });
    for index in 0..length {
        let (row, column) = ((index / 2) as f32, (index % 2) as f32);
        let expected =
            Rect::new(column * size, row * size, (column + 1.0) * size, (row + 1.0) * size);
        assert_eq!(paragraph.get_bounding_box(index), expected, "{index}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getBoundingBox_offset_larger_than_length_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_box_past_the_text_panics() {
    simple("abc", Setup::default()).get_bounding_box(4);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getBoundingBox_multiParagraph_truncatedByMaxLines
#[test]
fn the_box_of_a_character_cut_by_the_maximum_lines_is_where_the_text_stops() {
    let text = AnnotatedString::build(|builder| {
        for word in ["one", "two", "three", "four"] {
            builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
                builder.append(word);
            });
        }
    });
    let length = text.len();
    let paragraph = multi(text, Setup { max_lines: 2, ..Setup::default() });
    let last_visible = paragraph.get_bounding_box(5);
    for index in 6..length {
        let rect = paragraph.get_bounding_box(index);
        assert_eq!(rect.left, last_visible.right, "{index}");
        assert_eq!(rect.right, last_visible.right, "{index}");
        assert_eq!(rect.top, last_visible.top, "{index}");
        assert_eq!(rect.bottom, last_visible.bottom, "{index}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getHorizontalPosition
#[test]
fn a_horizontal_position_is_its_paragraph_s() {
    let size = 50.0;
    let text = paragraphs(&["aa", "aa", "aa"]);
    let length = text.len();
    let paragraph = multi(text, sized(size));
    for index in 0..length {
        let expected = size * (index % 2) as f32;
        assert_eq!(paragraph.get_horizontal_position(index, true), expected, "{index}");
    }
    assert_eq!(paragraph.get_horizontal_position(length, true), size * 2.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getHorizontalPosition_larger_than_length_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_horizontal_position_past_the_text_panics() {
    simple("abc", Setup::default()).get_horizontal_position(4, true);
}

/// "a", Hebrew, " ": three paragraphs going by their first strong character.
fn three_directions(setup: Setup) -> MultiParagraph {
    multi(paragraphs(&["a", &HEBREW.to_string(), " "]), setup)
}

/// The paragraph direction at each character offset (by Compose's character indices).
fn directions(paragraph: &MultiParagraph) -> Vec<ResolvedTextDirection> {
    let text = paragraph.text().to_string();
    let mut offsets: Vec<usize> = text.char_indices().map(|(offset, _)| offset).collect();
    offsets.push(text.len());
    offsets.into_iter().map(|offset| paragraph.get_paragraph_direction(offset)).collect()
}

use ResolvedTextDirection::{Ltr, Rtl};

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getParagraphDirection_textDirection_Default
#[test]
fn without_a_direction_each_paragraph_goes_by_its_first_strong_character() {
    let paragraph = three_directions(Setup {
        locale_list: Some(LocaleList::from_language_tags("en")),
        ..Setup::default()
    });
    assert_eq!(directions(&paragraph), [Ltr, Rtl, Ltr, Ltr]);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getParagraphDirection_textDirection_Content_withLtrLocale
#[test]
fn content_with_a_left_to_right_locale_falls_back_to_left_to_right() {
    let style = TextStyle {
        text_direction: Some(TextDirection::Content),
        locale_list: Some(LocaleList::from_language_tags("en")),
        ..TextStyle::DEFAULT
    };
    assert_eq!(
        directions(&three_directions(Setup { style, ..Setup::default() })),
        [Ltr, Rtl, Ltr, Ltr]
    );
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getParagraphDirection_textDirection_Content_withRtlLocale
#[test]
fn content_with_a_right_to_left_locale_falls_back_to_right_to_left() {
    let style = TextStyle {
        text_direction: Some(TextDirection::Content),
        locale_list: Some(LocaleList::from_language_tags("ar")),
        ..TextStyle::DEFAULT
    };
    assert_eq!(
        directions(&three_directions(Setup { style, ..Setup::default() })),
        [Ltr, Rtl, Rtl, Rtl]
    );
}

/// The direction a neutral paragraph whose direction is `Content` takes in `locale`.
fn neutral_direction_in(locale: &str) -> ResolvedTextDirection {
    let style = TextStyle {
        text_direction: Some(TextDirection::Content),
        locale_list: Some(LocaleList::from_language_tags(locale)),
        ..TextStyle::DEFAULT
    };
    simple("1", Setup { style, ..Setup::default() }).get_paragraph_direction(0)
}

/// As ICU's `ULocale.isRightToLeft` (D2049): a script given decides; else ICU's few common
/// languages, whatever their region; else the likely script of the language in its region.
#[test]
fn a_locale_s_direction_is_its_script_s_or_its_likely_script_s() {
    // Scripts given, right to left in ICU's script data, historic ones too.
    for locale in [
        "en-Arab", "tr-Hebr", "arc-Armi", "arc-Nbat", "arc-Palm", "pal-Phlp", "grc-Cprt",
        "hu-Hung", "ku-Yezi", "sy-Syrc", "ff-Adlm", "men-Mend",
    ] {
        assert_eq!(neutral_direction_in(locale), Rtl, "{locale}");
    }
    // A script given left to right wins over the language's own.
    for locale in ["ar-Latn", "fa-Cyrl", "he-Latn", "ur-Deva"] {
        assert_eq!(neutral_direction_in(locale), Ltr, "{locale}");
    }
    // ICU's fast path for common languages takes no region into account.
    for locale in ["ar-US", "he-DE", "fa-AF"] {
        assert_eq!(neutral_direction_in(locale), Rtl, "{locale}");
    }
    for locale in ["en-IR", "en-SA", "fr-MA", "tr-IQ", "ru-IL"] {
        assert_eq!(neutral_direction_in(locale), Ltr, "{locale}");
    }
    // Likely scripts by language. `sd-IN` is Sindhi's (Arabic): ICU 68's likely subtags have no
    // `sd_IN` (D2061).
    for locale in
        ["ur", "ps", "sd", "sd-IN", "ckb", "ug", "dv", "yi", "syr", "nqo", "rhg", "phn", "ae"]
    {
        assert_eq!(neutral_direction_in(locale), Rtl, "{locale}");
    }
    for locale in ["ku", "pa", "az", "uz", "kk", "ha", "ms", "hi", "sw", "am", "xyz", "und"] {
        assert_eq!(neutral_direction_in(locale), Ltr, "{locale}");
    }
    // Likely scripts by region.
    for locale in ["pa-PK", "az-IQ", "kk-CN", "ky-CN", "ha-CM", "ms-CC", "und-EG", "und-IL"] {
        assert_eq!(neutral_direction_in(locale), Rtl, "{locale}");
    }
    for locale in ["ug-KZ", "pa-IN", "az-AZ", "und-US"] {
        assert_eq!(neutral_direction_in(locale), Ltr, "{locale}");
    }
}

/// ICU 68.2's likely subtags (CLDR 38, androidx-main's `external/icu`), the reference (D2061):
/// languages of no known region (`_ZZ`) written in Arabic, Ancient Greek's likely script Cypriot
/// and uncoded languages' Hatran are right to left; Old Uyghur and Garay have no script data in
/// ICU 68 (Unicode 13), so a locale naming them is left to right there.
#[test]
fn a_locale_s_direction_is_icu_68_s() {
    for locale in [
        "dgl", "gwc", "gwt", "kzh", "mde", "oru", "phl", "scl", "kby", "kdh", "grc", "mis",
        "ku-IR", "und-015", "und-145", "UND-145", "zdj-KM",
    ] {
        assert_eq!(neutral_direction_in(locale), Rtl, "{locale}");
    }
    for locale in ["oui-Ougr", "ff-Gara", "ur-Aran", "grc-Linb", "kk-KZ", "und-419", "und-150"] {
        assert_eq!(neutral_direction_in(locale), Ltr, "{locale}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getParagraphDirection_textDirection_ForceLtr
#[test]
fn a_forced_left_to_right_direction_is_every_paragraph_s() {
    let style = TextStyle { text_direction: Some(TextDirection::Ltr), ..TextStyle::DEFAULT };
    assert_eq!(directions(&three_directions(Setup { style, ..Setup::default() })), [Ltr; 4]);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getParagraphDirection_textDirection_ForceRtl
#[test]
fn a_forced_right_to_left_direction_is_every_paragraph_s() {
    let style = TextStyle { text_direction: Some(TextDirection::Rtl), ..TextStyle::DEFAULT };
    assert_eq!(directions(&three_directions(Setup { style, ..Setup::default() })), [Rtl; 4]);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getParagraphDirection_larger_than_length_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_paragraph_direction_past_the_text_panics() {
    simple("abc", Setup::default()).get_paragraph_direction(4);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getBidiRunDirection
#[test]
fn a_run_direction_is_its_paragraph_s() {
    let hebrew = HEBREW.to_string();
    let paragraph =
        multi(paragraphs(&[&format!("a{hebrew}"), &format!("{hebrew}a")]), Setup::default());
    let second = 1 + hebrew.len();
    assert_eq!(paragraph.get_bidi_run_direction(0), Ltr);
    assert_eq!(paragraph.get_bidi_run_direction(1), Rtl);
    assert_eq!(paragraph.get_bidi_run_direction(second), Rtl);
    assert_eq!(paragraph.get_bidi_run_direction(second + hebrew.len()), Ltr);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getBidiRunDirection_larger_than_length_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_run_direction_past_the_text_panics() {
    simple("abc", Setup::default()).get_bidi_run_direction(4);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getWordBoundary
#[test]
fn a_word_is_found_in_its_paragraph() {
    let paragraph = multi(paragraphs(&["ab cd", "e f"]), Setup::default());
    let text = paragraph.text().to_string();
    let at = |c: char| text.find(c).unwrap();
    assert_eq!(paragraph.get_word_boundary(at('a')), TextRange::new(at('a'), at('b') + 1));
    assert_eq!(paragraph.get_word_boundary(at('d')), TextRange::new(at('c'), at('d') + 1));
    assert_eq!(paragraph.get_word_boundary(at('e')), TextRange::new(at('e'), at('e') + 1));
    assert_eq!(paragraph.get_word_boundary(at('f')), TextRange::new(at('f'), at('f') + 1));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getWordBoundary_larger_than_length_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_word_past_the_text_panics() {
    simple("abc", Setup::default()).get_word_boundary(4);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getCursorRect
#[test]
fn a_cursor_is_in_its_paragraph() {
    let size = 10.0;
    let text = paragraphs(&["aa", "aa", "aa"]);
    let length = text.len();
    let paragraph = multi(text, Setup { width: Some(2.0 * size), ..sized(size) });
    for index in 0..length {
        let (row, column) = ((index / 2) as f32, (index % 2) as f32);
        let (top, x) = (row * size, column * size);
        assert_eq!(paragraph.get_cursor_rect(index), Rect::new(x, top, x, top + size), "{index}");
    }
    // The text's end: after the last line.
    let top = (paragraph.line_count() - 1) as f32 * size;
    let x = 2.0 * size;
    assert_eq!(paragraph.get_cursor_rect(length), Rect::new(x, top, x, top + size));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getCursorRect_larger_than_length_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_cursor_past_the_text_panics() {
    simple("abc", Setup::default()).get_cursor_rect(4);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineForOffset
#[test]
fn an_offset_s_line_counts_the_lines_above_its_paragraph() {
    let paragraph = multi(paragraphs(&["a", "a\na"]), Setup::default());
    assert_eq!(paragraph.get_line_for_offset(0), 0);
    assert_eq!(paragraph.get_line_for_offset(1), 1);
    assert_eq!(paragraph.get_line_for_offset(3), 2);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineForOffset_larger_than_length_returnsLastLine
#[test]
fn the_line_past_the_text_is_the_last() {
    assert_eq!(simple("abc\ndef", Setup::default()).get_line_for_offset(8), 1);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineForOffset_longSecondParagraph_maxLines2
#[test]
fn the_line_of_the_end_of_a_cut_paragraph_is_its_last() {
    let size = 50.0;
    let long = "A".repeat(100_000);
    let setup = Setup { width: Some(100.0 * size), max_lines: 2, ..sized(size) };
    let paragraph = multi(paragraphs(&["AAA", &long]), setup);
    assert_eq!(paragraph.get_line_for_offset(100_003), 1);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineForOffset_longFirstParagraph_maxLines2
#[test]
fn the_line_of_the_end_of_a_text_cut_in_its_first_paragraph_is_the_last() {
    let size = 50.0;
    let long = "A".repeat(100_000);
    let setup = Setup { width: Some(100.0 * size), max_lines: 2, ..sized(size) };
    let paragraph = multi(paragraphs(&[&long, "AAA"]), setup);
    assert_eq!(paragraph.get_line_for_offset(100_003), 1);
}

/// "aa" and two Hebrew letters, each a paragraph, at twice the widest's width.
fn left_and_right(setup: Setup) -> (MultiParagraph, f32) {
    let text = paragraphs(&["aa", &HEBREW.to_string().repeat(2)]);
    let width = 2.0 * intrinsics(text.clone(), &setup).max_intrinsic_width();
    (multi(text, Setup { width: Some(width), ..setup }), width)
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineLeft
#[test]
fn a_line_s_left_is_its_paragraph_s() {
    let (paragraph, width) = left_and_right(sized(50.0));
    assert_eq!(paragraph.get_line_left(0), 0.0);
    assert_eq!(paragraph.get_line_left(1), width - 2.0 * 50.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineLeft_greaterThanOrEqual_lineCount_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_line_left_past_the_lines_panics() {
    let paragraph = simple("abc", Setup::default());
    paragraph.get_line_left(paragraph.line_count());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, lineContentForEmptyText
#[test]
fn an_empty_text_has_an_empty_line() {
    let paragraph = simple("", Setup::default());
    assert_eq!(paragraph.line_count(), 1);
    assert_eq!(paragraph.get_line_start(0), 0);
    assert_eq!(paragraph.get_line_end(0, false), 0);
    assert_eq!(paragraph.get_line_left(0), 0.0);
    assert_eq!(paragraph.get_line_right(0), 0.0);
    assert_eq!(paragraph.get_line_end(0, true), 0);
    assert!(!paragraph.is_line_ellipsized(0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, isLineEllipsized_lineIndexAdjustedToLocalParagraph_doesNotCrash
#[test]
fn a_line_of_a_later_paragraph_is_asked_of_it() {
    let text = AnnotatedString::build(|builder| {
        builder.append("first line\nsecond line");
        builder.push_paragraph_style(ParagraphStyle::DEFAULT);
        builder.with_style(
            SpanStyle { baseline_shift: Some(BaselineShift::SUBSCRIPT), ..SpanStyle::DEFAULT },
            |builder| {
                builder.append("last line");
            },
        );
        builder.pop();
    });
    let paragraph =
        multi(text, Setup { max_lines: 3, overflow: TextOverflow::Ellipsis, ..Setup::default() });
    assert_eq!(paragraph.line_count(), 3);
    assert!(!paragraph.is_line_ellipsized(2));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineRight
#[test]
fn a_line_s_right_is_its_paragraph_s() {
    let (paragraph, width) = left_and_right(sized(50.0));
    assert_eq!(paragraph.get_line_right(0), 2.0 * 50.0);
    assert_eq!(paragraph.get_line_right(1), width);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineRight_greaterThanOrEqual_lineCount_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_line_right_past_the_lines_panics() {
    let paragraph = simple("abc", Setup::default());
    paragraph.get_line_right(paragraph.line_count());
}

/// Three paragraphs of one "a" each, at `size`.
fn three_lines(size: f32) -> MultiParagraph {
    multi(paragraphs(&["a", "a", "a"]), sized(size))
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineTop
#[test]
fn a_line_s_top_is_below_the_paragraphs_above() {
    let paragraph = three_lines(50.0);
    for line in 0..paragraph.line_count() {
        assert_eq!(paragraph.get_line_top(line), 50.0 * line as f32, "{line}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineTop_greaterThanOrEqual_lineCount_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_line_top_past_the_lines_panics() {
    let paragraph = simple("abc", Setup::default());
    paragraph.get_line_top(paragraph.line_count());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineBaseline
#[test]
fn a_line_s_baseline_is_below_the_paragraphs_above() {
    let paragraph = three_lines(50.0);
    for line in 0..paragraph.line_count() {
        let expected = 50.0 * line as f32 + 50.0 * 0.8;
        assert_eq!(paragraph.get_line_baseline(line), expected, "{line}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineBaseline_greaterThanOrEqual_lineCount_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_line_baseline_past_the_lines_panics() {
    simple("abc", Setup::default()).get_line_baseline(1);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineBaseline_forFirstLine_equals_firstBaseline
#[test]
fn the_first_line_s_baseline_is_the_first_baseline() {
    let paragraph = multi(paragraphs(&["a", "a", "a"]), Setup::default());
    assert_eq!(paragraph.get_line_baseline(0), paragraph.first_baseline());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineBaseline_forLastLine_equals_lastBaseline
#[test]
fn the_last_line_s_baseline_is_the_last_baseline() {
    let paragraph = multi(paragraphs(&["a", "a", "a"]), Setup::default());
    assert_eq!(paragraph.get_line_baseline(2), paragraph.last_baseline());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineBottom
#[test]
fn a_line_s_bottom_is_below_the_paragraphs_above() {
    let paragraph = three_lines(50.0);
    for line in 0..paragraph.line_count() {
        assert_eq!(paragraph.get_line_bottom(line), 50.0 * (line + 1) as f32, "{line}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineBottom_greaterThanOrEqual_lineCount_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_line_bottom_past_the_lines_panics() {
    let paragraph = simple("abc", Setup::default());
    paragraph.get_line_bottom(paragraph.line_count());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineHeight
#[test]
fn a_line_s_height_is_its_paragraph_s() {
    let paragraph = three_lines(50.0);
    for line in 0..paragraph.line_count() {
        assert_eq!(paragraph.get_line_height(line), 50.0, "{line}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, getLineHeight_greaterThanOrEqual_lineCount_throw_exception
#[test]
#[should_panic(expected = "out of bounds")]
fn a_line_height_past_the_lines_panics() {
    let paragraph = simple("abc", Setup::default());
    paragraph.get_line_height(paragraph.line_count());
}

/// "aa" and two Hebrew letters, each a paragraph, aligned by `style`, at twice the widest's
/// width.
fn aligned(style: TextStyle) -> (MultiParagraph, f32) {
    left_and_right(Setup { style, ..Setup::default() })
}

fn align(text_align: TextAlign) -> TextStyle {
    TextStyle { text_align: Some(text_align), ..TextStyle::DEFAULT }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textAlign_defaultValue_alignsStart
#[test]
fn by_default_each_paragraph_aligns_to_its_start() {
    let (paragraph, width) = aligned(TextStyle::DEFAULT);
    assert_eq!(paragraph.get_line_left(0), 0.0);
    assert_eq!(paragraph.get_line_right(1), width);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textAlign_left_returnsZeroForGetLineLeft
#[test]
fn left_alignment_puts_every_paragraph_left() {
    let (paragraph, _) = aligned(align(TextAlign::Left));
    assert_eq!(paragraph.get_line_left(0), 0.0);
    assert_eq!(paragraph.get_line_left(1), 0.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textAlign_right_returnsWidthForGetLineRight
#[test]
fn right_alignment_puts_every_paragraph_right() {
    let (paragraph, width) = aligned(align(TextAlign::Right));
    assert_eq!(paragraph.get_line_right(0), width);
    assert_eq!(paragraph.get_line_right(1), width);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textAlign_center_textIsCentered
#[test]
fn center_alignment_centers_every_paragraph() {
    let size = 50.0;
    let (paragraph, width) =
        left_and_right(Setup { style: align(TextAlign::Center), ..sized(size) });
    let (left, right) = (width / 2.0 - size, width / 2.0 + size);
    assert_eq!(paragraph.get_line_left(0), left);
    assert_eq!(paragraph.get_line_right(0), right);
    assert_eq!(paragraph.get_line_left(1), left);
    assert_eq!(paragraph.get_line_right(1), right);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textAlign_justify_justifies
#[test]
fn justification_fills_every_paragraph_s_wrapped_lines() {
    let text = paragraphs(&["a a a", "\u{05D0} \u{05D0} \u{05D0}"]);
    let width = intrinsics(text.clone(), &Setup::default()).max_intrinsic_width() - 1.0;
    let setup = Setup { style: align(TextAlign::Justify), width: Some(width), ..Setup::default() };
    let paragraph = multi(text, setup);
    assert_eq!(paragraph.get_line_left(0), 0.0);
    assert_eq!(paragraph.get_line_right(0), width);
    assert_eq!(paragraph.get_line_left(2), 0.0);
    assert_eq!(paragraph.get_line_right(2), width);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textAlign_start_alignsStart
#[test]
fn start_alignment_follows_each_paragraph_s_direction() {
    let (paragraph, width) = aligned(align(TextAlign::Start));
    assert_eq!(paragraph.get_line_left(0), 0.0);
    assert_eq!(paragraph.get_line_right(1), width);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textAlign_end_alignsEnd
#[test]
fn end_alignment_follows_each_paragraph_s_direction() {
    let (paragraph, width) = aligned(align(TextAlign::End));
    assert_eq!(paragraph.get_line_right(0), width);
    assert_eq!(paragraph.get_line_left(1), 0.0);
}

/// "a .", Hebrew " .", "  .": the dots' positions in `direction` with `locale`, and the width.
fn dots(direction: TextDirection, locale: Option<&str>) -> ([f32; 3], f32) {
    let size = 50.0;
    let text = paragraphs(&["a .", "\u{05D0} .", "  ."]);
    let style = TextStyle {
        text_direction: Some(direction),
        locale_list: locale.map(LocaleList::from_language_tags),
        ..TextStyle::DEFAULT
    };
    let setup = Setup { style, ..sized(size) };
    let width = intrinsics(text.clone(), &setup).max_intrinsic_width();
    let paragraph = multi(text, Setup { width: Some(width), ..setup });
    let hebrew = '\u{05D0}'.len_utf8();
    let dots = [2, 3 + hebrew + 1, 3 + hebrew + 2 + 2];
    (dots.map(|offset| paragraph.get_horizontal_position(offset, true)), width)
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textDirection_content_withLtrLocale
#[test]
fn content_direction_with_a_left_to_right_locale_lays_each_paragraph_out_its_way() {
    let ([first, second, third], width) = dots(TextDirection::Content, Some("en"));
    assert_eq!(first, 2.0 * 50.0);
    assert_eq!(second, width - 2.0 * 50.0);
    assert_eq!(third, 2.0 * 50.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textDirection_content_withRtlLocale
#[test]
fn content_direction_with_a_right_to_left_locale_lays_neutral_paragraphs_right_to_left() {
    let ([first, second, third], width) = dots(TextDirection::Content, Some("ar"));
    assert_eq!(first, 2.0 * 50.0);
    assert_eq!(second, width - 2.0 * 50.0);
    assert_eq!(third, width - 2.0 * 50.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textDirection_forceLtr
#[test]
fn a_forced_left_to_right_direction_lays_every_paragraph_out_left_to_right() {
    let ([first, second, third], _) = dots(TextDirection::Ltr, None);
    assert_eq!(first, 2.0 * 50.0);
    assert_eq!(second, 2.0 * 50.0);
    assert_eq!(third, 2.0 * 50.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textDirection_forceRtl
#[test]
fn a_forced_right_to_left_direction_lays_every_paragraph_out_right_to_left() {
    let ([first, second, third], width) = dots(TextDirection::Rtl, None);
    assert_eq!(first, width - 2.0 * 50.0);
    assert_eq!(second, width - 2.0 * 50.0);
    assert_eq!(third, width - 2.0 * 50.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, lineHeight_returnsSameAsGiven
#[test]
fn every_paragraph_takes_the_line_height() {
    let style = TextStyle { line_height: 80.sp(), ..TextStyle::DEFAULT };
    let paragraph = multi(paragraphs(&["a\na\na", "a\na\na"]), Setup { style, ..sized(50.0) });
    assert_eq!(paragraph.get_line_height(1), 80.0);
    assert_eq!(paragraph.get_line_height(4), 80.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textIndent_onFirstLine
#[test]
fn every_paragraph_indents_its_first_line() {
    let (size, indent) = (50.0, 20.0);
    let style = TextStyle {
        text_indent: Some(TextIndent { first_line: indent.sp(), ..TextIndent::NONE }),
        ..TextStyle::DEFAULT
    };
    let width = 2.0 * size;
    let text = paragraphs(&["aaa", &HEBREW.to_string().repeat(3)]);
    let paragraph = multi(text, Setup { style, width: Some(width), ..sized(size) });
    let hebrew = HEBREW.len_utf8();
    assert_eq!(paragraph.get_horizontal_position(0, true), indent);
    assert_eq!(paragraph.get_horizontal_position(1, true), 0.0);
    assert_eq!(paragraph.get_horizontal_position(3, true), width - indent);
    assert_eq!(paragraph.get_horizontal_position(3 + hebrew, true), width);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, textIndent_onRestLine
#[test]
fn every_paragraph_indents_its_other_lines() {
    let (size, indent) = (50.0, 20.0);
    let style = TextStyle {
        text_indent: Some(TextIndent { rest_line: indent.sp(), ..TextIndent::NONE }),
        ..TextStyle::DEFAULT
    };
    let width = 2.0 * size;
    let text = paragraphs(&["aaa", &HEBREW.to_string().repeat(3)]);
    let paragraph = multi(text, Setup { style, width: Some(width), ..sized(size) });
    let hebrew = HEBREW.len_utf8();
    assert_eq!(paragraph.get_horizontal_position(0, true), 0.0);
    assert_eq!(paragraph.get_horizontal_position(2, true), indent);
    assert_eq!(paragraph.get_horizontal_position(3, true), width);
    assert_eq!(paragraph.get_horizontal_position(3 + 2 * hebrew, true), width - indent);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, annotatedString_haveParagraphStyle_withoutTextDirection
#[test]
fn a_paragraph_style_without_a_direction_takes_the_text_s() {
    let text = AnnotatedString::with_styles(
        "ab",
        [],
        [
            AnnotatedRange::new(
                ParagraphStyle {
                    text_direction: Some(TextDirection::Content),
                    ..ParagraphStyle::DEFAULT
                },
                0,
                1,
            ),
            AnnotatedRange::new(ParagraphStyle::DEFAULT, 1, 2),
        ],
    );
    let style = TextStyle { text_direction: Some(TextDirection::Rtl), ..TextStyle::DEFAULT };
    let paragraph = multi(text, Setup { style, ..Setup::default() });
    assert_eq!(paragraph.get_paragraph_direction(0), Ltr);
    assert_eq!(paragraph.get_paragraph_direction(1), Rtl);
}

/// "ab" at 20 sp with `placeholders`, unbounded.
fn with_placeholders(
    text: AnnotatedString,
    placeholders: Vec<AnnotatedRange<Placeholder>>,
) -> MultiParagraph {
    multi(text, Setup { placeholders, ..sized(20.0) })
}

fn above_baseline(width: TextUnit, height: TextUnit) -> Placeholder {
    Placeholder::new(width, height, PlaceholderVerticalAlign::AboveBaseline)
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, maxIntrinsicWidth_withPlaceholder_inEm
#[test]
fn a_placeholder_in_em_counts_in_the_widest_width() {
    let placeholders = vec![AnnotatedRange::new(above_baseline(2.em(), 1.em()), 0, 1)];
    let paragraph = with_placeholders(AnnotatedString::new("ab"), placeholders);
    assert_eq!(paragraph.max_intrinsic_width(), 20.0 * 2.0 + 20.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, maxIntrinsicWidth_withPlaceholder_inSp
#[test]
fn a_placeholder_in_sp_counts_in_the_widest_width() {
    let placeholders = vec![AnnotatedRange::new(above_baseline(30.sp(), 1.em()), 0, 1)];
    let paragraph = with_placeholders(AnnotatedString::new("ab"), placeholders);
    assert_eq!(paragraph.max_intrinsic_width(), 20.0 + 30.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, placeholderRects_withSingleParagraph
#[test]
fn a_placeholder_s_rect_in_one_paragraph() {
    let placeholders = vec![AnnotatedRange::new(above_baseline(2.em(), 1.em()), 0, 1)];
    let paragraph = with_placeholders(AnnotatedString::new("ab"), placeholders);
    let rects = paragraph.placeholder_rects();
    assert_eq!(rects.len(), 1);
    let baseline = paragraph.first_baseline();
    assert_eq!(rects[0], Some(Rect::new(0.0, baseline - 20.0, 40.0, baseline)));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, placeholderRects_withMultipleParagraphs
#[test]
fn placeholders_rects_in_each_paragraph() {
    let placeholders = vec![
        AnnotatedRange::new(above_baseline(2.em(), 1.em()), 0, 1),
        AnnotatedRange::new(above_baseline(2.em(), 1.em()), 2, 3),
    ];
    let paragraph = with_placeholders(paragraphs(&["ab", "cd"]), placeholders);
    let rects = paragraph.placeholder_rects();
    assert_eq!(rects.len(), 2);
    let (first, last) = (paragraph.first_baseline(), paragraph.last_baseline());
    assert_eq!(rects[0], Some(Rect::new(0.0, first - 20.0, 40.0, first)));
    assert_eq!(rects[1], Some(Rect::new(0.0, last - 20.0, 40.0, last)));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, placeholderRects_overlapWithParagraph
#[test]
#[should_panic(expected = "placeholder can not overlap with paragraph")]
fn a_placeholder_across_paragraphs_panics() {
    let placeholders = vec![AnnotatedRange::new(above_baseline(2.em(), 1.em()), 1, 3)];
    with_placeholders(paragraphs(&["ab", "cd"]), placeholders);
}

/// A text laid out within `constraints`, as Compose's constructor from a text.
fn constrained(
    text: AnnotatedString,
    style: TextStyle,
    constraints: Constraints,
) -> MultiParagraph {
    let intrinsics = MultiParagraphIntrinsics::new(
        text,
        &style,
        &[],
        Density::new(1.0),
        resolver(),
        true,
        test_locale_list(),
    );
    MultiParagraph::new(Rc::new(intrinsics), constraints, u32::MAX, TextOverflow::Clip)
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, setMinWidthConstraints_notSupported
#[test]
#[should_panic(
    expected = "Setting Constraints.minWidth and Constraints.minHeight is not supported"
)]
fn a_minimum_width_panics() {
    let constraints = Constraints::new(100, Constraints::INFINITY, 0, Constraints::INFINITY);
    constrained(AnnotatedString::new(""), TextStyle::DEFAULT, constraints);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, setMinHeightConstraints_notSupported
#[test]
#[should_panic(
    expected = "Setting Constraints.minWidth and Constraints.minHeight is not supported"
)]
fn a_minimum_height_panics() {
    let constraints = Constraints::new(0, Constraints::INFINITY, 100, Constraints::INFINITY);
    constrained(AnnotatedString::new(""), TextStyle::DEFAULT, constraints);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, multiParagraphConstruction_doesNotThrow_ifNotAllParagraghsFitVertically
#[test]
fn paragraphs_below_the_height_are_laid_out_in_none() {
    let text = AnnotatedString::build(|builder| {
        let centered =
            ParagraphStyle { text_align: Some(TextAlign::Center), ..ParagraphStyle::DEFAULT };
        builder.with_paragraph_style(centered, |builder| {
            builder.append("Lorem");
        });
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("Ipsum");
        });
    });
    let style = TextStyle { font_size: 20.sp(), ..TextStyle::DEFAULT };
    let paragraph = constrained(text, style, Constraints::new(0, 200, 0, 10));
    assert!(paragraph.line_count() >= 1);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, minInstrinsicWidth_includes_white_space
#[test]
fn the_narrowest_width_includes_a_trailing_space() {
    let style = TextStyle { font_size: 12.sp(), ..TextStyle::DEFAULT };
    let paragraph = simple("b ", Setup { style, ..Setup::default() });
    assert_eq!(paragraph.min_intrinsic_width(), 2.0 * 12.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, minInstrinsicWidth_returns_longest_word_width
#[test]
fn the_narrowest_width_is_the_longest_word_s() {
    let text: String = (1..=50).map(|length| "a".repeat(length) + " ").collect();
    let style = TextStyle { font_size: 12.sp(), ..TextStyle::DEFAULT };
    let paragraph = simple(&text, Setup { style, ..Setup::default() });
    assert_eq!(paragraph.min_intrinsic_width(), 51.0 * 12.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, minInstrinsicWidth_withStyledText
#[test]
fn the_narrowest_width_counts_a_word_s_span() {
    let text = AnnotatedString::build(|builder| {
        builder.append("a bb ccc");
        builder.add_style(SpanStyle { font_size: 24.sp(), ..SpanStyle::DEFAULT }, 1, 5);
    });
    let style = TextStyle { font_size: 12.sp(), ..TextStyle::DEFAULT };
    let paragraph = multi(text, Setup { style, ..Setup::default() });
    assert_eq!(paragraph.min_intrinsic_width(), 3.0 * 24.0);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTest.kt, mapEachParagraphStyle_containsAllAnnotations_excludingParagraphStyle
#[test]
fn each_paragraph_keeps_its_annotations_but_the_paragraph_styles() {
    let text = AnnotatedString::build(|builder| {
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.with_style(SpanStyle::DEFAULT, |builder| {
                builder.append("a");
            });
        });
        builder.with_annotation("annotation1", "value1", |builder| {
            builder.append("a");
        });
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("a");
        });
        builder.with_style(SpanStyle::DEFAULT, |builder| {
            builder.append("a");
        });
    });
    let paragraph = multi(text.clone(), Setup::default());
    let texts: Vec<AnnotatedString> = paragraph
        .paragraph_info_list()
        .iter()
        .map(|info| text.substring_without_paragraph_styles(info.start_index, info.end_index))
        .collect();
    assert_eq!(texts.len(), 4);
    assert_eq!(texts[0].span_styles(), [AnnotatedRange::new(SpanStyle::DEFAULT, 0, 1)]);
    assert_eq!(texts[1].get_string_annotations_with_tag("annotation1", 0, 1).len(), 1);
    assert!(texts[2].annotations().is_empty());
    assert_eq!(texts[3].span_styles(), [AnnotatedRange::new(SpanStyle::DEFAULT, 0, 1)]);
}

/// An empty text without a direction, in `locale_list` with `default_locale_list`: Compose's
/// `MultiParagraphIntegrationTextDirectionTest.multiParagraph` with the defaults.
fn empty_in(locale_list: Option<&str>, default_locale_list: &str) -> MultiParagraph {
    let style = TextStyle {
        locale_list: locale_list.map(LocaleList::from_language_tags),
        ..TextStyle::DEFAULT
    };
    let intrinsics = MultiParagraphIntrinsics::new(
        AnnotatedString::new(""),
        &style,
        &[],
        Density::new(1.0),
        resolver(),
        true,
        LocaleList::from_language_tags(default_locale_list),
    );
    MultiParagraph::new(Rc::new(intrinsics), Constraints::unbounded(), u32::MAX, TextOverflow::Clip)
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTextDirectionTest.kt, unspecifiedTextDirection_withLtrLocale_resolvesToLtr
#[test]
fn no_direction_with_a_left_to_right_default_locale_is_left_to_right() {
    assert_eq!(empty_in(None, "en").get_paragraph_direction(0), Ltr);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTextDirectionTest.kt, unspecifiedTextDirection_withRtlLocale_resolvesToRtl
#[test]
fn no_direction_with_a_right_to_left_default_locale_is_right_to_left() {
    assert_eq!(empty_in(None, "ar").get_paragraph_direction(0), Rtl);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTextDirectionTest.kt, unspecifiedTextDirection_withLtrLocaleList_resolvesToLtr
#[test]
fn no_direction_with_a_left_to_right_locale_is_left_to_right() {
    assert_eq!(empty_in(Some("en"), "en").get_paragraph_direction(0), Ltr);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTextDirectionTest.kt, unspecifiedTextDirection_withRtlLocaleList_resolvesToRtl
#[test]
fn no_direction_with_a_right_to_left_locale_is_right_to_left() {
    assert_eq!(empty_in(Some("ar"), "en").get_paragraph_direction(0), Rtl);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTextDirectionTest.kt, textDirection_content_withLtrLocale
#[test]
fn content_in_a_left_to_right_locale_goes_by_each_paragraph() {
    let ([first, second, third], width) = dots(TextDirection::Content, Some("en"));
    assert_eq!([first, second, third], [100.0, width - 100.0, 100.0]);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTextDirectionTest.kt, textDirection_content_withRtlLocale
#[test]
fn content_in_a_right_to_left_locale_lays_a_neutral_paragraph_right_to_left() {
    let ([first, second, third], width) = dots(TextDirection::Content, Some("ar"));
    assert!(width < i32::MAX as f32);
    assert_eq!([first, second, third], [100.0, width - 100.0, width - 100.0]);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTextDirectionTest.kt, textDirection_forceLtr
#[test]
fn forced_left_to_right_lays_each_paragraph_left_to_right() {
    let (dots, _) = dots(TextDirection::Ltr, None);
    assert_eq!(dots, [100.0; 3]);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphIntegrationTextDirectionTest.kt, textDirection_forceRtl
#[test]
fn forced_right_to_left_lays_each_paragraph_right_to_left() {
    let (dots, width) = dots(TextDirection::Rtl, None);
    assert_eq!(dots, [width - 100.0; 3]);
}

/// A measurer on the fixed measurer, keeping `cache_size` layouts.
fn text_measurer(cache_size: usize) -> compose_ui::TextMeasurer {
    compose_ui::TextMeasurer::new(
        resolver(),
        test_locale_list(),
        Density::new(1.0),
        compose_ui::LayoutDirection::Ltr,
        cache_size,
    )
}

fn measure_options(constraints: Constraints) -> compose_ui::TextMeasureOptions {
    compose_ui::TextMeasureOptions {
        style: TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT },
        constraints,
        ..compose_ui::TextMeasureOptions::default()
    }
}

#[test]
fn a_measured_text_of_paragraphs_is_as_wide_as_its_widest() {
    let text = paragraphs(&["aaaa", "bb"]);
    let options = measure_options(Constraints::new(0, 100, 0, Constraints::INFINITY));
    let result = text_measurer(0).measure(text, options);
    let paragraph = &result.multi_paragraph;
    assert_eq!(paragraph.paragraph_info_list().len(), 2);
    assert_eq!(paragraph.width(), 40.0);
    assert_eq!(result.size, compose_ui::IntSize::new(40, 20));
    assert_eq!(result.line_count, 2);
    assert_eq!((result.first_baseline, result.last_baseline), (8.0, 18.0));
    assert_eq!(paragraph.get_line_start(1), 4);
    assert_eq!(paragraph.get_line_top(1), 10.0);
    assert_eq!(result.get_bounding_box(5), Rect::new(10.0, 10.0, 20.0, 20.0));
}

#[test]
fn a_measured_text_of_paragraphs_aligns_each_in_the_widest_s_width() {
    let right = ParagraphStyle { text_align: Some(TextAlign::Right), ..ParagraphStyle::DEFAULT };
    let text = AnnotatedString::build(|builder| {
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("aaaa");
        });
        builder.with_paragraph_style(right, |builder| {
            builder.append("bb");
        });
    });
    let result = text_measurer(0).measure(text, measure_options(Constraints::unbounded()));
    assert_eq!(result.get_line_left(1), 20.0);
    assert_eq!(result.get_line_right(1), 40.0);
    // At a minimum width wider than the text, that width.
    let text = result.layout_input.text.clone();
    let wide = text_measurer(0).measure(text, measure_options(Constraints::new(60, 100, 0, 100)));
    assert_eq!(wide.size.width, 60);
    assert_eq!(wide.get_line_right(1), 60.0);
}

#[test]
fn a_measured_text_of_paragraphs_wraps_at_its_maximum_width() {
    let text = paragraphs(&["aa aa", "b"]);
    let result = text_measurer(0).measure(text, measure_options(Constraints::new(0, 30, 0, 100)));
    assert_eq!(result.line_count, 3);
    assert_eq!(result.size, compose_ui::IntSize::new(30, 30));
    assert_eq!(result.get_line_for_offset(5), 2);
}

#[test]
fn a_measured_text_of_paragraphs_ellipsized_without_wrapping_is_one_line() {
    let text = paragraphs(&["aaaa", "bb"]);
    let options = compose_ui::TextMeasureOptions {
        soft_wrap: false,
        overflow: TextOverflow::Ellipsis,
        ..measure_options(Constraints::new(0, 30, 0, 100))
    };
    let result = text_measurer(0).measure(text, options);
    assert_eq!(result.line_count, 1);
    assert_eq!(result.multi_paragraph.paragraph_info_list().len(), 1);
    assert!(result.multi_paragraph.did_exceed_max_lines());
    assert!(result.has_visual_overflow());
}

#[test]
fn a_text_of_paragraphs_measured_again_is_the_layout_kept() {
    let measurer = text_measurer(8);
    let options = || measure_options(Constraints::unbounded());
    let first = measurer.measure(paragraphs(&["aa", "b"]), options());
    let second = measurer.measure(paragraphs(&["aa", "b"]), options());
    assert!(Rc::ptr_eq(&first.multi_paragraph, &second.multi_paragraph));
    // Other paragraph styles lay it out anew.
    let centered =
        ParagraphStyle { text_align: Some(TextAlign::Center), ..ParagraphStyle::DEFAULT };
    let other = AnnotatedString::with_styles("aab", [], [AnnotatedRange::new(centered, 0, 2)]);
    let third = measurer.measure(other, options());
    assert!(!Rc::ptr_eq(&first.multi_paragraph, &third.multi_paragraph));
}

#[test]
fn a_text_without_paragraph_styles_is_its_one_paragraph() {
    let result = text_measurer(0).measure("aa\nb", measure_options(Constraints::unbounded()));
    let paragraph = &result.multi_paragraph;
    assert_eq!(paragraph.paragraph_info_list().len(), 1);
    assert!(paragraph.intrinsics().is_none());
    assert_eq!(paragraph.line_count(), 2);
    assert_eq!(paragraph.height(), 20.0);
    assert_eq!(paragraph.get_line_top(1), 10.0);
}

// ---- The constructors and `ParagraphIntrinsics` (D2051) ----

/// Compose's `Paragraph(text, style, constraints, …)`: the text laid out as one paragraph at the
/// maximum width, its spans in their styles.
#[test]
fn a_paragraph_of_a_text_is_laid_out_at_the_maximum_width() {
    let style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT };
    let paragraph = compose_ui::paragraph_of_text(
        "abc def",
        &style,
        Constraints::new(0, 50, 0, Constraints::INFINITY),
        Density::new(1.0),
        resolver(),
        test_locale_list(),
        &[],
        &[],
        u32::MAX,
        TextOverflow::Clip,
    );
    assert_eq!(paragraph.width(), 50.0);
    assert_eq!(paragraph.line_count(), 2);
    assert_eq!(paragraph.get_line_end(0, true), 3);
    // A span twice as large widens its letters and its line.
    let large = SpanStyle { font_size: 20.sp(), ..SpanStyle::default() };
    let paragraph = compose_ui::paragraph_of_text(
        "abc",
        &style,
        Constraints::new(0, 100, 0, Constraints::INFINITY),
        Density::new(1.0),
        resolver(),
        test_locale_list(),
        &[AnnotatedRange::new(large, 1, 2)],
        &[],
        1,
        TextOverflow::Clip,
    );
    assert_eq!(paragraph.get_bounding_box(1), Rect::new(10.0, 0.0, 30.0, 20.0));
    assert_eq!(paragraph.get_bounding_box(2).left, 30.0);
}

/// One paragraph's intrinsics leave paragraph styles out, and are Compose's `ParagraphIntrinsics`.
#[test]
fn a_paragraph_s_intrinsics_are_one_paragraph_s() {
    let style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT };
    let intrinsics = compose_ui::paragraph_intrinsics(
        &paragraphs(&["ab cd", "efg"]),
        &style,
        &[],
        Density::new(1.0),
        resolver(),
        true,
        test_locale_list(),
    );
    let widths: &dyn compose_ui::ParagraphIntrinsics = &intrinsics;
    // "ab cdefg": its longest word, and the whole line.
    assert_eq!(widths.min_intrinsic_width(), 50.0);
    assert_eq!(widths.max_intrinsic_width(), 80.0);
    assert!(!widths.has_stale_resolved_fonts());
    let paragraph = compose_ui::paragraph(
        &intrinsics,
        Constraints::new(0, 100, 0, Constraints::INFINITY),
        u32::MAX,
        TextOverflow::Clip,
    );
    assert_eq!((paragraph.width(), paragraph.line_count()), (100.0, 1));
    // Unbounded: as wide as its text.
    let paragraph =
        compose_ui::paragraph(&intrinsics, Constraints::unbounded(), u32::MAX, TextOverflow::Clip);
    assert_eq!(paragraph.width(), 80.0);
}

#[test]
#[should_panic(expected = "one paragraph's intrinsics, not 2")]
fn a_paragraph_of_many_paragraphs_intrinsics_panics() {
    let intrinsics = intrinsics(paragraphs(&["a", "b"]), &Setup::default());
    compose_ui::paragraph(&intrinsics, Constraints::unbounded(), u32::MAX, TextOverflow::Clip);
}

#[test]
#[should_panic(
    expected = "Setting Constraints.minWidth and Constraints.minHeight is not supported"
)]
fn a_paragraph_with_a_minimum_width_panics() {
    let intrinsics = intrinsics(AnnotatedString::new("a"), &Setup::default());
    let constraints = Constraints::new(10, 100, 0, Constraints::INFINITY);
    compose_ui::paragraph(&intrinsics, constraints, u32::MAX, TextOverflow::Clip);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/ParagraphIntegrationTest.kt, maxLines_withMaxLineEqualsZero_throwsException
/// A paragraph of no line panics (a negative count, Compose's `maxLines_withMaxLineNegative`,
/// has no counterpart: `max_lines` is a `u32`).
#[test]
#[should_panic(expected = "maxLines should be greater than 0")]
fn a_paragraph_of_no_line_panics() {
    let intrinsics = intrinsics(AnnotatedString::new(""), &Setup::default());
    compose_ui::paragraph(&intrinsics, Constraints::unbounded(), 0, TextOverflow::Clip);
}

/// Compose's `MultiParagraph(annotatedString, style, constraints, …)`: its intrinsics, then
/// laid out as `MultiParagraph(intrinsics, …)`.
#[test]
fn a_multi_paragraph_of_a_text_is_its_intrinsics_laid_out() {
    let style = TextStyle { font_size: 10.sp(), ..TextStyle::DEFAULT };
    let constraints = Constraints::new(0, 30, 0, Constraints::INFINITY);
    let paragraph = MultiParagraph::of_text(
        paragraphs(&["ab", "cdef"]),
        &style,
        constraints,
        Density::new(1.0),
        resolver(),
        test_locale_list(),
        &[],
        2,
        TextOverflow::Clip,
    );
    assert_eq!(paragraph.max_lines(), 2);
    assert_eq!(paragraph.paragraph_info_list().len(), 2);
    assert_eq!(paragraph.line_count(), 2);
    assert!(paragraph.did_exceed_max_lines());
    assert_eq!(paragraph.width(), 30.0);
    let intrinsics = paragraph.intrinsics().expect("laid out from intrinsics");
    assert_eq!(intrinsics.annotated_string().as_str(), "abcdef");
}
