//! Review of `basic-text-paragraphs` (D2053): `BasicText` laying a text with paragraph styles out
//! as a `MultiParagraph`. Its cache against Compose's `MultiParagraphLayoutCache`, a text changing
//! kind on the same node, the painted copies a paint change makes, and the selection's queries at
//! and past paragraph boundaries.
//!
//! In Compose's test font (each character a square of its size, D1124), 20 px a character.

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::{DrawOp, FixedFontMetrics, FixedTextMeasurer, UiHost};
use compose_ui::{
    AnnotatedString, Color, Constraints, Density, IntSize, IntrinsicSize, LocaleList, Offset,
    ParagraphStyle, Rect, ResolvedTextDirection, TextDirection, TextLayoutResult, UnitExt,
};

const TAG: &str = "text";
const FONT: f32 = 20.0;

fn style() -> TextStyle {
    TextStyle { font_size: FONT.sp(), ..TextStyle::DEFAULT }
}

/// Each of `texts` in a paragraph of its own, in no style of its own.
fn paragraphs(texts: &[&str]) -> AnnotatedString {
    AnnotatedString::build(|text| {
        for part in texts {
            text.with_paragraph_style(ParagraphStyle::DEFAULT, |text| {
                text.append(part);
            });
        }
    })
}

/// A rule in Compose's test font at density 1.
fn rule(content: impl Fn() + 'static) -> ComposeTestRule {
    let measurer = FixedTextMeasurer::with_metrics(FixedFontMetrics::SAMPLE_FONT);
    let mut ui =
        compose_test_with(IntSize::new(800, 800), Density::new(1.0), Box::new(measurer), content);
    ui.wait_for_idle();
    ui
}

/// The platform paragraphs alive in the rule's measurer.
fn live(ui: &mut ComposeTestRule) -> usize {
    ui.host().with_tree(|tree| {
        let measurer = tree.text_measurer_ref();
        let any: &dyn std::any::Any = &*measurer;
        any.downcast_ref::<FixedTextMeasurer>().expect("the fixed measurer").live()
    })
}

/// The results `on_text_layout` heard.
#[derive(Clone, Default)]
struct Heard(Rc<RefCell<Vec<TextLayoutResult>>>);

impl Heard {
    fn callback(&self) -> impl Fn(TextLayoutResult) + 'static {
        let heard = self.0.clone();
        move |result| heard.borrow_mut().push(result)
    }

    fn last(&self) -> TextLayoutResult {
        self.0.borrow().last().cloned().expect("a layout heard")
    }

    fn count(&self) -> usize {
        self.0.borrow().len()
    }
}

/// Asks the text's `Selectable`.
fn ask<R>(
    ui: &mut ComposeTestRule,
    question: impl FnOnce(
        &mut dyn compose_ui::Selectable,
        &compose_ui::platform::SharedTextMeasurer,
    ) -> R,
) -> R {
    let node = ui.on_node_with_tag(TAG).fetch_semantics_snapshot().id;
    let mut question = Some(question);
    let mut answer = None;
    ui.host().with_tree(|tree| {
        tree.with_selectable(node, &mut |selectable, measurer| {
            answer = question.take().map(|question| question(selectable, measurer));
        });
    });
    answer.expect("a selectable text")
}

/// `text` in a selection container, `max_lines` at most.
fn selectable(text: AnnotatedString, max_lines: u32) -> ComposeTestRule {
    rule(move || {
        let text = text.clone();
        SelectionContainer!({
            BasicAnnotatedText!(
                text.clone(),
                modifier = Modifier.test_tag(TAG).width(200.dp()),
                style = style(),
                max_lines = max_lines
            );
        });
    })
}

fn highlights(ui: &mut ComposeTestRule) -> Vec<Rect> {
    ui.host()
        .display_list()
        .ops
        .iter()
        .filter_map(|op| match op {
            DrawOp::DrawRect(rect, _) => Some(*rect),
            _ => None,
        })
        .collect()
}

// ---- The cache against Compose's `MultiParagraphLayoutCache` ---------------------------------

/// Compose's `layoutWithConstraints`: new constraints that keep the layout still make a new
/// `TextLayoutResult` (its input's constraints regenerated) and return true, so
/// `TextAnnotatedStringNode.measure` calls `onTextLayout` with it. Here a new maximum height that
/// doesn't clip the text keeps the size, and nothing is heard: the last result heard keeps the old
/// constraints (the plain path does the same).
#[test]
#[ignore = "review: a kept layout at new constraints isn't heard, where Compose's \
            layoutWithConstraints returns true and calls onTextLayout with the new input"]
fn a_kept_layout_at_new_constraints_is_heard_with_them() {
    let first = Constraints::new(0, 200, 0, 100);
    let second = Constraints::new(0, 200, 0, 150);
    let constraints = mutable_state_of(first);
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let handle = constraints.handle();
    let mut ui = rule(move || {
        let callback = callback.clone();
        BasicAnnotatedText!(
            paragraphs(&["Hi"]),
            modifier = Modifier.layout(move |scope, measurable, _| {
                let placeable = scope.measure(measurable, handle.get());
                scope.place(&placeable, 0, 0);
                scope.layout(placeable.width, placeable.height)
            }),
            style = style(),
            on_text_layout = Some(move |result| callback(result))
        );
    });
    assert_eq!(heard.count(), 1);
    let before = heard.last();
    constraints.set(second);
    ui.wait_for_idle();
    // The same layout, kept.
    assert!(Rc::ptr_eq(&before.multi_paragraph, &heard.last().multi_paragraph));
    assert_eq!(heard.count(), 2, "heard again, as Compose's");
    assert_eq!(heard.last().layout_input.constraints, second);
}

/// A text of paragraphs in an async font is laid out again once the font arrives, as Compose's
/// node reads `hasStaleResolvedFonts` in measure.
#[test]
fn a_text_of_paragraphs_is_laid_out_again_when_its_font_arrives() {
    let arriving: Rc<RefCell<Option<compose_ui::Typeface>>> = Rc::default();
    let font = compose_ui::Font::from_loader(
        Rc::new(ArrivingFont { typeface: arriving.clone() }),
        compose_ui::FontOptions {
            loading_strategy: Some(compose_ui::FontLoadingStrategy::Async),
            ..Default::default()
        },
    );
    let family = font.to_font_family();
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let mut host =
        UiHost::new(IntSize::new(400, 400), Density::new(1.0), Box::new(FixedTextMeasurer::new()));
    let text_style = TextStyle { font_family: Some(family), ..TextStyle::DEFAULT };
    host.set_content(move || {
        let callback = callback.clone();
        BasicAnnotatedText!(
            paragraphs(&["Hello", "World"]),
            style = text_style.clone(),
            on_text_layout = Some(move |result| callback(result))
        );
    });
    host.frame();
    assert_eq!(heard.count(), 1);
    std::thread::sleep(std::time::Duration::from_millis(20));
    host.frame();
    assert_eq!(heard.count(), 1, "nothing arrived: nothing laid out");
    *arriving.borrow_mut() = Some(compose_ui::Typeface::new("arrived"));
    std::thread::sleep(std::time::Duration::from_millis(20));
    host.frame();
    host.frame();
    assert_eq!(heard.count(), 2, "laid out again in the font that arrived");
    let result = heard.last();
    assert!(!result.multi_paragraph.intrinsics().unwrap().has_stale_resolved_fonts());
    host.frame();
    assert_eq!(heard.count(), 2);
}

/// A font that arrives when `typeface` is given one.
struct ArrivingFont {
    typeface: Rc<RefCell<Option<compose_ui::Typeface>>>,
}

impl compose_ui::TypefaceLoader for ArrivingFont {
    fn load_blocking(
        &self,
        _: &dyn compose_ui::platform::PlatformFontLoader,
        _: &compose_ui::Font,
    ) -> Result<Option<compose_ui::Typeface>, String> {
        Err("it's async".to_string())
    }

    fn await_load(
        self: Rc<Self>,
        _: Rc<dyn compose_ui::platform::PlatformFontLoader>,
        _: compose_ui::Font,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Option<compose_ui::Typeface>, String>>>,
    > {
        Box::pin(async move {
            loop {
                if let Some(typeface) = self.typeface.borrow_mut().take() {
                    return Ok(Some(typeface));
                }
                compose_runtime::delay(std::time::Duration::from_millis(10)).await;
            }
        })
    }
}

/// A new default locale list lays the paragraphs out again: one whose direction is `Content`
/// and has no strong character goes by the locale's direction (D2049), so the kept layout in the
/// old locale is stale.
#[test]
fn a_new_locale_list_lays_the_paragraphs_out_again() {
    let locales = mutable_state_of(LocaleList::from_language_tags("en"));
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let handle = locales.handle();
    let content =
        ParagraphStyle { text_direction: Some(TextDirection::Content), ..Default::default() };
    let text = AnnotatedString::build(|text| {
        text.append("Hello");
        text.with_paragraph_style(content, |text| {
            text.append("!!");
        });
    });
    let mut ui = rule(move || {
        let (callback, text) = (callback.clone(), text.clone());
        CompositionLocalProvider(compose_ui::LOCAL_LOCALE_LIST.provides(handle.get()), move || {
            let callback = callback.clone();
            BasicAnnotatedText!(
                text.clone(),
                style = style(),
                on_text_layout = Some(move |result| callback(result))
            );
        });
    });
    assert_eq!(heard.last().get_paragraph_direction(6), ResolvedTextDirection::Ltr);
    locales.set(LocaleList::from_language_tags("ar"));
    ui.wait_for_idle();
    assert_eq!(heard.last().get_paragraph_direction(6), ResolvedTextDirection::Rtl);
}

// ---- A text changing kind on the same node -----------------------------------------------------

/// Plain, paragraphs, plain, paragraphs: each kind keeps as many platform paragraphs alive as it
/// did the first time (the other kind's released), and none stay once the text leaves.
#[test]
fn changing_kind_releases_the_other_kinds_paragraphs() {
    let text = mutable_state_of(AnnotatedString::new("Hello World"));
    let shown = mutable_state_of(true);
    let (t, s) = (text.handle(), shown.handle());
    let mut ui = rule(move || {
        if s.get() {
            SelectionContainer!({
                BasicAnnotatedText!(
                    t.get(),
                    modifier = Modifier.test_tag(TAG).width(IntrinsicSize::Max),
                    style = style()
                );
            });
        }
    });
    let plain = AnnotatedString::new("Hello World");
    let styled = paragraphs(&["Hello", "World"]);
    let mut counts = Vec::new();
    for round in 0..3 {
        for kind in [&styled, &plain] {
            // Another text each round, so the update lays it out anew.
            let mut next = kind.clone();
            if round % 2 == 1 {
                next = AnnotatedString::build(|builder| {
                    builder.append_annotated(kind);
                    builder.append("");
                });
            }
            text.set(next);
            ui.wait_for_idle();
            // A frame more: what the last one dropped is given back.
            ui.wait_for_idle();
            counts.push(live(&mut ui));
        }
    }
    assert_eq!(counts[0], counts[2], "{counts:?}");
    assert_eq!(counts[2], counts[4], "{counts:?}");
    assert_eq!(counts[1], counts[3], "{counts:?}");
    assert_eq!(counts[3], counts[5], "{counts:?}");
    shown.set(false);
    ui.wait_for_idle();
    ui.wait_for_idle();
    assert_eq!(live(&mut ui), 0);
}

// ---- Painted copies on paint-only changes ------------------------------------------------------

/// A color animated through many values keeps at most a few painted copies of each paragraph,
/// and a new text lets them all go.
#[test]
fn painted_copies_are_bounded_and_go_with_the_layout() {
    let color = mutable_state_of(Color::BLACK);
    let text = mutable_state_of(paragraphs(&["Hello", "World"]));
    let (c, t) = (color.handle(), text.handle());
    let mut ui = rule(move || {
        BasicAnnotatedText!(t.get(), style = TextStyle { color: c.get(), ..style() });
    });
    ui.wait_for_idle();
    let base = live(&mut ui);
    let mut most = base;
    for step in 1..=12u32 {
        color.set(Color::from_rgb((step * 20) as u8, 0, 0));
        ui.wait_for_idle();
        most = most.max(live(&mut ui));
    }
    // Two paragraphs, four copies each kept (`REPAINTED_COPIES`), and what the last frame holds.
    assert!(most <= base + 2 * 5, "{base} then {most}");
    color.set(Color::BLACK);
    text.set(paragraphs(&["Hello", "Again"]));
    ui.wait_for_idle();
    ui.wait_for_idle();
    assert_eq!(live(&mut ui), base, "the copies went with the old layout");
}

/// After a paint change, the paragraphs are drawn from their painted copies frame after frame
/// (the layout made in the old paint kept): both stay alive for as long as the text shows, where
/// a plain text's restyle replaces its one paragraph.
#[test]
fn a_paint_change_keeps_both_the_layout_and_its_copies_alive() {
    let color = mutable_state_of(Color::BLACK);
    let c = color.handle();
    let mut ui = rule(move || {
        BasicAnnotatedText!(
            paragraphs(&["Hello", "World"]),
            style = TextStyle { color: c.get(), ..style() }
        );
    });
    ui.wait_for_idle();
    let before = live(&mut ui);
    color.set(Color::RED);
    ui.wait_for_idle();
    ui.wait_for_idle();
    let after = live(&mut ui);
    // Each paragraph and its red copy.
    assert_eq!(after, before + 2, "{before} then {after}");
}

// ---- Selection at and past paragraph boundaries ------------------------------------------------

#[test]
fn a_handle_at_a_paragraphs_end_is_on_the_next_paragraphs_line_as_composes() {
    // Compose's `getSelectionHandleCoordinates`: the line of the offset (the paragraph starting
    // there), x its horizontal position there, y that line's bottom.
    let mut ui = selectable(paragraphs(&["Hello", "World"]), u32::MAX);
    let end = ask(&mut ui, |text, measurer| text.handle_position(5, false, false, measurer));
    assert_eq!(end, Some(Offset::new(0.0, 40.0)));
    let start = ask(&mut ui, |text, measurer| text.handle_position(5, true, false, measurer));
    assert_eq!(start, Some(Offset::new(0.0, 40.0)));
    let last = ask(&mut ui, |text, measurer| text.handle_position(10, false, false, measurer));
    assert_eq!(last, Some(Offset::new(100.0, 40.0)));
}

#[test]
fn a_word_at_a_paragraph_boundary_is_the_next_paragraphs() {
    let mut ui = selectable(paragraphs(&["Hello", "World"]), u32::MAX);
    assert_eq!(ask(&mut ui, |text, _| text.word_at(5)), 5..10);
    assert_eq!(ask(&mut ui, |text, _| text.word_at(4)), 0..5);
}

/// A selection over a left-to-right paragraph and a right-to-left one: each highlighted in its
/// own direction, the second's at the right.
#[test]
fn a_selection_over_paragraphs_of_both_directions_is_highlighted_in_each() {
    let rtl = ParagraphStyle { text_direction: Some(TextDirection::Rtl), ..Default::default() };
    let text = AnnotatedString::build(|text| {
        text.append("Hello");
        text.with_paragraph_style(rtl, |text| {
            text.append("!!");
        });
    });
    let mut ui = selectable(text, u32::MAX);
    // A drag from after "Hel" to after the first "!" (its trailing edge, at 180).
    ui.host().send_pointer(compose_ui::PointerEvent::press(Offset::new(61.0, 10.0)));
    ui.host().send_pointer(compose_ui::PointerEvent::moved(Offset::new(181.0, 30.0), true));
    ui.host().send_pointer(compose_ui::PointerEvent::release(Offset::new(181.0, 30.0)));
    ui.wait_for_idle();
    let rects = highlights(&mut ui);
    // "lo", then the first "!", at the right end of the 200 px wide right-to-left paragraph.
    assert_eq!(rects, vec![Rect::new(60.0, 0.0, 100.0, 20.0), Rect::new(180.0, 20.0, 200.0, 40.0)]);
}

/// "Hello", "World", "Again" at most two lines: "Again" isn't laid out. A selectable's queries at
/// an offset in it answer as at the end of what's shown, as the plain path's one paragraph does
/// (Compose's `getBoundingBox` does; its other queries require the offset be laid out).
fn cut() -> ComposeTestRule {
    selectable(paragraphs(&["Hello", "World", "Again"]), 2)
}

#[test]
#[ignore = "review: word_at panics at an offset in a paragraph the maximum lines cut"]
fn word_at_in_a_cut_paragraph_does_not_panic() {
    let mut ui = cut();
    let _ = ask(&mut ui, |text, _| text.word_at(12));
}

#[test]
fn bounding_box_in_a_cut_paragraph_is_the_end_of_the_last_shown() {
    let mut ui = cut();
    let rect = ask(&mut ui, |text, measurer| text.bounding_box(12, measurer));
    // Compose's: the last paragraph's cursor rect at its end.
    assert_eq!((rect.left, rect.top, rect.bottom), (100.0, 20.0, 40.0));
}

#[test]
#[ignore = "review: cursor_rect panics at an offset in a paragraph the maximum lines cut"]
fn cursor_rect_in_a_cut_paragraph_does_not_panic() {
    let mut ui = cut();
    let _ = ask(&mut ui, |text, measurer| text.cursor_rect(12, measurer));
}

#[test]
#[ignore = "review: direction_at panics at an offset in a paragraph the maximum lines cut"]
fn direction_at_in_a_cut_paragraph_does_not_panic() {
    let mut ui = cut();
    let _ = ask(&mut ui, |text, measurer| text.direction_at(12, measurer));
}

#[test]
#[ignore = "review: line_height panics at an offset in a paragraph the maximum lines cut"]
fn line_height_in_a_cut_paragraph_does_not_panic() {
    let mut ui = cut();
    let _ = ask(&mut ui, |text, measurer| text.line_height(12, measurer));
}

#[test]
fn handles_and_lines_in_a_cut_paragraph_are_kept_to_what_shows() {
    let mut ui = cut();
    let handle = ask(&mut ui, |text, measurer| text.handle_position(12, false, false, measurer));
    assert_eq!(handle, Some(Offset::new(100.0, 40.0)));
    let line = ask(&mut ui, |text, measurer| text.range_of_line_containing(12, measurer));
    assert_eq!(line, 5..10);
    assert_eq!(ask(&mut ui, |text, measurer| text.last_visible_offset(measurer)), 10);
}

/// What a user can do to a cut text in a container: select a word in it, then select all; the
/// handles, highlight and toolbar follow without a panic.
#[test]
fn selecting_all_of_a_cut_text_of_paragraphs_does_not_panic() {
    let mut ui = cut();
    ui.host().send_pointer(compose_ui::PointerEvent::press(Offset::new(21.0, 10.0)));
    ui.host().send_pointer(compose_ui::PointerEvent::moved(Offset::new(39.0, 30.0), true));
    ui.host().send_pointer(compose_ui::PointerEvent::release(Offset::new(39.0, 30.0)));
    ui.wait_for_idle();
    let modifiers = if cfg!(target_os = "macos") {
        compose_ui::KeyModifiers { meta: true, ..Default::default() }
    } else {
        compose_ui::KeyModifiers { ctrl: true, ..Default::default() }
    };
    for kind in [compose_ui::KeyEventType::KeyDown, compose_ui::KeyEventType::KeyUp] {
        ui.host().send_key(compose_ui::KeyEvent {
            key: compose_ui::Key::Character('a'),
            kind,
            modifiers,
            from_soft_keyboard: false,
        });
    }
    ui.wait_for_idle();
    // Everything selected; only what's shown highlighted.
    let rects = highlights(&mut ui);
    assert_eq!(rects.len(), 2, "{rects:?}");
}

/// A selected word in a text of paragraphs that then shows fewer lines (a "show less" text in a
/// container): the selection is kept (the text didn't change), its handles are asked for where
/// its ends are, past what's laid out now. A plain text answers; a text of paragraphs panics in
/// `direction_at` (the handle's direction), where the plain path never did.
#[test]
#[ignore = "review: a touch selection past the lines a text of paragraphs now shows panics \
            (direction_at for the handles, at an offset the maximum lines cut)"]
fn a_touch_selection_in_lines_that_collapse_does_not_panic() {
    let max_lines = mutable_state_of(u32::MAX);
    let handle = max_lines.handle();
    let mut ui = rule(move || {
        SelectionContainer!({
            BasicAnnotatedText!(
                paragraphs(&["Hello", "World", "Again and again"]),
                modifier = Modifier.test_tag(TAG).width(400.dp()),
                style = style(),
                max_lines = handle.get()
            );
        });
    });
    // "and", on the third line.
    ui.on_node_with_tag(TAG)
        .perform_touch_input(|touch| touch.long_click_at(Offset::new(150.0, 50.0), 600));
    ui.wait_for_idle();
    let anchors = ui.host().with_tree(|tree| tree.selection_anchors());
    let ((_, start), (_, end)) = anchors.expect("a word selected");
    assert_eq!((start, end), (16, 19));
    max_lines.set(2);
    ui.wait_for_idle();
    ui.wait_for_idle();
}

/// The plain path's control for the test above: the same lines in one platform paragraph.
#[test]
fn a_touch_selection_in_plain_lines_that_collapse_is_answered() {
    let max_lines = mutable_state_of(u32::MAX);
    let handle = max_lines.handle();
    let mut ui = rule(move || {
        SelectionContainer!({
            BasicText!(
                "Hello\nWorld\nAgain and again",
                modifier = Modifier.test_tag(TAG).width(400.dp()),
                style = style(),
                max_lines = handle.get()
            );
        });
    });
    ui.on_node_with_tag(TAG)
        .perform_touch_input(|touch| touch.long_click_at(Offset::new(150.0, 50.0), 600));
    ui.wait_for_idle();
    let anchors = ui.host().with_tree(|tree| tree.selection_anchors());
    let ((_, start), (_, end)) = anchors.expect("a word selected");
    assert_eq!((start, end), (18, 21));
    max_lines.set(2);
    ui.wait_for_idle();
    ui.wait_for_idle();
}

// ---- Hand mutants the branch's tests let live ---------------------------------------------------

/// Compose's `newLayoutWillBeDifferent`: a layout that exceeded its lines is laid out again at a
/// taller height, even though it fits it: ellipsized at two lines by a short height, it shows all
/// its lines once the height's unbounded.
#[test]
fn a_layout_cut_by_its_height_is_laid_out_again_when_the_height_grows() {
    const LONG: &str = "Hello World! Hello World! Hello World! Hello World!";
    let width = LONG.len() as i32 * 20 / 4;
    let first = Constraints::new(0, width, 0, 54);
    let second = Constraints::new(0, width, 0, Constraints::INFINITY);
    let constraints = mutable_state_of(first);
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let handle = constraints.handle();
    let mut ui = rule(move || {
        let callback = callback.clone();
        BasicAnnotatedText!(
            AnnotatedString::with_paragraph_style(LONG.to_string(), ParagraphStyle::DEFAULT),
            modifier = Modifier.layout(move |scope, measurable, _| {
                let placeable = scope.measure(measurable, handle.get());
                scope.place(&placeable, 0, 0);
                scope.layout(placeable.width, placeable.height)
            }),
            style = style(),
            on_text_layout = Some(move |result| callback(result)),
            overflow = compose_ui::TextOverflow::Ellipsis
        );
    });
    assert_eq!(heard.last().line_count, 2);
    constraints.set(second);
    ui.wait_for_idle();
    let result = heard.last();
    assert!(result.line_count > 2, "{}", result.line_count);
    assert!(!result.is_line_ellipsized(1));
}

/// The intrinsic height kept for a width is forgotten with the paragraphs it was measured from:
/// at a new density, the same width in pixels has another height.
#[test]
fn the_kept_intrinsic_height_goes_with_the_density() {
    let asked = Rc::new(std::cell::Cell::new(0));
    let a = asked.clone();
    let mut ui = rule(move || {
        let a = a.clone();
        BasicAnnotatedText!(
            paragraphs(&["Hi"]),
            modifier = Modifier.layout(move |scope, measurable, constraints| {
                a.set(scope.min_intrinsic_height(measurable, 100));
                let placeable = scope.measure(measurable, constraints);
                scope.place(&placeable, 0, 0);
                scope.layout(placeable.width, placeable.height)
            }),
            style = style()
        );
    });
    assert_eq!(asked.get(), 20);
    ui.host().set_density(Density::new(2.0));
    ui.wait_for_idle();
    // "Hi" 80 px wide at 40 px a character: one line, 40 high.
    assert_eq!(asked.get(), 40);
}

/// A selection's highlight follows a new layout: a centered paragraph moves with the width.
#[test]
fn the_highlight_follows_a_new_layout() {
    let width = mutable_state_of(200.0f32);
    let handle = width.handle();
    let centered =
        ParagraphStyle { text_align: Some(compose_ui::TextAlign::Center), ..Default::default() };
    let text = AnnotatedString::build(|text| {
        text.append("Hello");
        text.with_paragraph_style(centered, |text| {
            text.append("World");
        });
    });
    let mut ui = rule(move || {
        let text = text.clone();
        SelectionContainer!({
            BasicAnnotatedText!(
                text.clone(),
                modifier = Modifier.test_tag(TAG).width(handle.get().dp()),
                style = style()
            );
        });
    });
    // From after "Hel" to after "Wor" (centered at 50 in 200 px: "Wor" ends at 110).
    ui.host().send_pointer(compose_ui::PointerEvent::press(Offset::new(61.0, 10.0)));
    ui.host().send_pointer(compose_ui::PointerEvent::moved(Offset::new(109.0, 30.0), true));
    ui.host().send_pointer(compose_ui::PointerEvent::release(Offset::new(109.0, 30.0)));
    ui.wait_for_idle();
    assert_eq!(highlights(&mut ui)[1], Rect::new(50.0, 20.0, 110.0, 40.0));
    width.set(300.0);
    ui.wait_for_idle();
    assert_eq!(highlights(&mut ui)[1], Rect::new(100.0, 20.0, 160.0, 40.0));
}

/// Compose's `TextLayoutResult_sameWidth_inRtlAndLtr_withLetterSpacing` lays the same cache out in
/// one direction, then the other; the branch's port uses two rules. On one node: a new layout
/// direction lays the paragraphs out again, as wide.
#[test]
fn a_new_layout_direction_lays_the_same_node_out_again_as_wide() {
    let direction = mutable_state_of(compose_ui::LayoutDirection::Ltr);
    let heard = Heard::default();
    let callback = Rc::new(heard.callback());
    let handle = direction.handle();
    let mut ui = rule(move || {
        let callback = callback.clone();
        CompositionLocalProvider(
            compose_ui::LOCAL_LAYOUT_DIRECTION.provides(handle.get()),
            move || {
                let callback = callback.clone();
                BasicAnnotatedText!(
                    AnnotatedString::with_paragraph_style(
                        "Hello World".to_string(),
                        ParagraphStyle::DEFAULT
                    ),
                    style = TextStyle { letter_spacing: 0.5.sp(), ..style() },
                    on_text_layout = Some(move |result| callback(result)),
                    overflow = compose_ui::TextOverflow::Ellipsis
                );
            },
        );
    });
    let ltr = heard.last();
    direction.set(compose_ui::LayoutDirection::Rtl);
    ui.wait_for_idle();
    let rtl = heard.last();
    assert_eq!(heard.count(), 2);
    assert_eq!(rtl.layout_input.layout_direction, compose_ui::LayoutDirection::Rtl);
    assert_eq!(rtl.get_paragraph_direction(0), ResolvedTextDirection::Rtl);
    assert_eq!(ltr.size.width, rtl.size.width);
}
