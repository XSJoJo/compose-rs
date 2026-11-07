//! A dropdown context menu opened by a right click, as Compose's internal `ContextMenuArea.kt`,
//! `ContextMenuGestures.kt` and `RightClickGestures.kt` (D2036).

use std::rc::Rc;

use compose_runtime::composable;
use compose_ui::{
    Alignment, IntOffset, Modifier, Offset, PointerEventPass, PointerInputScope,
    PopupPositionProvider,
};

use super::{
    ContextMenuBuilderBlock, ContextMenuPopup, ContextMenuPopupPositionProvider, ContextMenuState,
    ContextMenuStatus,
};

/// The context menu's right click, as Compose's `contextMenuGestures`.
pub trait ContextMenuGestures {
    /// Calls `on_open_gesture` with where a right click went down (consuming it), as Compose's
    /// `Modifier.contextMenuGestures(onOpenGesture)`.
    fn context_menu_gestures(self, on_open_gesture: impl Fn(Offset) + 'static) -> Modifier;

    /// Opens `state` where a right click went down, as Compose's
    /// `Modifier.contextMenuGestures(state)`.
    fn context_menu_gestures_of(self, state: Rc<ContextMenuState>) -> Modifier;
}

/// Compose's `ContextMenuKey`, the gestures' key.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ContextMenuKey;

impl compose_runtime::ComposeArg for ContextMenuKey {
    fn same(&self, _: &ContextMenuKey) -> bool {
        true
    }
}

impl ContextMenuGestures for Modifier {
    fn context_menu_gestures(self, on_open_gesture: impl Fn(Offset) + 'static) -> Modifier {
        let on_open_gesture: Rc<dyn Fn(Offset)> = Rc::new(on_open_gesture);
        self.pointer_input(ContextMenuKey, move |scope| {
            let on_open_gesture = on_open_gesture.clone();
            async move { on_right_click_down(&scope, &*on_open_gesture).await }
        })
    }

    fn context_menu_gestures_of(self, state: Rc<ContextMenuState>) -> Modifier {
        self.context_menu_gestures(move |offset| {
            state.set_status(ContextMenuStatus::open(offset));
        })
    }
}

/// Calls `on_down` at each right click's press, consuming it and its release, as Compose's
/// `onRightClickDown`.
#[doc(hidden)]
pub async fn on_right_click_down(scope: &PointerInputScope, on_down: &dyn Fn(Offset)) {
    loop {
        // Compose's `awaitEachGesture`: a gesture starts with every pointer up.
        while scope.any_pressed() {
            scope.await_pointer_event(PointerEventPass::Final).await;
        }
        let down = loop {
            let mut event = scope.await_pointer_event(PointerEventPass::Main).await;
            if event.buttons.is_secondary_pressed()
                && event.changes.iter().all(|change| change.changed_to_down())
            {
                break event.changes.swap_remove(0);
            }
        };
        down.consume();
        on_down(down.position);
        if let Some(up) = scope.wait_for_up_or_cancellation().await {
            up.consume();
        }
    }
}

/// `content` in a box (`propagate_min_constraints`) whose right click opens `state`'s menu of the
/// items `block` adds, at the click, while `enabled`; `on_open_gesture` told first, `on_dismiss`
/// when a click outside or the back key dismisses it, as Compose's `ContextMenuArea`.
#[composable]
#[allow(non_snake_case)]
pub fn ContextMenuArea(
    state: Rc<ContextMenuState>,
    on_dismiss: impl Fn() + 'static,
    block: ContextMenuBuilderBlock,
    #[default(crate::Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(None)] on_open_gesture: Option<impl Fn() + 'static>,
    content: impl Fn() + 'static,
) {
    let modifier = if enabled {
        let (state, on_open_gesture) = (state.clone(), on_open_gesture.clone());
        modifier.context_menu_gestures(move |offset| {
            if let Some(on_open_gesture) = &on_open_gesture {
                on_open_gesture();
            }
            state.set_status(ContextMenuStatus::open(offset));
        })
    } else {
        modifier
    };
    let (content, on_dismiss, block) = (content.clone(), on_dismiss.clone(), block.clone());
    crate::layout::Box(
        modifier,
        Alignment::TOP_START,
        true,
        compose_runtime::callback(move |_| {
            content();
            ContextMenu(state.clone(), on_dismiss.clone(), Modifier, block.clone());
        }),
    );
}

/// `state`'s menu while it's open, at its offset in the layout around it, as Compose's
/// `ContextMenu`.
#[composable]
#[allow(non_snake_case)]
pub fn ContextMenu(
    state: Rc<ContextMenuState>,
    on_dismiss: impl Fn() + 'static,
    #[default(crate::Modifier)] modifier: Modifier,
    block: ContextMenuBuilderBlock,
) {
    let open = match state.status() {
        ContextMenuStatus::Open(offset) => Some(offset),
        ContextMenuStatus::Closed => None,
    };
    if let Some(offset) = open {
        let position = IntOffset::new(offset.x.round() as i32, offset.y.round() as i32);
        let provider = compose_runtime::remember_keyed(position, || {
            Rc::new(ContextMenuPopupPositionProvider::at(position, None))
                as Rc<dyn PopupPositionProvider>
        });
        ContextMenuPopup(
            provider,
            on_dismiss.clone(),
            modifier,
            super::compute_context_menu_colors(),
            block.clone(),
        );
    }
}
