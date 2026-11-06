//! Pinning a lazy layout's items, as Compose's `LazyLayoutPinnableItem` and
//! `LazyLayoutPinnedItemList` (D1325): an item's content pinned (what's focused in it, through
//! `LOCAL_PINNABLE_CONTAINER`) puts the item in its layout's pinned list, which the layout's
//! measurement reads to keep it composed and placed.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use compose_runtime::{OwnedState, composable, mutable_state_of};
use compose_ui::{LOCAL_PINNABLE_CONTAINER, PinnableContainer, PinnedHandle};

/// The items of a lazy layout pinned now, in the order they were pinned, as Compose's
/// `LazyLayoutPinnedItemList`: a state, so a measurement reading it measures again when an item
/// is pinned or released. Clones share the list.
#[derive(Clone)]
pub struct LazyLayoutPinnedItemList {
    items: Rc<OwnedState<Vec<PinnedItem>>>,
}

impl Default for LazyLayoutPinnedItemList {
    fn default() -> Self {
        LazyLayoutPinnedItemList { items: Rc::new(mutable_state_of(Vec::new())) }
    }
}

impl compose_runtime::ComposeArg for LazyLayoutPinnedItemList {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.items, &other.items)
    }
}

impl LazyLayoutPinnedItemList {
    pub fn new() -> LazyLayoutPinnedItemList {
        LazyLayoutPinnedItemList::default()
    }

    /// The items pinned now (read as a state).
    pub fn to_vec(&self) -> Vec<PinnedItem> {
        self.items.get()
    }

    /// How many items are pinned (read as a state).
    pub fn len(&self) -> usize {
        self.items.with(Vec::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn pin(&self, item: PinnedItem) {
        self.items.handle().edit(|items| items.push(item));
    }

    fn release(&self, item: &PinnedItem) {
        self.items.handle().edit(|items| items.retain(|it| it != item));
    }
}

/// An item pinned, as Compose's `LazyLayoutPinnedItemList.PinnedItem`: its key, and its index
/// as it was last composed.
#[derive(Clone)]
pub struct PinnedItem(Rc<PinnableItem>);

impl PartialEq for PinnedItem {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl PinnedItem {
    pub fn key(&self) -> u64 {
        self.0.key
    }

    pub fn index(&self) -> usize {
        self.0.index.get()
    }
}

/// An item as a pinnable container, as Compose's private `LazyLayoutPinnableItem`: pinned the
/// first time, it's in the list and pins the container around it; released the last time, or
/// gone from the composition, it's out of both.
struct PinnableItem {
    this: Weak<PinnableItem>,
    key: u64,
    index: Cell<usize>,
    list: LazyLayoutPinnedItemList,
    /// As `pinsCount`.
    pins: Cell<u32>,
    parent: RefCell<Option<Rc<dyn PinnableContainer>>>,
    parent_handle: RefCell<Option<PinnedHandle>>,
    disposed: Cell<bool>,
}

impl PinnableItem {
    /// As `parentPinnableContainer`'s setter: a new parent takes over the pin.
    fn set_parent(&self, parent: Option<Rc<dyn PinnableContainer>>) {
        let same = match (&*self.parent.borrow(), &parent) {
            (None, None) => true,
            (Some(one), Some(other)) => Rc::ptr_eq(one, other),
            _ => false,
        };
        if same {
            return;
        }
        if self.pins.get() > 0 {
            if let Some(handle) = self.parent_handle.borrow_mut().take() {
                handle.release();
            }
            *self.parent_handle.borrow_mut() = parent.as_ref().map(|parent| parent.pin());
        }
        *self.parent.borrow_mut() = parent;
    }

    fn handle(&self) -> PinnedItem {
        PinnedItem(self.this.upgrade().expect("a pinnable item pinned is alive"))
    }

    fn unpin(&self) {
        self.list.release(&self.handle());
        if let Some(handle) = self.parent_handle.borrow_mut().take() {
            handle.release();
        }
    }

    fn release(&self) {
        // Released while it leaves the composition: done already.
        if self.disposed.get() {
            return;
        }
        let pins = self.pins.get();
        assert!(pins > 0, "compose-foundation: a pinned item released more than it was pinned");
        self.pins.set(pins - 1);
        if pins == 1 {
            self.unpin();
        }
    }

    /// Gone from the composition, as `onDisposed`.
    fn on_disposed(&self) {
        self.disposed.set(true);
        if self.pins.replace(0) > 0 {
            self.unpin();
        }
    }
}

impl PinnableContainer for PinnableItem {
    fn pin(&self) -> PinnedHandle {
        assert!(
            !self.disposed.get(),
            "compose-foundation: pin called on an item that left the composition"
        );
        if self.pins.get() == 0 {
            self.list.pin(self.handle());
            let parent = self.parent.borrow().as_ref().map(|parent| parent.pin());
            *self.parent_handle.borrow_mut() = parent;
        }
        self.pins.set(self.pins.get() + 1);
        let this = self.this.clone();
        PinnedHandle::new(move || {
            if let Some(this) = this.upgrade() {
                this.release();
            }
        })
    }
}

/// `content`, item `index` of key `key` of a lazy layout, pinnable: what's in it pinning the
/// container (`LOCAL_PINNABLE_CONTAINER`, a focused node) puts the item in `pinned_item_list`
/// and pins the container around the layout, as Compose's `LazyLayoutPinnableItem`; it's out of
/// the list when the last pin is released, or when it leaves the composition.
#[composable(non_restartable)]
#[allow(non_snake_case)]
pub fn LazyLayoutPinnableItem(
    key: u64,
    index: usize,
    pinned_item_list: LazyLayoutPinnedItemList,
    content: impl Fn() + 'static,
) {
    let list = pinned_item_list.clone();
    let item = compose_runtime::remember_keyed((key, pinned_item_list), || {
        Rc::new_cyclic(|this| PinnableItem {
            this: this.clone(),
            key,
            index: Cell::new(index),
            list,
            pins: Cell::new(0),
            parent: RefCell::new(None),
            parent_handle: RefCell::new(None),
            disposed: Cell::new(false),
        })
    });
    item.index.set(index);
    item.set_parent(LOCAL_PINNABLE_CONTAINER.current());
    let disposed = item.clone();
    compose_runtime::DisposableEffect(item.clone(), move |scope| {
        scope.on_dispose(move || disposed.on_disposed())
    });
    let container: Rc<dyn PinnableContainer> = item;
    compose_runtime::CompositionLocalProvider(
        LOCAL_PINNABLE_CONTAINER.provides(Some(container)),
        move || content(),
    );
}
