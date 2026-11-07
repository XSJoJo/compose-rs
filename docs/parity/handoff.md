# Continuing the parity work

How the Compose parity work has been run, so someone with their own checkout can pick it up. The goal and the ledgers are in [README.md](README.md); the big gaps are in [missing.md](missing.md).

## The Compose reference

All the parity work compares against **androidx `frameworks/support` at commit `d153d87ed76372ece0c99dac837ff07ab845d495`** (2026-10-25, "Merge changes I51ab3288,I3969e236 into androidx-main"): the ledgers were made there and every port since read that tree. In a `repo` checkout of androidx-main the git repository is `frameworks/support`. To continue with the same reference:
```
git -C <androidx-main>/frameworks/support checkout d153d87ed76372ece0c99dac837ff07ab845d495
```
Moving to a newer androidx means re-checking the ledgers' test lists and `api/current.txt` against the new tip; record it as a decision. The checkout is read only: never build it or run anything in it that downloads.

## Where the state lives

- **What's done and what's left:** the ledgers in this folder (one row per Compose test file, with its status) and [missing.md](missing.md). They are kept current with every merge.
- **Why things are the way they are:** [docs/decisions.md](../decisions.md) indexes every decision (`D<number>`), with the text in `docs/decisions/<area>.md`. Decision numbers are allocated in blocks per area; before using a number, check it isn't used on `origin/main` or on any open branch (`git log --all -S'| D1234 |'`).
- **Work in flight:** the section [In flight](#in-flight) below, updated when work stops. Unmerged branches are pushed as `wip/<name>`.

## How a piece of work goes

1. **Build** on a branch from `origin/main`, test first (TDD). Compose's tests are rewritten by hand in Rust, never translated by script. The Compose source to compare against is androidx-main's `frameworks/support/compose` (read-only: never build it).
2. **Record** each choice that differs from Compose, or that a later reader would question, as a decision; update the ledger rows and missing.md.
3. **Check:** `cargo test --workspace --no-fail-fast`, the feature-off build `cargo test --workspace --exclude compose-runtime --no-fail-fast`, `cargo clippy --workspace --all-targets` (no warnings), `cargo fmt --check`.
4. **Review** adversarially, by someone (or an agent) who didn't write it, bounded to about two hours. The reviewer compares with Compose, tries hand mutants, writes tests for what they find on a `review-<branch>` branch (failing ones `#[ignore = "review: <bug>"]`), and checks no existing test was weakened:
   ```
   git diff --no-color <base> <tip> -- ':(glob)crates/*/tests/**' | grep '^-[^-]'
   ```
   (`-- 'crates/*/tests'` matches nothing; use the glob form.) Blockers are fixed before merging; should-fix findings may merge with their ignored tests and be fixed next.
5. **Merge** with [`tools/parity/check_and_push.sh`](../../tools/parity/check_and_push.sh) `<sha>...` on `main`: it merges, resolves `docs/decisions.md` conflicts (`resolve_decisions.py` keeps both sides' rows and recounts), runs all the checks and pushes only if all pass. Add `--gradle` when Android code changed. After a runtime change, also run the feature-off build.

## Cost and safety checks

- **Cost:** retired instructions, which stay stable while other builds run (timings don't):
  ```
  CARGO_PROFILE_RELEASE_LTO=fat CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
    python3 benchmarks/compare_instructions.py <base> list_scroll:3000 first_frame_list_items:100 --rounds 5
  ```
  and separately with `--dumps` to compare the display lists. Iteration counts are in the script's SCENARIOS table. A change past its spread needs a reason in its decision.
- **Miri**, when compose-ui's tree code or anything `unsafe` changes: the slice listed in [docs/09-testing.md](../09-testing.md), under Stacked Borrows and again with `-Zmiri-tree-borrows`. Keep `unsafe` to a minimum.
- **Disk:** builds per worktree are large. Use `CARGO_INCREMENTAL=0` in worktrees, and delete a worktree's `target/` when its branch is merged.
- **Ledger totals:** `python3 tools/parity/ledger_count.py docs/parity/<ledger>.md` recounts statuses from the rows.

## Ignored tests

Every `#[ignore]` carries its reason in its attribute; `grep -rn -A1 '#\[ignore' crates benchmarks` lists them, and `cargo test -p <crate> --test suite <name> -- --ignored` runs one. A test is ignored only for one of the reasons below; a bug found in review gets `#[ignore = "review: <bug>"]` and a fix on the next branch. On `main` there are 31 (review findings fixed since: lazy beyond-bounds layouts, D1480; a `GraphicsLayer`'s readers and its first drawing's send, D1339; a container's toolbar after a cancelled handle press or a text changed in place, D2045, D2046):

| Tests | Why ignored | To make them pass |
|---|---|---|
| `text_field_toolbar.rs`: `a_long_press_on_an_empty_field_shows_the_toolbar_but_no_handle`, `an_editable_field_s_toolbar_has_autofill`, `the_cursor_s_toolbar_has_autofill`; `text_field_context_menu.rs` and `text_field_text_context_menu_builder.rs`: the three `btf2_contextMenu_*_noSelection_itemsMatch_afterApi26` each (compose-foundation, 9 in all) | Compose's text toolbar and context menu offer an "Autofill" item; compose-rs has no autofill, so it offers none (D2035). An empty field's toolbar is then empty and doesn't show; a ported gesture test works around it by putting a clip on the clipboard first (D2043). | Build autofill (the `autofill` feature in [ui-input](ui-input.md), about 166 tests: `ContentType`, `ContentDataType`, the semantics, Android's `AutofillManager` bridge), add its item to `TextContextMenuItems` as Compose's `TextFieldSelectionManager`/`TextFieldTextContextMenuBuilder` do (shown when the field is editable and has no selection, API 26+), then remove the ignores and D2043's clipboard workaround. |
| compose-animation `shared_transition.rs`: `detaching_shared_element_and_reattaching_in_new_position_before_animating`; compose-foundation `review_shared_elements_2.rs`: `moved_content_detaches_and_attaches_its_modifier_nodes` | Written before moved nodes were detached and attached (D1923, now on main). | The second passes since D1923 and only needs its ignore removed. The first still fails: D1923 runs a moved node's `on_detach` after the node is already under its new parent, so a shared node's `bounds_before_detached` reads its position through the new parent ((0,0) instead of (200,100)) and the transition doesn't start from where it was. Compose detaches in `removeAt`, while the node is still under its old parent. Fix: record a moved node's position in root at removal, or resolve a detaching moved node's coordinates through the parent it left. Being fixed on `moved-node-fixes`. |
| compose-ui `review_moved_node_attach.rs` (6) | A review of D1923 found: content taken out of movable content that was itself removed (or is moving and placed after it) is never detached nor attached (`Applier::remove` flags a node only when its parent is still attached); siblings removed by one `remove` are detached first to last, and contents moved into a new layout in reverse tree order, where Compose's `removeAt` goes last to first and in the order taken out; a panic in a moved node's `on_detach` leaves `detaching_moved` set (no drop guard). | Being fixed on `moved-node-fixes`. If lost: also flag a node removed from a detached parent whose entries are still attached; queue a removal's `Detach` steps last to first; pre-detach in `attach_subtree` in removal order; restore `detaching_moved` (and reset `removed_parents`/`removed_detached`) with a drop guard. |
| compose-foundation `review_style_api.rs` (6) | A review of the Style API (D1580–D1586) found: a resolver's first collection animates animated values in from their defaults (Compose shows them at once: `previous` is null); a child collected before its parent animates the parent's nested style in; a lone `left`/`top` offset grows the placement node (Compose's NaN side rounds to 0); a property named as a predefined one is the same property (identity by name); a Press clears a focus set by hand (Compose sets only the interaction's flag); a child advertises an ancestor's other nested keys. | Being fixed on `style-api-fixes`. If lost: skip animations on a resolver's first collection, and treat a recollection before the first frame as first; add `left + right` with NaN as Compose does; namespace the predefined names or identify statics by address; set only the interaction's flag in `listen`; record nested keys only when `nested_style_key` is `None`. |
| `lazy_layout.rs`: `an_item_removed_and_added_back_reuses_its_node` | Ignored in Compose too (b/369188686): a node reactivated for its key is measured again. | Un-ignore when Compose fixes it upstream and the fix is ported. |
| compose `review_c6.rs`: `a_call_macro_imported_under_another_name_calls_the_function_it_names`; `review_c4.rs`: `a_name_before_equals_in_a_user_pascal_case_macro_is_a_capture` | Accepted limitations of the `#[composable]` macro (D114: a call macro calls by its declared name; D98: a named argument's name is not a use). Kept to document them. | Not planned; they would need name resolution the macro can't do. |
| `perf_review2.rs`, compose-android `retained_groups.rs` (`timing_one_path_reset_for_many_shapes`), compose-ui `mutable_paths.rs` (`timing_idle_frames_with_many_path_drawings`), `benchmarks/tests/layer_cost.rs` | Not failures: timing tools run by hand (`cargo test --release -- --ignored --nocapture`), too slow or noisy for the suite. | Nothing to do. |

`#[cfg_attr(miri, ignore = "spawns a process")]` marks tests Miri can't run; they run in the normal suite.

## Priorities

Core modules (runtime, ui, foundation, animation) before the big Material 3 components. Jank and frame times matter most for Android performance (docs/12).

Never merge the `ime-logging` branch (it records everything typed) or `parked/nav-compose-extras`.

## In flight

As of 2026-11-07, `origin/main` is at dcbf5590 (12,895 tests).

| Branch | What | State |
|---|---|---|
| `moved-node-fixes` | The moved-node review's findings (nested movable content, detach orders, a panic guard) and a moved shared element's start position | In progress |
| `multi-paragraph-2` | **Merged without a review** (the weekly quota was short): the paragraph API review's findings (D2049), `fillPathForRange` (D2050), the constructors and `ParagraphIntrinsics` (D2051), `paint` with a gradient spanning paragraphs (D2052). Worth a review: D2049's locale-direction table, D2052's paint (it panics on a paragraph that only answers queries, such as `BasicText`'s `on_text_layout` result). Next: `BasicText` and the fields laying out paragraph styles as a `MultiParagraph` (D2053 and up), then AndroidParagraphTest (115) | Merged, review pending |
| `autofill`, then `autofill-2` | Compose's autofill (D1680–D1687): `ContentType`, semantics, `AutofillManager`, text fields, the toolbar's Autofill item (un-ignores 9 tests), Android's bridge. **Held off main by a review blocker:** on Android the manager walks the whole tree on every frame that recomposes or measures (~1,400 instructions per node; feed scroll frames +270 %), even with no autofill service. `autofill-2` (from `review-autofill` 89b34db2, main merged in) tracks only changed nodes and gates on `AutofillManager.isEnabled()`, fixes a removed focused field never exiting and a stale selection-handle semantics bug that is on main too | In progress |
| `style-api-fixes` | The Style API's review findings: a first collection animating in, a lone offset growing the node, names colliding, a press clearing a hand-set focus, nested keys; predefined property names namespaced (D1588 and up) | In progress |

Next, once those land: `DeferredSharedElementTest`, the moved shared element that doesn't animate from its old place, and the shared elements of `LookaheadScopeTest` and Navigation 3; touch selection's bidi tests (`TextSelectionGesturesBidiTest`); graphics leftovers (`ImageShader`, BrushPainter, FilterQuality); place-with-layer leftovers (zIndex, hit testing through layer transforms); style API, border parity, AndroidView interop, magnifier, autofill. missing.md has the full list.
