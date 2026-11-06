# Touch text selection

Design note, written before the code, from Compose's `TextFieldSelectionState`, `BasicTextField`'s `TextFieldCursorHandle` and `TextFieldSelectionHandles`, `AndroidSelectionHandles.android.kt`, `AndroidCursorHandle.android.kt`, `SelectionManager` and their tests. It covers [`touch-text-selection`](../foundation-text-misc.md#touch-text-selection): the cursor and selection handles, a finger's long press and drags, in fields of either kind and in `SelectionContainer`. The text toolbar (`text-context-menu`) and the magnifier (`magnifier`) are other features: this one leaves them a place (below) and builds neither.

## Where it starts

- **Fields.** compose-rs has one field node for both fields: the value-based `BasicTextField` and the state-based `BasicTextFieldOfState` build the same `TextFieldNode`, the latter tied to its `TextFieldState` by a `StateLink` (D1130). The node already takes a mouse's presses, drags, double and triple clicks, and a finger's tap and double tap (D1047: a finger's first press only taps).
- **Containers.** `SelectionContainer` is compose-ui's `SelectionContainerNode` (`text_selection.rs`): the tree finds its `Selectable` texts and holds one `TextSelection`; presses, drags and multi-clicks come from the mouse, and a finger's press acts as a mouse's.
- **Popups.** `Popup` and `PopupWithPositionProvider` are overlays (`overlay.rs`): a node composed where the popup is, positioned after each layout from its anchor's bounds (the layout around it), drawn over the rest and hit first, with semantics of its own.

## Handles: popups, as Compose's

**Decision: each handle is a popup composed inside the field (or the container), as Compose's `HandlePopup`.** Its node is 40 dp square at least (Compose's `MinTouchTargetSizeForHandles`, the handle's 25 dp drawn at its side), carries `SelectionHandleInfo` in its semantics (which handle, the position it points at in the text's coordinates, its anchor, whether visible), draws the handle's shape in `LOCAL_TEXT_SELECTION_COLORS`' `handle_color`, and takes the drags of its handle.

Rejected: drawing the handles from the field's node, outside its bounds, with a hit test of its own. It would need a tree feature to hit beyond a node's bounds, and the handles would have no semantics nodes: Compose's tests find them by `SelectionHandleInfo` on nodes of their own and touch them by their bounds, and accessibility services see them as popup windows. Popups have all of that already.

**What composes them.** The field's composable remembers a `TouchHandles`, a small shared object, and gives it to both the field's element and a `TextFieldHandles` composable inside the inner field's layout (where Compose composes them: a decoration that never calls the inner field shows none). `TouchHandles` holds two snapshot states the node writes:
- **which handles show** (the cursor's; the start's and the end's, with which way each points): composition reads it, so a handle comes and goes by recomposition, as Compose's `derivedStateOf { …visible }`;
- **where they point**, in the text's coordinates and in the inner field's (scrolled): the handle's semantics and the popup's position provider read it, not composition, so a moving handle doesn't recompose, as Compose's `offsetProvider`.

The node writes them at the end of its measurement, where its cursor and selection rects are refreshed, only while handles may show (touch interaction and focus); once none show it stops. A field never touched by a finger writes nothing.

## Gestures: in the existing nodes

**Decision: the field's node and the container's node detect the touch gestures themselves, in their pointer input, as they detect the mouse's now; the handles' nodes detect their own drags.** Rejected: Compose's coroutine detectors (`detectDragGesturesAfterLongPress`, `detectTapAndPress`) as suspending pointer input. compose-rs has them for applications, but the field's node is one node with its own state machine for presses (D1047, D96) and a second detector beside it would race it for the same events.

- **Long press.** A finger held past `long_press_timeout` without moving past the touch slop: the field selects the word under it, or puts the cursor at the end of an empty region (Compose's `isPositionOnText`), and drags after it extend by words, never reversing (`TextFieldTextDragObserver`). The press's timer is a delayed task the node starts at the press, as `clickable`'s long click does.
- **Handles.** A handle's drag goes past the touch slop, then each move asks the field to move that end to where the finger is: the position at the drag's start in the text, plus the finger's movement in the window, plus how far the field scrolled since (Compose's `handleDragPosition`, which adds the text layout's movement). The handle's node writes the drag into `TouchHandles` and asks the field's node to measure again; the field resolves the offset with its paragraph there, as it does a tap (`pending_tap`). A selection handle's drag is `CharacterWithWordAccelerate`; the cursor's moves by characters.
- **Touch or not.** The node keeps Compose's `isDirectTouchInteraction`: true at first, false when the field is focused in keyboard input mode, set by each pointer event (a mouse's or a touchpad's: false), false after a hardware key. Handles show only while it's true.

## One logic for both fields

Both fields are one node, so the gestures, the handles' visibility and their positions are written once, over the node's text, cursor and anchor, in the shown text's offsets (the output or visual transformation mapped as for the mouse). The state-based field's edits go through its `StateLink` as every other edit of the node: a selection set by a handle's drag goes through the input transformation and into the state at the end of the measurement that resolved it, as a tap's does. What differs between them is only what Compose's two fields do differently, and the deprecated value-based field's tests aren't ported (its rows are `n/a`).

## The container

`SelectionContainerNode` gains the same long press (a word of the text under the finger, `Unit::Word`) and the container composes its own `TouchHandles` popups, start and end, positioned by the tree's `TextSelection` through the `Selectable`s' handle positions (a new `Selectable::handle_position`, Compose's `getHandlePosition`). The container's handles come after the field's: the field's tests are most of the feature's.

## Hooks left for the toolbar and the magnifier

- **Toolbar.** The node keeps Compose's `TextToolbarState` (`None`, `Cursor`, `Selection`) as it goes: a tap on the cursor handle toggles `Cursor`, a long press sets `Selection`, an edit or a tap sets `None`. Nothing reads it until `text-context-menu`.
- **Magnifier.** `TouchHandles` keeps the dragged handle and its position in the text (Compose's `draggingHandle` and `handleDragPosition`), which the magnifier will follow.

## Costs

- A field: one `remember` and one state read in composition, and an empty group for the handles; a branch at the end of its measurement. No state written, no popup, no allocation past the first composition, unless a finger touches it or it's focused in touch mode.
- A text outside a container: nothing. A container: its handles' composable, read once.
- Checked with `compare_instructions.py`: `field_keystroke`, `field_keystroke_direct` and `field_focus` within their spread.

## Steps

1. The handles' infrastructure, the cursor handle of a field (`TextFieldCursorHandleTest`).
2. The selection handles of a field, long press and drags (`TextFieldSelectionHandlesTest`, `TextFieldLongPressTest`, `TextFieldScrolledSelectionGestureTest`, the handle cases of `TextFieldDoubleTapTest`).
3. The container's long press and handles (`SelectionHandlesTest`, `SelectionHandlePopupPositionTest`, the touch parts of `SelectionContainerTest` and `SelectionContainerFocusTest`).
4. `MultiWidgetSelectionDelegateTest` and `SelectionManagerGetSelectedRegionRectTest`, through the `Selectable`'s handle positions and bounds.
