//! Declarative, state-aware, animatable styles, as Compose foundation's
//! `androidx.compose.foundation.style` package (`@ExperimentalFoundationStyleApi`: experimental,
//! still changing upstream). The design is in docs/parity/design/style-api.md.
//!
//! A [`CommonStyle`] sets [`StyleProperty`]s, some only in a state (`pressed`, `hovered`, …,
//! of a [`StyleState`]), some animated. A [`StyleResolver`] resolves it for its state, bound to
//! a [`Modifier::style_resolver`](compose_ui::Modifier::style_resolver) node; the modifiers that
//! apply properties read it (`style_appearance`, `style_placement`, `style_graphics_layer`, the
//! style paddings), each observing only what it reads:
//!
//! ```ignore
//! let state = remember(|| MutableStyleState::new(Some(source.as_source().clone())));
//! let resolver = remember_observer(|| StyleResolver::new(style.clone(), state.clone()));
//! Box!(modifier = Modifier
//!     .style_resolver(resolver.clone())
//!     .style_external_padding(resolver.clone())
//!     .style_graphics_layer(resolver.clone())
//!     .style_placement(resolver.clone())
//!     .style_appearance(resolver.clone())
//!     .style_border_padding(resolver.clone())
//!     .style_content_padding(resolver.clone()));
//! ```
//!
//! Compose's first design (`Style`, `StyleScope`, `Modifier.styleable`) is deprecated and not
//! ported (D1580).

mod animations;
mod appearance;
mod graphics_layer;
mod padding;
mod placement;
pub mod properties;
mod property;
mod resolver;
mod scope;
mod state;

pub use appearance::AppearanceScope;
pub use placement::PlacementModifierScope;
pub use property::{
    AnyStyleProperty, Breadth, ErasedProperty, Fill, ProvidableStyleProperty, Shadows,
    StyleInterpolate, StyleProperty, lerp_dp, lerp_text_unit, style_local_of, style_local_of_with,
    style_property_of, style_property_of_with,
};
pub use resolver::{StyleResolver, StyleResolverScope};
pub use scope::{
    CommonStyle, CommonStyleScope, CommonStyleScopeExt, CustomStyle, CustomStyleScope,
    CustomStyleScopeExt, NestedStyleKey, StyleBlock, StylePropertyAccessorScope,
    StylePropertyAccessorScopeExt,
};
pub use state::{
    MutableStyleState, ProcessInteraction, StyleState, StyleStateKey, remember_updated_style_state,
};

use compose_ui::{GraphicsLayerScope, Modifier};
use std::rc::Rc;

/// The style package's modifiers.
pub trait StyleModifier {
    /// Binds `resolver` here, as `Modifier.styleResolver(styleResolver)`: it's resolved when the
    /// node is attached and again when what its style read changes, and the `style_*` modifiers
    /// read it. A resolver binds to one node at a time.
    fn style_resolver(self, resolver: StyleResolver) -> Modifier;

    /// Draws what `block` sets around the content, as `Modifier.appearance { … }`: the drop
    /// shadow, the background, the content, the foreground, the border, the inner shadow, all
    /// in the shape. The block runs while drawing: what it reads draws again, nothing else.
    /// Elements are the same when their blocks are (by identity): build the modifier in a
    /// `remember` so a recomposition doesn't make a new one.
    fn appearance(self, block: impl Fn(&mut AppearanceScope) + 'static) -> Modifier;

    /// Draws the shape, background, foreground, border and shadows `resolver` resolves, as
    /// `Modifier.styleAppearance(styleResolver)`: a change of one of them draws again, nothing
    /// else.
    fn style_appearance(self, resolver: StyleResolver) -> Modifier;

    /// Sizes and offsets the content as `block` sets, as `Modifier.placement { … }`: offsets
    /// from each side, minimum and maximum sizes, and a size as a distance or a fraction of the
    /// space. The block runs while measuring: what it reads measures again. Elements are the
    /// same when their blocks are (by identity).
    fn placement(self, block: impl Fn(&mut PlacementModifierScope) + 'static) -> Modifier;

    /// Sizes and offsets the content as `resolver` resolves, as
    /// `Modifier.stylePlacement(styleResolver)`: a change of one of those properties measures
    /// again.
    fn style_placement(self, resolver: StyleResolver) -> Modifier;

    /// Places the content with a graphics layer whose properties `block` gives, when
    /// `layer_required` says so (always without one), as the style package's
    /// `Modifier.graphicsLayer(layerRequired, block)` (compose-rs's `graphics_layer` is the
    /// ui one). The block runs where the layer's properties are computed: what it reads updates
    /// the layer, not the layout; what the predicate reads places again.
    fn graphics_layer_when(
        self,
        layer_required: Option<Rc<dyn Fn() -> bool>>,
        block: impl Fn(&mut GraphicsLayerScope) + 'static,
    ) -> Modifier;

    /// Places the content with a layer of the alpha, transforms and clip `resolver` resolves,
    /// when one of them is set, as `Modifier.styleGraphicsLayer(styleResolver)`. Rotations about
    /// X and Y, the camera distance and the color filter make a layer but aren't applied
    /// (compose-rs's layers don't have them yet).
    fn style_graphics_layer(self, resolver: StyleResolver) -> Modifier;

    /// Pads the content by the content padding `resolver` resolves (inside the border), as
    /// `Modifier.styleContentPadding(styleResolver)`.
    fn style_content_padding(self, resolver: StyleResolver) -> Modifier;

    /// Pads the content by the external padding `resolver` resolves (outside the background
    /// and border), as `Modifier.styleExternalPadding(styleResolver)`.
    fn style_external_padding(self, resolver: StyleResolver) -> Modifier;

    /// Pads the content by the border's width `resolver` resolves (a hairline a pixel, a
    /// fraction of a pixel rounded up), as `Modifier.styleBorderPadding(styleResolver)`.
    fn style_border_padding(self, resolver: StyleResolver) -> Modifier;
}

impl StyleModifier for Modifier {
    fn style_resolver(self, resolver: StyleResolver) -> Modifier {
        resolver::style_resolver(self, resolver)
    }
    fn appearance(self, block: impl Fn(&mut AppearanceScope) + 'static) -> Modifier {
        appearance::appearance(self, block)
    }
    fn style_appearance(self, resolver: StyleResolver) -> Modifier {
        appearance::style_appearance(self, resolver)
    }
    fn placement(self, block: impl Fn(&mut PlacementModifierScope) + 'static) -> Modifier {
        placement::placement(self, block)
    }
    fn style_placement(self, resolver: StyleResolver) -> Modifier {
        placement::style_placement(self, resolver)
    }
    fn graphics_layer_when(
        self,
        layer_required: Option<Rc<dyn Fn() -> bool>>,
        block: impl Fn(&mut GraphicsLayerScope) + 'static,
    ) -> Modifier {
        graphics_layer::graphics_layer_when(self, layer_required, block)
    }
    fn style_graphics_layer(self, resolver: StyleResolver) -> Modifier {
        graphics_layer::style_graphics_layer(self, resolver)
    }
    fn style_content_padding(self, resolver: StyleResolver) -> Modifier {
        padding::style_content_padding(self, resolver)
    }
    fn style_external_padding(self, resolver: StyleResolver) -> Modifier {
        padding::style_external_padding(self, resolver)
    }
    fn style_border_padding(self, resolver: StyleResolver) -> Modifier {
        padding::style_border_padding(self, resolver)
    }
}
