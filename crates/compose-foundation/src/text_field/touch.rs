//! A field's touch selection, as Compose's `TextFieldSelectionState`'s: whether the user is
//! touching it (`isDirectTouchInteraction`), the cursor handle shown after a tap
//! (`showCursorHandle`), which handles show and where (`getCursorHandleState`,
//! `getSelectionHandleState`), and the handles' drags (D2008).
//!
//! Both fields' node keeps it, so both fields have it. A field no finger touches, or focused in
//! keyboard input mode, asks nothing of it but a branch at each measurement.

use std::rc::Rc;

use compose_ui::platform::{ParagraphHandle, TextMeasurer};
use compose_ui::{EventContext, Offset, PointerEvent, PointerType, Rect, ResolvedTextDirection};

use super::TextFieldNode;
use crate::selection_handles::{
    Handle, HandlePositions, ShownHandles, TouchHandles, previous_char,
    selection_handle_coordinates,
};

/// What a field's node keeps for touch selection.
pub(crate) struct Touch {
    /// Shared with its handles' popups; `None` for a node without a composable around (never, as
    /// both fields give one).
    handles: Option<Rc<TouchHandles>>,
    /// The user's last interaction with the field was a finger's (or a stylus'), as Compose's
    /// `isDirectTouchInteraction`: true at first, false once a mouse or a hardware key is used,
    /// or the field is focused in keyboard input mode.
    direct: bool,
    /// The cursor handle shows, as Compose's `showCursorHandle`, set by a tap: the shown text it
    /// was set over, an edit of which hides it.
    cursor_handle: Option<String>,
    /// What the handles were last told.
    shown: ShownHandles,
    positions: HandlePositions,
    /// Whether the handles were told they may show (`TouchHandles::active`).
    active: bool,
    /// The handle drag the field follows: the handle, where it pointed in the text at the drag's
    /// start (Compose's `getAdjustedCoordinates`, a pixel up), and the field's scroll then.
    drag: Option<(Handle, Offset, Offset)>,
    /// The offset under a selection handle's drag when the selection was last adjusted, as
    /// Compose's `previousRawDragOffset`: none at the drag's start.
    raw: Option<usize>,
    /// The timer of a finger's press becoming a long press.
    long_press_job: Option<compose_runtime::Job>,
    /// What the field resolves at its next measurement, with its paragraph: the long press, or
    /// where the finger dragged after it (in the text's coordinates).
    pending: Option<Pending>,
    /// A long press's drag under way, as Compose's `TextFieldTextDragObserver` (its
    /// `directDragGestureInitiator` is `Touch` meanwhile: no handle shows).
    long: Option<LongDrag>,
}

#[derive(Clone, Copy)]
enum Pending {
    LongPress(Offset),
    DragTo(Offset),
}

/// A long press's drag: where it went down in the text, and the start of the word it selected
/// (`dragBeginOffsetInText`), none when it went down past the text (`isPositionOnText`).
#[derive(Clone, Copy)]
struct LongDrag {
    at: Offset,
    begin: Option<usize>,
}

impl Touch {
    pub(super) fn new(handles: Option<Rc<TouchHandles>>) -> Touch {
        Touch {
            handles,
            direct: true,
            cursor_handle: None,
            shown: ShownHandles::default(),
            positions: HandlePositions::default(),
            active: false,
            drag: None,
            raw: None,
            long_press_job: None,
            pending: None,
            long: None,
        }
    }

    /// A new element's handles: the composable's, the same while it's composed.
    pub(super) fn update(&mut self, handles: &Option<Rc<TouchHandles>>) {
        let same = match (&self.handles, handles) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same {
            self.handles = handles.clone();
            self.shown = ShownHandles::default();
            self.positions = HandlePositions::default();
            self.active = false;
            self.drag = None;
        }
    }

    /// Reused for another field, or detached: no handle shows, here or in the popups it told
    /// (the composable's `TouchHandles` outlives a node its decoration made anew).
    pub(super) fn reset(&mut self, field: Option<compose_ui::NodeHandle>) {
        if let Some(handles) = &self.handles {
            handles.forget(field);
        }
        self.shown = ShownHandles::default();
        self.active = false;
        self.direct = true;
        self.cursor_handle = None;
        self.drag = None;
        self.pending = None;
        self.long = None;
        if let Some(job) = self.long_press_job.take() {
            job.cancel();
        }
    }
}

impl TextFieldNode {
    /// What the shown text is: the transformed one when it's transformed.
    fn shown_str(&self) -> &str {
        self.shown_text.as_deref().unwrap_or(&self.text)
    }

    /// A pointer's event on the field: a mouse's (or a touchpad's) ends the touch interaction, a
    /// finger's starts it, as Compose's `detectDirectTouchInteraction`.
    pub(super) fn touch_pointer_event(&mut self, event: &PointerEvent) {
        self.touch.direct = event.pointer_type != PointerType::Mouse;
    }

    /// A tap placed the cursor: its handle shows over a text that isn't empty, in an editable
    /// field, as Compose's `onTapTextField` (`showCursorHandle = true`).
    pub(super) fn touch_tapped(&mut self) {
        if self.touch.direct && !self.read_only && self.touch.handles.is_some() {
            let text = self.shown_str();
            self.touch.cursor_handle = (!text.is_empty()).then(|| text.to_owned());
        }
    }

    /// The field gained or lost the focus: in keyboard input mode the touch interaction ends, as
    /// Compose's `applyCurrentInputMode`; unfocused, the cursor handle goes, as when Compose's
    /// visibility observer ends.
    pub(super) fn touch_focus_changed(&mut self, focused: bool, context: &mut EventContext<'_>) {
        let Some(handles) = &self.touch.handles else { return };
        if focused {
            handles.field.set(Some(context.handle()));
            if self.touch.direct && context.input_mode() != compose_ui::InputMode::Touch {
                self.touch.direct = false;
            }
        } else {
            self.touch.cursor_handle = None;
            self.touch.drag = None;
        }
    }

    /// A hardware key ends the touch interaction: the handles hide until a finger comes back.
    pub(super) fn touch_hardware_key(&mut self, context: &mut EventContext<'_>) {
        self.touch.direct = false;
        if self.touch.shown.any() {
            context.invalidate_measure();
        }
    }

    /// A finger went down: in the long-press timeout, unless it lifts or moves past the slop
    /// first, the press becomes a long press (`detectDragGesturesAfterLongPress`).
    pub(super) fn start_long_press(&mut self, context: &mut EventContext<'_>) {
        self.cancel_long_press();
        let handle = context.handle();
        let Some(scope) = handle.coroutine_scope() else { return };
        let timeout = context.view_configuration().long_press_timeout_millis;
        self.touch.long_press_job = Some(scope.launch(async move {
            compose_runtime::delay(std::time::Duration::from_millis(timeout)).await;
            handle.with_node::<TextFieldNode, _>(|node, context| {
                node.touch.long_press_job = None;
                node.long_pressed(context);
            });
        }));
    }

    pub(super) fn cancel_long_press(&mut self) {
        if let Some(job) = self.touch.long_press_job.take() {
            job.cancel();
        }
    }

    /// The long-press timeout passed with the finger still down and still: the field resolves
    /// the long press at its next measurement.
    fn long_pressed(&mut self, context: &mut compose_ui::NodeContext<'_>) {
        let Some(press) = self.press.as_mut().filter(|press| press.long_possible && self.enabled)
        else {
            return;
        };
        press.long = true;
        self.touch.pending = Some(Pending::LongPress(press.at));
        context.invalidate_measurement();
    }

    /// The finger moved to `at` (in the text) after a long press.
    pub(super) fn long_press_dragged(&mut self, at: Offset) {
        self.touch.pending = Some(Pending::DragTo(at));
    }

    /// The long press's finger lifted, or its input was cancelled: the handles may show again.
    pub(super) fn long_press_lifted(&mut self) {
        self.touch.long = None;
        self.touch.pending = None;
    }

    /// Resolves a long press or its drag, at a measurement, as Compose's
    /// `TextFieldTextDragObserver`: a long press on the text selects the word under it (from a
    /// collapsed selection, by `SelectionAdjustment.Word`); past the text, it puts the cursor
    /// there and shows its handle. Then the drag selects from that word to the word under the
    /// finger, never reversed, and never collapsing once it isn't; begun past the text, it
    /// starts once the finger reaches another offset.
    pub(super) fn follow_long_press(
        &mut self,
        measurer: &mut dyn TextMeasurer,
        paragraph: ParagraphHandle,
    ) {
        let Some(pending) = self.touch.pending.take() else { return };
        let layout = measurer.paragraph(paragraph);
        let on_text = |at: Offset| {
            let line = layout.get_line_for_vertical_position(at.y);
            at.x >= layout.get_line_left(line) && at.x <= layout.get_line_right(line)
        };
        match pending {
            Pending::LongPress(at) => {
                self.touch.cursor_handle = None;
                if !on_text(at) {
                    let offset = self.tap_offset_in(measurer, paragraph, at).0;
                    self.cursor = offset;
                    self.anchor = offset;
                    let text = self.shown_str();
                    self.touch.cursor_handle = (!text.is_empty()).then(|| text.to_owned());
                    self.touch.long = Some(LongDrag { at, begin: None });
                } else {
                    self.touch.long = Some(LongDrag { at, begin: None });
                    if self.text.is_empty() {
                        return;
                    }
                    let offset = self.tap_offset_in(measurer, paragraph, at).0;
                    let word = self.unit_at(super::Unit::Word, offset);
                    self.anchor = word.start;
                    self.cursor = word.end;
                    self.touch.long = Some(LongDrag { at, begin: Some(word.start) });
                }
            }
            Pending::DragTo(at) => {
                let Some(mut long) = self.touch.long else { return };
                if self.text.is_empty() {
                    return;
                }
                let end = self.tap_offset_in(measurer, paragraph, at).0;
                let (anchor, cursor) = match long.begin {
                    // Begun past the text and still past it: a cursor, until it leaves the
                    // offset it began at.
                    None if !on_text(at) => {
                        let start = self.tap_offset_in(measurer, paragraph, long.at).0;
                        if start == end { (end, end) } else { self.word_selection(start, end) }
                    }
                    begin => {
                        let start = match begin {
                            Some(begin) => begin,
                            None => self.tap_offset_in(measurer, paragraph, long.at).0,
                        };
                        if begin.is_none() && start == end {
                            return;
                        }
                        self.word_selection(start, end)
                    }
                };
                // Begun past the text, the drag goes on from where its first selection started,
                // before it's put in order (Compose's `newSelection.start`): from the end of the
                // last word when it went back.
                let begun = anchor;
                // Never reversed, as Compose's.
                let (anchor, cursor) = (anchor.min(cursor), anchor.max(cursor));
                if self.anchor == self.cursor || anchor != cursor {
                    self.anchor = anchor;
                    self.cursor = cursor;
                }
                if long.begin.is_none() && anchor != cursor {
                    long.begin = Some(begun);
                    self.touch.long = Some(long);
                }
            }
        }
        self.preferred_x = None;
    }

    /// The selection from the word at `start` to the word at `end`, as
    /// `SelectionAdjustment.Word`: each end at the far side of its word.
    fn word_selection(&self, start: usize, end: usize) -> (usize, usize) {
        let first = self.unit_at(super::Unit::Word, start);
        let last = self.unit_at(super::Unit::Word, end);
        if end >= start { (first.start, last.end.max(first.end)) } else { (first.end, last.start) }
    }

    /// Follows a handle's drag its node told, at a measurement with the paragraph laid out, as
    /// Compose's `detectCursorHandleDragGestures` and `detectSelectionHandleDragGestures`: the
    /// finger's place in the text is where the handle pointed at the drag's start, moved by the
    /// finger, and by how far the text scrolled since (`handleDragPosition`).
    pub(super) fn follow_handle_drag(
        &mut self,
        measurer: &mut dyn TextMeasurer,
        paragraph: ParagraphHandle,
    ) {
        let Some(handles) = &self.touch.handles else { return };
        if handles.moved.borrow().is_empty() && !handles.ended.get() {
            return;
        }
        let handles = handles.clone();
        let moves = std::mem::take(&mut *handles.moved.borrow_mut());
        for (handle, delta) in moves {
            let (handle, start, scroll) = *self.touch.drag.get_or_insert_with(|| {
                let pointed = self.touch.positions.text[handle.index()];
                (handle, Offset::new(pointed.x, pointed.y - 1.0), self.scroll)
            });
            let target = start + delta + (self.scroll - scroll);
            let offset = self.tap_offset_in(measurer, paragraph, target).0;
            match handle {
                Handle::Cursor => {
                    self.cursor = offset;
                    self.anchor = offset;
                }
                _ => {
                    let start_handle = handle == Handle::SelectionStart;
                    let layout = measurer.paragraph(paragraph);
                    let (anchor, cursor) = self.accelerated(start_handle, offset, &*layout);
                    self.touch.raw = Some(offset);
                    // A selection never collapses while its handles are dragged, as Compose's.
                    if anchor != cursor {
                        self.anchor = anchor;
                        self.cursor = cursor;
                    }
                }
            }
            self.preferred_x = None;
        }
        if handles.ended.replace(false) {
            self.touch.drag = None;
            self.touch.raw = None;
        }
    }

    /// The selection a selection handle's drag to `raw` makes, as Compose's
    /// `SelectionAdjustment.CharacterWithWordAccelerate` over one text: growing from an end at a
    /// word's boundary, by words (snapped to the far side of the word under the finger, but never
    /// onto the other end); shrinking, or within a word, by characters; to another line, by
    /// words again.
    fn accelerated(
        &self,
        start_handle: bool,
        raw: usize,
        layout: &dyn compose_ui::Paragraph,
    ) -> (usize, usize) {
        let line = |offset: usize| layout.get_line_for_offset(self.shown(offset));
        let (start, end) = (self.anchor, self.cursor);
        let (previous, other) = if start_handle { (start, end) } else { (end, start) };
        let crossed = if start_handle { raw > end } else { start > raw };
        let previous_raw = self.touch.raw;
        let snapped = || {
            // The word under the finger, kept to its line (a word broken across lines).
            let word = self.unit_at(super::Unit::Word, raw);
            let raw_line = line(raw);
            let word_start = if line(word.start) == raw_line {
                word.start
            } else {
                self.original_of(layout.get_line_start(raw_line))
            };
            let word_end = if line(word.end) == raw_line {
                word.end
            } else {
                self.original_of(layout.get_line_end(raw_line, false))
            };
            if word_start == other {
                word_end
            } else if word_end == other {
                word_start
            } else if start_handle != crossed {
                if raw <= word_end { word_start } else { word_end }
            } else if raw >= word_start {
                word_end
            } else {
                word_start
            }
        };
        let moved_to = match previous_raw {
            Some(previous_raw) if previous_raw == raw => previous,
            Some(previous_raw) if line(previous_raw) != line(raw) => snapped(),
            _ => {
                let expanding = match previous_raw {
                    None => true,
                    Some(previous_raw) => {
                        if start_handle != crossed {
                            raw < previous_raw
                        } else {
                            raw > previous_raw
                        }
                    }
                };
                let word = self.unit_at(super::Unit::Word, previous);
                if !expanding {
                    raw
                } else if previous == word.start || previous == word.end {
                    snapped()
                } else {
                    raw
                }
            }
        };
        if start_handle { (moved_to, end) } else { (start, moved_to) }
    }

    /// Tells the handles which show and where, at the end of a measurement, as Compose's
    /// `getCursorHandleState` and `getSelectionHandleState`: only when it changed, and nothing
    /// at all while none shows and none may.
    pub(super) fn tell_handles(&mut self, measurer: &dyn TextMeasurer, paragraph: ParagraphHandle) {
        let may_show =
            self.focused && self.enabled && self.touch.direct && self.touch.long.is_none();
        if !may_show && !self.touch.shown.any() && !self.touch.active {
            return;
        }
        let Some(handles) = self.touch.handles.clone() else { return };
        self.tell_handles_now(&handles, may_show, measurer, paragraph);
    }

    #[inline(never)]
    fn tell_handles_now(
        &mut self,
        handles: &TouchHandles,
        may_show: bool,
        measurer: &dyn TextMeasurer,
        paragraph: ParagraphHandle,
    ) {
        // An edit of what's shown hides the cursor handle, as Compose's `observeTextChanges`.
        if self.touch.cursor_handle.as_deref().is_some_and(|text| text != self.shown_str()) {
            self.touch.cursor_handle = None;
        }
        let may_show =
            may_show && self.window.as_ref().is_none_or(|window| window.is_window_focused());
        let mut shown = ShownHandles::default();
        let mut positions = self.touch.positions;
        let dragging = self.touch.drag.map(|(handle, _, _)| handle);
        if may_show != self.touch.active {
            self.touch.active = may_show;
            handles.active.set(may_show);
        }
        // The field's bounds clipped by its ancestors, after its last placement; its own bounds
        // until they're known.
        let size = self.size;
        let visible = handles.visible.get().filter(|_| may_show).unwrap_or(Rect::new(
            0.0,
            0.0,
            size.width as f32,
            size.height as f32,
        ));
        let scroll = self.scroll;
        let in_view = |point: Offset| {
            let local = point - scroll;
            local.x >= visible.left
                && local.y >= visible.top
                && local.x <= visible.right
                && local.y <= visible.bottom
        };
        let mut place = |handle: Handle, point: Offset| {
            positions.text[handle.index()] = point;
            positions.anchor[handle.index()] = point - scroll;
        };
        if may_show && self.anchor == self.cursor {
            if self.touch.cursor_handle.is_some()
                && !self.read_only
                && let Some(rect) = self.drawn_cursor_rect()
            {
                let point = Offset::new((rect.left + rect.right) / 2.0, rect.bottom);
                if dragging == Some(Handle::Cursor) || in_view(point) {
                    shown.cursor = true;
                    place(Handle::Cursor, point);
                }
            }
        } else if may_show {
            let layout = measurer.paragraph(paragraph);
            let crossed = self.anchor > self.cursor;
            for (handle, offset) in
                [(Handle::SelectionStart, self.anchor), (Handle::SelectionEnd, self.cursor)]
            {
                let start = handle == Handle::SelectionStart;
                let offset = self.shown(offset);
                let size = compose_ui::Size::new(layout.width(), layout.height());
                let Some(point) = selection_handle_coordinates(
                    &*layout,
                    self.shown_str(),
                    offset,
                    (start, crossed),
                    size,
                ) else {
                    continue;
                };
                if dragging != Some(handle) && !in_view(point) {
                    continue;
                }
                // As Compose's: the start's character, or the one before the end.
                let direction_offset =
                    if start { offset } else { previous_char(self.shown_str(), offset) };
                let direction = layout.get_bidi_run_direction(direction_offset);
                let ltr = (direction == ResolvedTextDirection::Ltr) != crossed;
                let left = if start { ltr } else { !ltr };
                if start {
                    shown.start = Some(left);
                } else {
                    shown.end = Some(left);
                }
                place(handle, point);
            }
        }
        if shown != self.touch.shown {
            self.touch.shown = shown;
            handles.shown.set(shown);
        }
        if positions != self.touch.positions {
            self.touch.positions = positions;
            handles.positions.set(positions);
        }
    }
}
