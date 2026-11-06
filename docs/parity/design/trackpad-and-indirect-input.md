# Trackpad gestures, indirect pointers and pointer icons

A short design note for three pieces of Compose's input that compose-rs lacked: the trackpad's pans and pinches with ui-test's `TrackpadInjectionScope`, indirect pointer input (`IndirectPointerEvent`) with its injection, and pointer icons. Decisions: D1674 on.

## 1. Trackpad pans and pinches (D1674)

**What Compose has.** `PointerEventType` has `PanStart`, `PanMove`, `PanEnd`, `ScaleStart`, `ScaleChange` and `ScaleEnd`; a change carries `panOffset` and `scaleFactor`. On Android 14 and later a trackpad's two-finger swipe arrives as a fake finger classified `TWO_FINGER_SWIPE` and a pinch as two fake fingers classified `PINCH`; `MotionEventAdapter` reinterprets them as one pointer at the cursor, not pressed, of the mouse's type, and `PointerEvent.type` names the pan or pinch event. `HitPathTracker` treats a pan's pointer as hovering, suppresses new hit tests while it lasts (what scrolls away from under the cursor exits, nothing new enters), and a pinch's fingers don't hover. ui-test's `TrackpadInjectionScope` shares the cursor with the mouse (`CursorInputState`, `currentInputSource`) and adds `panStart`/`panMoveBy`/`panEnd`, `scaleStart`/`scaleChangeBy`/`scaleEnd` and their gestures (`pan`, `panWithVelocity`, `scale`).

**compose-rs.**
- `PointerEventType` gains the six types (`is_pan`, `is_scale`), `input::PointerEvent` gains `pan_offset` and `scale_factor` (`PointerEvent::pan`, `PointerEvent::scale`), and the dispatch copies them into the change. A host sends them as the mouse's pointer at the cursor.
- The dispatch: a `PanStart` or `ScaleStart` captures its path as a press does; the moves and the end go there, the end releasing it. A pan hovers: its hover is the captured path less what's no longer under the cursor (`still_under`), and so is a hover refresh after a layout while a pan lasts (`UiTree::panning`). A pinch doesn't touch the hover.
- **A hover move that brings the cursor over a node is that node's `Enter`, not an `Enter` and the move**, as `HitPathTracker` retypes it. compose-rs delivered both; the trackpad's and the mouse's move tests are Compose's sequences with it.
- compose-test: the cursor is one, its source (`CursorSource`: mouse or trackpad) as Compose's; the mouse's and the trackpad's buttons, moves, enter, exit and cancel are one code (`cursor_*`), with their own messages. A pan or pinch is what a node hears in Compose on Android 14 and later: buttons held are released first (a release, then a hover move), then `PanStart`, `PanMove`s whose `pan_offset` is the scroll's distance (the fingers' move turned around, as Android's `AXIS_GESTURE_SCROLL_*_DISTANCE`), `PanEnd`, then a hover move; a pinch is `ScaleStart`, a `ScaleChange` of 1 for the second finger down, the changes, one of 1 for that finger up, `ScaleEnd`, then a hover move. Outside the window nothing is sent, the gesture still in progress.

**Where compose-rs differs, kept.**
- A press from a cursor not hovering comes over the node with it (an `Enter` with the press's buttons, then the press); Compose's trackpad has the press alone and the `Enter` after the release. compose-rs's dispatch updates the hover on a press (its mouse's tests rely on it); changing that is the hover-on-press work in pointer-event-model.
- A pressed drag crossing a node's edge is pressed moves, not a pressed `Exit` and `Enter`, as the mouse's (D1657's ledger notes).
- A second button pressed or released while one is held is its own `Press` or `Release` event (as the mouse's, D1657); Compose's says `Move`.
- After a pinch, Compose's node hears `Enter` (its exit lost on the way, b/461873914); compose-rs's cursor never stopped hovering, so it's a move.
- Android's own fields of those events (`classification`, `AXIS_GESTURE_*`) have no counterpart: there's no `MotionEvent`.
- The hosts don't send trackpad gestures yet: Android needs the classification across JNI, the desktop winit's pinch and its pixel deltas. foundation's `TrackpadScrollingLogic` and `transformable`'s pans and scales are foundation's.

## 2. Indirect pointer input (planned)

`IndirectPointerEvent` (`changes`, `type`, `primaryDirectionalMotionAxis`), `IndirectPointerInputChange` (Compose's fields, `consume`, `copy`), `IndirectPointerInputModifierNode` (`onIndirectPointerEvent(event, pass)`, `onCancelIndirectPointerInput`). Compose's `FocusOwnerImpl.dispatchIndirectPointerEvent` sends one to the focused target's indirect node (its last local one, else the nearest ancestor's), and that node's ancestors of the kind: `Initial` from the outermost in, `Main` back out, `Final` in again; a focus change cancels the focused one's stream. In compose-rs: a node kind and `ModifierNode::as_indirect_pointer_input`, dispatched by the tree along the key path's nodes (`key_path`), `UiHost::send_indirect_pointer` for hosts and tests (Compose's `RootForTest.sendIndirectPointerEvent`), and compose-test's `perform_indirect_pointer_input` with `IndirectPointerInjectionScope` (the touch scope's gestures in the input device's coordinates). Android's touch-navigation (`IndirectPointerNavigationGestureDetector`, the primary axis from the device's motion ranges) is the host's.

## 3. Pointer icons (planned, if small)

`PointerIcon` as Compose's (`Default`, `Crosshair`, `Text`, `Hand`), `Modifier.pointer_hover_icon(icon, override_descendants)`: the innermost hovered node's icon wins unless an ancestor overrides its descendants, as Compose's `PointerHoverIconModifierNode` traversal; the hosts set the cursor from `UiHost::pointer_icon` as today.
