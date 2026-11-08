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

## Coverage by area

From the ledgers after the audit of 2026-11-08, in Compose `@Test` methods. "Partial" counts whole files, of which part is ported, so the work left there is smaller than the number. A few files appear in two ledgers (22 between ui-core and ui-input, `SystemGestureExclusionTest` in both foundation ledgers), so the rows don't add up to a total.

| Area (ledger) | Ported or covered | Partial (files, tests) | Ready to port now (`port`) | Waiting on a feature | n/a | The biggest missing features |
|---|---:|---:|---:|---:|---:|---|
| Runtime ([runtime](runtime.md)) | 731 | 9, 303 | 0 | 0 | 968 | The rest of error traces (11), `rememberSerializable` (needs serde: a decision for the user), saved-state owners; 4 `rememberSaveable` tests are portable now |
| ui: layout, nodes, drawing, graphics ([ui-core](ui-core.md)) | 1,291 | 49, 1,249 | 0 | 332 | 293 | Progressive blur (60), brush extras (42), media queries (42), a graphics layer's shape semantics (28), rulers (23), non-linear font scaling (21) |
| ui: input, focus, semantics, hosts ([ui-input](ui-input.md)) | 1,057 | 46, 1,293 | 62 | 654 | 459 | Android view interop (279), scroll capture (44), platform text input (43), media queries (42), the clipboard API (30) |
| ui-text and ui-test ([ui-text-and-test](ui-text-and-test.md)) | 1,478 | 42, 1,138 | 284 | 409 | 533 | Android text device tests (68), fonts (60), device configuration override (35), text actions (27), line-height style modes (25) |
| foundation: layout, gestures, scrolling, lazy, pager ([foundation-layout-lazy](foundation-layout-lazy.md)) | 1,527 | 51, 1,353 | 16 | 101 | 132 | Layout modifiers (`Modifier.visible`, `matchParentSize`, `absolutePadding`, `Arrangement.Absolute`), foundation reading trackpad and indirect pointer events, lazy semantics (`IndexForKey`, `CollectionItemInfo` set by the lazy layouts), gesture detectors (`awaitEachGesture`, `PressGestureScope`), `scrollableArea`, `fitInside`/`fitOutside` |
| foundation: text, fields, selection, the rest ([foundation-text-misc](foundation-text-misc.md)) | 1,376 | 51, 1,492 | 179 | 820 | 1,070 | Android IME details (194), handwriting (84), text field styles (67), polygon shapes (64), text links (56), `TextFieldLayoutStateCache` (45), content receiver (40) |
| animation and Navigation 3 ([animation-navigation](animation-navigation.md)) | 910 | 8, 97 | 0 | 58 | 154 | Animated image vectors (22), Navigation 3 with movable entries (19) and shared elements, lookahead visual debugging (16); 6 animation tests unblocked once `AnimatedContent` uses lookahead |
| Material 3 ([material3](material3.md), not audited) | 765 | 47, 1,243 | 189 | 539 | 34 | Carousel (99), floating toolbar (82), search bar (66), wide navigation rail (46), wavy progress (44) — planned last |
| lifecycle ([lifecycle](lifecycle.md)) | see ledger | | | | | `flowWithLifecycle` and `collectAsStateWithLifecycle` (21; the flows exist), saved state (`SavedStateHandle`) |

**The core modules aren't complete.** None of runtime, ui or foundation has every portable test ported. The quickest ways to raise coverage are the `port` rows (nothing blocks them: about 540 tests across ui and foundation, none left in ui-core) and the rest of the `partial` files. Each ledger's rows say what is left in each file.

**Keeping these numbers right:** `python3 -I tools/parity/ledger_count.py docs/parity/<ledger>.md` recounts a ledger's statuses from its rows (not every ledger's format parses: foundation-layout-lazy and animation-navigation were recounted by hand). `python3 -I tools/parity/citations.py docs/parity/<ledger>.md <androidx-main>` shows, for each row not yet ported, how many of its Compose tests are cited by a port (`// Compose: <path>, <test names>`): a row with all cited is likely `ported`, a `port` or `feature:` row with citations likely `partial`. Ports without that citation comment can't be counted this way, so cite each port.

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
- **Locale directions** (D2061) are generated from ICU 68.2 in androidx-main's `external/icu`: `python3 -I tools/locale/generate_locale_direction.py crates/compose-ui/src/locale_direction.rs <androidx-main>` rewrites the tables and reproduces the committed file.
- **Disk:** builds per worktree are large. Use `CARGO_INCREMENTAL=0` in worktrees, and delete a worktree's `target/` when its branch is merged.
- **Ledger totals:** `python3 tools/parity/ledger_count.py docs/parity/<ledger>.md` recounts statuses from the rows.

## Ignored tests

Every `#[ignore]` carries its reason in its attribute; `grep -rn -A1 '#\[ignore' crates benchmarks` lists them, and `cargo test -p <crate> --test suite <name> -- --ignored` runs one. A test is ignored only for one of the reasons below; a bug found in review gets `#[ignore = "review: <bug>"]` and a fix on the next branch. On `main` there are 9 (review findings fixed since: a text cut by its maximum lines panicking on selection queries and a text's own layout result not painting, D2053, D2054; a state field's text changed in code not told to autofill, D1689; lazy beyond-bounds layouts, D1480; a `GraphicsLayer`'s readers and its first drawing's send, D1339; a container's toolbar after a cancelled handle press or a text changed in place, D2045, D2046):

| Tests | Why ignored | To make them pass |
|---|---|---|
| `lazy_layout.rs`: `an_item_removed_and_added_back_reuses_its_node` | Ignored in Compose too (b/369188686): a node reactivated for its key is measured again. | Un-ignore when Compose fixes it upstream and the fix is ported. |
| compose `review_c6.rs`: `a_call_macro_imported_under_another_name_calls_the_function_it_names`; `review_c4.rs`: `a_name_before_equals_in_a_user_pascal_case_macro_is_a_capture` | Accepted limitations of the `#[composable]` macro (D114: a call macro calls by its declared name; D98: a named argument's name is not a use). Kept to document them. | Not planned; they would need name resolution the macro can't do. |
| compose-foundation `popup_alignment.rs`: `popup_correct_position_alignment_top_end_rtl`, `…center_end_rtl`, `…bottom_end_rtl` (branch `ui-core-ports`) | Android moves a popup window back into the screen (`PopupProperties.clippingEnabled`, true by default): at x -10 it's at 0. compose-rs's popups are overlays of the window, not clipped. | `clipping_enabled` in `PopupProperties` and the overlay's position kept in the window when it's set (`UiTree::position_overlays`); check the other popups' tests (menus, tooltips) don't move. ui-core 3.49. |
| compose-skia `draw_modifier.rs`: `modifier_combined_modifiers` (branch `ui-core-ports`) | A node both layout and draw draws in its own coordinates, where Compose draws it in its content's (the coordinator below it), with its content's size (D1341). | Drawing a layout node's draw in the coordinates of what it places (render.rs), then the shared elements' overlay (D1341) no longer making up for it. |
| compose-foundation `lookahead_toggle_placement.rs`: `toggle_placement_in_modifier_and_verify_lookahead_root_placement` (branch `ui-core-ports`) | A node whose layout modifier doesn't place its content stays placed (and its children: they're placed as its policy measures them), where Compose's `LayoutNode.isPlaced` and `isPlacedInLookahead` turn false and its children aren't placed or drawn (ui-core §5, difference 13). | A placed state per layer: the content a block didn't place, and the children inside it, unplaced (not drawn, not hit, `on_unplaced`), placed again when the block places it. |
| `perf_review2.rs`, compose-android `retained_groups.rs` (`timing_one_path_reset_for_many_shapes`), compose-ui `mutable_paths.rs` (`timing_idle_frames_with_many_path_drawings`), `benchmarks/tests/layer_cost.rs` | Not failures: timing tools run by hand (`cargo test --release -- --ignored --nocapture`), too slow or noisy for the suite. | Nothing to do. |

`#[cfg_attr(miri, ignore = "spawns a process")]` marks tests Miri can't run; they run in the normal suite.

## Priorities

Core modules (runtime, ui, foundation, animation) before the big Material 3 components. Jank and frame times matter most for Android performance (docs/12).

Never merge the `ime-logging` branch (it records everything typed) or `parked/nav-compose-extras`.

## In flight

As of 2026-11-08, `origin/main` is at 2d8df37d (13,634 tests). In flight: `ui-text-ports`, pushed as it goes to `wip/ui-text-ports` (the session may stop at the quota limit: merge what's there through `tools/parity/check_and_push.sh`, then carry on with the rows still tagged `port`). Branches marked "review pending" were merged without the usual review; review them first.

| Branch | What | State |
|---|---|---|
| `wip/ui-text-ports` | Porting ui-text-and-test's `port` rows (37 files, ~284 tests), biggest first | In progress |
| `ui-core-ports` | All 15 of ui-core's `port` rows (129 tests): 13 now `ported`, `TestRuleExecutesLayoutPassesWhenWaitingForIdleTest` `partial` (4 of 9; 3 `AndroidView` n/a, 2 compose in a placement block, which compose-rs can't), `AndroidTileModeTest` `feature:tile-mode-is-supported` (no drawn cases: 9 Android conversions n/a, 5 `isSupported`). 106 tests ported, 5 of them ignored (above: popup clipping 3, D1341 1, a modifier not placing its content 1). Then the largest `partial` remainders: all of `ClipDrawTest` but its Android view one (16), and 19 of `draw/DrawModifierTest` (its `drawWithCache` invalidations, moves and reuse; the `GraphicsLayer` recording half left). Tests only, no production change; D1925 records two kept differences (a path's outline compares by contents; a default layer scope's size is zero). **Not reviewed** | Unmerged |
| `deferred-shared-elements` | All 15 of `DeferredSharedElementTest` (11 Skia pixel tests, 4 bounds tests): a deferred transition hands its `SharedMutableTransformState` to the shared elements inside it, and the hand-off starts from the preview's bounds and velocity (D1340); a moving shared element is drawn in the overlay where it is placed (D1341, an older D1329 bug; the cause is a compose-ui gap: a layout-and-draw node draws in its own coordinates where Compose draws in its content's, recorded in missing.md). **Not reviewed**; worth a review: the hand-off velocity (Compose's test checks it loosely) | Merged, review pending |
| `magnifier` | Compose foundation's magnifier (D2072–D2074): `Modifier.magnifier`, its node and a recording test magnifier; text selection's magnifier in containers and fields; Android's `android.widget.Magnifier` through JNI (built and linted, not run on a phone). MagnifierTest 21/21, AbstractSelectionMagnifierTests 28 (over containers and fields), SelectionContainerMagnifierTest 3/3, TextFieldMagnifierTest 16/19 (3 wait on drag and drop). `SelectionContainer` now measures its content in its own constraints, minimums included, as Compose's `SimpleLayout`. Cost where the platform has a magnifier (Android, the test rule): `field_keystroke` +1.2 %, `field_keystroke_direct` +1.6 % (a node per field observing one state). **Not reviewed**; worth a review: D2073's finding 3, a global-position-aware delegate added and removed during a gesture stopped a container's handles following a graphics layer's move (a possible compose-ui bug, worked around) | Merged, review pending |
| `bidi-selection` | All 39 of `TextSelectionGesturesBidiTest`, passing; one fix: a text's direction at an offset is its paragraph's on an empty line, as Compose's `getTextDirectionForOffset` (D2062). Reviewed after: the end of a cut text now says what the cut says (D2071); its text-of-paragraphs path tested (`review_bidi_selection.rs`) | Merged, reviewed |
| `a11y-cut-text` | Android accessibility line and page traversal stop at a cut text's last shown line instead of panicking (D2070); a plain text cut by its maximum lines changes the same way, a recorded divergence from Compose (whose iterators return a reversed or overlong range there). Reviewed after: its right-to-left guards and the shown end after a line feed now tested (`review_a11y_cut_text.rs`) | Merged, reviewed |
| `multi-paragraph-2` | The paragraph API's second part (D2049–D2052). Merged first, reviewed after; the review's findings are fixed by D2053, D2054 (a text's own layout result paints), D2060 (blend modes on each glyph's paint) and D2061 (locale directions from ICU 68.2) | Merged, reviewed |
| `autofill-2` | Compose's autofill (D1680–D1688), tracking only changed nodes, off while Android's autofill is. Merged first, reviewed after; the review's blocker (a state field's text changed in code not reported) and its other findings are fixed by D1689. With a manager on: `list_scroll` +1.7 %, `feed_step` +2.1 %, `field_keystroke_direct` +3.6 %, `first_frame_list_items` +4.2 %. Not run on a phone | Merged, reviewed |

Next: the text fields laying out paragraph styles once they hold styled text, and AndroidParagraphTest (115); the shared elements of `LookaheadScopeTest` and Navigation 3; graphics leftovers (`ImageShader`, BrushPainter, FilterQuality); place-with-layer leftovers (zIndex, hit testing through layer transforms); style API, border parity, AndroidView interop, magnifier, autofill. missing.md has the full list.
