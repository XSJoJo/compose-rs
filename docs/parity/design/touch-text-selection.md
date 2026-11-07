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

**A frame behind.** The node writes the states during a measurement, so a handle comes or goes in the frame after the measurement that decides it: its popup is composed by the next recomposition, as Compose's handles follow a `derivedStateOf` the measurement's writes invalidate. Its position is read by the popup's position provider after the layout that wrote it, so a handle already shown moves in the same frame.

**Scrolling.** A state-based field scrolls by a remembered `ScrollState` unless given one, as Compose's `rememberScrollState()` default: its scroll follows the selection only while the cursor shows, so a tap or a long press lands under the scroll the user sees, and the viewport is the whole field, the text the content (D2017).

**Visible bounds.** A handle hides when its point leaves the field's bounds clipped by its ancestors (Compose's `visibleBounds()`): while handles may show, a node of no size in the inner field follows those bounds after each layout and measures the field again when they change, a frame behind as Compose's (D2011). **Its node made anew.** `TouchHandles` is the composable's and outlives a node its decoration makes anew: a node detached or reset takes back what it told (D2011).

## Gestures: in the existing nodes

**Decision: the field's node and the container's node detect the touch gestures themselves, in their pointer input, as they detect the mouse's now; the handles' nodes detect their own drags.** Rejected: Compose's coroutine detectors (`detectDragGesturesAfterLongPress`, `detectTapAndPress`) as suspending pointer input. compose-rs has them for applications, but the field's node is one node with its own state machine for presses (D1047, D96) and a second detector beside it would race it for the same events.

- **Long press.** A finger held past `long_press_timeout` without moving past the touch slop: the field selects the word under it, or puts the cursor at the end of an empty region (Compose's `isPositionOnText`), and drags after it extend by words, never reversing (`TextFieldTextDragObserver`). The press's timer is a delayed task the node starts at the press, as `clickable`'s long click does.
- **Double tap.** A finger's second tap starts the long press's flow at once, as Compose's `touchSelectionSubsequentPress`: the word under it, no handle while it drags, its drag past the touch slop, or at once when held past the long-press timeout (D2018, D2021).
- **Handles.** A handle's drag goes past the touch slop, then each move asks the field to move that end to where the finger is: the position at the drag's start in the text, plus the finger's movement in the window, plus how far the field scrolled since (Compose's `handleDragPosition`, which adds the text layout's movement). The handle's node writes the drag into `TouchHandles` and asks the field's node to measure again; the field resolves the offset with its paragraph there, as it does a tap (`pending_tap`). A selection handle's drag is `CharacterWithWordAccelerate`; the cursor's moves by characters.
- **Touch or not.** The node keeps Compose's `isDirectTouchInteraction`: true at first, false when the field is focused in keyboard input mode, set by each pointer event (a mouse's or a touchpad's: false), false after a hardware key. Handles show only while it's true.

## One logic for both fields

Both fields are one node, so the gestures, the handles' visibility and their positions are written once, over the node's text, cursor and anchor, in the shown text's offsets (the output or visual transformation mapped as for the mouse). The state-based field's edits go through its `StateLink` as every other edit of the node: a selection set by a handle's drag goes through the input transformation and into the state at the end of the measurement that resolved it, as a tap's does. What differs between them is only what Compose's two fields do differently, and the deprecated value-based field's tests aren't ported (its rows are `n/a`).

## The container

`SelectionContainerNode` gains the same long press (a word of the text under the finger, `Unit::Word`; on a text only, as Compose's long press is each text's), its drag by words, a finger's double tap selecting a word at once (D2020), and a finger's tap clearing the selection; a mouse selects as before (D2013). It keeps Compose's `isInTouchMode`, set by each pointer event's type. A long press, once it fires, takes its moves in the initial pass: Compose's texts take the drag ahead of a scrolling column or lazy list inside the container (D2016).

**Its handles.** Foundation's `SelectionContainer` remembers a `ContainerHandles` (compose-ui's: the tree holds the selection) and composes the start and end handles after its content while it says they show (touch mode, the focus within, a selection not trivially collapsed). The tree writes which show and where after each selection change, and after a layout or a graphics layer's move that moved the container or the texts of the selection's ends (`take_layout_callbacks`: nothing without a selection, D2016): each end's text says where (`Selectable::handle_position`, Compose's `getHandlePosition`), in the container's coordinates, hidden (`None`, Compose's `Offset.Unspecified`) outside the container's visible bounds (clipped by the clips around it and the window, Compose's `visibleBounds()`) unless it's dragged. A hidden handle stays composed, not drawn, its semantics not visible. Its drag pushes a selection request the tree resolves with the container's texts, by `CharacterWithWordAccelerate` across them (D2020). The popups are the field's, asking a `HandleSource` (a field's `TouchHandles`, a container's handles, or an offset provider for Compose's `SelectionHandle` in tests) where they point; a container's are 25 dp, a field's 40 (Compose's `minTouchTargetSize` only for fields).

**A text's touch target.** A long press takes the text it's on, else the nearest whose minimum touch target holds it, put in its bounds, the end then following the finger; only within the container, whose node takes the touches (D2029).

**Where a drag lands.** As Compose's `SelectionLayoutBuilder`: the texts in order, each before, on or after the drag's place, those both ends are past left out, the end on the first text it's on or in the gap before the first it's before, a gap's end in the text on the other end's side; the start adjusted again at each move from the selection as it is (D2028).

**Auto-scroll.** Each drag's move asks the scrolling ancestors of the text under it to show the drag's place (in touch mode with the cursor's place at its end's offset, D2030) (stretched to the text's nearest point), and the texts holding a part of the selection pin their lazy items, which a lazy list places beside those laid out, before the first and after the last, in order (D2023, D2024); a text leaving lets go of its pin at once (D2024).

**Haptics.** A change of the selection in touch mode, its clearing included, performs `TextHandleMove`, with the `LOCAL_HAPTIC_FEEDBACK` read where the container is composed, as Compose's `SelectionManager`.

## A field's haptics

As Compose's `TextFieldSelectionState`: `LongPress` when a long press or a later tap of a multi-tap starts a selection, `TextHandleMove` at each move of a long press's drag or a handle's that changes the selection's ends; a mouse's gestures feel nothing. The field resolves those gestures at its measurement, so it performs them at its end, reading `LOCAL_HAPTIC_FEEDBACK` where it's composed only when it has some: nothing in composition (D2027).

## Hooks left for the toolbar and the magnifier

- **Toolbar.** The node keeps Compose's `TextToolbarState` (`None`, `Cursor`, `Selection`) as it goes: a tap on the cursor handle toggles `Cursor`, a long press sets `Selection`, an edit or a tap sets `None`. Nothing reads it until `text-context-menu`.
- **Magnifier.** `TouchHandles` keeps the dragged handle and its position in the text (Compose's `draggingHandle` and `handleDragPosition`), which the magnifier will follow.

## Costs

- A field: one `remember` and one state read in composition, and an empty group for the handles; a branch at the end of its measurement. No state written, no popup, no allocation past the first composition, unless a finger touches it or it's focused in touch mode.
- A text outside a container: nothing. A container: its handles' composable and the haptics' composition local, read once; after each layout, a check that no selection exists; with one, a comparison of where the container and its ends' texts are with where they were (D2016).
- Checked with `compare_instructions.py`: `field_keystroke`, `field_keystroke_direct` and `field_focus` within their spread.

## Steps

1. The handles' infrastructure, the cursor handle of a field (`TextFieldCursorHandleTest`).
2. The selection handles of a field, long press and drags (`TextFieldSelectionHandlesTest`, `TextFieldLongPressTest`, `TextFieldScrolledSelectionGestureTest`, the handle cases of `TextFieldDoubleTapTest`).
3. The container's long press and handles (`SelectionHandlesTest`, `SelectionHandlePopupPositionTest`, the touch parts of `SelectionContainerTest` and `SelectionContainerFocusTest`).
4. `MultiWidgetSelectionDelegateTest` and `SelectionManagerGetSelectedRegionRectTest`, through the `Selectable`'s handle positions and bounds: `Selectable` answers what Compose's `MultiWidgetSelectionDelegate` does (boxes, lines, the last visible offset, line heights), and the tree gives the selected region's bounds for the toolbar (D2014).
