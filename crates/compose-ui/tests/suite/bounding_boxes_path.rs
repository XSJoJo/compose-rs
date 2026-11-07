//! `TextLayoutResult::fill_path_for_range` (D2050): Compose's `MultiParagraphBoundingBoxesForRangeTest`
//! cases on `fillPathForRange` and `calculatePathFromBoundingBoxes`, laid out by the fixed
//! measurer with Compose's test font metrics (`FixedFontMetrics::SAMPLE_FONT`: one em wide,
//! lines one em high), as Compose's lay out with `BASIC_MEASURE_FONT`. Compose compares paths by
//! their symmetric difference (`Path.combine(Xor, …)`), which only a platform computes here: these
//! compare which points the paths hold (pixel centres), or their commands.

use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, SharedTextMeasurer};
use compose_ui::{
    AnnotatedString, Constraints, CornerRadius, Density, LayoutDirection, LocaleList, Offset,
    ParagraphStyle, Path, PathCommand, PathFillType, Rect, TextDirection, TextLayoutResult,
    TextMeasureOptions, TextMeasurer, TextStyle, UnitExt, create_font_family_resolver,
};

/// Compose's `simpleTextLayoutResult`: `text` in `style` at `font_size`, `width` wide (unbounded
/// when `None`).
fn layout(
    text: impl Into<AnnotatedString>,
    style: TextStyle,
    font_size: f32,
    width: Option<f32>,
    max_lines: u32,
) -> TextLayoutResult {
    let resolver = create_font_family_resolver(&SharedTextMeasurer::new(Box::new(
        FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT),
    )));
    let measurer = TextMeasurer::new(
        resolver,
        LocaleList::from_language_tags("en"),
        Density::new(1.0),
        LayoutDirection::Ltr,
        0,
    );
    // As Compose's test lays each paragraph out at the width given: a fixed width.
    let (min_width, max_width) = width
        .map_or((0, Constraints::INFINITY), |width| (width.ceil() as i32, width.ceil() as i32));
    let style = TextStyle { font_size: font_size.sp(), ..TextStyle::DEFAULT }.merge(&style);
    measurer.measure(
        text,
        TextMeasureOptions {
            style,
            max_lines,
            constraints: Constraints::new(min_width, max_width, 0, Constraints::INFINITY),
            ..TextMeasureOptions::default()
        },
    )
}

fn simple(text: &str, font_size: f32) -> TextLayoutResult {
    layout(text, TextStyle::DEFAULT, font_size, None, u32::MAX)
}

/// `layout`'s path of `start..end`.
fn filled(
    layout: &TextLayoutResult,
    start: usize,
    end: usize,
    corner_radius: CornerRadius,
    merge_lines: bool,
) -> Path {
    let path = Path::new();
    layout.fill_path_for_range(start, end, &path, corner_radius, merge_lines);
    path
}

/// The pixel centres in `bounds` `path` holds.
fn covered(path: &Path, bounds: Rect) -> Vec<(i32, i32)> {
    let data = path.data();
    let mut points = Vec::new();
    for y in bounds.top.floor() as i32..bounds.bottom.ceil() as i32 {
        for x in bounds.left.floor() as i32..bounds.right.ceil() as i32 {
            if data.contains(Offset::new(x as f32 + 0.5, y as f32 + 0.5)) {
                points.push((x, y));
            }
        }
    }
    points
}

/// Compose's `assertPathsEquivalent`: the same bounds, within a pixel, and the same points.
fn assert_paths_equivalent(actual: &Path, expected: &Path) {
    let (actual_bounds, expected_bounds) = (actual.get_bounds(), expected.get_bounds());
    for (got, want) in [
        (actual_bounds.left, expected_bounds.left),
        (actual_bounds.top, expected_bounds.top),
        (actual_bounds.right, expected_bounds.right),
        (actual_bounds.bottom, expected_bounds.bottom),
    ] {
        assert!((got - want).abs() <= 1.0, "{actual_bounds:?} vs {expected_bounds:?}");
    }
    let area = Rect::new(
        expected_bounds.left - 2.0,
        expected_bounds.top - 2.0,
        expected_bounds.right + 2.0,
        expected_bounds.bottom + 2.0,
    );
    assert_eq!(covered(actual, area), covered(expected, area));
}

/// The radii of each corner curve: its end less its start, across and down.
fn corner_radii(path: &Path) -> Vec<(f32, f32)> {
    let mut current = Offset::ZERO;
    let mut radii = Vec::new();
    for command in path.data().commands() {
        match *command {
            PathCommand::MoveTo(to) | PathCommand::LineTo(to) => current = to,
            PathCommand::QuadTo(_, to) => {
                radii.push(((to.x - current.x).abs(), (to.y - current.y).abs()));
                current = to;
            }
            PathCommand::CubicTo(_, _, to) => current = to,
            PathCommand::Close => {}
        }
    }
    radii
}

fn assert_near(actual: f32, expected: f32, tolerance: f32) {
    assert!((actual - expected).abs() <= tolerance, "{actual} is not {expected} ± {tolerance}");
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, fillPathForRange_invalidRange_throwsIllegalArgumentException
#[test]
#[should_panic(expected = "out of range")]
fn a_path_for_a_reversed_range_panics() {
    let layout = simple("abcdef", 10.0);
    layout.fill_path_for_range(5, 2, &Path::new(), CornerRadius::ZERO, true);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, calculatePathFromBoundingBoxes_onAndroidPath_mergesAndClonesMultiLineBoxes
#[test]
fn rounded_lines_merged_or_not_cover_their_boxes() {
    let layout = simple("abcdefghijkl\nabcdefgh", 10.0);
    let length = layout.layout_input.text.len();
    for merge_lines in [true, false] {
        let path = filled(&layout, 0, length, CornerRadius::new(3.0, 3.0), merge_lines);
        assert!(!path.is_empty());
        assert_eq!(path.get_bounds(), Rect::new(0.0, 0.0, 120.0, 20.0), "{merge_lines}");
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, textLayoutResult_fillPathForRange_mergesMultiLineContourAndPopulatesTarget
#[test]
fn merged_lines_span_from_the_first_box_to_the_last() {
    let style = TextStyle { line_height: 24.sp(), ..TextStyle::DEFAULT };
    let layout = layout("abc def ghi", style, 10.0, Some(35.0), u32::MAX);
    let boxes = layout.get_bounding_boxes_for_range(0, 11);
    let path = filled(&layout, 0, 11, CornerRadius::new(4.0, 4.0), true);
    assert!(!path.is_empty());
    let bounds = path.get_bounds();
    assert_near(bounds.top, boxes.first().unwrap().top, 0.5);
    assert_near(bounds.bottom, boxes.last().unwrap().bottom, 0.5);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, textLayoutResult_fillPathForRange_reusesAndClearsDirtyTargetPath
#[test]
fn the_target_is_rewound_and_keeps_its_fill_type() {
    let layout = simple("abcdef", 10.0);
    let target = Path::new();
    target.set_fill_type(PathFillType::EvenOdd);
    target.add_rect(Rect::new(500.0, 500.0, 800.0, 800.0));
    layout.fill_path_for_range(1, 3, &target, CornerRadius::ZERO, true);
    assert_near(target.get_bounds().left, 10.0, 0.5);
    assert_near(target.get_bounds().right, 30.0, 0.5);
    assert_eq!(target.fill_type(), PathFillType::EvenOdd);
    // An empty range rewinds it, keeping its fill type.
    layout.fill_path_for_range(2, 2, &target, CornerRadius::ZERO, true);
    assert!(target.is_empty());
    assert_eq!(target.fill_type(), PathFillType::EvenOdd);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, fillPathForRange_mergeLinesFalse_matchesGetPathForRange_singleLineSubstring
#[test]
fn unmerged_a_part_of_a_line_is_its_range_s_path() {
    let layout = simple("abcdefgh", 10.0);
    let path = filled(&layout, 2, 6, CornerRadius::ZERO, false);
    assert_paths_equivalent(&path, &layout.get_path_for_range(2, 6));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, fillPathForRange_mergeLinesFalse_matchesGetPathForRange_multiLineSubstring
#[test]
fn unmerged_a_range_over_wrapped_lines_is_its_range_s_path() {
    // 12 characters of 10 px wrapped at 40 px: three lines of four.
    let layout = layout("abcdefghijkl", TextStyle::DEFAULT, 10.0, Some(40.0), u32::MAX);
    assert_eq!(layout.line_count, 3);
    let path = filled(&layout, 1, 11, CornerRadius::ZERO, false);
    assert_paths_equivalent(&path, &layout.get_path_for_range(1, 11));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, fillPathForRange_mergeLinesFalse_matchesGetPathForRange_emptyRange
#[test]
fn unmerged_an_empty_range_is_an_empty_path() {
    let layout = simple("abcdefgh", 10.0);
    assert!(filled(&layout, 4, 4, CornerRadius::ZERO, false).is_empty());
    assert!(layout.get_path_for_range(4, 4).is_empty());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, fillPathForRange_mergeLinesFalse_matchesGetPathForRange_multiParagraph
#[test]
fn unmerged_a_range_across_paragraphs_is_its_range_s_path() {
    let text = AnnotatedString::build(|builder| {
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("abcd");
        });
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("efgh");
        });
    });
    let layout = layout(text, TextStyle::DEFAULT, 10.0, None, u32::MAX);
    let path = filled(&layout, 1, 7, CornerRadius::ZERO, false);
    assert_paths_equivalent(&path, &layout.get_path_for_range(1, 7));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, fillPathForRange_mergeLinesFalse_matchesGetPathForRange_customLineHeight
#[test]
fn unmerged_lines_of_a_set_height_are_their_range_s_path() {
    let style = TextStyle { line_height: 24.sp(), ..TextStyle::DEFAULT };
    let layout = layout("abcdefgh", style, 10.0, Some(40.0), u32::MAX);
    let path = filled(&layout, 1, 7, CornerRadius::ZERO, false);
    assert_paths_equivalent(&path, &layout.get_path_for_range(1, 7));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, fillPathForRange_mergeLinesFalse_matchesGetPathForRange_bidiMixedDirection
#[test]
fn unmerged_runs_of_both_directions_are_their_range_s_path() {
    // b, c, and two Hebrew letters: two runs apart.
    let text = "abc\u{05D0}\u{05D1}\u{05D2}def";
    let style = TextStyle { text_direction: Some(TextDirection::Ltr), ..TextStyle::DEFAULT };
    let layout = layout(text, style, 10.0, Some(1000.0), u32::MAX);
    let (start, end) = (1, text.char_indices().nth(5).unwrap().0);
    let path = filled(&layout, start, end, CornerRadius::ZERO, false);
    assert_eq!(layout.get_bounding_boxes_for_range(start, end).len(), 2);
    assert_paths_equivalent(&path, &layout.get_path_for_range(start, end));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, fillPathForRange_ellipticalCornerRadius_preservesHorizontalRadiusWhenVerticalIsSmaller
#[test]
fn elliptical_corners_keep_their_radii_where_they_fit() {
    // Line 0 [0, 0, 120, 20], line 1 [0, 20, 80, 40]: six corners, five out and one in.
    let layout = simple("aaaaaa\naaaa", 20.0);
    let length = layout.layout_input.text.len();
    let path = filled(&layout, 0, length, CornerRadius::new(18.0, 6.0), true);
    let radii = corner_radii(&path);
    assert_eq!(radii.len(), 6);
    for (x, y) in radii {
        assert_near(x, 18.0, 0.01);
        assert_near(y, 6.0, 0.01);
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, fillPathForRange_ellipticalCornerRadiusOverflowingVerticalEdge_scalesUniformlyPreservingAspectRatio
#[test]
fn elliptical_corners_too_large_scale_down_keeping_their_proportions() {
    // v0 (0, 0), v1 (120, 0), v2 (120, 20), v3 (80, 20), v4 (80, 40), v5 (0, 40).
    let layout = simple("aaaaaa\naaaa", 20.0);
    let length = layout.layout_input.text.len();
    // 24 by 16: v0 and v5 have 20 px down, the others 10 px: scaled by 10 / 16.
    let radii = corner_radii(&filled(&layout, 0, length, CornerRadius::new(24.0, 16.0), true));
    assert_eq!(radii.len(), 6);
    for (index, (x, y)) in radii.into_iter().enumerate() {
        assert_near(x / y, 1.5, 0.01);
        let expected = if index == 0 || index == 5 { (24.0, 16.0) } else { (15.0, 10.0) };
        assert_near(x, expected.0, 0.01);
        assert_near(y, expected.1, 0.01);
    }
    // 30 by 8: the step's corners v2 and v3 have 20 px across: scaled by 20 / 30.
    let radii = corner_radii(&filled(&layout, 0, length, CornerRadius::new(30.0, 8.0), true));
    assert_eq!(radii.len(), 6);
    for (index, (x, y)) in radii.into_iter().enumerate() {
        assert_near(x / y, 3.75, 0.01);
        let expected = if index == 2 || index == 3 { (20.0, 16.0 / 3.0) } else { (30.0, 8.0) };
        assert_near(x, expected.0, 0.01);
        assert_near(y, expected.1, 0.01);
    }
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, fillPathForRange_reusedAcrossDifferentRanges_doesNotLeakScratchState
#[test]
fn a_target_filled_again_and_again_is_as_a_new_one() {
    let text = "abcdef\nabcd\nabc\u{05D0}\u{05D1}\u{05D2}def";
    let style = TextStyle { text_direction: Some(TextDirection::Ltr), ..TextStyle::DEFAULT };
    let reused_layout = layout(text, style.clone(), 10.0, None, u32::MAX);
    let reused = Path::new();
    reused_layout.fill_path_for_range(0, 11, &reused, CornerRadius::new(4.0, 4.0), true);
    assert!(!reused.is_empty());
    reused_layout.fill_path_for_range(4, 4, &reused, CornerRadius::ZERO, true);
    assert!(reused.is_empty());
    // Compose's character offsets 13..17 are bytes 13..20 here (two Hebrew letters).
    let offset = |chars: usize| text.char_indices().nth(chars).map_or(text.len(), |(at, _)| at);
    for (start, end, merge_lines) in
        [(13, 17, false), (0, 4, true), (2, 10, true), (0, text.chars().count(), false)]
    {
        let (start, end) = (offset(start), offset(end));
        let boxes = reused_layout.get_bounding_boxes_for_range(start, end);
        let radius = CornerRadius::new(4.0, 2.0);
        reused_layout.fill_path_for_range(start, end, &reused, radius, merge_lines);
        let fresh_layout = layout(text, style.clone(), 10.0, None, u32::MAX);
        assert_eq!(boxes, fresh_layout.get_bounding_boxes_for_range(start, end));
        let fresh = filled(&fresh_layout, start, end, radius, merge_lines);
        assert_eq!(reused.data().commands(), fresh.data().commands(), "{start}..{end}");
    }
    // Many ranges, then one more.
    let long_text = "a".repeat(80);
    let long_reused = simple(&long_text, 10.0);
    for end in 1..=70 {
        long_reused.fill_path_for_range(0, end, &reused, CornerRadius::ZERO, true);
    }
    long_reused.fill_path_for_range(2, 9, &reused, CornerRadius::ZERO, true);
    let expected = filled(&simple(&long_text, 10.0), 2, 9, CornerRadius::ZERO, true);
    assert_eq!(reused.data().commands(), expected.data().commands());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, calculatePath_withTextLayoutResultLineIndices_mergesOnlyConsecutiveLines
#[test]
fn boxes_of_lines_apart_are_not_merged() {
    // Line 0 "abcdef", line 1 empty, line 2 "abcd": nothing in between.
    let layout = simple("abcdef\n\nabcd", 10.0);
    let length = layout.layout_input.text.len();
    let path = filled(&layout, 0, length, CornerRadius::ZERO, true);
    assert!(covered(&path, Rect::new(0.0, 12.0, 40.0, 18.0)).is_empty());
    assert!(!covered(&path, Rect::new(0.0, 0.0, 60.0, 10.0)).is_empty());
    assert!(!covered(&path, Rect::new(0.0, 20.0, 40.0, 30.0)).is_empty());
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/MultiParagraphBoundingBoxesForRangeTest.kt, calculateBoundingBoxList_beyondLaidOutParagraphs_returnsEmpty
#[test]
fn ranges_past_the_lines_laid_out_have_no_boxes_nor_path() {
    let layout = layout("01234\n56789", TextStyle::DEFAULT, 10.0, None, 1);
    assert!(layout.get_bounding_boxes_for_range(2, 2).is_empty());
    assert!(layout.get_bounding_boxes_for_range(6, 11).is_empty());
    assert!(filled(&layout, 6, 11, CornerRadius::ZERO, true).is_empty());
}

/// Merged, each step of the outline is where the lines meet, and an unrounded outline is a
/// polygon of straight sides: one contour for three lines.
#[test]
fn merged_lines_are_one_outline_stepping_at_their_seams() {
    let layout = simple("abcdef\nabcdefgh\nab", 10.0);
    let length = layout.layout_input.text.len();
    let path = filled(&layout, 0, length, CornerRadius::ZERO, true);
    let commands = path.data().commands().to_vec();
    let point = |x: f32, y: f32| Offset::new(x, y);
    use PathCommand::{Close, LineTo, MoveTo};
    assert_eq!(
        commands,
        [
            MoveTo(point(0.0, 0.0)),
            LineTo(point(60.0, 0.0)),
            LineTo(point(60.0, 10.0)),
            LineTo(point(80.0, 10.0)),
            LineTo(point(80.0, 20.0)),
            LineTo(point(20.0, 20.0)),
            LineTo(point(20.0, 30.0)),
            LineTo(point(0.0, 30.0)),
            Close,
        ]
    );
    // Unmerged: a rectangle a line.
    let unmerged = filled(&layout, 0, length, CornerRadius::ZERO, false);
    let moves = unmerged.data().commands().iter().filter(|c| matches!(c, MoveTo(_))).count();
    assert_eq!(moves, 3);
}

/// Left edges within half a pixel of the first line's are the first line's, so the outline's
/// sides stay straight: a paragraph indented 0.3 px, one not, one right-aligned in 100 px.
#[test]
fn left_edges_within_half_a_pixel_of_the_first_are_one() {
    use compose_ui::{TextAlign, TextIndent};
    let indented = ParagraphStyle {
        text_indent: Some(TextIndent { first_line: 0.3.sp(), rest_line: 0.sp() }),
        ..ParagraphStyle::DEFAULT
    };
    let right = ParagraphStyle { text_align: Some(TextAlign::Right), ..ParagraphStyle::DEFAULT };
    let text = AnnotatedString::build(|builder| {
        builder.with_paragraph_style(indented, |builder| {
            builder.append("ab");
        });
        builder.with_paragraph_style(ParagraphStyle::DEFAULT, |builder| {
            builder.append("ab");
        });
        builder.with_paragraph_style(right, |builder| {
            builder.append("abcdefghi");
        });
    });
    let layout = layout(text, TextStyle::DEFAULT, 10.0, Some(100.0), u32::MAX);
    let boxes = layout.get_bounding_boxes_for_range(0, layout.layout_input.text.len());
    assert_eq!(boxes.len(), 3, "{boxes:?}");
    assert_near(boxes[0].left, 0.3, 0.001);
    assert_near(boxes[1].left, 0.0, 0.001);
    assert!(boxes[2].left > 1.0, "{boxes:?}");
    let path = filled(&layout, 0, layout.layout_input.text.len(), CornerRadius::ZERO, true);
    let data = path.data();
    let commands = data.commands();
    assert_eq!(commands.iter().filter(|c| matches!(c, PathCommand::MoveTo(_))).count(), 1);
    // The second line's left is the first's: no side at 0.
    let xs: Vec<f32> = commands
        .iter()
        .filter_map(|command| match command {
            PathCommand::MoveTo(at) | PathCommand::LineTo(at) => Some(at.x),
            _ => None,
        })
        .collect();
    assert!(xs.iter().all(|&x| x != 0.0), "{xs:?}");
    assert!(xs.contains(&boxes[0].left) && xs.contains(&boxes[2].left), "{xs:?}");
    // The last line's left within half a pixel of the first's: a rectangle at the first's.
    let two_lines = layout.get_line_end(1, false);
    let path = filled(&layout, 0, two_lines, CornerRadius::ZERO, true);
    assert_eq!(path.get_bounds(), Rect::new(boxes[0].left, 0.0, boxes[0].right, boxes[1].bottom));
}
