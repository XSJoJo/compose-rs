//! The predefined style properties, as Compose's `StyleProperties.kt`: what `style_appearance`,
//! `style_placement`, `style_graphics_layer` and the style paddings read. Compose keeps them
//! internal while it moves to properties a design system defines (they're set through its
//! deprecated `StyleScope`); compose-rs makes them public so those modifiers have something to
//! read without the deprecated scope (D1580). The content fill and the text properties are
//! locals, inherited by the resolvers below (D1586).

use compose_ui::{
    BaselineShift, ColorFilter, Dp, FontFamily, FontStyle, FontSynthesis, FontWeight, Hyphens,
    LineBreak, Shape, TextAlign, TextDecoration, TextDirection, TextMotion, TextUnit,
    TransformOrigin,
};

use super::property::{
    Breadth, Fill, ProvidableStyleProperty, Shadows, lerp_dp, lerp_text_unit, style_local_of,
    style_local_of_with, style_property_of, style_property_of_with,
};

fn zero_dp() -> Dp {
    Dp(0.0)
}

macro_rules! dp_properties {
    ($($(#[$doc:meta])* $name:ident = $label:literal;)*) => {
        $(
            $(#[$doc])*
            pub static $name: ProvidableStyleProperty<Dp> =
                style_property_of_with($label, |a, b, t| lerp_dp(*a, *b, t), zero_dp);
        )*
    };
}

dp_properties! {
    /// The padding inside the border, at the start, as `contentPaddingStart`.
    CONTENT_PADDING_START = "contentPaddingStart";
    /// As `contentPaddingEnd`.
    CONTENT_PADDING_END = "contentPaddingEnd";
    /// As `contentPaddingTop`.
    CONTENT_PADDING_TOP = "contentPaddingTop";
    /// As `contentPaddingBottom`.
    CONTENT_PADDING_BOTTOM = "contentPaddingBottom";
    /// The padding outside the background and border, at the start, as `externalPaddingStart`.
    EXTERNAL_PADDING_START = "externalPaddingStart";
    /// As `externalPaddingEnd`.
    EXTERNAL_PADDING_END = "externalPaddingEnd";
    /// As `externalPaddingTop`.
    EXTERNAL_PADDING_TOP = "externalPaddingTop";
    /// As `externalPaddingBottom`.
    EXTERNAL_PADDING_BOTTOM = "externalPaddingBottom";
    /// The offset from the left, as `left`.
    LEFT = "left";
    /// As `right`.
    RIGHT = "right";
    /// As `top`.
    TOP = "top";
    /// As `bottom`.
    BOTTOM = "bottom";
    /// As `minWidth`.
    MIN_WIDTH = "minWidth";
    /// As `maxWidth`.
    MAX_WIDTH = "maxWidth";
    /// As `minHeight`.
    MIN_HEIGHT = "minHeight";
    /// As `maxHeight`.
    MAX_HEIGHT = "maxHeight";
    /// The border's width, as `borderWidth`.
    BORDER_WIDTH = "borderWidth";
}

/// The width, a distance or a fraction, as `width`.
pub static WIDTH: ProvidableStyleProperty<Breadth> = style_property_of("width", || Breadth::None);
/// As `height`.
pub static HEIGHT: ProvidableStyleProperty<Breadth> = style_property_of("height", || Breadth::None);
/// The border's fill, as `borderFill`.
pub static BORDER_FILL: ProvidableStyleProperty<Fill> =
    style_property_of("borderFill", || Fill::None);
/// As `background`.
pub static BACKGROUND: ProvidableStyleProperty<Fill> =
    style_property_of("background", || Fill::None);
/// Over the content, as `foreground`.
pub static FOREGROUND: ProvidableStyleProperty<Fill> =
    style_property_of("foreground", || Fill::None);
/// Whether the content is clipped to the shape, as `clip`.
pub static CLIP: ProvidableStyleProperty<bool> = style_property_of("clip", || false);
/// The shape of the background, border, shadows and clip, as `shape`.
pub static SHAPE: ProvidableStyleProperty<Shape> = style_property_of("shape", || Shape::Rectangle);
/// As `dropShadow`.
pub static DROP_SHADOW: ProvidableStyleProperty<Shadows> =
    style_property_of("dropShadow", || Shadows::None);
/// As `innerShadow`.
pub static INNER_SHADOW: ProvidableStyleProperty<Shadows> =
    style_property_of("innerShadow", || Shadows::None);
/// As `alpha`.
pub static ALPHA: ProvidableStyleProperty<f32> = style_property_of("alpha", || 1.0);
/// As `scaleX`.
pub static SCALE_X: ProvidableStyleProperty<f32> = style_property_of("scaleX", || 1.0);
/// As `scaleY`.
pub static SCALE_Y: ProvidableStyleProperty<f32> = style_property_of("scaleY", || 1.0);
/// In pixels, as `translationX`.
pub static TRANSLATION_X: ProvidableStyleProperty<f32> = style_property_of("translationX", || 0.0);
/// As `translationY`.
pub static TRANSLATION_Y: ProvidableStyleProperty<f32> = style_property_of("translationY", || 0.0);
/// As `rotationX`: read, not applied (compose-rs's layers don't rotate about X yet).
pub static ROTATION_X: ProvidableStyleProperty<f32> = style_property_of("rotationX", || 0.0);
/// As `rotationY`: read, not applied (compose-rs's layers don't rotate about Y yet).
pub static ROTATION_Y: ProvidableStyleProperty<f32> = style_property_of("rotationY", || 0.0);
/// In degrees, clockwise, as `rotationZ`.
pub static ROTATION_Z: ProvidableStyleProperty<f32> = style_property_of("rotationZ", || 0.0);
/// As `transformOriginX`.
pub static TRANSFORM_ORIGIN_X: ProvidableStyleProperty<f32> =
    style_property_of("transformOriginX", || TransformOrigin::CENTER.pivot_fraction_x);
/// As `transformOriginY`.
pub static TRANSFORM_ORIGIN_Y: ProvidableStyleProperty<f32> =
    style_property_of("transformOriginY", || TransformOrigin::CENTER.pivot_fraction_y);
/// As `cameraDistance`: read, not applied (compose-rs's layers have no camera yet).
pub static CAMERA_DISTANCE: ProvidableStyleProperty<f32> =
    style_property_of("cameraDistance", || 1.0);
/// As `zIndex`: read, not applied, as Compose's (no modifier reads it).
pub static Z_INDEX: ProvidableStyleProperty<f32> = style_property_of("zIndex", || 0.0);
/// As `colorFilter`: read, not applied (compose-rs's layers have no color filter yet).
pub static COLOR_FILTER: ProvidableStyleProperty<Option<ColorFilter>> =
    style_property_of("colorFilter", || None);
/// The content's fill, a local, as `contentFill`: read with `content_fill`, `content_color` and
/// `content_brush`.
pub static CONTENT_FILL: ProvidableStyleProperty<Fill> =
    style_local_of("contentFill", || Fill::None);

fn lerp_text(a: &TextUnit, b: &TextUnit, t: f32) -> TextUnit {
    lerp_text_unit(*a, *b, t)
}

/// The text's font family, a local, as `fontFamily`.
pub static FONT_FAMILY: ProvidableStyleProperty<FontFamily> =
    style_local_of("fontFamily", || FontFamily::Default);
/// The first line's indent, a local, as `textIndentFirstLine`.
pub static TEXT_INDENT_FIRST_LINE: ProvidableStyleProperty<TextUnit> =
    style_local_of_with("textIndentFirstLine", lerp_text, || TextUnit::UNSPECIFIED);
/// The other lines' indent, a local, as `textIndentRestLine`.
pub static TEXT_INDENT_REST_LINE: ProvidableStyleProperty<TextUnit> =
    style_local_of_with("textIndentRestLine", lerp_text, || TextUnit::UNSPECIFIED);
/// As `fontSize`, a local.
pub static FONT_SIZE: ProvidableStyleProperty<TextUnit> =
    style_local_of_with("fontSize", lerp_text, || TextUnit::UNSPECIFIED);
/// As `lineHeight`, a local.
pub static LINE_HEIGHT: ProvidableStyleProperty<TextUnit> =
    style_local_of_with("lineHeight", lerp_text, || TextUnit::UNSPECIFIED);
/// As `baselineShift`, a local.
pub static BASELINE_SHIFT: ProvidableStyleProperty<BaselineShift> =
    style_local_of("baselineShift", || BaselineShift::NONE);
/// As `lineBreak`, a local: none for Compose's `LineBreak.Unspecified`.
pub static LINE_BREAK: ProvidableStyleProperty<Option<LineBreak>> =
    style_local_of("lineBreak", || None);
/// As `textDecoration`, a local.
pub static TEXT_DECORATION: ProvidableStyleProperty<TextDecoration> =
    style_local_of("textDecoration", || TextDecoration::NONE);
/// As `letterSpacing`, a local.
pub static LETTER_SPACING: ProvidableStyleProperty<TextUnit> =
    style_local_of_with("letterSpacing", lerp_text, || TextUnit::UNSPECIFIED);
/// As `fontStyle`, a local.
pub static FONT_STYLE: ProvidableStyleProperty<FontStyle> =
    style_local_of("fontStyle", || FontStyle::Normal);
/// As `textAlign`, a local: none for Compose's `TextAlign.Unspecified`.
pub static TEXT_ALIGN: ProvidableStyleProperty<Option<TextAlign>> =
    style_local_of("textAlign", || None);
/// As `textDirection`, a local: none for Compose's `TextDirection.Unspecified`.
pub static TEXT_DIRECTION: ProvidableStyleProperty<Option<TextDirection>> =
    style_local_of("textDirection", || None);
/// As `hyphens`, a local: none for Compose's `Hyphens.Unspecified`.
pub static HYPHENS: ProvidableStyleProperty<Option<Hyphens>> = style_local_of("hyphens", || None);
/// As `fontSynthesis`, a local.
pub static FONT_SYNTHESIS: ProvidableStyleProperty<FontSynthesis> =
    style_local_of("fontSynthesis", || FontSynthesis::None);
/// As `fontWeight`, a local, animating as Compose's font weight `lerp`.
pub static FONT_WEIGHT: ProvidableStyleProperty<FontWeight> =
    style_local_of_with("fontWeight", |a, b, t| FontWeight::lerp(*a, *b, t), || FontWeight::NORMAL);
/// As `textMotion`, a local: animated while two motions differ, as Compose's.
pub static TEXT_MOTION: ProvidableStyleProperty<TextMotion> = style_local_of_with(
    "textMotion",
    |a, b, _| if a == b { *a } else { TextMotion::Animated },
    || TextMotion::Static,
);
