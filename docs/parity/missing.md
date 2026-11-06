# What's missing for parity with Compose

A short overview, written on 2026-10-31. The [ledgers](README.md) have the detail. Numbers are the Compose tests the ledgers count as waiting on a feature, tallied roughly: they show size, not exact remaining work, and some ledger tags lag behind what's done.

Out of scope, by decision: Material 2, Wear, TV, Glance, remote, Navigation Compose. iOS and WASM are paused.

## Written, not on main yet

These are on local branches, so they're not lost, but each still has to finish its review or fixes:

| Work | Branch | State |
|---|---|---|
| Stretch and glow overscroll | `worktree-agent-ad25708f190fdec81` | Being built, not reviewed. Design: [stretch-overscroll.md](design/stretch-overscroll.md). |

## The big missing things

### 1. Modern text fields (the largest gap)
- **`TextFieldState` and the state-based `BasicTextField`** (about 700 tests; [design](design/text-field-state.md)). Step 1 of six is built: `TextFieldState` (`edit`, its helpers, saver, `remember_text_field_state`), `TextFieldBuffer` with its `ChangeList`, `TextFieldLineLimits`, and `BasicTextFieldOfState` over the value field's node and the keyboard path (D1128 to D1131); about 150 of Compose's tests ported. **Still missing:** undo in the state (`UndoState`, Compose's merge rules); `InputTransformation` and `KeyboardActionHandler`; `OutputTransformation` and the single line's new lines shown as spaces; `on_text_layout`, `scroll_state`, the default width of ten "H" and the rest of `BasicTextFieldTest`, the key, semantics, scroll and focus tests; Material 3's state-based `TextField`s; styles in the state; `BasicSecureTextField` (about 100).
- **Touch text selection** (about 230): handles and selection gestures in text and fields.
- **Text context menu** (about 290), **magnifier** (about 80), **handwriting** (about 85), **autofill** (about 165), **text links** (about 55).

### 2. Layout and composition foundations
- **The rest of lookahead:** lazy items' lookahead positions, the `SubcomposeLayout` cases, and most of `LookaheadScopeTest`. The scope, the pass and `approach_layout` are in.
- **Lazy layouts on subcomposition (C8):** `SubcomposeLayout`, its reuse and paused precomposition are in, and lazy layouts move onto it next, so items compose during measure as in Compose. Today they compose in rounds, the root of several differences.
- **Place-with-layer** in compose-ui (`placeWithLayer`, what `offset_with` wants too). Until it lands, `animate_item` has a visible bug: modifiers before it in the chain neither move nor fade (D1472), and lookahead scopes aren't told apart; its grid and staggered tests are partly ported.
- **Shared elements and `animateBounds`** (about 95), which wait on lookahead.
- **`Modifier.onLayoutRectChanged` and its callbacks** (about 160).
- **`FlexBox`** (about 95) and **the grid layout** (about 80).
- **Android view interop**, `AndroidView` and hosting (about 280).
- **Runtime:** composition observers (about 28), error traces (about 33), composition data (about 15).

### 3. Graphics
- **`GraphicsLayer` as an object** (about 95), **color spaces** (about 70), offscreen canvas and bitmaps.
- **`Painter`** is in (D1901): `Painter`, `BitmapPainter`, `ColorPainter`, `Modifier.paint`, `Image(painter)` (`PainterImage`) and Material's `Icon(painter)` with a color or a `ColorProducer` tint. Still missing: `BrushPainter`, `FilterQuality`, and `painterResource`.
- **Blend modes and color filters** are in (D1900): `BlendMode`, `ColorFilter`'s tint, lighting and matrix, `ColorMatrix`, on paints, images and layers, on Skia and Android (checked on the phone with the demo's `graphics` tab: filters, blend modes, tinted icons and a vector). Still missing: `Brush.composite`, a blend mode on `drawImage`, and a graphics layer's `colorFilter` and `blendMode`.
- **Vector graphics** are in (D1902): `ImageVector` and its builder, `PathNode`, `PathParser`, `VectorPainter` with `remember_vector_painter`, groups with transforms and clip paths, a recorded drawing kept per size, `Image(imageVector)` (`VectorImage`). Still missing: trim paths (waits on `PathMeasure`), stroke joins and miters, vector composition (`Group`/`Path` composables and `rememberVectorPainter { }`), and **animated image vectors** (about 20).
- **Blur** (about 80), **drop and inner shadows** (about 45), brush and path extras, polygon shapes (about 65).

### 4. Foundation
- **Style API** (about 160), **border parity** (about 80).
- **Gestures and scrolling:** `transformable` (about 40), 2D scrolling (about 45), `bringIntoView` (about 90 across two rows), public lazy layout API (about 40), `BeyondBoundsLayout` (about 35).

### 5. Material 3
- **Whole components missing:** carousel (about 100), floating toolbar (about 80), search bar (about 65), wide navigation rail (about 45), wavy progress (about 45), short navigation bar (about 35), loading indicator (about 20), FAB menu (about 15).
- **Done lately:** toggle buttons (D1570), the button group with its overflow (D1571), the split button (D1572).
- **Dynamic color** (about 25), `TimeScroll` and time picker dialogs, Material transitions.
- **Adaptive libraries,** window size classes and the navigation suite, planned last.
- Many existing components are `partial`: they exist, with tests or details left.

### 6. Input and platform
- **Indirect pointer input** (about 125), **pointer icons** (about 60), scroll capture (about 45), clipboard API (about 30), content capture, drag and drop, rotary input, media queries.
- **Locale and device data:** the date and time pickers use small tables for a few languages. Real data needs `icu4x`, a decision for the user.

### 7. The test framework
`compose-test` lacks parts of ui-test that ports will need: pointer, trackpad and indirect injection (about 175 tests of its own), device configuration override, and capture to image.

## Not verified on a device

Written and tested on the host, but not yet confirmed on a phone:

- **TalkBack's speech and announcements.** The accessibility tree was read directly, but no agent can enable a service.
- **A keyboard that really composes.** Gboard on the test phone doesn't compose, so long-lived composition hasn't run on a real keyboard (D1671).
- A real D-pad and a real mouse, rotation, voice typing.
- Android's real `RippleDrawable` (`feature:android-platform-ripple`).

## Known costs against Compose's shape

- A focus change in a text field costs about 10 % more than before the decoration box work, and a keystroke about 5 %.
- Movable content moved across compositions re-runs its body, and doesn't move between different trees, such as into a `Dialog` (D1308).
