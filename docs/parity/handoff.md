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

Every `#[ignore]` carries its reason in its attribute; `grep -rn -A1 '#\[ignore' crates benchmarks` lists them, and `cargo test -p <crate> --test suite <name> -- --ignored` runs one. A test is ignored only for one of the reasons below; a bug found in review gets `#[ignore = "review: <bug>"]` and a fix on the next branch. On `main` there are 13, 9 once `paragraph-paint-fixes` merges (review findings fixed since: lazy beyond-bounds layouts, D1480; a `GraphicsLayer`'s readers and its first drawing's send, D1339; a container's toolbar after a cancelled handle press or a text changed in place, D2045, D2046; a paragraph's blend mode, an unspecified solid brush and the locale table, D2060, D2061):

| Tests | Why ignored | To make them pass |
|---|---|---|
| compose-skia `review_multi_paragraph_2.rs` (5; 1 once `paragraph-paint-fixes` merges) | A review of D2049–D2052 found: `paint` panics on a text's or a field's own `on_text_layout` result (a paragraph that only answers queries); a blend mode other than SrcOver wipes the area around the glyphs (the paragraphs go through a layer, where Compose sets the mode on each glyph's paint); the locale direction table disagrees with ICU 68's likely subtags both ways (`zdj`, `ku-IR`, `und-145`… are right to left; `Aran`, `Syrj`… have no script properties); an unspecified solid brush paints over the paragraph's colour. | `paint` on layout results: `basic-text-paragraphs`; the other four are fixed on `paragraph-paint-fixes` (D2060, D2061). If lost: give a snapshot `MultiParagraph` a lazy drawable source. |
| `lazy_layout.rs`: `an_item_removed_and_added_back_reuses_its_node` | Ignored in Compose too (b/369188686): a node reactivated for its key is measured again. | Un-ignore when Compose fixes it upstream and the fix is ported. |
| compose `review_c6.rs`: `a_call_macro_imported_under_another_name_calls_the_function_it_names`; `review_c4.rs`: `a_name_before_equals_in_a_user_pascal_case_macro_is_a_capture` | Accepted limitations of the `#[composable]` macro (D114: a call macro calls by its declared name; D98: a named argument's name is not a use). Kept to document them. | Not planned; they would need name resolution the macro can't do. |
| `perf_review2.rs`, compose-android `retained_groups.rs` (`timing_one_path_reset_for_many_shapes`), compose-ui `mutable_paths.rs` (`timing_idle_frames_with_many_path_drawings`), `benchmarks/tests/layer_cost.rs` | Not failures: timing tools run by hand (`cargo test --release -- --ignored --nocapture`), too slow or noisy for the suite. | Nothing to do. |

`#[cfg_attr(miri, ignore = "spawns a process")]` marks tests Miri can't run; they run in the normal suite.

## Priorities

Core modules (runtime, ui, foundation, animation) before the big Material 3 components. Jank and frame times matter most for Android performance (docs/12).

Never merge the `ime-logging` branch (it records everything typed) or `parked/nav-compose-extras`.

## In flight

As of 2026-11-07, `origin/main` is at 48996705 (13,217 tests; 11,800 with the runtime feature off; 13,217 with `COMPOSE_TEST_AUTOFILL=1`). In flight: `paragraph-paint-fixes` (below, not merged); the other branches below are merged. Two were merged without the usual review because the weekly quota ran short; review them first.

| Branch | What | State |
|---|---|---|
| `paragraph-paint-fixes` | The paragraph API review's fixes: a blend mode on each glyph's paint, not a layer (D2060: the platform's `painted` takes it, `layout_blended` lays a text out in it on the desktop, a paint extra on Android); an unspecified solid brush keeps the colour; a locale's direction from tables generated from ICU 68.2's data, allocating nothing (D2061; two merged tests corrected to ICU 68). Un-ignores 4 of the review's 5. Kotlin changed: merge with `--gradle` | Ready to review |
| `multi-paragraph-2` | **Merged without a review** (the weekly quota was short): the paragraph API review's findings (D2049), `fillPathForRange` (D2050), the constructors and `ParagraphIntrinsics` (D2051), `paint` with a gradient spanning paragraphs (D2052). Worth a review: D2049's locale-direction table, D2052's paint (it panics on a paragraph that only answers queries, such as `BasicText`'s `on_text_layout` result). Next: `BasicText` and the fields laying out paragraph styles as a `MultiParagraph` (D2053 and up), then AndroidParagraphTest (115) | Merged, review pending |
| `autofill-2` | **Merged without a second review** (the weekly quota was short): Compose's autofill (D1680–D1688). After its first review's blocker, it tracks only changed nodes and is switched off while Android's `AutofillManager.isEnabled()` is false. Cost with a manager on, against none: `list_scroll` +1.8 %, `feed_step` +2.3 %, `field_keystroke_direct` +3.5 %, `first_frame_list_items` +4.3 % (the first frame's scan); scroll frames on the pixel screens within 0.2 %. Worth a review: D1688's semantics observation (reads while computing semantics go to their own observer; a write in a frame invalidates them in that frame) and `UiTree::semantics_tree` applying pending writes first. `COMPOSE_TEST_AUTOFILL=1` runs the suite with a manager installed. Not run on a phone | Merged, review pending |

Next: `BasicText` and the text fields laying out paragraph styles as a `MultiParagraph` (D2053 and up) and AndroidParagraphTest; `DeferredSharedElementTest` and the shared elements of `LookaheadScopeTest` and Navigation 3; touch selection's bidi tests (`TextSelectionGesturesBidiTest`); graphics leftovers (`ImageShader`, BrushPainter, FilterQuality); place-with-layer leftovers (zIndex, hit testing through layer transforms); style API, border parity, AndroidView interop, magnifier, autofill. missing.md has the full list.
