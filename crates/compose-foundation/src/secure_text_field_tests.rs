//! Compose's `OutputTransformationPerGlyphTest` and `PasswordInputTransformationTest`, on the
//! secure field's internals. Offsets are bytes (D1100): a face is 4, the rainbow flag 14.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_ui::TextRange;

use super::*;
use crate::output_transformation::{map_from_transformed, map_to_transformed, transformed_text};
use crate::text_field_buffer::TextFieldCharSequence;

/// A rainbow flag: a white flag, a variation selector, a joiner and a rainbow (14 bytes).
const COMPLEX_EMOJI: &str = "\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}";
/// A grinning face (4 bytes).
const REGULAR_EMOJI: &str = "\u{1F600}";
const SKIN_TONE_EMOJI: &str = "\u{1F44D}\u{1F3FD}";
const REGIONAL_FLAG_EMOJI: &str = "\u{1F1FA}\u{1F1F8}";

fn buffer(text: &str, selection: TextRange) -> TextFieldBuffer {
    TextFieldBuffer::new(TextFieldCharSequence::new(Rc::from(text), selection, None))
}

fn per_glyph(transform: impl Fn(TextRange) -> Option<String> + 'static) -> PerGlyph {
    PerGlyph(Box::new(transform))
}

fn shown(text: &str, transform: impl Fn(TextRange) -> Option<String> + 'static) -> String {
    let mut buffer = buffer(text, TextRange::ZERO);
    per_glyph(transform).transform_output(&mut buffer);
    buffer.as_str().to_string()
}

fn star(_: TextRange) -> Option<String> {
    Some("*".into())
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, replacesAllCharacters_withMask
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, doesNotReplace_whenTransformReturnsNull
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, singleCharacter_replacesCorrectly
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, replacesComplexEmoji_asSingleGlyph
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, replacesRegularEmoji_asSingleGlyph
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, singleComplexEmojiOnly_replacesAsSingleGlyph
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, consecutiveEmojis_replacesEachAsSingleGlyph
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, combiningDiacriticalMark_replacesAsSingleGlyph
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, skinToneModifiedEmoji_replacesAsSingleGlyph
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, regionalIndicatorFlagEmoji_replacesAsSingleGlyph
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, crlf_replacesAsSingleGlyph
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, emptyString_doesNothing
#[test]
fn each_glyph_is_masked_once() {
    let consecutive = format!("{COMPLEX_EMOJI}{REGULAR_EMOJI}{COMPLEX_EMOJI}{REGULAR_EMOJI}");
    let cases = [
        ("hello".to_string(), "*****"),
        ("a".to_string(), "*"),
        (format!("a{COMPLEX_EMOJI}b"), "***"),
        (format!("a{REGULAR_EMOJI}b"), "***"),
        (COMPLEX_EMOJI.to_string(), "*"),
        (consecutive, "****"),
        ("ae\u{301}b".to_string(), "***"),
        (format!("a{SKIN_TONE_EMOJI}b"), "***"),
        (format!("a{REGIONAL_FLAG_EMOJI}b"), "***"),
        ("a\r\nb".to_string(), "***"),
    ];
    for (text, masked) in cases {
        assert_eq!(shown(&text, star), masked, "{text:?}");
    }
    assert_eq!(shown("hello", |_| None), "hello");
    let calls = Rc::new(Cell::new(0));
    let counted = calls.clone();
    let empty = shown("", move |_| {
        counted.set(counted.get() + 1);
        Some("*".into())
    });
    assert_eq!((empty.as_str(), calls.get()), ("", 0));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, replacesOnlySpecificCharacters
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, replacesMixedContent_correctly
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, replacesGlyph_withLongerString
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, replacesGlyph_withEmptyString
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, replacesAllGlyphs_withEmptyString
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, replacesGlyphs_withComplexEmojiMask
#[test]
fn each_glyph_is_what_the_function_gives_for_its_range() {
    let ls = shown("hello", |range| (range.start == 2 || range.start == 3).then(|| "L".into()));
    assert_eq!(ls, "heLLo");
    // "a" [0, 1), the flag [1, 15), "b" [15, 16), the face [16, 20).
    let mixed = format!("a{COMPLEX_EMOJI}b{REGULAR_EMOJI}");
    let named = shown(&mixed, |range| match range.start {
        0 => Some("A".into()),
        1 => Some("C".into()),
        16 => Some("R".into()),
        _ => None,
    });
    assert_eq!(named, "ACbR");
    assert_eq!(shown(&format!("a{REGULAR_EMOJI}c"), |_| Some("xx".into())), "xxxxxx");
    let without_ls =
        shown("hello", |range| (range.start == 2 || range.start == 3).then(String::new));
    assert_eq!(without_ls, "heo");
    assert_eq!(shown(&format!("a{COMPLEX_EMOJI}b"), |_| Some(String::new())), "");
    let flags = shown(&format!("a{REGULAR_EMOJI}b"), |_| Some(COMPLEX_EMOJI.into()));
    assert_eq!(flags, COMPLEX_EMOJI.repeat(3));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, passesCorrectGlyphRanges_inReverseOrder
// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, adjustsSelection_whenGlyphLengthsChange
#[test]
fn glyphs_are_visited_from_the_end_and_the_selection_follows() {
    let visited: Rc<RefCell<Vec<TextRange>>> = Rc::default();
    let into = visited.clone();
    // "a" [0, 1), the face [1, 5), "e" and its accent [5, 8), the flag [8, 22), "b" [22, 23).
    shown(&format!("a{REGULAR_EMOJI}e\u{301}{COMPLEX_EMOJI}b"), move |range| {
        into.borrow_mut().push(range);
        Some("*".into())
    });
    let expected =
        [(22, 23), (8, 22), (5, 8), (1, 5), (0, 1)].map(|(start, end)| TextRange::new(start, end));
    assert_eq!(*visited.borrow(), expected);

    let mut selected = buffer(&format!("a{COMPLEX_EMOJI}b"), TextRange::new(1, 15));
    per_glyph(star).transform_output(&mut selected);
    assert_eq!((selected.as_str(), selected.selection()), ("***", TextRange::new(1, 2)));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/OutputTransformationPerGlyphTest.kt, transformedTextFieldState_mapsOffsetsForwardAndBackward
#[test]
fn offsets_map_both_ways_through_whole_glyphs() {
    // "a" [0, 1), the flag [1, 15), "b" [15, 16); shown as three faces [0, 4), [4, 8), [8, 12).
    let text: Rc<str> = Rc::from(format!("a{COMPLEX_EMOJI}b"));
    let value = TextFieldCharSequence::new(text, TextRange::ZERO, None);
    let transformation = OutputTransformation::new(per_glyph(|_| Some(REGULAR_EMOJI.into())));
    let (shown, mapping) = transformed_text(&value, &transformation).expect("transformed");
    assert_eq!(shown, REGULAR_EMOJI.repeat(3));
    let to = |offset| map_to_transformed(TextRange::at(offset), &mapping, None);
    let forward = [to(0), to(1), to(5), to(15), to(16)];
    let expected = [(0, 0), (4, 4), (4, 8), (8, 8), (12, 12)];
    assert_eq!(forward, expected.map(|(start, end)| TextRange::new(start, end)));
    let from = |offset| map_from_transformed(TextRange::at(offset), &mapping);
    let backward = [from(0), from(2), from(4), from(6), from(8), from(10), from(12)];
    let expected = [(0, 0), (0, 1), (1, 1), (1, 15), (15, 15), (15, 16), (16, 16)];
    assert_eq!(backward, expected.map(|(start, end)| TextRange::new(start, end)));
}

/// A password transformation in `mode`, under the platform's settings, its hides counted.
fn password(
    mode: TextObfuscationMode,
    touch: bool,
    physical: bool,
) -> (PasswordInputTransformation, Rc<Cell<u32>>) {
    let hides = Rc::new(Cell::new(0));
    let counted = hides.clone();
    let transformation = PasswordInputTransformation::for_test(
        move || counted.set(counted.get() + 1),
        mode,
        PasswordVisibility { touch, physical },
    );
    (transformation, hides)
}

/// "****" with "d" typed at its end, on a physical keyboard or not.
fn typed(transformation: &PasswordInputTransformation, from_hardware: bool) {
    let mut typed = buffer("****", TextRange::at(4));
    typed.replace_part(4, 4, "d", 0, 1, from_hardware);
    transformation.transform_input(&mut typed);
}

// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/PasswordInputTransformationTest.kt, touchSource_respectsTouchSetting_true
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/PasswordInputTransformationTest.kt, touchSource_respectsTouchSetting_false
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/PasswordInputTransformationTest.kt, hardwareSource_respectsPhysicalSetting_true
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/PasswordInputTransformationTest.kt, hardwareSource_respectsPhysicalSetting_false
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/PasswordInputTransformationTest.kt, revealLastTypedMode_alwaysReveals
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/PasswordInputTransformationTest.kt, hiddenMode_neverReveals
// Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/PasswordInputTransformationTest.kt, visibleMode_neverReveals
#[test]
fn a_typed_character_is_revealed_by_the_mode_and_the_keyboards_setting() {
    use TextObfuscationMode::*;
    let cases = [
        (System, true, false, false, true),
        (System, false, true, false, false),
        (System, false, true, true, true),
        (System, true, false, true, false),
        (RevealLastTyped, false, false, true, true),
        (Hidden, true, true, false, false),
        (Visible, true, true, false, false),
    ];
    for (mode, touch, physical, from_hardware, reveals) in cases {
        let (transformation, hides) = password(mode, touch, physical);
        typed(&transformation, from_hardware);
        let case = format!("{mode:?} touch {touch} physical {physical} hardware {from_hardware}");
        assert_eq!(transformation.revealed(), reveals.then_some(4), "{case}");
        assert_eq!(hides.get(), u32::from(reveals), "{case}");
    }
}

/// Two characters at once, or one with a selection after it, reveal nothing and hide what was;
/// a glyph of several characters is one; the timer starts again only for a new place.
#[test]
fn only_a_single_glyph_typed_alone_is_revealed() {
    let (transformation, hides) = password(TextObfuscationMode::RevealLastTyped, true, true);
    let mut emoji = buffer("ab", TextRange::at(2));
    emoji.replace(2, 2, COMPLEX_EMOJI);
    transformation.transform_input(&mut emoji);
    assert_eq!((transformation.revealed(), hides.get()), (Some(2), 1));
    let mut again = buffer("ab", TextRange::at(2));
    again.replace(2, 2, "c");
    transformation.transform_input(&mut again);
    assert_eq!((transformation.revealed(), hides.get()), (Some(2), 1), "the same place");
    let mut pasted = buffer("ab", TextRange::at(2));
    pasted.replace(2, 2, "cd");
    transformation.transform_input(&mut pasted);
    assert_eq!(transformation.revealed(), None);
    let mut selected = buffer("ab", TextRange::at(2));
    selected.replace(2, 2, "c");
    selected.set_selection(TextRange::new(0, 3));
    transformation.transform_input(&mut selected);
    assert_eq!(transformation.revealed(), None);
}
