//! The default dropdown of a text context menu, as Compose's `DefaultTextContextMenuDropdownProvider`:
//! the menu's items in the context menu's popup, at the data provider's position, closed by a
//! click outside or the back key (D2037).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_runtime::composable;
use compose_ui::{
    Alignment, ColorFilter, ContentScale, IntOffset, IntSize, LayoutDirection, Modifier, Painter,
    PopupPositionProvider, PopupProperties, Rect,
};

use super::data::{TextContextMenuComponent, TextContextMenuSession};
use super::provider::{
    AnchorCoordinates, BasicTextContextMenuProvider, ContextMenuBlock,
    LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER, ProvideBasicTextContextMenu, ProviderLocal,
    TextContextMenuDataProvider, basic_text_context_menu_provider,
};
use crate::context_menu::{
    ContextMenuBuilderBlock, ContextMenuColumnBuilder, ContextMenuIcon,
    ContextMenuPopupPositionProvider, ContextMenuScope, ContextMenuSpec,
    DEFAULT_CONTEXT_MENU_COLORS,
};

thread_local! {
    /// The default dropdown's menu, one block for every provider (a provider is remembered by its
    /// block).
    static OPEN_CONTEXT_MENU: ContextMenuBlock = Rc::new(|session, data, anchor| {
        OpenContextMenu(session, data, anchor)
    });
}

fn open_context_menu() -> ContextMenuBlock {
    OPEN_CONTEXT_MENU.with(Rc::clone)
}

/// A field's or a container's default dropdown where none is provided (D2040): made at its first
/// right click, its menu composed by [`DefaultDropdownMenu`] in the handles' composable, anchored
/// at the node the click was on.
pub(crate) struct DefaultDropdown {
    provider: Rc<BasicTextContextMenuProvider>,
    anchor: Cell<Option<compose_ui::LayoutCoordinates>>,
}

/// A [`DefaultDropdown`] in a state, equal to itself only.
#[derive(Clone)]
pub(crate) struct DropdownSlot(Rc<DefaultDropdown>);

impl compose_runtime::ComposeArg for DropdownSlot {
    fn same(&self, other: &DropdownSlot) -> bool {
        self == other
    }
}

impl PartialEq for DropdownSlot {
    fn eq(&self, other: &DropdownSlot) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

/// The default dropdown in `slot`, made at the first call, anchored at `anchor`.
pub(crate) fn default_dropdown(
    slot: &compose_runtime::OwnedState<Option<DropdownSlot>>,
    anchor: compose_ui::LayoutCoordinates,
) -> Rc<dyn super::TextContextMenuProvider> {
    let made = slot.get().unwrap_or_else(|| {
        let made = DropdownSlot(Rc::new(DefaultDropdown {
            provider: Rc::new(BasicTextContextMenuProvider::new(open_context_menu())),
            anchor: Cell::new(None),
        }));
        slot.set(Some(made.clone()));
        made
    });
    made.0.anchor.set(Some(anchor));
    made.0.provider.clone()
}

/// The open menu of a default dropdown, if any: composed where its anchor's handles are.
#[composable]
#[allow(non_snake_case)]
pub(crate) fn DefaultDropdownMenu(slot: Option<DropdownSlot>) {
    let Some(slot) = slot else { return };
    let anchored = slot.0.clone();
    let anchor: AnchorCoordinates = Rc::new(move || anchored.anchor.get());
    slot.0.provider.context_menu(anchor);
}

/// Provides `LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER` with the default dropdown over `content`,
/// in a box (`propagate_min_constraints`) its menus are anchored to, as Compose's
/// `ProvideDefaultTextContextMenuDropdown(modifier, content)`.
#[doc(hidden)]
#[composable]
pub fn ProvideDefaultTextContextMenuDropdown(
    #[default(crate::Modifier)] modifier: Modifier,
    content: impl Fn() + 'static,
) {
    ProvideBasicTextContextMenu(
        modifier,
        ProviderLocal(&LOCAL_TEXT_CONTEXT_MENU_DROPDOWN_PROVIDER),
        open_context_menu(),
        content,
    );
}

/// Remembers the default dropdown's provider, as Compose's `defaultTextContextMenuDropdown`: its
/// caller composes its menu (`context_menu`) where it anchors it.
#[doc(hidden)]
pub fn default_text_context_menu_dropdown() -> Rc<BasicTextContextMenuProvider> {
    basic_text_context_menu_provider(open_context_menu())
}

/// The open menu: a focusable popup at the data provider's position in the anchor, its items
/// following the data, a click outside or the back key closing the session, as Compose's
/// `OpenContextMenu`.
#[composable]
#[allow(non_snake_case)]
fn OpenContextMenu(
    session: Rc<dyn TextContextMenuSession>,
    data_provider: Rc<dyn TextContextMenuDataProvider>,
    anchor: AnchorCoordinates,
) {
    // Where the menu goes in the anchor, asked as it's placed, as Compose's
    // `dataProvider.position(anchorLayoutCoordinates())` in `calculatePosition` (D2039): the
    // tree answers coordinates' queries there. The anchor's latest block is the one asked.
    let latest = compose_runtime::remember(|| Rc::new(RefCell::new(anchor.clone())));
    *latest.borrow_mut() = anchor;
    let provider = compose_runtime::remember_keyed(data_provider.clone(), {
        let data_provider = data_provider.clone();
        move || {
            let position = move || {
                let anchor = latest.borrow().clone();
                match anchor().filter(compose_ui::LayoutCoordinates::is_attached) {
                    Some(anchor) => {
                        let at = data_provider.position(&anchor);
                        IntOffset::new(at.x.round() as i32, at.y.round() as i32)
                    }
                    None => IntOffset::ZERO,
                }
            };
            Rc::new(MaintainWindowPositionPopupPositionProvider::new(
                ContextMenuPopupPositionProvider::new(position, None),
            )) as Rc<dyn PopupPositionProvider>
        }
    });
    let closing = session.clone();
    compose_ui::PopupWithPositionProvider(
        provider,
        Some(compose_runtime::callback(move || closing.close())),
        PopupProperties { focusable: true, ..PopupProperties::default() },
        compose_runtime::callback(move || {
            // Read here, so a change of what it reads composes the items again (Compose's
            // `derivedStateOf(dataProvider::data)`).
            let data = data_provider.data();
            let session = session.clone();
            let block: ContextMenuBuilderBlock = Rc::new(move |scope: &mut ContextMenuScope| {
                for component in &data.components {
                    match component {
                        TextContextMenuComponent::Item(item) => {
                            let icon = item.leading_icon.clone().map(|painter| {
                                let icon: ContextMenuIcon =
                                    Rc::new(move |color| IconBox(painter.clone(), color));
                                icon
                            });
                            let (action, session) = (item.on_click.clone(), session.clone());
                            scope.item(item.label.clone(), Modifier, true, icon, move || {
                                action(&*session)
                            });
                        }
                        TextContextMenuComponent::Separator => scope.separator(),
                    }
                }
            });
            ContextMenuColumnBuilder(Modifier, DEFAULT_CONTEXT_MENU_COLORS, block);
        }),
    );
}

/// An item's icon, `ICON_SIZE`, tinted, as Compose's `IconBox` (Material 3's `Icon`'s parts).
fn IconBox(painter: Painter, tint: compose_ui::Color) {
    let filter = tint.is_specified().then(|| ColorFilter::tint(tint));
    crate::layout::Box(
        Modifier.size(ContextMenuSpec::ICON_SIZE).paint_with(
            painter,
            true,
            Alignment::CENTER,
            ContentScale::Fit,
            1.0,
            filter,
        ),
        Alignment::TOP_START,
        false,
        compose_runtime::callback(|_| {}),
    );
}

/// Delegates to its provider, but keeps the last position while only the anchor moved, so a
/// scroll or a move of what the menu's anchored to leaves it where it is; a new window size,
/// layout direction or menu size places it again, as Compose's
/// `MaintainWindowPositionPopupPositionProvider`.
struct MaintainWindowPositionPopupPositionProvider {
    provider: ContextMenuPopupPositionProvider,
    previous: Cell<Option<(IntSize, LayoutDirection, IntSize, IntOffset)>>,
}

impl MaintainWindowPositionPopupPositionProvider {
    fn new(provider: ContextMenuPopupPositionProvider) -> Self {
        MaintainWindowPositionPopupPositionProvider { provider, previous: Cell::new(None) }
    }
}

impl PopupPositionProvider for MaintainWindowPositionPopupPositionProvider {
    fn position(
        &self,
        anchor: Rect,
        window: IntSize,
        direction: LayoutDirection,
        popup: IntSize,
    ) -> IntOffset {
        if let Some((window_was, direction_was, popup_was, position)) = self.previous.get()
            && (window_was, direction_was, popup_was) == (window, direction, popup)
        {
            return position;
        }
        let position = self.provider.position(anchor, window, direction, popup);
        self.previous.set(Some((window, direction, popup, position)));
        position
    }
}
