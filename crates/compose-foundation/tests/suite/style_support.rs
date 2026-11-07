//! What the ported style tests share: the setters of Compose's deprecated `StyleScope`, as the
//! predefined properties they set (Compose's `Style.toCommonStyle`), and its `styleable` chain
//! over a resolver.

use compose_foundation::style::properties::*;
use compose_foundation::style::*;
use compose_runtime::remember_observer;
use compose_ui::{Brush, Color, Dp, Modifier, Shape};

/// The deprecated `StyleScope`'s setters, for the tests written with them.
#[allow(dead_code)] // Not every test file uses every setter.
pub trait StyleSetters {
    fn content_padding(&mut self, all: Dp);
    fn content_padding_each(&mut self, start: Dp, top: Dp, end: Dp, bottom: Dp);
    fn external_padding_each(&mut self, start: Dp, top: Dp, end: Dp, bottom: Dp);
    fn width(&mut self, value: Dp);
    fn height(&mut self, value: Dp);
    fn width_fraction(&mut self, fraction: f32);
    fn height_fraction(&mut self, fraction: f32);
    fn size(&mut self, value: Dp);
    fn size_of(&mut self, width: Dp, height: Dp);
    fn fill_size(&mut self);
    fn min_width(&mut self, value: Dp);
    fn max_width(&mut self, value: Dp);
    fn min_height(&mut self, value: Dp);
    fn max_height(&mut self, value: Dp);
    fn left(&mut self, value: Dp);
    fn top(&mut self, value: Dp);
    fn right(&mut self, value: Dp);
    fn bottom(&mut self, value: Dp);
    fn background(&mut self, color: Color);
    fn background_brush(&mut self, brush: Brush);
    fn foreground(&mut self, color: Color);
    fn border(&mut self, width: Dp, color: Color);
    fn border_brush(&mut self, width: Dp, brush: Brush);
    fn shape(&mut self, shape: Shape);
    fn clip(&mut self, clip: bool);
    fn alpha(&mut self, alpha: f32);
    fn scale(&mut self, scale: f32);
    fn translation(&mut self, x: f32, y: f32);
    fn rotation_z(&mut self, degrees: f32);
}

impl StyleSetters for dyn CommonStyleScope + '_ {
    fn content_padding(&mut self, all: Dp) {
        self.content_padding_each(all, all, all, all);
    }
    fn content_padding_each(&mut self, start: Dp, top: Dp, end: Dp, bottom: Dp) {
        self.provide(&CONTENT_PADDING_START, start);
        self.provide(&CONTENT_PADDING_TOP, top);
        self.provide(&CONTENT_PADDING_END, end);
        self.provide(&CONTENT_PADDING_BOTTOM, bottom);
    }
    fn external_padding_each(&mut self, start: Dp, top: Dp, end: Dp, bottom: Dp) {
        self.provide(&EXTERNAL_PADDING_START, start);
        self.provide(&EXTERNAL_PADDING_TOP, top);
        self.provide(&EXTERNAL_PADDING_END, end);
        self.provide(&EXTERNAL_PADDING_BOTTOM, bottom);
    }
    fn width(&mut self, value: Dp) {
        self.provide(&WIDTH, Breadth::Distance(value));
    }
    fn height(&mut self, value: Dp) {
        self.provide(&HEIGHT, Breadth::Distance(value));
    }
    fn width_fraction(&mut self, fraction: f32) {
        self.provide(&WIDTH, Breadth::Fraction(fraction));
    }
    fn height_fraction(&mut self, fraction: f32) {
        self.provide(&HEIGHT, Breadth::Fraction(fraction));
    }
    fn size(&mut self, value: Dp) {
        self.size_of(value, value);
    }
    fn size_of(&mut self, width: Dp, height: Dp) {
        self.width(width);
        self.height(height);
    }
    fn fill_size(&mut self) {
        self.width_fraction(1.0);
        self.height_fraction(1.0);
    }
    fn min_width(&mut self, value: Dp) {
        self.provide(&MIN_WIDTH, value);
    }
    fn max_width(&mut self, value: Dp) {
        self.provide(&MAX_WIDTH, value);
    }
    fn min_height(&mut self, value: Dp) {
        self.provide(&MIN_HEIGHT, value);
    }
    fn max_height(&mut self, value: Dp) {
        self.provide(&MAX_HEIGHT, value);
    }
    fn left(&mut self, value: Dp) {
        self.provide(&LEFT, value);
    }
    fn top(&mut self, value: Dp) {
        self.provide(&TOP, value);
    }
    fn right(&mut self, value: Dp) {
        self.provide(&RIGHT, value);
    }
    fn bottom(&mut self, value: Dp) {
        self.provide(&BOTTOM, value);
    }
    fn background(&mut self, color: Color) {
        self.provide(&BACKGROUND, Fill::Color(color));
    }
    fn background_brush(&mut self, brush: Brush) {
        self.provide(&BACKGROUND, Fill::Brush(brush));
    }
    fn foreground(&mut self, color: Color) {
        self.provide(&FOREGROUND, Fill::Color(color));
    }
    fn border(&mut self, width: Dp, color: Color) {
        self.provide(&BORDER_WIDTH, width);
        self.provide(&BORDER_FILL, Fill::Color(color));
    }
    fn border_brush(&mut self, width: Dp, brush: Brush) {
        self.provide(&BORDER_WIDTH, width);
        self.provide(&BORDER_FILL, Fill::Brush(brush));
    }
    fn shape(&mut self, shape: Shape) {
        self.provide(&SHAPE, shape);
    }
    fn clip(&mut self, clip: bool) {
        self.provide(&CLIP, clip);
    }
    fn alpha(&mut self, alpha: f32) {
        self.provide(&ALPHA, alpha);
    }
    fn scale(&mut self, scale: f32) {
        self.provide(&SCALE_X, scale);
        self.provide(&SCALE_Y, scale);
    }
    fn translation(&mut self, x: f32, y: f32) {
        self.provide(&TRANSLATION_X, x);
        self.provide(&TRANSLATION_Y, y);
    }
    fn rotation_z(&mut self, degrees: f32) {
        self.provide(&ROTATION_Z, degrees);
    }
}

/// The deprecated `Modifier.styleable(styleResolver)`: the resolver and the modifiers reading
/// it, in Compose's order.
pub fn styleable(modifier: Modifier, resolver: &StyleResolver) -> Modifier {
    modifier
        .style_resolver(resolver.clone())
        .style_external_padding(resolver.clone())
        .style_graphics_layer(resolver.clone())
        .style_placement(resolver.clone())
        .style_appearance(resolver.clone())
        .style_border_padding(resolver.clone())
        .style_content_padding(resolver.clone())
}

/// The deprecated `Modifier.styleable(state, style)`, call site by call site: a resolver of
/// `style` for `state` (one of its own if none), remembered.
pub fn styled(
    modifier: Modifier,
    state: Option<&MutableStyleState>,
    style: impl Fn(&mut dyn CommonStyleScope) + 'static,
) -> Modifier {
    let state = state.cloned();
    let resolver = remember_observer(move || {
        let state = state.unwrap_or_else(|| MutableStyleState::new(None));
        StyleResolver::new(CommonStyle::new(style), state)
    });
    styleable(modifier, &resolver)
}
