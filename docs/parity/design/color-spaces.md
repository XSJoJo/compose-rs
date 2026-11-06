# Color spaces and wide-gamut colors

Design note, written before anything was built, from Compose's `colorspace` package, `Color.kt`, `AndroidColor.android.kt` and `AndroidPaint.android.kt`. It covers `feature:color-spaces` (missing.md 3.40) and the parts of `color-api-basics` (3.39) it can't go without.

## Where a color's space lives

**In the color, as Compose's.** `Color` becomes Compose's `Color(value: ULong)`: 64 bits, packed as Compose packs them. An sRGB color is its ARGB in the high 32 bits and zeros below, the space's id (6 bits) being 0; any other color has red, green and blue as half floats (16 bits each), alpha in 10 bits and the space's id in the low 6 bits. `Color::UNSPECIFIED` becomes Compose's `0x10` (black, transparent, in the unspecified space, id 16), where it was the ARGB `0x00000001`.

The other ways were worse:

- **Keeping 32 bits and a table.** ARGB uses all 32 bits; a wide color would need values stolen from real ones (compose-rs already steals one for `UNSPECIFIED`) and an interner shared across threads that never frees. Compose's `Color.value` couldn't be ported.
- **An enum of an ARGB and a wide color.** 16 bytes, twice the 64-bit form.

The cost is 4 more bytes in each `Color` and in what holds one (a `Paint`, a text style, a modifier). What reads a color keeps a fast path: an sRGB color's ARGB is one shift (`to_argb`), its components a shift and a division, and no conversion runs for it. Before the API changed, the representation alone was measured with `compare_instructions.py` at fat LTO (the result is at the end).

**The API is Compose's.** `red()`, `green()`, `blue()` and `alpha()` become floats, as Compose's (they were bytes; their callers move to `to_argb()` or the floats). `Color::new(red, green, blue, alpha)` and `Color::new_with(…, color_space)`, `from_value`/`value`, `color_space`, `convert`, `copy`, `to_argb`, `luminance`, `composite_over` (in the background's space), `lerp` (in Oklab, through the packed half floats, as Compose's), `hsv`, `hsl`, `take_or_else`, `is_unspecified`. `Debug` keeps `#AARRGGBB` for sRGB colors, so the display-list dumps don't change; `Display` is Compose's `toString`.

**The `colorspace` module** is Compose's package: `ColorSpace` (a shared handle, `Arc`, as a Kotlin reference: compared by Compose's `equals`), `ColorModel`, `RenderIntent`, `WhitePoint`, `Illuminant`, `Adaptation`, `TransferParameters`, `Rgb`, `Connector` and `ColorSpaces`. `Rgb`'s constructors make a `ColorSpace`; `as_rgb()` gives an RGB space's own values (Kotlin's cast). The named spaces are constants that dereference to the one space each (`ColorSpaces::DISPLAY_P3` is usable as a `ColorSpace` and as its `Rgb`), built once on first use. Connectors between named spaces are cached in a table read without a lock (one `OnceLock` per pair and intent), as Compose's map; a connector is a shared handle compared by identity, as Compose's. A space of the app's own is `ColorSpace::custom` with a `CustomColorSpace` (Compose's subclass), never cached, and can't go in a `Color`, as Compose's (its id is -1).

## The display list and the wire format

**The display list doesn't change shape.** Its paints, shadow colors, color filters and gradients already hold `Color`s; they hold wide ones now. No operation is added, and an sRGB color records the same operation, so existing screens record the same lists (checked with `--dumps`).

**Skia** draws a wide color as a float color: converted to extended sRGB (`ColorSpaces::EXTENDED_SRGB`, which keeps what's outside sRGB as values below 0 or above 1) and set with `Paint::set_color4f` in sRGB. An sRGB color takes the path it took. Skia-safe has no `SkColorSpace::MakeRGB`, so a paint can't name the color's own space; extended sRGB holds every space's colors, and the surface clips what it can't show.

**Android's wire format** keeps its bytes for sRGB colors. A paint's flags byte gets a fifth bit: 16, *a color long follows*, written after what's there now (the blur radius). The ARGB slot still holds the color in sRGB, what a replayer ignoring the bit would draw. The long is Android's `@ColorLong`, made as Compose's `toSupportedColorLong`: Compose's ids 0 to 15 are Android's; the spaces Android's paints don't take (unspecified, Oklab, CIE XYZ and Lab) are converted to sRGB first, so they don't set the bit; BT.2020 HLG and PQ (Compose's 17 and 18) become Android's 16 and 17. The replayer sets the color with `Paint.setColor(long)` (API 29, the apps' minimum), except HLG and PQ below API 34, where it keeps the ARGB, as Compose's `toColorLong` does.

Gradient stops, shadow colors and color filters stay ARGB on the wire for now: a wide color there is drawn as its sRGB conversion on Android. They're left in missing.md.

## Android's replayer

`Replayer.kt` reads the bit after the blur radius and calls `paint.setColor(long)` (after `paint.color = argb`, which it does now). Nothing else changes; a paint without the bit reads as before.

## What waits

- `Color.VectorConverter(colorSpace)`: compose-rs's `Animate for Color` animates in Oklab and returns sRGB; a wide color animates as its sRGB conversion until the animation converter takes a space.
- Android interop (`toAndroidColorSpace`, `toComposeColorSpace`, `toColorLong`/`fromColorLong` as public API): n/a, compose-rs has no Kotlin Compose to meet; the wire format's conversion is internal.

## What held

Written after building it (D1911, D1912).

- **The cost.** The 64-bit representation alone, measured before the API changed (`compare_instructions.py origin/main --rounds 5`, fat LTO): the 38 scenarios moved between -0.53 % and +0.40 %, within their spreads (0.04 % to 1.5 %). With the API, `lerp` through the connectors cost `list_selection` and `field_focus` 4 %: two sRGB colors (and two unspecified ones, a text style's background) now take the connectors' steps directly, the sRGB EOTF of each byte from a table of the same values, and the two came back within noise. At the end (with D1913 too), against main: the display-list dumps the same; 38 scenarios from -0.36 % to +0.37 %, most within their spreads, a few text and animation scenarios (`field_focus` +0.32 %, `stress_animation` +0.26 %, `field_keystroke_direct` +0.37 %) just past them: what's left is likely the 8-byte color in paints and text styles, not looked into further.
- **`lerp`'s values changed.** Compose's half-float Oklab numbers replaced compose-rs's: about 13 % of results moved by up to 2 per channel, and `lerp(a, b, 0.0)` isn't always `a`, as in Compose, so a resting color lerped at an end (Material's app bar and list item colors) may move by a step. ColorTest's 1,001 `lerpValues` are all checked (D1914).
- **Offscreen bitmaps** (D1913) came after, on this note's `Color`: a bitmap's space is kept, its pixels drawn as sRGB.
- **Bytes to floats** found the callers that did arithmetic on the byte components without a type error (`alpha() as f32 / 255.0` compiled on a float too): every `as` on a component was checked by hand, and the renderers' and tests' byte reads go through `to_argb()`.
- **`Rgb` as a handle** dereferencing to `ColorSpace`, and the named spaces as constants dereferencing to it, made `&ColorSpaces::DISPLAY_P3` usable wherever a `&ColorSpace` or an `&Rgb` is taken, as Kotlin's subclass is.
- **Compose's `chromaticAdaptation` changes the white point it's given in place**, and the RGB connector's absolute intent then divides those cone responses, not the white points: ported as it is, as `testAdaptedConnectorWithRenderIntent`'s numbers need it.
- **The replayer's ARGB mask path** (a path drawn through a cached alpha mask) took the paint's ARGB: a wide paint draws its path instead.
- **Not seen:** a wide color on a wide-gamut phone screen; the replayer's color long is built and linted, its bytes tested on the host.
