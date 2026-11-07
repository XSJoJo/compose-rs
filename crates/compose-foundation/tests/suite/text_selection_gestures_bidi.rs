//! A `SelectionContainer`'s gestures over bidirectional text, from Compose's
//! `TextSelectionGesturesBidiTest` as `SingleTextSelectionGesturesBidiTest` runs it over one text
//! and `MultiTextSelectionGesturesBidiTest` over a column of texts, one for each line (each test
//! both ways): a finger's long press and its drags across direction runs, a mouse's and a
//! trackpad's clicks and drags by characters, words and paragraphs (each mouse test with the
//! trackpad too).
//!
//! The text is three lines of words of five letters, Latin and Hebrew in turn, in a layout left
//! to right: its paragraphs are laid out left to right whatever their first letter (Compose's
//! `TextDirection.Unspecified`), so a Hebrew word shows mirrored inside its line and a position
//! at a character's right side, two pixels in (`isRtl`), is the offset before it. Each check is
//! the asserter's, with the direction at each end of the selection (`startLayoutDirection`,
//! `endLayoutDirection`: what the text says at the end's offset, Compose's
//! `getTextDirectionForOffset`), on the harness of `text_selection_gestures.rs`.

#![allow(non_snake_case)]

use compose_ui::ResolvedTextDirection;

use crate::text_selection_gestures::{CURSORS, Cursor, Gestures, RTL_CHAR, gestures_over};

use ResolvedTextDirection::{Ltr, Rtl};

/// The tests' `textContent`:
/// ```text
/// hello בבבבב hello
/// בבבבב hello בבבבב
/// hello בבבבב hello
/// ```
fn content() -> String {
    let (ltr, rtl) = ("hello", RTL_CHAR.to_string().repeat(5));
    format!("{ltr} {rtl} {ltr}\n{rtl} {ltr} {rtl}\n{ltr} {rtl} {ltr}")
}

/// Over one text, then over a text for each line.
fn variants() -> impl Iterator<Item = bool> {
    [false, true].into_iter()
}

fn bidi(multi: bool) -> Gestures {
    let mut it = gestures_over(content(), false, multi);
    // The asserter's directions, `Ltr` at first.
    it.directions = Some((Ltr, Ltr));
    it
}

/// Over one text and over several, with each cursor.
fn variants_and_cursors() -> impl Iterator<Item = (bool, Cursor)> {
    variants().flat_map(|multi| CURSORS.map(|kind| (multi, kind)))
}

/// Sets what the asserter's `startLayoutDirection` says.
fn start_direction(it: &mut Gestures, direction: ResolvedTextDirection) {
    let (_, end) = it.directions.expect("directions");
    it.directions = Some((direction, end));
}

/// Sets what the asserter's `endLayoutDirection` says.
fn end_direction(it: &mut Gestures, direction: ResolvedTextDirection) {
    let (start, _) = it.directions.expect("directions");
    it.directions = Some((start, direction));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenTouch_withLongPress_selectsSingleWord
#[test]
fn whenTouch_withLongPress_selectsSingleWord() {
    for multi in variants() {
        let mut it = bidi(multi);
        let at = it.character_side(26, false);
        it.long_click(at);
        it.expect(|it| {
            it.selection = Some((24, 29));
            it.handles_shown = true;
            it.toolbar_shown = true;
            it.haptics += 1;
        });
    }
}

/// As the tests' `touchLongPressThenDragForwardsAndBackTest`: a long press on the middle line's
/// Latin word, dragged to `forward` and back to `backward` (each `(offset, isRtl)`).
fn long_press_then_drag_forwards_and_back(
    forward: (usize, bool),
    forward_selection: (usize, usize),
    forward_end: ResolvedTextDirection,
    backward: (usize, bool),
    backward_selection: (usize, usize),
    backward_end: ResolvedTextDirection,
) {
    for multi in variants() {
        let mut it = bidi(multi);
        let at = it.character_side(26, false);
        it.long_press(at);
        it.expect(|it| {
            it.selection = Some((24, 29));
            it.handles_shown = true;
            it.haptics += 1;
        });
        let to = it.character_side(forward.0, forward.1);
        it.touch_drag_to(to);
        it.expect(|it| {
            it.selection = Some(forward_selection);
            end_direction(it, forward_end);
            it.haptics += 1;
        });
        let to = it.character_side(backward.0, backward.1);
        it.touch_drag_to(to);
        it.expect(|it| {
            it.selection = Some(backward_selection);
            end_direction(it, backward_end);
            if forward_selection != backward_selection {
                it.haptics += 1;
            }
        });
        it.up();
        it.expect(|it| it.toolbar_shown = true);
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenTouch_withLongPressThenDragLeftAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragLeftAndBack_selectsWordsThenChars() {
    long_press_then_drag_forwards_and_back((22, true), (29, 18), Rtl, (19, true), (29, 18), Rtl);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenTouch_withLongPressThenDragUpAndBack_ltrToRtl_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragUpAndBack_ltrToRtl_selectsWordsThenChars() {
    long_press_then_drag_forwards_and_back((3, false), (29, 0), Ltr, (8, true), (29, 6), Rtl);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenTouch_withLongPressThenDragUpAndBack_rtlToLtr_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragUpAndBack_rtlToLtr_selectsWordsThenChars() {
    long_press_then_drag_forwards_and_back((8, true), (29, 6), Rtl, (13, false), (29, 12), Ltr);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenTouch_withLongPressThenDragRightAndBack_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragRightAndBack_selectsWordsThenChars() {
    long_press_then_drag_forwards_and_back((31, true), (24, 35), Ltr, (34, true), (24, 35), Ltr);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenTouch_withLongPressThenDragDownAndBack_ltrToRtl_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragDownAndBack_ltrToRtl_selectsWordsThenChars() {
    long_press_then_drag_forwards_and_back((51, false), (24, 53), Ltr, (44, true), (24, 47), Ltr);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenTouch_withLongPressThenDragDownAndBack_rtlToLtr_selectsWordsThenChars
#[test]
fn whenTouch_withLongPressThenDragDownAndBack_rtlToLtr_selectsWordsThenChars() {
    long_press_then_drag_forwards_and_back((44, true), (24, 47), Ltr, (38, false), (24, 41), Ltr);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withSingleClick_collapsedSelectionAtClick, whenTrackpad_withSingleClick_collapsedSelectionAtClick
#[test]
fn whenMouse_withSingleClick_collapsedSelectionAtClick() {
    for (multi, kind) in variants_and_cursors() {
        let mut it = bidi(multi);
        let at = it.character_side(26, false);
        it.cursor(kind, |cursor| cursor.click(at));
        it.expect(|it| it.selection = Some((26, 26)));
    }
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withSingleClickThenRelease_collapsedSelection, whenTrackpad_withSingleClickThenRelease_collapsedSelection
#[test]
fn whenMouse_withSingleClickThenRelease_collapsedSelection() {
    for (multi, kind) in variants_and_cursors() {
        let mut it = bidi(multi);
        let at = it.character_side(26, false);
        it.cursor(kind, |cursor| {
            cursor.move_to(at);
            cursor.press();
        });
        it.expect(|it| it.selection = Some((26, 26)));
        it.cursor(kind, |cursor| cursor.release());
        it.check();
    }
}

/// As the tests' `mouseClicksThenDragTest` and `trackpadClicksThenDragTest`: `clicks` at the
/// middle line's Latin word, then a drag to `to` (`(offset, isRtl)`), with each cursor.
fn clicks_then_drag(
    clicks: u32,
    first: (usize, usize),
    first_start: ResolvedTextDirection,
    to: (usize, bool),
    second: (usize, usize),
    second_directions: (ResolvedTextDirection, ResolvedTextDirection),
) {
    for (multi, kind) in variants_and_cursors() {
        let mut it = bidi(multi);
        let at = it.character_side(26, false);
        it.cursor(kind, |cursor| {
            cursor.move_to(at);
            cursor.press();
            for _ in 1..clicks {
                cursor.advance_event_time(16);
                cursor.release();
                cursor.advance_event_time(16);
                cursor.press();
            }
        });
        it.expect(|it| {
            it.selection = Some(first);
            it.directions = Some((first_start, Ltr));
        });
        let to = it.character_side(to.0, to.1);
        it.cursor_drag_to(kind, to);
        it.expect(|it| {
            it.selection = Some(second);
            it.directions = Some(second_directions);
        });
        it.cursor(kind, |cursor| cursor.release());
        it.check();
    }
}

/// As the tests' `mouseSingleClickThenDragTest`: characters from the click.
fn single_click_then_drag(to: (usize, bool), selection: (usize, usize)) {
    clicks_then_drag(1, (26, 26), Ltr, to, selection, (Ltr, Rtl));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withSingleClickThenDragLeft_selectsCharacters, whenTrackpad_withSingleClickThenDragLeft_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragLeft_selectsCharacters() {
    single_click_then_drag((21, true), (26, 21));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withSingleClickThenDragUp_selectsCharacters, whenTrackpad_withSingleClickThenDragUp_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragUp_selectsCharacters() {
    single_click_then_drag((8, true), (26, 8));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withSingleClickThenDragRight_selectsCharacters, whenTrackpad_withSingleClickThenDragRight_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragRight_selectsCharacters() {
    single_click_then_drag((32, true), (26, 32));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withSingleClickThenDragDown_selectsCharacters, whenTrackpad_withSingleClickThenDragDown_selectsCharacters
#[test]
fn whenMouse_withSingleClickThenDragDown_selectsCharacters() {
    single_click_then_drag((44, true), (26, 44));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withDoubleClick_selectsWord, whenTrackpad_withDoubleClick_selectsWord
#[test]
fn whenMouse_withDoubleClick_selectsWord() {
    for (multi, kind) in variants_and_cursors() {
        let mut it = bidi(multi);
        let at = it.character_side(26, false);
        it.cursor(kind, |cursor| {
            cursor.click(at);
            cursor.click(at);
        });
        it.expect(|it| it.selection = Some((24, 29)));
    }
}

/// As the tests' `mouseDoubleClickThenDragTest`: words from the double click's.
fn double_click_then_drag(
    to: (usize, bool),
    selection: (usize, usize),
    end: ResolvedTextDirection,
) {
    clicks_then_drag(2, (24, 29), Ltr, to, selection, (Ltr, end));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withDoubleClickThenDragLeft_selectsWords, whenTrackpad_withDoubleClickThenDragLeft_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragLeft_selectsWords() {
    double_click_then_drag((21, true), (29, 18), Rtl);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withDoubleClickThenDragUp_selectsWords, whenTrackpad_withDoubleClickThenDragUp_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragUp_selectsWords() {
    double_click_then_drag((8, true), (29, 6), Rtl);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withDoubleClickThenDragRight_selectsWords, whenTrackpad_withDoubleClickThenDragRight_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragRight_selectsWords() {
    double_click_then_drag((32, true), (24, 35), Ltr);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withDoubleClickThenDragDown_selectsWords, whenTrackpad_withDoubleClickThenDragDown_selectsWords
#[test]
fn whenMouse_withDoubleClickThenDragDown_selectsWords() {
    double_click_then_drag((44, true), (24, 47), Ltr);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withTripleClick_selectsParagraph, whenTrackpad_withTripleClick_selectsParagraph
#[test]
fn whenMouse_withTripleClick_selectsParagraph() {
    for (multi, kind) in variants_and_cursors() {
        let mut it = bidi(multi);
        let at = it.character_side(26, false);
        it.cursor(kind, |cursor| {
            cursor.click(at);
            cursor.click(at);
            cursor.click(at);
        });
        it.expect(|it| {
            it.selection = Some((18, 35));
            start_direction(it, Rtl);
        });
    }
}

/// As the tests' `mouseTripleClickThenDragTest`: paragraphs from the triple click's.
fn triple_click_then_drag(
    to: (usize, bool),
    selection: (usize, usize),
    start: ResolvedTextDirection,
) {
    clicks_then_drag(3, (18, 35), Rtl, to, selection, (start, Ltr));
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withTripleClickThenDragLeft_selectsParagraphs, whenTrackpad_withTripleClickThenDragLeft_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragLeft_selectsParagraphs() {
    triple_click_then_drag((21, true), (18, 35), Rtl);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withTripleClickThenDragUp_selectsParagraphs, whenTrackpad_withTripleClickThenDragUp_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragUp_selectsParagraphs() {
    triple_click_then_drag((8, true), (35, 0), Ltr);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withTripleClickThenDragRight_selectsParagraphs, whenTrackpad_withTripleClickThenDragRight_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragRight_selectsParagraphs() {
    triple_click_then_drag((32, true), (18, 35), Rtl);
}

// Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/selection/gestures/TextSelectionGesturesBidiTest.kt, whenMouse_withTripleClickThenDragDown_selectsParagraphs, whenTrackpad_withTripleClickThenDragDown_selectsParagraphs
#[test]
fn whenMouse_withTripleClickThenDragDown_selectsParagraphs() {
    triple_click_then_drag((44, true), (18, 53), Rtl);
}

/// The positions the tests use are where the layout says: a sanity check of the harness's
/// `characterPosition`, so a wrong selection above is the gestures', not the positions'.
#[test]
fn the_bidi_texts_characters_are_where_compose_lays_them_out() {
    for multi in variants() {
        let mut it = bidi(multi);
        // The middle line's paragraph is left to right: its first Hebrew word shows mirrored at
        // its start, the Latin word after it.
        let first = it.character_box(18);
        let last = it.character_box(22);
        assert!(last.left < first.left, "{last:?} left of {first:?}");
        let latin = it.character_box(24);
        assert!(latin.left > first.right, "{latin:?} right of {first:?}");
    }
}

/// Not Compose's test: an end on an empty line says its paragraph's direction, as Compose's
/// `getTextDirectionForOffset` (D2062), where the run there says left to right: a click on the
/// empty last line of a text laid out right to left, over one text (past its last line feed) and
/// over a text for each line (an empty text).
#[test]
fn a_click_on_an_empty_line_says_its_paragraphs_direction() {
    for (multi, kind) in variants_and_cursors() {
        let mut it = gestures_over("abc\n".to_string(), true, multi);
        it.directions = Some((Rtl, Rtl));
        let at = it.bottom_start();
        it.cursor(kind, |cursor| cursor.click(at));
        it.expect(|it| it.selection = Some((4, 4)));
        // Inside the line of letters, the run's.
        let at = it.character_side(1, false);
        it.cursor(kind, |cursor| cursor.click(at));
        it.expect(|it| {
            it.selection = Some((1, 1));
            it.directions = Some((Ltr, Ltr));
        });
        // At the start of that line (no line before it, the next character on it), the run's
        // too: the word selected starts there.
        it.cursor(kind, |cursor| {
            cursor.click(at);
            cursor.click(at);
        });
        it.directions = None;
        it.expect(|it| it.selection = Some((0, 3)));
        let found = it.ui.host().with_tree(|tree| tree.selection_directions());
        assert_eq!(found.map(|(start, _)| start), Some(Ltr), "the word's start");
    }
}
