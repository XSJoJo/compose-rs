//! A `SelectionContainer`'s text toolbar and right-click menu, as Compose's `SelectionManager`'s
//! `contextMenuAreaModifier` (a right click's dropdown, the toolbar's handler) and
//! `addSelectionContainerTextContextMenuComponents` (copy, select all) (D2041).
//!
//! compose-ui's tree holds the selection: it decides when the toolbar shows (Compose's
//! `updateSelectionToolbar`) and where (`derivedContentRect`), and runs the items, through the
//! container's `ContainerHandles`. A container pays three modifier nodes and a state, and nothing
//! at each measurement or for its texts until a toolbar is asked for or a menu opens.

use std::rc::Rc;

use compose_runtime::{OwnedState, composable};
use compose_ui::{
    ContainerHandles, ContainerMenuAction, ContainerMenuItems, Modifier, text_action_labels,
};

use super::dropdown::{DefaultDropdownMenu, DropdownSlot, default_dropdown};
use super::modifier::{
    ComputeContentBounds, TextContextMenuBuilder, TextContextMenuBuilderWithContext,
    ToolbarRequester, append_text_context_menu_components_with_context,
    show_text_context_menu_on_secondary_click_or, text_context_menu_toolbar_handler,
    translate_root_to_destination,
};
use super::{TextContextMenuBuilderScope, TextContextMenuKeys};

/// What a `SelectionContainer` remembers of its toolbar and menu: the modifier its box takes and
/// its default dropdown, made at its first right click where none is provided.
pub(crate) struct ContainerMenu {
    dropdown: Rc<OwnedState<Option<DropdownSlot>>>,
    modifier: Modifier,
}

impl compose_runtime::ComposeArg for ContainerMenu {
    fn same(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}

impl ContainerMenu {
    pub(crate) fn new(handles: Rc<ContainerHandles>) -> ContainerMenu {
        // The tree shows and hides the toolbar through the handler's requester.
        let requester = ToolbarRequester::new();
        let requests = requester.clone();
        handles.set_toolbar_requests(Rc::new(move |show| {
            if show {
                requests.show();
            } else {
                requests.hide();
            }
        }));
        // Its items, asked of the tree where the menu's collected, and again where it's built
        // (a right click's menu in composition, after the click collected it), so an open menu
        // follows the selection, as Compose's builder reads it while its menu shows.
        let with_context: TextContextMenuBuilderWithContext = {
            let handles = handles.clone();
            Rc::new(move |context| {
                let collected = handles.menu_items(context);
                let handles = handles.clone();
                let builder: TextContextMenuBuilder = Rc::new(move |scope| {
                    let items = handles.current_menu_items().unwrap_or(collected);
                    container_components(&handles, items, scope)
                });
                builder
            })
        };
        // Where the toolbar goes: the tree's rect, from the root to the destination; the last
        // while there's none, as Compose's handler keeps it.
        let compute: ComputeContentBounds = {
            let handles = handles.clone();
            Rc::new(move |destination| {
                let root = handles.toolbar_content_rect()?;
                let local = handles.coordinates()?;
                Some(translate_root_to_destination(root, &local, destination))
            })
        };
        let dropdown = Rc::new(compose_runtime::mutable_state_of(None));
        let default = {
            let dropdown = dropdown.clone();
            Rc::new(move |anchor| Some(default_dropdown(&dropdown, anchor)))
        };
        let modifier = append_text_context_menu_components_with_context(Modifier, with_context);
        let modifier = text_context_menu_toolbar_handler(modifier, requester, None, None, compute);
        let modifier = show_text_context_menu_on_secondary_click_or(modifier, default);
        ContainerMenu { dropdown, modifier }
    }

    /// What the container's box takes after its selection's modifier.
    pub(crate) fn modifier(&self) -> Modifier {
        self.modifier.clone()
    }
}

/// The container's default dropdown's open menu, if any, composed after its content; its texts
/// aren't the container's to select, as Compose composes the menu outside the container's
/// `LocalSelectionRegistrar`.
#[composable]
#[allow(non_snake_case)]
pub(crate) fn ContainerDropdownMenu(menu: Rc<ContainerMenu>) {
    let Some(slot) = menu.dropdown.get() else { return };
    crate::DisableSelection(compose_runtime::callback(move || {
        DefaultDropdownMenu(Some(slot.clone()));
    }));
}

/// The container's items, as Compose's `addSelectionContainerTextContextMenuComponents`: copy
/// over a selection of some characters, closing the menu; select all while not everything's
/// selected, closing it unless the toolbar's asked for in touch mode; between separators.
fn container_components(
    handles: &Rc<ContainerHandles>,
    items: ContainerMenuItems,
    scope: &mut TextContextMenuBuilderScope,
) {
    scope.separator();
    let actions = [
        (
            items.copy,
            TextContextMenuKeys::Copy,
            text_action_labels::COPY,
            ContainerMenuAction::Copy,
        ),
        (
            items.select_all,
            TextContextMenuKeys::SelectAll,
            text_action_labels::SELECT_ALL,
            ContainerMenuAction::SelectAll,
        ),
    ];
    for (shown, key, label, action) in actions {
        if !shown {
            continue;
        }
        let handles = handles.clone();
        scope.item(key, label, None, move |session| {
            if handles.run_menu_action(action) {
                session.close();
            }
        });
    }
    scope.separator();
}
