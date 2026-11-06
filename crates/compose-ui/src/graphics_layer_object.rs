//! A graphics layer as an object, as Compose's `androidx.compose.ui.graphics.layer.GraphicsLayer`:
//! a recording not tied to a layout node, with its own transform, alpha, clip, shadow, color
//! filter and blend mode, drawn where a drawing asks ([`DrawScope::draw_layer`]), any number of
//! times.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::color_filter::{BlendMode, ColorFilter};
use crate::display_list::{Boundary, DisplayList, DrawOp};
use crate::draw::DrawScope;
use crate::draw_modifiers::ContentDrawScope;
use crate::graphics::{Color, Outline};
use crate::graphics_layer::{CompositingStrategy, LayerProperties};
use crate::unit::{IntOffset, IntSize, Offset, Rect, Size};

/// A layer's properties, Compose's defaults: opaque, untransformed, unclipped, no shadow,
/// blending `SrcOver`, unfiltered, its pivot its center.
#[derive(Clone, PartialEq, Debug)]
struct Properties {
    top_left: IntOffset,
    pivot_offset: Offset,
    alpha: f32,
    scale_x: f32,
    scale_y: f32,
    translation_x: f32,
    translation_y: f32,
    rotation_z: f32,
    shadow_elevation: f32,
    ambient_shadow_color: Color,
    spot_shadow_color: Color,
    blend_mode: BlendMode,
    color_filter: Option<ColorFilter>,
    clip: bool,
    compositing_strategy: CompositingStrategy,
    /// Set by `set_*_outline`; else its bounds.
    outline: Option<Outline>,
}

impl Default for Properties {
    fn default() -> Self {
        Properties {
            top_left: IntOffset::new(0, 0),
            pivot_offset: Offset::UNSPECIFIED,
            alpha: 1.0,
            scale_x: 1.0,
            scale_y: 1.0,
            translation_x: 0.0,
            translation_y: 0.0,
            rotation_z: 0.0,
            shadow_elevation: 0.0,
            ambient_shadow_color: Color::BLACK,
            spot_shadow_color: Color::BLACK,
            blend_mode: BlendMode::SrcOver,
            color_filter: None,
            clip: false,
            compositing_strategy: CompositingStrategy::Auto,
            outline: None,
        }
    }
}

thread_local! {
    /// How deep in draw passes this thread is, and the layers changed in them.
    static DRAW_PASS: RefCell<(u32, Vec<std::rc::Weak<LayerCell>>)> = const { RefCell::new((0, Vec::new())) };
}

/// A draw pass, as `UiTree::draw` runs one: a layer changed in it (recorded, a property set:
/// what drawing code does) tells what drew it at the pass's end, only if it changed after it
/// was last drawn, so a drawing recording then drawing a layer each pass doesn't draw itself
/// again forever, and one drawing a layer another drawing records later in the pass draws
/// again in the next frame.
pub(crate) struct DrawPass;

impl DrawPass {
    pub(crate) fn enter() -> DrawPass {
        DRAW_PASS.with(|pass| pass.borrow_mut().0 += 1);
        DrawPass
    }
}

impl Drop for DrawPass {
    fn drop(&mut self) {
        let changed = DRAW_PASS.with(|pass| {
            let mut pass = pass.borrow_mut();
            pass.0 -= 1;
            if pass.0 == 0 { std::mem::take(&mut pass.1) } else { Vec::new() }
        });
        for layer in changed.iter().filter_map(std::rc::Weak::upgrade) {
            if layer.generation.get() != layer.drawn_generation.get() {
                layer.version.handle().update(|version| version.wrapping_add(1));
            }
        }
    }
}

struct LayerCell {
    /// Counts its changes; `drawn_generation` is the count it was last drawn at.
    generation: Cell<u64>,
    drawn_generation: Cell<u64>,
    recording: RefCell<DisplayList>,
    /// Where the recording's (0, 0) is in it: a content recorded where its node draws.
    recorded_origin: Cell<Offset>,
    size: Cell<IntSize>,
    density: Cell<f32>,
    properties: RefCell<Properties>,
    released: Cell<bool>,
    /// Read where it's drawn and bumped when it changes, so what draws it draws again.
    version: compose_runtime::OwnedState<u64>,
}

/// A graphics layer, as Compose's `GraphicsLayer`: shared as a Kotlin reference is (a clone is
/// the same layer). Record into it with [`record`](GraphicsLayer::record) (or
/// [`ContentDrawScope::record_layer`] for a node's content), draw it with
/// [`DrawScope::draw_layer`]. A change of its recording or its properties draws again what
/// drew it.
#[derive(Clone)]
pub struct GraphicsLayer(Rc<LayerCell>);

macro_rules! properties {
    ($($(#[$doc:meta])* $field:ident, $set:ident: $ty:ty;)*) => {
        $(
            $(#[$doc])*
            pub fn $field(&self) -> $ty {
                self.0.properties.borrow().$field
            }
            #[doc = concat!("Sets [`", stringify!($field), "`](GraphicsLayer::", stringify!($field), ").")]
            pub fn $set(&self, $field: $ty) {
                self.update(|properties| properties.$field = $field);
            }
        )*
    };
}

impl Default for GraphicsLayer {
    fn default() -> Self {
        GraphicsLayer::new()
    }
}

impl GraphicsLayer {
    /// An empty layer, as `GraphicsContext.createGraphicsLayer()`.
    pub fn new() -> GraphicsLayer {
        GraphicsLayer(Rc::new(LayerCell {
            generation: Cell::new(0),
            drawn_generation: Cell::new(0),
            recording: RefCell::default(),
            recorded_origin: Cell::new(Offset::ZERO),
            size: Cell::new(IntSize::ZERO),
            density: Cell::new(1.0),
            properties: RefCell::default(),
            released: Cell::new(false),
            version: compose_runtime::mutable_state_of(0),
        }))
    }

    /// It changed: what drew it draws again, now outside a draw pass, at its end inside one
    /// ([`DrawPass`]).
    fn changed(&self) {
        let cell = &self.0;
        cell.generation.set(cell.generation.get().wrapping_add(1));
        let deferred = DRAW_PASS.with(|pass| {
            let mut pass = pass.borrow_mut();
            if pass.0 == 0 {
                return false;
            }
            if !pass.1.iter().any(|kept| std::ptr::eq(kept.as_ptr(), Rc::as_ptr(cell))) {
                pass.1.push(Rc::downgrade(cell));
            }
            true
        });
        if !deferred {
            cell.version.handle().update(|version| version.wrapping_add(1));
        }
    }

    fn update(&self, change: impl FnOnce(&mut Properties)) {
        let mut properties = self.0.properties.borrow_mut();
        let before = properties.clone();
        change(&mut properties);
        let changed = *properties != before;
        drop(properties);
        if changed {
            self.changed();
        }
    }

    /// Records `block` at `size`, in the layer's coordinates, with `scope`'s density and
    /// direction, as `record(density, layoutDirection, size) { … }`: what it drew before is
    /// replaced.
    pub fn record(
        &self,
        scope: &DrawScope<'_>,
        size: IntSize,
        block: impl FnOnce(&mut DrawScope<'_>),
    ) {
        let mut recording = DisplayList::default();
        let mut layer_scope = DrawScope {
            list: &mut recording,
            origin: Offset::ZERO,
            size: Size::new(size.width as f32, size.height as f32),
            density: scope.density,
            layout_direction: scope.layout_direction,
            locals: scope.locals,
        };
        block(&mut layer_scope);
        self.store(recording, Offset::ZERO, size, scope.density.density);
    }

    fn store(&self, recording: DisplayList, origin: Offset, size: IntSize, density: f32) {
        *self.0.recording.borrow_mut() = recording;
        self.0.recorded_origin.set(origin);
        self.0.size.set(size);
        self.0.density.set(density);
        self.0.released.set(false);
        self.changed();
    }

    /// Its size: what it was last recorded at, as `size`.
    pub fn size(&self) -> IntSize {
        self.0.size.get()
    }

    properties! {
        /// Where it's drawn in what draws it, as `topLeft`.
        top_left, set_top_left: IntOffset;
        /// What it scales and rotates around, in its own coordinates; `Offset::UNSPECIFIED`
        /// (the default) for its center, as `pivotOffset`.
        pivot_offset, set_pivot_offset: Offset;
        /// Its opacity, as `alpha`.
        alpha, set_alpha: f32;
        scale_x, set_scale_x: f32;
        scale_y, set_scale_y: f32;
        translation_x, set_translation_x: f32;
        translation_y, set_translation_y: f32;
        /// Degrees, clockwise, as `rotationZ`.
        rotation_z, set_rotation_z: f32;
        /// Pixels: a shadow of its outline under it above 0, as `shadowElevation`.
        shadow_elevation, set_shadow_elevation: f32;
        ambient_shadow_color, set_ambient_shadow_color: Color;
        spot_shadow_color, set_spot_shadow_color: Color;
        /// How it's composited with what's under it, as `blendMode`.
        blend_mode, set_blend_mode: BlendMode;
        /// What recolors it as it's composited, as `colorFilter`.
        color_filter, set_color_filter: Option<ColorFilter>;
        /// Whether it clips what it draws to its outline, as `clip`.
        clip, set_clip: bool;
        compositing_strategy, set_compositing_strategy: CompositingStrategy;
    }

    /// Its outline: what it clips to and casts a shadow of, its bounds unless set, as
    /// `outline`.
    pub fn outline(&self) -> Outline {
        let size = self.size();
        self.0.properties.borrow().outline.clone().unwrap_or(Outline::Rectangle(Rect::new(
            0.0,
            0.0,
            size.width as f32,
            size.height as f32,
        )))
    }

    /// Its outline, as `setOutline`.
    pub fn set_outline(&self, outline: Outline) {
        self.update(|properties| properties.outline = Some(outline));
    }

    /// A rectangle from `top_left` of `size` (`Size::UNSPECIFIED`: to its far corner), as
    /// `setRectOutline`.
    pub fn set_rect_outline(&self, top_left: Offset, size: Size) {
        let rect = self.outline_rect(top_left, size);
        self.set_outline(Outline::Rectangle(rect));
    }

    /// A rectangle with corners of `corner_radius`, as `setRoundRectOutline`.
    pub fn set_round_rect_outline(&self, top_left: Offset, size: Size, corner_radius: f32) {
        let rect = self.outline_rect(top_left, size);
        self.set_outline(Outline::Rounded(rect, [corner_radius; 4]));
    }

    /// `path`'s contents now, as `setPathOutline`.
    pub fn set_path_outline(&self, path: &crate::path::Path) {
        self.set_outline(Outline::generic(path));
    }

    fn outline_rect(&self, top_left: Offset, size: Size) -> Rect {
        let layer = self.size();
        let width = if size.width.is_nan() { layer.width as f32 - top_left.x } else { size.width };
        let height =
            if size.height.is_nan() { layer.height as f32 - top_left.y } else { size.height };
        Rect::new(top_left.x, top_left.y, top_left.x + width, top_left.y + height)
    }

    /// Whether it was released, as `isReleased`.
    pub fn is_released(&self) -> bool {
        self.0.released.get()
    }

    /// Drops its recording: it draws nothing until recorded again, as
    /// `GraphicsContext.releaseGraphicsLayer`.
    pub fn release(&self) {
        self.0.recording.borrow_mut().clear();
        self.0.released.set(true);
        self.changed();
    }

    /// Whether `other` is this layer.
    pub fn ptr_eq(&self, other: &GraphicsLayer) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for GraphicsLayer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "GraphicsLayer({:?})", self.size())
    }
}

impl compose_runtime::ComposeArg for GraphicsLayer {
    fn same(&self, other: &Self) -> bool {
        self.ptr_eq(other)
    }
}

/// A layer kept where it's composed, as `rememberGraphicsLayer()`. Named `remember_…` as it takes
/// a slot.
pub fn remember_graphics_layer() -> GraphicsLayer {
    compose_runtime::remember(GraphicsLayer::new)
}

impl ContentDrawScope<'_, '_> {
    /// Records into `layer` what `block` draws, the content included where it calls
    /// `draw_content`, at this drawing's size, as `layer.record { drawContent() }` in
    /// `drawWithContent`.
    pub fn record_layer(
        &mut self,
        layer: &GraphicsLayer,
        block: impl FnOnce(&mut ContentDrawScope<'_, '_>),
    ) {
        let mut recording = DisplayList::default();
        let size = self.scope.size;
        let origin = self.scope.origin;
        {
            let mut layer_scope = DrawScope {
                list: &mut recording,
                origin,
                size,
                density: self.scope.density,
                layout_direction: self.scope.layout_direction,
                locals: self.scope.locals,
            };
            let mut content =
                ContentDrawScope { scope: &mut layer_scope, content: &mut *self.content };
            block(&mut content);
        }
        let size = IntSize::new(size.width.round() as i32, size.height.round() as i32);
        layer.store(recording, origin, size, self.scope.density.density);
    }
}

impl DrawScope<'_> {
    /// Draws `layer` at its `top_left`, its properties applied, as `drawLayer`: its transform
    /// around its pivot, its shadow, its clip to its outline, then its recording in an offscreen
    /// buffer for its alpha (under `Auto` below 1, under `Offscreen` always), color filter or
    /// blend mode; under `ModulateAlpha`, its drawings' alphas multiplied instead. Nothing once
    /// released. What drew it draws again when it changes.
    pub fn draw_layer(&mut self, layer: &GraphicsLayer) {
        let cell = &layer.0;
        cell.version.get();
        cell.drawn_generation.set(cell.generation.get());
        if cell.released.get() {
            return;
        }
        let properties = cell.properties.borrow().clone();
        let size = cell.size.get();
        let size = Size::new(size.width as f32, size.height as f32);
        let origin =
            self.origin + Offset::new(properties.top_left.x as f32, properties.top_left.y as f32);
        let pivot = if properties.pivot_offset.x.is_nan() {
            Offset::new(size.width / 2.0, size.height / 2.0)
        } else {
            properties.pivot_offset
        };
        let shadow = properties.shadow_elevation.max(0.0);
        let outline = (properties.clip || shadow > 0.0).then(|| layer.outline().translate(origin));
        let painted =
            properties.blend_mode != BlendMode::SrcOver || properties.color_filter.is_some();
        let mut strategy = properties.compositing_strategy;
        if painted {
            strategy = CompositingStrategy::Offscreen;
        }
        let layer_properties = LayerProperties {
            alpha: properties.alpha.clamp(0.0, 1.0),
            pivot: pivot + origin,
            scale: (properties.scale_x, properties.scale_y),
            rotation: properties.rotation_z,
            translation: Offset::new(properties.translation_x, properties.translation_y),
            clip: properties.clip,
            outline,
            shadow_elevation: shadow,
            ambient_shadow_color: properties.ambient_shadow_color,
            spot_shadow_color: properties.spot_shadow_color,
            compositing_strategy: strategy,
            bounds: Rect::new(origin.x, origin.y, origin.x + size.width, origin.y + size.height),
            density: cell.density.get(),
        };
        for op in layer_properties.operations() {
            match op {
                // A filter or a blend mode goes into the offscreen buffer's paint.
                DrawOp::SaveLayerAlpha { bounds, alpha } if painted => {
                    let filter = properties.color_filter.as_ref();
                    self.save_layer(
                        Rect::new(
                            bounds.left - self.origin.x,
                            bounds.top - self.origin.y,
                            bounds.right - self.origin.x,
                            bounds.bottom - self.origin.y,
                        ),
                        alpha,
                        filter,
                        properties.blend_mode,
                    );
                }
                op => self.list.push(op),
            }
        }
        let modulated = layer_properties.modulated_alpha();
        self.replay_layer(&cell.recording.borrow(), origin - cell.recorded_origin.get(), modulated);
        for _ in 0..layer_properties.restores() {
            self.list.push(DrawOp::Restore);
        }
    }

    /// Copies `recording` moved by `by`, its shaders, filters and the groups it draws with it,
    /// its drawings' alphas multiplied by `modulated` if any.
    fn replay_layer(&mut self, recording: &DisplayList, by: Offset, modulated: Option<f32>) {
        let list = &mut *self.list;
        let bases = (list.shaders.len() as u32, list.color_filters.len() as u32);
        list.shaders.extend_from_slice(&recording.shaders);
        list.color_filters.extend_from_slice(&recording.color_filters);
        let moved = by != Offset::ZERO;
        if moved {
            list.push(DrawOp::Translate(by));
        }
        let start = list.ops.len();
        crate::display_list::extend_shifted(&mut list.ops, &recording.ops, bases);
        if let Some(alpha) = modulated {
            for op in &mut list.ops[start..] {
                match op {
                    DrawOp::DrawImage { alpha: image, .. } => *image *= alpha,
                    op => {
                        if let Some(paint) = op.paint_mut() {
                            *paint = paint.with_alpha(alpha);
                        }
                    }
                }
            }
        }
        list.boundaries.extend(
            recording
                .boundaries
                .iter()
                .map(|boundary| Boundary { at: boundary.at + start, key: boundary.key }),
        );
        list.laid_out_paragraphs.extend(recording.laid_out_paragraphs.iter().cloned());
        if moved {
            list.push(DrawOp::Translate(Offset::new(0.0 - by.x, 0.0 - by.y)));
        }
    }
}
