# Movable content across compositions, without composing it again

A design, not built (D1511). It follows D1308, which moves content between compositions sharing an applier and composes all of it where it arrives, and it asks the coordinator to choose before building.

## What was measured

- **Compose's device tests pass already.** The four `AndroidInstrumentedMovableContentTests` are ported in `crates/compose-foundation/tests/suite/subcompose_layout.rs`. `AndroidMovableContentTests` was already there, though the ledger still had it waiting. Across a `SubcomposeLayout`'s edge, what's remembered stays. Of these tests, only `boxWithConstraintsAndIndirectContent` counts what the body does: 4 runs, for one composition and three moves.
- **The body runs again on every move between compositions.** `take_from_another` and `rehome_parked` mark what they take `transplanted`, and the restart of the adopting placeholder forces its whole subtree. Compose composes it again only if the locals differ.
  - In compose-rs, `BoxWithConstraints` provides `LOCAL_CONSTRAINTS_HINT`, so a move out of one re-runs the body under Compose's rule as well.
  - So the gap is a move between compositions whose locals are the same: a lazy list's or a `SubcomposeLayout`'s slot and the composition around it, when nothing is provided between them.
- **Dialogs and popups aren't other trees here.** `Dialog` and `Popup` (`compose-ui/src/window.rs`) compose in their caller's composition, as layers of its `UiTree`. Content moving into or out of one therefore moves within a composition: its state stays, and its body runs again only because the dialog provides other locals, as Compose's rule would do.
  - Other appliers exist only in separate hosts: `UiHost`'s own `Composition::new` and the desktop's windows.
  - No ported test moves content between appliers. Compose's `moveToDialog_elidedRememberObserver` is a move within one composition here.
- **`Group` has no spare room.** It's 144 bytes (the assertion in `slot_table.rs`). A `u32` field makes it 152, since a probe at 144 fails, so a per-group generation costs 8 bytes on every group.

## Why the body runs

Three kinds of handle name a place by table index. Moving a group to another table re-allocates it, so these handles go stale. Running the body again refreshes those held by nodes, because each node's composable runs and updates them.

| Handle | Holds | After a move to another table |
|---|---|---|
| `CompositionLocalAnchor` (`UiTree` nodes' `locals`, material3's ripple and text field, any app's) | composition, provider group, provider version | its check fails, since the version is that provider's but the group is now elsewhere, and it reads defaults |
| `CompositeKeyToken` (`UiTree` nodes' `composite_key`) | composition index, group | unchecked: it names a freed group, or one given to another composable since |
| The adopt's locals check (`force_if_moved`) | the provider the content last ran under | `Parked::provider` is `NONE` for a transplant, since the old provider is in the other table, so it forces |

Scopes and contexts already follow the move (`move_group`).

## The proposal

1. **Compare locals across compositions.**
   - At the transplant, before the groups move, take the visible entries at the parked group's provider in the source, which `entries_visible_from` already walks.
   - At the adopt, compare them with the target's, holder by holder (`Rc::ptr_eq`, as `same_locals_at` does within a composition).
   - If they're the same, don't force.
   - This runs once per transplant and costs nothing elsewhere.
2. **Forward stale handles.**
   - `move_group` records `(from, old group) → (to, new group)` in a runtime map, for provider groups and for every group of the moved content.
   - An anchor whose check fails looks its version up there. A provider's version is unique, so a hit is certain.
   - A token gets the group's `key` (16 bytes instead of 8). It's checked: not `Free`, same key. When the check fails, it follows the map.
   - A source index given to a new group with the same key would still be taken for the old one. That's rare (a token inside moved content whose old index is reused by the same composable), and its only effect is a wrong `rememberSaveable` key.
   - **Map lifetime:** stale entries are pruned when a lookup finds the target gone, and in a sweep at a frame's end when the map passed 1,024 entries.
   - **Cost:** nothing on a frame without a transplant. The map is consulted only after a check fails, which is on a cold path.
3. **Drop `transplanted` forcing.** The restart then goes through `force_if_moved` with the source's locals, as an in-composition move does.

**Estimate:** about 300 lines in the runtime (`movable.rs`, `local.rs`, `saveable.rs`, a map in `Runtime`). Nothing changes in compose-ui except that `CompositeKeyToken` grows. The tests are runtime ones: body counts across a shared-applier subcomposition, with locals the same and different; an anchor read after a move; a token's hash after a move and after the source reuses the index.

**Rejected:**
- A per-group generation for tokens: 8 bytes on every group of every app, to rule out the rare same-key reuse.
- Rewriting the handles by walking the moved nodes through a new `Applier` hook: it reaches `UiTree` nodes only, and leaves material3's anchors and an app's stale.

## Between appliers

Moving state into another applier needs a second node path. The moved groups keep their slots and scopes, but every node group under them emits a new node in the target applier, and the source's nodes are released there.
- This means the composer re-inserting node groups that it found, which is what `Parked::nodes` and the attach path assume never happens.
- Estimate: 400 lines or more, through `composer.rs`'s node groups.
- Nothing in compose-rs asks for it until content moves between two hosts' trees (desktop windows), since dialogs and popups share their caller's tree.
- **Proposed:** leave it, as D1308 did, and note it under the multi-window work.

## The choice to make

Build parts 1 to 3 (about 300 lines, no change to the default build's hot paths), or keep D1308's behaviour. Moves across compositions with the same locals would then stop running their bodies.
