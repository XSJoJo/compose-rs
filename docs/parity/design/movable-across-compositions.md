# Movable content across compositions, without composing it again

Built as proposed, but for the move between appliers (D1511, on the coordinator's choice). It revises D1308, which moved content between compositions sharing an applier and composed all of it where it arrived. What follows was written before building; where the code differs, D1511 says.

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
   - A token gets the group's `key` (16 bytes instead of 8; 12 as built, D1511 on review). It's checked: not `Free`, same key. When the check fails, it follows the map.
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

## As built

Parts 1 to 3, with the tests: `content_moved_across_compositions_under_the_same_locals_isnt_composed_again`, `..._under_other_locals_composes_again` and `anchors_and_key_tokens_in_moved_content_follow_it` in `crates/compose-runtime/tests/suite/movable_content.rs`. A token whose place is gone hashes as nothing (`None`), where it had panicked.

On review (D1511):
- **What's compared** is what the content last composed under: its nearest provider of any composition, kept in its movable group and read when it leaves the table it last composed in. The map where it was parked, which part 1 proposed, already holds a static value changed in the frame it moves.
- **Handles follow their groups, as Compose's `Anchor`s do** (the third review). Part 2's runtime map, keyed by place and then by departure (the second review), grew with every move and every removal, and each move rewrote it by a scan: 2,000 bounces of 100-node content kept 202,001 departures, the last 250 moves taking 18.8 s. It's gone, with the stays it counted and its sweep. What a handle finds its group by is now the group's own, moved with it by `move_group`, and gone with it:
  - **An anchor** (`CompositionLocalAnchor`, 16 bytes and `Copy` as before) names its provider's place cell, with the provider's version. Each provider's entries, its group's first slot, make a cell when they're made (a thread-local slab, `PLACES`, of a composition and a group each), free it when they're dropped with their group, and `move_group` rewrites it when the provider moves to another table, as Compose rewrites an `Anchor`'s location. An anchor reads its provider where its cell says if the cell is still the one it named (a generation) and the provider still at its version, defaults otherwise. One cell lives per provider alive, whatever moves. The cells are read and written through `try_with`, so a node's `on_detach` after the thread-local is gone reads defaults. Both handles are plain integers, so `Send`: on another thread they name something of that thread's, or nothing, wrongly but safely. As first built in this round the anchor held the entries weakly (`Weak<ProviderEntries>`), which made it a clone: `counter_click` cloned some 400 a click, drawing, and cost +1.98 % (spread 0.51 %); the cell keeps it `Copy`, a layout node and its draw scopes as they were.
  - **A key token** (`CompositeKeyToken`, 12 bytes, `Copy`) names a place and its group's identity. A node group's identity is its node's bits folded with its key, its composition's generation and its tree (a number per applier on the thread: node handles are unique only in their tree, every tree's starting alike), computed where it's asked, not kept: a layout node's emission pays the fold. Another group's is a number given the first time a token names it, kept in its table's named groups (`MovableState::named`, made at the first, the group flagged `NAMED`). A group moving to another table takes its identity there, a node group's as it was folded where it was; a group freed takes its own with it (`SlotTable::free`). A token is checked at its place first; if its group isn't there, the live compositions' named groups are looked through for its identity, which is where content that moved is. One entry lives per named group (one a token named, or a node group that moved), however often it moves, and none for content gone.
  - **Dead tokens name nothing:** the group gone took its identity, a group at its index since is another node or has another number, a composition at its composition's index has another generation, and another tree's node at its node's handle another tree. The exceptions are a 31-bit fold alike (once in 2^31, the price of 12 bytes) and a group `ReusableContent` reuses under another key, which lives on: its tokens read the reused content (missing.md).
  - **Measured** (tests in `movable_content.rs`): 100-node content bounced 10,000 times between two compositions keeps 101 named groups and as many provider places after its second move as after its last, and the fastest 500 moves of the last 2,500 take less than three times the fastest of the first (`what_handles_keep_stays_flat_over_ten_thousand_moves`); 1,000 contents moved and removed keep the same counts from the second cycle on (a content's provider place made before the last one's is let go), and every handle of content gone reads defaults or names nothing (`what_handles_keep_stays_flat_over_a_thousand_contents_moved_and_removed`). The reviewer's scenarios (bouncing while composing or not, compositions made anew, thousands of groups changing, nested content) check every handle made, and that a gone node's token names nothing.
- **Cost:** a layout node's token folds its node at its emission, and its anchor reads its provider's cell from the entries; nothing else outside moves. Against fc5282ad (before the second review), fat LTO, 5 rounds: `counter_click` −0.02 %, `first_frame_big` −0.29 %, `list_scroll` −1.40 %, `feed_step` −1.04 %, `field_keystroke_direct` −1.19 % retired instructions (spreads 0.41, 0.47, 0.27, 0.75, 0.34 %), display lists the same. None is above its spread; the last three are below: a read through a `CompositionLocalMap` or an anchor now makes its accessor scope (a clone) only for a computed value, which took `field_keystroke_direct` from −0.14 % to −1.19 %; `list_scroll`'s −1.26 % before that isn't explained here. The cost of the earlier rounds, for the record: as first built (16-byte token) `list_scroll` was +0.40 % against d0eef2b6, at 12 bytes −0.04 %; with departures (the second review) +0.27 % to +0.34 % against fc5282ad.
