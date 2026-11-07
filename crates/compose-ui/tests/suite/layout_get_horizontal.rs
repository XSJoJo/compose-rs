//! Compose's `LayoutGetHorizontalTest` and `LayoutGetHorizontalMultiLineTest` (its internal
//! `LayoutHelper.getHorizontalPosition(offset, usePrimaryDirection, upstream)`), adapted through
//! `Paragraph::get_horizontal_position(offset, use_primary_direction)`, which is the helper's
//! downstream position (an offset at a line break on the line it starts, as Android's
//! `Layout.getPrimaryHorizontal`): the downstream assertions are ported, the upstream ones have
//! no counterpart. Laid out by the fixed measurer with Compose's test font metrics (10 px a
//! character), in a forced direction, as Compose's tests' `TextDirectionHeuristics.LTR`/`RTL`.
//! Compose's offsets are characters; here they're bytes (a Hebrew letter is two).

use std::rc::Rc;

use compose_ui::platform::{FixedFontMetrics, FixedTextMeasurer, TextLayoutOptions, TextMeasurer};
use compose_ui::{
    AnnotatedString, Constraints, Density, LayoutDirection, Paragraph, TextDirection, TextStyle,
    UnitExt,
};

const LTR: TextDirection = TextDirection::Ltr;
const RTL: TextDirection = TextDirection::Rtl;

/// `text` at 10 px a character, `width` wide (as Android's layout width), in paragraph direction
/// `direction`.
fn layout(text: &str, width: i32, direction: TextDirection) -> Rc<dyn Paragraph> {
    let mut measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let style =
        TextStyle { font_size: 10.sp(), text_direction: Some(direction), ..TextStyle::DEFAULT };
    let options = TextLayoutOptions {
        direction: if direction == RTL { LayoutDirection::Rtl } else { LayoutDirection::Ltr },
        ..TextLayoutOptions::default()
    };
    let handle = measurer.layout_annotated(
        &AnnotatedString::from(text.to_string()),
        &style,
        options,
        Constraints::new(width, width, 0, Constraints::INFINITY),
        &Density::new(1.0),
    );
    measurer.paragraph(handle)
}

/// The byte offset of character `index` of `text` (its end past the last).
fn at(text: &str, index: usize) -> usize {
    text.char_indices().nth(index).map_or(text.len(), |(offset, _)| offset)
}

/// The downstream primary and secondary positions at character `index`.
fn positions(paragraph: &dyn Paragraph, index: usize) -> (f32, f32) {
    let offset = at(paragraph.text(), index);
    (
        paragraph.get_horizontal_position(offset, true),
        paragraph.get_horizontal_position(offset, false),
    )
}

/// Twelve Latin letters: |L1 L2 L3 L4 L5|L6 L7 L8 L9 LA|LB LC|.
const LATIN: &str = "abcdefghijkl";
/// Twelve Hebrew letters: R1 to RC.
const HEBREW: &str = "\u{05D1}\u{05D2}\u{05D3}\u{05D4}\u{05D5}\u{05D6}\u{05D7}\u{05D8}\u{05D9}\u{05DA}\u{05DB}\u{05DC}";

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_LTRText_LTRParagraph_FirstCharacter
#[test]
fn ltr_text_in_an_ltr_paragraph_before_its_first_character() {
    assert_eq!(positions(&*layout(LATIN, 50, LTR), 0), (0.0, 0.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_LTRText_LTRParagraph_LineBreakOffset
#[test]
fn ltr_text_in_an_ltr_paragraph_at_a_line_break() {
    // Before L6: at the start of the second line.
    assert_eq!(positions(&*layout(LATIN, 50, LTR), 5), (0.0, 0.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_LTRText_LTRParagraph_LastOffset
#[test]
fn ltr_text_in_an_ltr_paragraph_at_its_end() {
    assert_eq!(positions(&*layout(LATIN, 50, LTR), 12), (20.0, 20.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_LTRText_LTRParagraph_Other
#[test]
fn ltr_text_in_an_ltr_paragraph_inside_a_line() {
    // Before L8.
    assert_eq!(positions(&*layout(LATIN, 50, LTR), 7), (20.0, 20.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_LTRText_RTLParagraph_FirstCharacter
#[test]
fn ltr_text_in_an_rtl_paragraph_before_its_first_character() {
    assert_eq!(positions(&*layout(LATIN, 50, RTL), 0), (50.0, 0.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_LTRText_RTLParagraph_LineBreakOffset
#[test]
fn ltr_text_in_an_rtl_paragraph_at_a_line_break() {
    assert_eq!(positions(&*layout(LATIN, 50, RTL), 5), (50.0, 0.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_LTRText_RTLParagraph_LastOffset
#[test]
fn ltr_text_in_an_rtl_paragraph_at_its_end() {
    // |         LB LC|: after LC, the paragraph's run ends left of LB, the text's right of LC.
    assert_eq!(positions(&*layout(LATIN, 50, RTL), 12), (30.0, 50.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_LTRText_RTLParagraph_Other
#[test]
fn ltr_text_in_an_rtl_paragraph_inside_a_line() {
    assert_eq!(positions(&*layout(LATIN, 50, RTL), 7), (20.0, 20.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_RTLText_RTLParagraph_FirstCharacter
#[test]
fn rtl_text_in_an_rtl_paragraph_before_its_first_character() {
    assert_eq!(positions(&*layout(HEBREW, 50, RTL), 0), (50.0, 50.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_RTLText_RTLParagraph_LineBreakOffset
#[test]
fn rtl_text_in_an_rtl_paragraph_at_a_line_break() {
    assert_eq!(positions(&*layout(HEBREW, 50, RTL), 5), (50.0, 50.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_RTLText_RTLParagraph_LastCharacter
#[test]
fn rtl_text_in_an_rtl_paragraph_at_its_end() {
    assert_eq!(positions(&*layout(HEBREW, 50, RTL), 12), (30.0, 30.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_RTLText_RTLParagraph_Other
#[test]
fn rtl_text_in_an_rtl_paragraph_inside_a_line() {
    assert_eq!(positions(&*layout(HEBREW, 50, RTL), 7), (30.0, 30.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_RTLText_LTRParagraph_FirstCharacter
#[test]
fn rtl_text_in_an_ltr_paragraph_before_its_first_character() {
    assert_eq!(positions(&*layout(HEBREW, 50, LTR), 0), (0.0, 50.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_RTLText_LTRParagraph_LineBreakOffset
#[test]
fn rtl_text_in_an_ltr_paragraph_at_a_line_break() {
    assert_eq!(positions(&*layout(HEBREW, 50, LTR), 5), (0.0, 50.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_RTLText_LTRParagraph_LastCharacter
#[test]
fn rtl_text_in_an_ltr_paragraph_at_its_end() {
    // |RC RB         |: after RB.
    assert_eq!(positions(&*layout(HEBREW, 50, LTR), 12), (20.0, 0.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_RTLText_LTRParagraph_Other
#[test]
fn rtl_text_in_an_ltr_paragraph_inside_a_line() {
    assert_eq!(positions(&*layout(HEBREW, 50, LTR), 7), (30.0, 30.0));
}

/// L1, R1 to R3, then L2 to L9.
const LTR_RTL_INSIDE: &str = "a\u{05D1}\u{05D2}\u{05D3}efghijkl";
/// R1, L1 to L3, then R2 to R9.
const RTL_LTR_INSIDE: &str =
    "\u{05D0}abc\u{05D4}\u{05D5}\u{05D6}\u{05D7}\u{05D8}\u{05DA}\u{05DB}\u{05DC}";

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_BiDiTransitionNotLineBreakOffset_FromLTRToRTL_LTRParagraph
#[test]
fn from_ltr_to_rtl_inside_a_line_of_an_ltr_paragraph() {
    // |L1 R3 R2 R1 L2|, before R1.
    assert_eq!(positions(&*layout(LTR_RTL_INSIDE, 50, LTR), 1), (10.0, 40.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_BiDiTransitionNotLineBreakOffset_FromLTRToRTL_RTLParagraph
#[test]
fn from_ltr_to_rtl_inside_a_line_of_an_rtl_paragraph() {
    // |L2 R3 R2 R1 L1|, before R1.
    assert_eq!(positions(&*layout(LTR_RTL_INSIDE, 50, RTL), 1), (40.0, 50.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_BiDiTransitionNotLineBreakOffset_FromRTLtoLTR_RTLParagraph
#[test]
fn from_rtl_to_ltr_inside_a_line_of_an_rtl_paragraph() {
    // |R2 L1 L2 L3 R1|, before L1.
    assert_eq!(positions(&*layout(RTL_LTR_INSIDE, 50, RTL), 1), (40.0, 10.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_BiDiTransitionNotLineBreakOffset_FromRTLtoLTR_LTRParagraph
#[test]
fn from_rtl_to_ltr_inside_a_line_of_an_ltr_paragraph() {
    // |R1 L1 L2 L3 R2|, before L1.
    assert_eq!(positions(&*layout(RTL_LTR_INSIDE, 50, LTR), 1), (10.0, 0.0));
}

/// L1 to L5, then R1 to R7.
const LTR_THEN_RTL: &str = "abcde\u{05D5}\u{05D6}\u{05D7}\u{05D8}\u{05DA}\u{05DB}\u{05DC}";
/// R1 to R5, then L1 to L7.
const RTL_THEN_LTR: &str = "\u{05D0}\u{05D1}\u{05D2}\u{05D3}\u{05D4}abcdefg";

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_BiDiTransitionLineBreakOffset_FromLTRToRTL_LTRParagraph
#[test]
fn from_ltr_to_rtl_at_a_line_break_of_an_ltr_paragraph() {
    // |L1 L2 L3 L4 L5|R5 R4 R3 R2 R1|, before R1.
    assert_eq!(positions(&*layout(LTR_THEN_RTL, 50, LTR), 5), (0.0, 50.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_BiDiTransitionLineBreakOffset_FromLTRToRTL_RTLParagraph
#[test]
fn from_ltr_to_rtl_at_a_line_break_of_an_rtl_paragraph() {
    assert_eq!(positions(&*layout(LTR_THEN_RTL, 50, RTL), 5), (50.0, 50.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_BiDiTransitionLineBreakOffset_FromRTLToLTR_RTLParagraph
#[test]
fn from_rtl_to_ltr_at_a_line_break_of_an_rtl_paragraph() {
    // |R5 R4 R3 R2 R1|L1 L2 L3 L4 L5|, before L1.
    assert_eq!(positions(&*layout(RTL_THEN_LTR, 50, RTL), 5), (50.0, 0.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_BiDiTransitionLineBreakOffset_FromRTLToLTR_LTRParagraph
#[test]
fn from_rtl_to_ltr_at_a_line_break_of_an_ltr_paragraph() {
    assert_eq!(positions(&*layout(RTL_THEN_LTR, 50, LTR), 5), (0.0, 0.0));
}

/// L1, L2, then R1 to RA (a line break after R3).
const LTR_TO_RTL_BREAK_IN_RUN: &str =
    "ab\u{05D0}\u{05D1}\u{05D2}\u{05D3}\u{05D4}\u{05D5}\u{05D6}\u{05D7}\u{05D8}\u{05DA}\u{05DB}";
/// R1, R2, then L1 to LA (a line break after L3).
const RTL_TO_LTR_BREAK_IN_RUN: &str = "\u{05D0}\u{05D1}abcdefghij";

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_LineBreakOffset_FromMiddle_FromLTRToRTL_LTRParagraph
#[test]
fn a_line_break_inside_an_rtl_run_after_ltr_in_an_ltr_paragraph() {
    // |L1 L2 R3 R2 R1|R8 R7 R6 R5 R4|, before R4.
    assert_eq!(positions(&*layout(LTR_TO_RTL_BREAK_IN_RUN, 50, LTR), 5), (0.0, 50.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_LineBreakOffset_FromMiddle_FromLTRToRTL_RTLParagraph
#[test]
fn a_line_break_inside_an_rtl_run_after_ltr_in_an_rtl_paragraph() {
    // Compose's text here is a letter shorter; the second line is the same.
    let text = &LTR_TO_RTL_BREAK_IN_RUN[..LTR_TO_RTL_BREAK_IN_RUN.len() - "\u{05DB}".len()];
    assert_eq!(positions(&*layout(text, 50, RTL), 5), (50.0, 50.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_LineBreakOffset_FromMiddle_FromRTLToLTR_LTRParagraph
#[test]
fn a_line_break_inside_an_ltr_run_after_rtl_in_an_ltr_paragraph() {
    // |R2 R1 L1 L2 L3|L4 L5 L6 L7 L8|, before L4.
    assert_eq!(positions(&*layout(RTL_TO_LTR_BREAK_IN_RUN, 50, LTR), 5), (0.0, 0.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_LineBreakOffset_FromMiddle_FromRTLToLTR_RTLParagraph
#[test]
fn a_line_break_inside_an_ltr_run_after_rtl_in_an_rtl_paragraph() {
    // |L1 L2 L3 R2 R1|L4 L5 L6 L7 L8|, before L4.
    assert_eq!(positions(&*layout(RTL_TO_LTR_BREAK_IN_RUN, 50, RTL), 5), (50.0, 0.0));
}

/// L1 to L7, then R1 to R5 (a line break after L5).
const LTR_RUN_TO_RTL: &str = "abcdefg\u{05D0}\u{05D1}\u{05D2}\u{05D3}\u{05D4}";
/// R1 to R7, then L1 to L5 (a line break after R5).
const RTL_RUN_TO_LTR: &str = "\u{05D0}\u{05D1}\u{05D2}\u{05D3}\u{05D4}\u{05D5}\u{05D6}abcde";

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_LineBreakOffset_ToMiddle_FromLTRToRTL_LTRParagraph
#[test]
fn a_line_break_before_a_line_turning_rtl_in_an_ltr_paragraph() {
    // |L1 L2 L3 L4 L5|L6 L7 R3 R2 R1|, before L6.
    assert_eq!(positions(&*layout(LTR_RUN_TO_RTL, 50, LTR), 5), (0.0, 0.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_LineBreakOffset_ToMiddle_FromLTRToRTL_RTLParagraph
#[test]
fn a_line_break_before_a_line_turning_rtl_in_an_rtl_paragraph() {
    // |L1 L2 L3 L4 L5|R3 R2 R1 L6 L7|, before L6.
    assert_eq!(positions(&*layout(LTR_RUN_TO_RTL, 50, RTL), 5), (50.0, 30.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_LineBreakOffset_ToMiddle_FromRTLToLTR_LTRParagraph
#[test]
fn a_line_break_before_a_line_turning_ltr_in_an_ltr_paragraph() {
    // |R5 R4 R3 R2 R1|R7 R6 L1 L2 L3|, before R6.
    assert_eq!(positions(&*layout(RTL_RUN_TO_LTR, 50, LTR), 5), (0.0, 20.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BidiText_LineBreakOffset_ToMiddle_FromRTLToLTR_RTLParagraph
#[test]
fn a_line_break_before_a_line_turning_ltr_in_an_rtl_paragraph() {
    // |R5 R4 R3 R2 R1|L1 L2 L3 R7 R6|, before R6.
    assert_eq!(positions(&*layout(RTL_RUN_TO_LTR, 50, RTL), 5), (50.0, 50.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BiDi_Whitspace
#[test]
fn a_line_break_after_a_space_between_runs() {
    // R1 R2 SP R3 R4 SP | L1 L2 SP L3 L4 SP | L5 L6: before L1.
    let text = "\u{05D0}\u{05D1} \u{05D2}\u{05D3} ab cd ef";
    assert_eq!(positions(&*layout(text, 50, LTR), 6), (0.0, 0.0));
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalTest.kt, getHorizontal_BiDi_singleLine_Whitespace
#[test]
fn a_line_break_after_a_space_in_a_line_of_both_directions() {
    // R1 R2 SP L1 L2 SP | L3 L4 SP R3 R4 SP | L5 L6: before L3.
    let text = "\u{05D0}\u{05D1} ab cd \u{05D3}\u{05D4} ef";
    assert_eq!(positions(&*layout(text, 50, LTR), 6), (0.0, 0.0));
}

/// The downstream primary and secondary positions at each character of `text`, laid out three
/// characters wide.
fn every_position(text: &str, direction: TextDirection) -> Vec<(f32, f32)> {
    let paragraph = layout(text, 30, direction);
    (0..text.chars().count()).map(|index| positions(&*paragraph, index)).collect()
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalMultiLineTest.kt, ltrTextLtrDirection
#[test]
fn ltr_lines_of_one_character_in_an_ltr_paragraph() {
    // Each line's character at its start (0), its line feed at its end (10).
    let expected = [(0.0, 0.0), (10.0, 10.0)].repeat(4);
    assert_eq!(every_position(&"a\n".repeat(4), LTR), expected);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalMultiLineTest.kt, rtlTextRtlDirection
#[test]
fn rtl_lines_of_one_character_in_an_rtl_paragraph() {
    // Each line's start is 30, its end 20.
    let expected = [(30.0, 30.0), (20.0, 20.0)].repeat(4);
    assert_eq!(every_position(&"\u{05D0}\n".repeat(4), RTL), expected);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalMultiLineTest.kt, rtlTextLtrDirection
#[test]
fn rtl_lines_of_one_character_in_an_ltr_paragraph() {
    // The character: primary at the line's start, secondary right of it; its line feed the
    // other way round.
    let expected = [(0.0, 10.0), (10.0, 0.0)].repeat(4);
    assert_eq!(every_position(&"\u{05D0}\n".repeat(4), LTR), expected);
}

// Compose: compose/ui/ui-text/src/androidDeviceTest/kotlin/androidx/compose/ui/text/android/LayoutGetHorizontalMultiLineTest.kt, ltrTextRtlDirection
#[test]
fn ltr_lines_of_one_character_in_an_rtl_paragraph() {
    let expected = [(30.0, 20.0), (20.0, 30.0)].repeat(4);
    assert_eq!(every_position(&"a\n".repeat(4), RTL), expected);
}
