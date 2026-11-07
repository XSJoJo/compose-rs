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

Every `#[ignore]` carries its reason in its attribute; `grep -rn -A1 '#\[ignore' crates benchmarks` lists them, and `cargo test -p <crate> --test suite <name> -- --ignored` runs one. A test is ignored only for one of the reasons below; a bug found in review gets `#[ignore = "review: <bug>"]` and a fix on the next branch. On `main` at 251a0f46 there are 23:

| Tests | Why ignored | To make them pass |
|---|---|---|
| `text_field_toolbar.rs`: `a_long_press_on_an_empty_field_shows_the_toolbar_but_no_handle`, `an_editable_field_s_toolbar_has_autofill`, `the_cursor_s_toolbar_has_autofill`; `text_field_context_menu.rs` and `text_field_text_context_menu_builder.rs`: the three `btf2_contextMenu_*_noSelection_itemsMatch_afterApi26` each (compose-foundation, 9 in all) | Compose's text toolbar and context menu offer an "Autofill" item; compose-rs has no autofill, so it offers none (D2035). An empty field's toolbar is then empty and doesn't show; a ported gesture test works around it by putting a clip on the clipboard first (D2043). | Build autofill (the `autofill` feature in [ui-input](ui-input.md), about 166 tests: `ContentType`, `ContentDataType`, the semantics, Android's `AutofillManager` bridge), add its item to `TextContextMenuItems` as Compose's `TextFieldSelectionManager`/`TextFieldTextContextMenuBuilder` do (shown when the field is editable and has no selection, API 26+), then remove the ignores and D2043's clipboard workaround. |
| `review_lazy_nested_scroll.rs` (compose-foundation, 4) | A review of D1479 found: a grid's and a staggered grid's two-viewport limit counts items, not lines; a `BeyondBoundsLayout` called from an event handler never sees the item beyond (its remeasure is deferred); one found before user scrolling went off still lays out items. | Being fixed on `lazy-beyond-bounds-fixes` (D1480). If that branch is lost: the limit must use Compose's `visibleLinesAverageMainAxisSize` for grids and the plain average for staggered grids; the remeasure must be synchronous from event handlers as Compose's `remeasureSync`, or the block told `false` once; the guard must be checked at each call, not only at lookup. |
| `review_text_context_menu_4.rs` (compose-foundation, 2) | A review of D2041 found: a handle press cancelled before the touch slop leaves a `SelectionContainer`'s toolbar hidden (Compose's `onCancel` shows it again); a selected text changed in place keeps the toolbar shown (Compose's `onSelectableChangeCallback` releases the selection). | Being fixed on `text-context-menu-5`. If lost: call the toolbar's show on a cancelled handle press (`selection_handles.rs`, `HandleNode::on_cancel_pointer_input`) and tell the selection tree when a selected text's content changes (`text.rs` bumps its version without telling it). |
| `lazy_layout.rs`: `an_item_removed_and_added_back_reuses_its_node` | Ignored in Compose too (b/369188686): a node reactivated for its key is measured again. | Un-ignore when Compose fixes it upstream and the fix is ported. |
| compose `review_c6.rs`: `a_call_macro_imported_under_another_name_calls_the_function_it_names`; `review_c4.rs`: `a_name_before_equals_in_a_user_pascal_case_macro_is_a_capture` | Accepted limitations of the `#[composable]` macro (D114: a call macro calls by its declared name; D98: a named argument's name is not a use). Kept to document them. | Not planned; they would need name resolution the macro can't do. |
| `perf_review2.rs`, compose-android `retained_groups.rs` (`timing_one_path_reset_for_many_shapes`), compose-ui `mutable_paths.rs` (`timing_idle_frames_with_many_path_drawings`), `benchmarks/tests/layer_cost.rs` | Not failures: timing tools run by hand (`cargo test --release -- --ignored --nocapture`), too slow or noisy for the suite. | Nothing to do. |

`#[cfg_attr(miri, ignore = "spawns a process")]` marks tests Miri can't run; they run in the normal suite.

Branches in flight add their own (see [In flight](#in-flight)):
- `shared-elements-3`: `testDetachingSharedElementAndReattachingInNewPositionBeforeAnimating` (shared_transition.rs) and the review's `moved_content_detaches_and_attaches_its_modifier_nodes` wait on moved nodes being detached and attached (`moved-node-attach`, D1923). With that merged the second passes; the first still fails: a moved shared element records (0, 0) during its transition instead of animating from where it was, a shared-element gap still to diagnose (compare Compose's `SharedBoundsNode` on detach and attach). Its older `lazy_staggered_grid_jumps.rs` ignores are fixed on main (D1477) and go on merge.
- `review-moved-node-attach`: 6 review findings on D1923's ordering (siblings removed together and two moved contents detached first to last where Compose's `removeAt` goes last to first; content taken out of removed or moving content never detached and attached; a panic in `on_detach` leaving the old parent set). To fix on that branch before merging.

## Priorities

Core modules (runtime, ui, foundation, animation) before the big Material 3 components. Jank and frame times matter most for Android performance (docs/12).

Never merge the `ime-logging` branch (it records everything typed) or `parked/nav-compose-extras`.

## In flight

As of 2026-11-07, `origin/main` is at 97e0f32e (12,371 tests).

| Branch | What | State |
|---|---|---|
| `moved-node-attach` | A node movable content moves is detached and attached, as in Compose (D1923) | Reviewed (`review-moved-node-attach`); findings pending |
| `shared-elements-3` | Shared elements: all of SharedTransitionTest (D1332–D1336), layers told per drawing (D1337), first compositions send their writes (D1338) | Under review |
| `text-context-menu-5` | Context-menu follow-ups: stale menu items, a container toolbar after a cancelled handle press or a changed text | In progress |
| `lazy-beyond-bounds-fixes` | Lazy BeyondBoundsLayout: grid limit in lines, calls from event handlers, user scrolling off (D1480) | In progress |

Next, once those land: `DeferredSharedElementTest` and the moved shared element that doesn't animate from its old place; touch selection's bidi tests (`TextSelectionGesturesBidiTest`); graphics leftovers (`ImageShader`, BrushPainter, FilterQuality); place-with-layer leftovers (zIndex, hit testing through layer transforms); style API, border parity, AndroidView interop, magnifier, autofill. missing.md has the full list.
