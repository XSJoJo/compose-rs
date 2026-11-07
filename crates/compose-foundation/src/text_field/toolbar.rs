//! A field's text toolbar, as Compose's `TextFieldSelectionState`'s: its `TextToolbarState`, set
//! by the gestures, shown while its content rect is visible (`derivedVisibleContentBounds`), with
//! the field's items (`addBasicTextFieldTextContextMenuComponents`) (D2032).
//!
//! A field whose toolbar state is `None`, showing none and no right click's menu, pays two
//! branches at each measurement.

use std::rc::Rc;

use compose_runtime::OwnedState;
use compose_ui::platform::{ParagraphHandle, TextMeasurer};
use compose_ui::{EventContext, LayoutCoordinates, NodeContext, Offset, Rect, text_action_labels};

use super::TextFieldNode;
use crate::key_mapping::KeyCommand;
use crate::selection_handles::{TouchHandles, selection_handle_coordinates};
use crate::text_context_menu::dropdown::default_dropdown;
use crate::text_context_menu::modifier::{
    ToolbarHandler, ToolbarHost, show_text_context_menu_at, translate_root_to_destination,
};
use crate::text_context_menu::{
    TextContextMenuBuilder, TextContextMenuBuilderScope, TextContextMenuKeys,
    TextContextMenuSession,
};

/// Which toolbar the user asked for, as Compose's `TextToolbarState`: the cursor's (a tap on its
/// handle), the selection's (a long press, a double tap, `SetSelection`), or none.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum ToolbarState {
    #[default]
    None,
    Cursor,
    Selection,
}

/// Which of the field's items show, as Compose's `canShow*MenuItem`. No autofill: compose-rs has
/// none to ask, so its item, which would do nothing, isn't shown (D2035).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct FieldMenu {
    cut: bool,
    copy: bool,
    paste: bool,
    select_all: bool,
}

/// Whether the clipboard holds something to paste, as Compose's internal `ClipboardPasteState`:
/// known when it's updated (a field updates it when a toolbar or a menu shows), never reading the clip
/// itself (a clipboard's `has_clip` and `has_text` mustn't), which
/// would show Android's "pasted from your clipboard" notice (b/422754681). Compose's takes the
/// clipboard when made; this one is given it at each update, the field's being the tree's while
/// it's focused (D2035).
#[doc(hidden)]
#[derive(Clone, Copy, Default, Debug)]
pub struct ClipboardPasteState {
    has_clip: bool,
    has_text: bool,
}

impl ClipboardPasteState {
    /// Whether it held text at the last update.
    pub fn has_text(&self) -> bool {
        self.has_text
    }

    /// Whether it held anything at the last update.
    pub fn has_clip(&self) -> bool {
        self.has_clip
    }

    /// Asks `clipboard` again, as Compose's `update`.
    pub fn update(&mut self, clipboard: &dyn compose_ui::ClipboardManager) {
        self.has_clip = clipboard.has_clip();
        self.has_text = self.has_clip && clipboard.has_text();
    }
}

/// What a field's toolbar reads, in its shared `TouchHandles`: where it goes in the inner field,
/// where the inner field is in the root, and which items show. States, so the toolbar follows
/// them.
pub(crate) struct FieldToolbar {
    bounds: OwnedState<Option<Rect>>,
    pub(crate) origin: OwnedState<Offset>,
    menu: OwnedState<FieldMenu>,
}

impl TouchHandles {
    fn field_toolbar(&self) -> &FieldToolbar {
        self.toolbar.get_or_init(|| FieldToolbar {
            bounds: compose_runtime::mutable_state_of(None),
            origin: compose_runtime::mutable_state_of(self.origin.get()),
            menu: compose_runtime::mutable_state_of(FieldMenu::default()),
        })
    }
}

/// An action of the field's menu.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MenuAction {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

/// What a field's node keeps of its toolbar.
#[derive(Default)]
pub(crate) struct Toolbar {
    pub(super) state: ToolbarState,
    /// The shown text when the state was set: an edit of it sets it back to `None`, as Compose's
    /// `observeTextChanges`.
    text: Option<String>,
    /// The cursor handle's taps followed.
    taps: u32,
    /// The toolbar was asked to show and not to hide since.
    requested: bool,
    /// What the clipboard held when a toolbar or a menu last showed.
    paste: ClipboardPasteState,
    handler: ToolbarHandler,
    /// The right click's menu's job, while it may show: the items follow the field until it ends,
    /// as Compose's builder reads the field's state while its menu shows.
    dropdown: Option<compose_runtime::Job>,
}

impl Toolbar {
    /// Hides it and forgets what was asked, its node detached or reused.
    pub(super) fn reset(&mut self) {
        self.state = ToolbarState::None;
        self.text = None;
        self.requested = false;
        self.handler.hide();
        self.dropdown = None;
    }
}

impl TextFieldNode {
    /// Sets the toolbar state, as Compose's `updateTextToolbarState`.
    pub(super) fn set_toolbar_state(&mut self, state: ToolbarState) {
        self.touch.toolbar.state = state;
        self.touch.toolbar.text =
            (state != ToolbarState::None).then(|| self.shown_text_now().to_owned());
    }

    /// A mouse's multi-click or drag selected: the selection's toolbar is asked for, as Compose's
    /// `TextFieldMouseSelectionObserver.updateSelection` sets `Selection`; it shows once a finger
    /// touches the field again (D2041).
    pub(super) fn mouse_selected(&mut self) {
        if self.touch.toolbar.state != ToolbarState::Selection {
            self.set_toolbar_state(ToolbarState::Selection);
        }
    }

    fn shown_text_now(&self) -> &str {
        self.shown_text.as_deref().unwrap_or(&self.text)
    }

    /// Where the toolbar goes in the inner field, as Compose's `derivedVisibleContentBounds`:
    /// shown for a cursor in `Cursor` or a selection in `Selection`, while the user touches the
    /// focused field and drags nothing, around the cursor or the selection (from its first line's
    /// top to its last's bottom, between its ends), kept to the visible bounds; none past them.
    fn toolbar_rect(
        &self,
        measurer: &dyn TextMeasurer,
        paragraph: ParagraphHandle,
    ) -> Option<Rect> {
        let collapsed = self.anchor == self.cursor;
        let asked = match self.touch.toolbar.state {
            ToolbarState::Cursor => collapsed,
            ToolbarState::Selection => !collapsed,
            ToolbarState::None => false,
        };
        let shows = asked
            && self.focused
            && self.enabled
            && self.touch.direct
            && self.touch.long.is_none()
            && self.touch.drag.is_none()
            && self.window.as_ref().is_none_or(|window| window.is_window_focused());
        if !shows {
            return None;
        }
        let scroll = self.scroll;
        let content = if collapsed {
            self.drawn_cursor_rect()?.translate(Offset::ZERO - scroll)
        } else {
            let layout = measurer.paragraph(paragraph);
            let (start, end) = (self.anchor.min(self.cursor), self.anchor.max(self.cursor));
            let (start, end) = (self.shown(start), self.shown(end));
            let size = compose_ui::Size::new(layout.width(), layout.height());
            let text = self.shown_text_now();
            let start_point =
                selection_handle_coordinates(&*layout, text, start, (true, false), size)?;
            let end_point =
                selection_handle_coordinates(&*layout, text, end, (false, false), size)?;
            let top = |offset: usize| layout.get_line_top(layout.get_line_for_offset(offset));
            let left = start_point.x.min(end_point.x);
            let right = start_point.x.max(end_point.x);
            let right = if left == right { right + 1.0 } else { right };
            Rect::new(left, top(start).min(top(end)), right, start_point.y.max(end_point.y))
                .translate(Offset::ZERO - scroll)
        };
        // Placed first, as Compose's needs `textLayoutCoordinates`: the inner field's visible
        // bounds and its place in the root are known once it's placed with handles possible.
        let visible = self.touch.handles_visible()?;
        content.overlaps(visible).then(|| content.intersect(visible))
    }

    /// Shows or hides the toolbar at the end of a measurement, as Compose's
    /// `observeTextToolbarVisibility`: shown when its rect appears, hidden when it goes; its rect
    /// and items told while it shows, the items while its right click's menu may show too.
    /// Nothing while it's asked for nothing and shows nothing.
    pub(super) fn tell_toolbar(
        &mut self,
        scope: &mut compose_ui::MeasureScope<'_>,
        paragraph: ParagraphHandle,
    ) {
        let tapped = self
            .touch
            .handles()
            .is_some_and(|handles| handles.cursor_taps.get() != self.touch.toolbar.taps);
        if self.touch.toolbar.state == ToolbarState::None
            && !self.touch.toolbar.requested
            && !tapped
            && self.touch.toolbar.dropdown.is_none()
        {
            return;
        }
        self.tell_toolbar_now(scope, paragraph);
    }

    #[inline(never)]
    fn tell_toolbar_now(
        &mut self,
        scope: &mut compose_ui::MeasureScope<'_>,
        paragraph: ParagraphHandle,
    ) {
        let Some(handles) = self.touch.handles().cloned() else { return };
        // Taps on the cursor handle toggle the cursor's toolbar.
        let taps = handles.cursor_taps.get();
        for _ in self.touch.toolbar.taps..taps {
            let toggled = if self.touch.toolbar.state == ToolbarState::Cursor {
                ToolbarState::None
            } else {
                ToolbarState::Cursor
            };
            self.set_toolbar_state(toggled);
        }
        self.touch.toolbar.taps = taps;
        // An edit of what's shown hides it.
        if self.touch.toolbar.text.as_deref().is_some_and(|text| text != self.shown_text_now()) {
            self.set_toolbar_state(ToolbarState::None);
        }
        let rect = self.toolbar_rect(&*scope.text_measurer(), paragraph);
        let toolbar = handles.field_toolbar();
        if toolbar.bounds.with(|bounds| *bounds != rect) && rect.is_some() {
            toolbar.bounds.set(rect);
        }
        // The right click's menu's items follow the field too while it may show (Compose's
        // builder reads the field's state, shown or not); closed, it's forgotten.
        let dropdown = match &self.touch.toolbar.dropdown {
            Some(job) if job.is_active() => true,
            Some(_) => {
                self.touch.toolbar.dropdown = None;
                false
            }
            None => false,
        };
        if rect.is_some() || dropdown {
            let menu = self.field_menu();
            if toolbar.menu.with(|it| *it != menu) {
                toolbar.menu.set(menu);
            }
        }
        match (rect.is_some(), self.touch.toolbar.requested) {
            (true, false) => {
                self.touch.toolbar.requested = true;
                // Shown from a task, outside the measurement, as Compose's flow collects.
                let Some(handle) = scope.handle() else { return };
                compose_runtime::platform::launch(async move {
                    handle.with_node::<TextFieldNode, _>(|node, context| {
                        if node.touch.toolbar.requested {
                            node.update_clipboard_entry();
                            node.touch.toolbar.handler.show::<TextFieldNode>(context, None, None);
                        }
                    });
                });
            }
            (false, true) => {
                self.touch.toolbar.requested = false;
                self.touch.toolbar.handler.hide();
            }
            _ => {}
        }
    }

    /// A right click at `click`, in the field's coordinates: the dropdown of its menu there, as
    /// Compose's `showTextContextMenuOnSecondaryClick` with its `onPreShowContextMenu`
    /// (`updateClipboardEntry`, before the menu shows), through the provided dropdown or the
    /// field's default (D2040).
    pub(super) fn right_click(&mut self, click: Offset, context: &mut EventContext<'_>) {
        self.touch_right_click(context);
        self.update_clipboard_entry();
        let Some(handles) = self.touch.handles().cloned() else { return };
        let own: TextContextMenuBuilder = {
            let handles = handles.clone();
            Rc::new(move |scope| field_components(&handles, scope))
        };
        self.touch.toolbar.dropdown =
            show_text_context_menu_at(context, click, Some(own), |anchor| {
                Some(default_dropdown(&handles.dropdown, anchor))
            });
    }

    /// Asks the clipboard what it holds, as Compose's `updateClipboardEntry` when a toolbar or a
    /// menu shows, and tells the items.
    pub(super) fn update_clipboard_entry(&mut self) {
        if let Some(clipboard) = self.touch.clipboard().cloned() {
            self.touch.toolbar.paste.update(&*clipboard);
        }
        if let Some(handles) = self.touch.handles().cloned() {
            let menu = self.field_menu();
            let toolbar = handles.field_toolbar();
            if toolbar.menu.with(|it| *it != menu) {
                toolbar.menu.set(menu);
            }
        }
    }

    /// Which items show, as Compose's `canShow*MenuItem`: paste from what the clipboard held when
    /// a toolbar or a menu showed (Compose's `ClipboardPasteState`).
    fn field_menu(&self) -> FieldMenu {
        let editable = self.enabled && !self.read_only;
        let collapsed = self.anchor == self.cursor;
        let password = self.is_password();
        FieldMenu {
            cut: !collapsed && !password && editable,
            copy: !collapsed && !password,
            paste: editable && self.touch.toolbar.paste.has_text(),
            select_all: self.anchor.abs_diff(self.cursor) != self.text.len(),
        }
    }

    /// Runs a menu's action on the field, as Compose's items: then the menu closes and the
    /// toolbar state goes back to `None`, but for "Select all", which asks for the selection's
    /// toolbar and closes the menu only while no toolbar shows. Whether one shows is Compose's
    /// `textToolbarShown`: shown, not asked for (a selection scrolled out of view hides it).
    fn menu_action(
        &mut self,
        action: MenuAction,
        session: &dyn TextContextMenuSession,
        context: &mut EventContext<'_>,
    ) {
        self.adopt_state_in(context);
        let toolbar_shown = self.touch.toolbar.handler.is_showing();
        let mut close = true;
        let mut state = ToolbarState::None;
        match action {
            MenuAction::Cut => {
                self.run_key_command(KeyCommand::Cut, false, context);
            }
            MenuAction::Copy => {
                self.run_key_command(KeyCommand::Copy, false, context);
                // From the toolbar, the selection collapses to its end (`cancelSelection`).
                if toolbar_shown {
                    let end = self.anchor.max(self.cursor);
                    self.anchor = end;
                    self.cursor = end;
                }
            }
            MenuAction::Paste => {
                self.run_key_command(KeyCommand::Paste, false, context);
            }
            MenuAction::SelectAll => {
                self.run_key_command(KeyCommand::SelectAll, false, context);
                close = !toolbar_shown;
                state = ToolbarState::Selection;
            }
        }
        if close {
            session.close();
        }
        self.preferred_x = None;
        context.invalidate_measure();
        context.invalidate_semantics();
        self.publish_state_in(super::EditSource::Other, context);
        self.set_toolbar_state(state);
    }
}

impl ToolbarHost for TextFieldNode {
    fn toolbar_content_bounds(
        &mut self,
        destination: &LayoutCoordinates,
        context: &mut NodeContext<'_>,
    ) -> Option<Rect> {
        let handles = self.touch.handles()?;
        let toolbar = handles.toolbar.get()?;
        let local = toolbar.bounds.get()?;
        let root = local.translate(toolbar.origin.get());
        Some(translate_root_to_destination(
            root,
            &context.require_layout_coordinates(),
            destination,
        ))
    }

    /// The field's items, as Compose's `addBasicTextFieldTextContextMenuComponents`: cut, copy,
    /// paste and select all, each only when it applies, between separators (no autofill, D2035).
    /// What applies is a state the field writes, so the toolbar follows it.
    fn own_text_context_menu_components(&mut self, scope: &mut TextContextMenuBuilderScope) {
        if let Some(handles) = self.touch.handles() {
            field_components(handles, scope);
        }
    }
}

/// The field's items, reading what applies from its handles (D2032), for its toolbar and its
/// right click's menu (D2040).
fn field_components(handles: &Rc<TouchHandles>, scope: &mut TextContextMenuBuilderScope) {
    {
        let menu = handles.field_toolbar().menu.get();
        scope.separator();
        let items = [
            (menu.cut, TextContextMenuKeys::Cut, text_action_labels::CUT, MenuAction::Cut),
            (menu.copy, TextContextMenuKeys::Copy, text_action_labels::COPY, MenuAction::Copy),
            (menu.paste, TextContextMenuKeys::Paste, text_action_labels::PASTE, MenuAction::Paste),
            (
                menu.select_all,
                TextContextMenuKeys::SelectAll,
                text_action_labels::SELECT_ALL,
                MenuAction::SelectAll,
            ),
        ];
        for (shown, key, label, action) in items {
            if !shown {
                continue;
            }
            let handles = handles.clone();
            scope.item(key, label, None, move |session| {
                let Some(field) = handles.field.get() else { return };
                field.with_node_in_event::<TextFieldNode, _>(|node, context| {
                    node.menu_action(action, session, context)
                });
            });
        }
        scope.separator();
    }
}
