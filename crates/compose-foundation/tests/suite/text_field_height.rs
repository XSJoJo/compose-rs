//! A state-based field's height in lines and its size, from Compose's `HeightInLinesModifierTest`,
//! `HeightInLinesModifierCalculationTest`, `TextFieldSingleLineHeightTest` and
//! `TextFieldSizeModifierTest`: a multi-line field between its fewest and most lines high, a
//! single line as high as its text, the parent's constraints winning, and the intrinsic heights
//! of a field in lines.
//!
//! Compose's `heightInLines` modifier is internal; its tests of the modifier itself (its
//! inspectable value, the modifier given back when nothing limits it) have no counterpart, and
//! its validation is `TextFieldLineLimits::multi_line`'s and `BasicText`'s `min_lines`'.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{
    Constraints, IntSize, IntrinsicSize, Layout, LayoutDirection, TextLayoutResult, TextStyle,
    UnitExt, measure_policy,
};

const LONG_TEXT: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod \
    tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud \
    exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat. Duis aute irure dolor in \
    reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.";

fn cell<T: PartialEq + 'static>(value: T) -> MutableState<T> {
    Box::leak(Box::new(mutable_state_of(value))).handle()
}

type Heard = Rc<RefCell<Option<TextLayoutResult>>>;

/// What the boxes around fields of `texts` with `limits`, each 100 dp wide, measure, and what
/// their fields last laid out, as Compose's `HeightObservingText`.
fn heights_of(
    fields: Vec<(&'static str, TextFieldLineLimits)>,
) -> (ComposeTestRule, Vec<Rc<Cell<i32>>>, Vec<Heard>) {
    let heights: Vec<Rc<Cell<i32>>> = fields.iter().map(|_| Rc::new(Cell::new(-1))).collect();
    let layouts: Vec<Heard> = fields.iter().map(|_| Rc::default()).collect();
    let (into_heights, into_layouts) = (heights.clone(), layouts.clone());
    let ui = compose_test(move || {
        let (fields, into_heights, into_layouts) =
            (fields.clone(), into_heights.clone(), into_layouts.clone());
        Column!({
            for (index, &(text, limits)) in fields.iter().enumerate() {
                let height = into_heights[index].clone();
                let layout = into_layouts[index].clone();
                key(index, || {
                    observing_field(text, limits, TextStyle::DEFAULT, height, layout);
                });
            }
        });
    });
    (ui, heights, layouts)
}

/// Compose's `HeightObservingText`: a box hearing its size around a field 100 dp wide.
fn observing_field(
    text: &'static str,
    limits: TextFieldLineLimits,
    style: TextStyle,
    height: Rc<Cell<i32>>,
    layout: Heard,
) {
    let state = remember_text_field_state(text, None);
    Box!(modifier = Modifier.on_size_changed(move |size: IntSize| height.set(size.height)), {
        let layout = layout.clone();
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.required_width(100.dp()),
            text_style = style.clone(),
            line_limits = limits,
            on_text_layout = Some(move |result: TextLayoutResult| {
                *layout.borrow_mut() = Some(result);
            }),
        );
    });
}

fn heard(layout: &Heard) -> TextLayoutResult {
    layout.borrow().clone().expect("laid out")
}

fn multi_line(min: u32, max: u32) -> TextFieldLineLimits {
    TextFieldLineLimits::multi_line(min, max)
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, minLines_shortInputText
#[test]
fn a_short_text_is_its_fewest_lines_high() {
    let (_ui, heights, layouts) =
        heights_of(vec![("abc", multi_line(2, u32::MAX)), ("1\n2", multi_line(2, u32::MAX))]);
    assert_eq!(heard(&layouts[0]).line_count, 1);
    assert_eq!(heights[0].get(), heights[1].get());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, maxLines_shortInputText
#[test]
fn a_short_text_under_its_most_lines_is_its_own_height() {
    let (_ui, heights, layouts) = heights_of(vec![("abc", multi_line(1, 5))]);
    let layout = heard(&layouts[0]);
    assert_eq!(layout.line_count, 1);
    assert_eq!(layout.size.height, heights[0].get());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, maxLines_notApplied_infiniteMaxLines
#[test]
fn no_most_lines_leaves_a_long_text_its_height() {
    // Compose's `MultiLine(minHeightInLines = Int.MAX_VALUE)` takes the most lines' default,
    // `Int.MAX_VALUE`, too.
    let (_ui, heights, layouts) = heights_of(vec![(LONG_TEXT, multi_line(u32::MAX, u32::MAX))]);
    assert_eq!(heard(&layouts[0]).size.height, heights[0].get());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, minLines_invalidValue
#[test]
#[should_panic]
fn no_lines_at_least_is_refused() {
    multi_line(0, u32::MAX);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, maxLines_invalidValue
#[test]
#[should_panic]
fn no_lines_at_most_is_refused() {
    multi_line(1, 0);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, minLines_greaterThan_maxLines_invalidValue
#[test]
#[should_panic]
fn more_lines_at_least_than_at_most_is_refused() {
    multi_line(2, 1);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, minLines_longInputText
#[test]
fn a_long_text_over_its_fewest_lines_is_its_own_height() {
    let (_ui, heights, layouts) = heights_of(vec![(LONG_TEXT, multi_line(2, u32::MAX))]);
    let layout = heard(&layouts[0]);
    assert!(layout.line_count > 2, "{} lines", layout.line_count);
    assert_eq!(layout.size.height, heights[0].get());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, maxLines_longInputText
#[test]
fn a_long_text_is_its_most_lines_high() {
    let (_ui, heights, layouts) =
        heights_of(vec![(LONG_TEXT, multi_line(1, 2)), ("1\n2", multi_line(1, 2))]);
    assert!(heard(&layouts[0]).line_count > 2);
    assert_eq!(heights[0].get(), heights[1].get());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, minLines_densityChange
#[test]
fn the_fewest_lines_follow_the_density() {
    let density = cell(compose_ui::Density::new(1.0));
    let height = Rc::new(Cell::new(-1));
    let into = height.clone();
    let mut ui = compose_test(move || {
        let into = into.clone();
        CompositionLocalProvider(compose_ui::LOCAL_DENSITY.provides(density.get()), move || {
            let style = TextStyle::DEFAULT;
            observing_field("abc", multi_line(2, u32::MAX), style, into.clone(), Rc::default());
        });
    });
    ui.wait_for_idle();
    let at_one = height.get();
    density.set(compose_ui::Density::new(2.0));
    ui.wait_for_idle();
    assert!((height.get() - at_one * 2).abs() <= 1, "{at_one} then {}", height.get());
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, minLines_layoutDirectionChange
#[test]
fn the_fewest_lines_keep_their_height_right_to_left() {
    let direction = cell(LayoutDirection::Ltr);
    let size = Rc::new(Cell::new(IntSize::ZERO));
    let layout: Heard = Rc::default();
    let (into_size, into_layout) = (size.clone(), layout.clone());
    let mut ui = compose_test(move || {
        let (into_size, into_layout) = (into_size.clone(), into_layout.clone());
        let provided = compose_ui::LOCAL_LAYOUT_DIRECTION.provides(direction.get());
        CompositionLocalProvider(provided, move || {
            let state = remember_text_field_state("aaaaa ".repeat(5), None);
            let (into_size, into_layout) = (into_size.clone(), into_layout.clone());
            Box!(modifier = Modifier.on_size_changed(move |it: IntSize| into_size.set(it)), {
                let into_layout = into_layout.clone();
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.required_width(100.dp()),
                    line_limits = multi_line(2, u32::MAX),
                    on_text_layout = Some(move |result: TextLayoutResult| {
                        *into_layout.borrow_mut() = Some(result);
                    }),
                );
            });
        });
    });
    ui.wait_for_idle();
    let left_to_right = size.get();
    let first = heard(&layout);
    assert!(first.line_count >= 2);
    for line in 0..first.line_count as usize {
        assert_eq!(first.get_line_left(line), 0.0, "line {line} at the left");
    }
    *layout.borrow_mut() = None;
    direction.set(LayoutDirection::Rtl);
    ui.wait_for_idle();
    let right_to_left = size.get();
    let second = heard(&layout);
    assert_eq!(right_to_left.height, left_to_right.height);
    assert!(second.line_count >= 2);
    for line in 0..second.line_count as usize {
        // The text is laid out at the field's width less the cursor's (D1137): Compose's is at
        // the field's, within a pixel.
        let right = second.get_line_right(line) + 2.0;
        assert!(
            (right - right_to_left.width as f32).abs() <= 1.0,
            "line {line} at the right edge: {right} of {}",
            right_to_left.width
        );
    }
}

/// What a column of intrinsic height `intrinsic` measures around a label and, under it, a field
/// of `text` with `limits` filling its width; the label's and the field's layouts.
fn label_and_field(
    intrinsic: IntrinsicSize,
    text: &'static str,
    limits: TextFieldLineLimits,
) -> (i32, TextLayoutResult, TextLayoutResult) {
    let size = Rc::new(Cell::new(IntSize::ZERO));
    let (label, field): (Heard, Heard) = (Rc::default(), Rc::default());
    let (into_size, into_label, into_field) = (size.clone(), label.clone(), field.clone());
    let mut ui = compose_test(move || {
        let into_size = into_size.clone();
        let (into_label, into_field) = (into_label.clone(), into_field.clone());
        Column!(
            modifier =
                Modifier.on_size_changed(move |it: IntSize| into_size.set(it)).height(intrinsic),
            {
                let (into_label, into_field) = (into_label.clone(), into_field.clone());
                Layout!(
                    measure_policy = measure_policy(|scope, measurables, constraints| {
                        let label = scope.measure(
                            measurables[0],
                            Constraints { min_width: 0, min_height: 0, ..constraints },
                        );
                        let field_constraints = Constraints {
                            min_height: 0,
                            max_height: (constraints.max_height - label.height).clamp(0, 1000),
                            ..constraints
                        };
                        let field = scope.measure(measurables[1], field_constraints);
                        scope.place(&label, 0, 0);
                        scope.place(&field, 0, label.height);
                        scope.layout(constraints.max_width, label.height + field.height)
                    }),
                    {
                        let into_label = into_label.clone();
                        BasicText!(
                            "Label",
                            on_text_layout = Some(move |result: TextLayoutResult| {
                                *into_label.borrow_mut() = Some(result);
                            }),
                        );
                        let state = remember_text_field_state(text, None);
                        let into_field = into_field.clone();
                        BasicTextFieldOfState!(
                            state,
                            modifier = Modifier.fill_max_width(),
                            line_limits = limits,
                            on_text_layout = Some(move |result: TextLayoutResult| {
                                *into_field.borrow_mut() = Some(result);
                            }),
                        );
                    }
                );
            }
        );
    });
    ui.wait_for_idle();
    (size.get().height, heard(&label), heard(&field))
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, heightInLines_minIntrinsicHeight_textLongerThanMaxLines
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, heightInLines_maxIntrinsicHeight_textLongerThanMaxLines
#[test]
fn a_fields_intrinsic_height_is_its_most_lines_for_a_longer_text() {
    for intrinsic in [IntrinsicSize::Min, IntrinsicSize::Max] {
        let (height, label, field) =
            label_and_field(intrinsic, "One\nTwo\nThree", multi_line(1, 2));
        // Compose's `toInt()` of a whole pixel on Android; the test measurer's lines are 17.5 px,
        // and a field is its text's height rounded up.
        let two_lines = field.get_line_bottom(1).ceil() as i32;
        assert_eq!(height, label.size.height + two_lines, "{intrinsic:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, heightInLines_minIntrinsicHeight_textBetweenMinMaxLines
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, heightInLines_maxIntrinsicHeight_textBetweenMinMaxLines
#[test]
fn a_fields_intrinsic_height_is_its_text_between_its_lines() {
    for intrinsic in [IntrinsicSize::Min, IntrinsicSize::Max] {
        let (height, label, field) =
            label_and_field(intrinsic, "One\nTwo\nThree", multi_line(2, 5));
        let three_lines = field.get_line_bottom(2).ceil() as i32;
        assert_eq!(height, label.size.height + three_lines, "{intrinsic:?}");
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, heightInLines_minIntrinsicHeight_textShorterThanMinLines
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierTest.kt, heightInLines_maxIntrinsicHeight_textShorterThanMinLines
#[test]
fn a_fields_intrinsic_height_is_its_fewest_lines_for_a_shorter_text() {
    for intrinsic in [IntrinsicSize::Min, IntrinsicSize::Max] {
        let (height, label, field) =
            label_and_field(intrinsic, "One\nTwo", multi_line(4, u32::MAX));
        let two_lines = field.get_line_bottom(1) as i32;
        assert!(height > label.size.height + two_lines, "{intrinsic:?}: {height}");
    }
}

/// The heights fields of `texts` with `limits` measure, in a column.
fn field_heights(fields: Vec<(String, TextFieldLineLimits)>) -> Vec<i32> {
    let heights: Vec<Rc<Cell<i32>>> = fields.iter().map(|_| Rc::new(Cell::new(-1))).collect();
    let into = heights.clone();
    let mut ui = compose_test(move || {
        let (fields, into) = (fields.clone(), into.clone());
        Column!({
            for (index, (text, limits)) in fields.iter().enumerate() {
                let height = into[index].clone();
                key(index, || {
                    let state = remember_text_field_state(text, None);
                    BasicTextFieldOfState!(
                        state,
                        modifier =
                            Modifier.on_size_changed(move |it: IntSize| height.set(it.height)),
                        line_limits = *limits,
                    );
                });
            }
        });
    });
    ui.wait_for_idle();
    heights.iter().map(|height| height.get()).collect()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldSizeModifierTest.kt, singleLine_fixedHeight_ignoresTextContent
#[test]
fn a_single_lines_height_is_not_its_texts_length() {
    let single = TextFieldLineLimits::SingleLine;
    let heights = field_heights(vec![("text".into(), single), ("text".repeat(100), single)]);
    assert!(heights[0] > 0);
    assert_eq!(heights[1], heights[0]);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldSizeModifierTest.kt, multiLine_clampsHeight_betweenMinAndMax
#[test]
fn a_multi_line_height_is_between_its_lines() {
    let limits = multi_line(2, 5);
    let heights = field_heights(vec![
        ("1\n2".into(), limits),
        ("1\n".repeat(5), limits),
        ("1\n".repeat(10), limits),
    ]);
    assert_eq!(heights[2], heights[1], "ten lines held to five");
    assert!(heights[1] > heights[0], "five lines taller than two");
}

/// The height a field of five lines, at least `min` and at most `max` lines high, measures at
/// `constraints` (its width 200 dp).
fn height_given(
    min: u32,
    max: u32,
    constraints: impl Fn(Constraints) -> Constraints + 'static,
) -> i32 {
    let height = Rc::new(Cell::new(-1));
    let into = height.clone();
    let constraints = Rc::new(constraints);
    let mut ui = compose_test(move || {
        let (into, constraints) = (into.clone(), constraints.clone());
        Layout!(
            measure_policy = measure_policy(move |scope, measurables, given| {
                let placeable = scope.measure(measurables[0], constraints(given));
                into.set(placeable.height);
                scope.place(&placeable, 0, 0);
                scope.layout(placeable.width, placeable.height)
            }),
            {
                let state = remember_text_field_state("1\n2\n3\n4\n5", None);
                BasicTextFieldOfState!(
                    state,
                    modifier = Modifier.required_width(200.dp()),
                    line_limits = multi_line(min, max),
                );
            }
        );
    });
    ui.wait_for_idle();
    height.get()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldSizeModifierTest.kt, multiLine_parentForcesTightHeight_smallerThanMinLines
#[test]
fn a_tight_height_under_the_fewest_lines_wins() {
    assert_eq!(height_given(3, u32::MAX, |given| Constraints::fixed(given.max_width, 40)), 40);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldSizeModifierTest.kt, multiLine_parentForcesMaxHeight_smallerThanMaxLines
#[test]
fn a_most_height_under_the_most_lines_wins() {
    assert_eq!(height_given(1, 5, |given| Constraints { max_height: 40, ..given }), 40);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldSizeModifierTest.kt, emptyText_hasNonZeroMinSize
#[test]
fn an_empty_field_has_a_size() {
    let size = Rc::new(Cell::new(IntSize::ZERO));
    let into = size.clone();
    let mut ui = compose_test(move || {
        let into = into.clone();
        let state = remember_text_field_state("", None);
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.on_size_changed(move |it: IntSize| into.set(it)),
        );
    });
    ui.wait_for_idle();
    assert!(size.get().width > 0 && size.get().height > 0, "{:?}", size.get());
}

/// A value-based field of one line at most, wrapping, lays its text out on several lines at its
/// width: only a single line is laid out unbounded. Its layout is read through its semantics
/// (the value-based field has no `on_text_layout` here).
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldSizeModifierTest.kt, btf1_maxLinesOne_softWrapTrue_doesNotForceInfiniteWidth
#[test]
fn a_value_field_of_one_line_at_most_wraps_at_its_width() {
    let mut ui = compose_test(|| {
        BasicTextField!(
            "a ".repeat(50),
            |_| {},
            modifier = Modifier.required_width(100.dp()).test_tag("field"),
            max_lines = 1,
        );
    });
    let mut results = Vec::new();
    ui.on_node_with_tag("field").perform_semantics_action_with(
        &compose_ui::SemanticsActions::GET_TEXT_LAYOUT_RESULT,
        |get| {
            get(&mut results);
        },
    );
    let layout = results.pop().expect("a layout");
    assert!(layout.line_count > 1, "{} lines", layout.line_count);
}

/// Arabic and Thai together, a tall script, as Compose's tests' `tallText`.
const TALL_TEXT: &str = "\u{0627}\u{0644}\u{0646}\u{0635}\u{0E17}\u{0E35}\u{0E48}";

/// A rule whose measurer lays Arabic and Thai out taller than Latin, as their fonts are.
fn tall_scripts_test(content: impl Fn() + 'static) -> ComposeTestRule {
    compose_test_with(
        DEFAULT_SIZE,
        compose_ui::Density::default(),
        Box::new(compose_ui::platform::FixedTextMeasurer::new().with_tall_scripts()),
        content,
    )
}

/// A single line's height before and after its text becomes `TALL_TEXT`.
fn single_line_height_to_tall_text(text: &'static str) -> (i32, i32) {
    let state = **Box::leak(Box::new(TextFieldState::new(text)));
    let height = Rc::new(Cell::new(-1));
    let into = height.clone();
    let mut ui = tall_scripts_test(move || {
        let into = into.clone();
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.on_size_changed(move |it: IntSize| into.set(it.height)),
            line_limits = TextFieldLineLimits::SingleLine,
        );
    });
    ui.wait_for_idle();
    let before = height.get();
    state.set_text_and_place_cursor_at_end(TALL_TEXT);
    ui.wait_for_idle();
    (before, height.get())
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldSingleLineHeightTest.kt, singleLineTextField_fromEmptyToTallText_updatesHeight
#[test]
fn a_single_line_grows_from_empty_to_a_tall_text() {
    let (empty, tall) = single_line_height_to_tall_text("");
    assert!(empty < tall, "{empty} then {tall}");
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldSingleLineHeightTest.kt, singleLineTextField_fromLatinToTallText_updatesHeight
#[test]
fn a_single_line_grows_from_latin_to_a_tall_text() {
    let (latin, tall) = single_line_height_to_tall_text("TEXT");
    assert!(latin < tall, "{latin} then {tall}");
}

/// The heights of two fields of `TALL_TEXT` side by side.
fn tall_heights(
    first: impl Fn(Modifier) + 'static,
    second: impl Fn(Modifier) + 'static,
) -> (i32, i32) {
    let heights = Rc::new(Cell::new((-1, -1)));
    let into = heights.clone();
    let (first, second) = (Rc::new(first), Rc::new(second));
    let mut ui = tall_scripts_test(move || {
        let (a, b) = (into.clone(), into.clone());
        let (first, second) = (first.clone(), second.clone());
        Row!({
            let (a, b) = (a.clone(), b.clone());
            first(Modifier.on_size_changed(move |it: IntSize| a.set((it.height, a.get().1))));
            second(Modifier.on_size_changed(move |it: IntSize| b.set((b.get().0, it.height))));
        });
    });
    ui.wait_for_idle();
    heights.get()
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldSingleLineHeightTest.kt, legacy_maxLines1_hasSameHeightAsSingleLine_withTallText
#[test]
fn a_value_field_of_one_line_at_most_is_a_single_lines_height() {
    let (max_lines_one, single_line) = tall_heights(
        |modifier| {
            BasicTextField!(TALL_TEXT.to_string(), |_| {}, modifier = modifier, max_lines = 1)
        },
        |modifier| {
            BasicTextField!(TALL_TEXT.to_string(), |_| {}, modifier = modifier, single_line = true)
        },
    );
    assert_eq!(max_lines_one, single_line);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/TextFieldSingleLineHeightTest.kt, BTF2_multiLineTextField_maxLines1_hasDifferentHeightThanSingleLine_withTallText
#[test]
fn a_state_field_of_one_line_at_most_is_shorter_than_a_single_line() {
    let (multi, single) = tall_heights(
        |modifier| {
            let state = remember_text_field_state(TALL_TEXT, None);
            BasicTextFieldOfState!(state, modifier = modifier, line_limits = multi_line(1, 1));
        },
        |modifier| {
            let state = remember_text_field_state(TALL_TEXT, None);
            BasicTextFieldOfState!(
                state,
                modifier = modifier,
                line_limits = TextFieldLineLimits::SingleLine
            );
        },
    );
    assert!(multi < single, "{multi} against {single}");
}

/// What `text` measures in `style`, as Compose's `computeSizeForDefaultText` (a text of "H",
/// one line or two joined by a new line).
fn height_of_text(measurer: &compose_ui::TextMeasurer, style: &TextStyle, text: &str) -> i32 {
    let options = compose_ui::TextMeasureOptions {
        style: style.clone(),
        max_lines: 1000,
        ..Default::default()
    };
    measurer.measure(text, options).size.height
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/HeightInLinesModifierCalculationTest.kt, heightInLinesCalculation
#[test]
fn a_fields_height_in_lines_is_its_lines_of_default_text() {
    use compose_ui::{LineHeightAlignment, LineHeightMode, LineHeightStyle, LineHeightTrim};
    let mut styles = vec![TextStyle { font_size: 24.sp(), ..TextStyle::DEFAULT }];
    for alignment in [
        LineHeightAlignment::TOP,
        LineHeightAlignment::CENTER,
        LineHeightAlignment::PROPORTIONAL,
        LineHeightAlignment::BOTTOM,
    ] {
        for trim in [
            LineHeightTrim::None,
            LineHeightTrim::Both,
            LineHeightTrim::FirstLineTop,
            LineHeightTrim::LastLineBottom,
        ] {
            for mode in [LineHeightMode::Fixed, LineHeightMode::Minimum, LineHeightMode::Tight] {
                styles.push(TextStyle {
                    font_size: 24.sp(),
                    line_height: 32.sp(),
                    line_height_style: Some(LineHeightStyle { alignment, trim, mode }),
                    ..TextStyle::DEFAULT
                });
            }
        }
    }
    for text in ["", "text"] {
        for (min, max) in [(2, 5), (3, 3)] {
            for style in &styles {
                let measured = Rc::new(Cell::new((-1, -1)));
                let into = measured.clone();
                let style = style.clone();
                let label = format!("{:?} {:?}", style.line_height, style.line_height_style);
                let mut ui = compose_test(move || {
                    let measurer = compose_ui::remember_text_measurer(8);
                    let into = into.clone();
                    let style_for_field = style.clone();
                    let style = style.clone();
                    Layout!(
                        measure_policy = measure_policy(move |scope, measurables, constraints| {
                            let field = scope.measure(measurables[0], constraints);
                            let first = height_of_text(&measurer, &style, "H");
                            let line = height_of_text(&measurer, &style, "H\nH") - first;
                            let min_height = if min == 1 { 0 } else { first + line * (min - 1) };
                            let max_height = first + line * (max - 1);
                            // An empty field measures as one "H" high, as Compose's
                            // `EmptyTextReplacement`.
                            let shown = if text.is_empty() { "H" } else { text };
                            let raw = height_of_text(&measurer, &style, shown);
                            into.set((field.height, raw.clamp(min_height, max_height)));
                            scope.layout(0, 0)
                        }),
                        {
                            let state = remember_text_field_state(text, None);
                            BasicTextFieldOfState!(
                                state,
                                line_limits = multi_line(min as u32, max as u32),
                                text_style = style_for_field.clone(),
                            );
                        }
                    );
                });
                ui.wait_for_idle();
                let (field, expected) = measured.get();
                assert_eq!(field, expected, "text {text:?}, lines {min} to {max}, {label}");
            }
        }
    }
}
