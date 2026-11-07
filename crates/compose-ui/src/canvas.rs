//! Drawing into bitmaps, as Compose's `Canvas(imageBitmap)` and `CanvasDrawScope`
//! (docs/06-ui.md, "Offscreen bitmaps"): what's drawn is a display list's operations, which the
//! platform's [`BitmapRasterizer`] (Skia's on desktop) draws into the bitmap's pixels.

use std::sync::{Arc, RwLock};

use crate::alignment::LayoutDirection;
use crate::color_filter::ColorFilter;
use crate::display_list::{DisplayList, DrawOp};
use crate::draw::DrawScope;
use crate::graphics::Paint;
use crate::image::ImageBitmap;
use crate::path::Path;
use crate::unit::{Density, IntOffset, IntSize, Offset, Rect, Size};

/// Draws display lists into bitmaps' pixels: the platform's, set once by the host
/// ([`set_bitmap_rasterizer`]), as the image decoder is.
pub trait BitmapRasterizer: Send + Sync {
    /// Draws `list`'s operations over `pixels` (premultiplied RGBA, `width` × `height`, a row
    /// after another), in place. Its paragraphs aren't drawn: a bitmap's canvas has no text
    /// measurer.
    fn draw(&self, list: &DisplayList, width: u32, height: u32, pixels: &mut [u8]);
}

static RASTERIZER: RwLock<Option<Arc<dyn BitmapRasterizer>>> = RwLock::new(None);

/// Sets the rasterizer that bitmaps' canvases draw with: the platform host's, at startup.
pub fn set_bitmap_rasterizer(rasterizer: Arc<dyn BitmapRasterizer>) {
    *RASTERIZER.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(rasterizer);
}

/// Draws `list` into `image`'s pixels with the platform's rasterizer: nothing, and a warning,
/// where no platform set one (headless tests; Android, for now).
pub(crate) fn rasterize(image: &ImageBitmap, list: &DisplayList) {
    let rasterizer = RASTERIZER.read().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    let Some(rasterizer) = rasterizer else {
        log::warn!("compose-ui: no bitmap rasterizer: drawing into a bitmap draws nothing here");
        return;
    };
    let (width, height) = (image.width(), image.height());
    image.update_pixels(|pixels| rasterizer.draw(list, width, height, pixels));
}

/// Immediate drawing into a bitmap, as Compose's `Canvas(imageBitmap)`: each drawing is in the
/// bitmap's pixels when the call returns, but what's drawn into a layer (`save_layer`), drawn
/// when the layer is restored. Its transforms and clips last until the `restore` of the save
/// before them, as a canvas's. Paints name the shaders and color filters of the canvas
/// ([`paint_with_color_filter`](Canvas::paint_with_color_filter)), as a `DrawScope`'s name its
/// list's.
///
/// Each drawing copies the bitmap's pixels once (a renderer may hold the ones it drew):
/// [`CanvasDrawScope::draw`] draws a whole recording with one copy.
pub struct Canvas {
    image: ImageBitmap,
    /// The shaders and color filters its paints name.
    tables: DisplayList,
    /// The state before any save (transforms and clips), then each open save: its `Save` or
    /// layer, and what followed it (only transforms and clips outside a layer, everything in
    /// one).
    frames: Vec<Frame>,
}

struct Frame {
    ops: Vec<DrawOp>,
    layer: bool,
}

impl Canvas {
    /// A canvas drawing into `image`, as `Canvas(image)`.
    pub fn new(image: &ImageBitmap) -> Canvas {
        Canvas {
            image: image.clone(),
            tables: DisplayList::default(),
            frames: vec![Frame { ops: Vec::new(), layer: false }],
        }
    }

    /// The bitmap it draws into.
    pub fn image(&self) -> &ImageBitmap {
        &self.image
    }

    /// `paint` drawing through `color_filter`, as `Paint.colorFilter`.
    pub fn paint_with_color_filter(&mut self, paint: Paint, color_filter: ColorFilter) -> Paint {
        paint.with_color_filter(self.tables.push_color_filter(color_filter))
    }

    /// Saves the transform and clip, as `save`.
    pub fn save(&mut self) {
        self.frames.push(Frame { ops: vec![DrawOp::Save], layer: false });
    }

    /// Saves, and draws what follows into a layer bounded by `bounds`, composited at the
    /// matching restore with `paint`'s alpha, color filter and blend mode, as
    /// `saveLayer(bounds, paint)`.
    pub fn save_layer(&mut self, bounds: Rect, paint: Paint) {
        let op = DrawOp::SaveLayer {
            bounds,
            alpha: paint.color.alpha(),
            color_filter: paint.color_filter(),
            blend_mode: paint.blend_mode,
        };
        self.frames.push(Frame { ops: vec![op], layer: true });
    }

    /// Undoes what followed the last save, as `restore`; a layer is drawn now. A restore with
    /// no save does nothing, as Skia's.
    pub fn restore(&mut self) {
        if self.frames.len() == 1 {
            return;
        }
        let Some(mut frame) = self.frames.pop() else { return };
        let in_layer = self.frames.iter().any(|frame| frame.layer);
        if in_layer {
            frame.ops.push(DrawOp::Restore);
            self.frames.last_mut().expect("the root frame").ops.extend(frame.ops);
        } else if frame.layer {
            frame.ops.push(DrawOp::Restore);
            self.rasterize(frame.ops);
        }
    }

    /// Runs `block` between a save and its restore, as `withSave`.
    pub fn with_save(&mut self, block: impl FnOnce(&mut Canvas)) {
        self.save();
        block(self);
        self.restore();
    }

    /// Runs `block` in a layer, as `withSaveLayer`.
    pub fn with_save_layer(&mut self, bounds: Rect, paint: Paint, block: impl FnOnce(&mut Canvas)) {
        self.save_layer(bounds, paint);
        block(self);
        self.restore();
    }

    fn state(&mut self, op: DrawOp) {
        self.frames.last_mut().expect("the root frame").ops.push(op);
    }

    /// Moves what follows by `(dx, dy)`, as `translate`.
    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.state(DrawOp::Translate(Offset::new(dx, dy)));
    }

    /// Scales what follows about the origin, as `scale(sx, sy)`.
    pub fn scale(&mut self, sx: f32, sy: f32) {
        self.state(DrawOp::Transform {
            pivot: Offset::ZERO,
            scale: (sx, sy),
            rotation: 0.0,
            translation: Offset::ZERO,
        });
    }

    /// Scales what follows about `(pivot_x, pivot_y)`, as `Canvas.scale(sx, sy, pivotX, pivotY)`.
    pub fn scale_around(&mut self, sx: f32, sy: f32, pivot_x: f32, pivot_y: f32) {
        self.translate(pivot_x, pivot_y);
        self.scale(sx, sy);
        self.translate(-pivot_x, -pivot_y);
    }

    /// Rotates what follows by `degrees` (clockwise) about the origin, as `rotate(degrees)`.
    pub fn rotate(&mut self, degrees: f32) {
        self.state(DrawOp::Transform {
            pivot: Offset::ZERO,
            scale: (1.0, 1.0),
            rotation: degrees,
            translation: Offset::ZERO,
        });
    }

    /// Rotates what follows about `(pivot_x, pivot_y)`, as
    /// `Canvas.rotate(degrees, pivotX, pivotY)`.
    pub fn rotate_around(&mut self, degrees: f32, pivot_x: f32, pivot_y: f32) {
        self.translate(pivot_x, pivot_y);
        self.rotate(degrees);
        self.translate(-pivot_x, -pivot_y);
    }

    /// Clips what follows to `rect`, as `clipRect(rect)`.
    pub fn clip_rect(&mut self, rect: Rect) {
        self.state(DrawOp::ClipRect(rect));
    }

    /// Clips what follows to `path`, as `clipPath(path)`.
    pub fn clip_path(&mut self, path: &Path) {
        self.state(DrawOp::ClipPath { path: path.data(), offset: Offset::ZERO });
    }

    pub fn draw_line(&mut self, from: Offset, to: Offset, paint: Paint) {
        self.draw(DrawOp::DrawLine(from, to, paint));
    }

    pub fn draw_rect(&mut self, rect: Rect, paint: Paint) {
        self.draw(DrawOp::DrawRect(rect, paint));
    }

    /// A rectangle with corners of radii `radius_x` and `radius_y`, as `drawRoundRect`.
    pub fn draw_round_rect(&mut self, rect: Rect, radius_x: f32, radius_y: f32, paint: Paint) {
        if radius_x == radius_y {
            self.draw(DrawOp::DrawRoundRect(rect, [radius_x; 4], paint));
        } else {
            let path = Path::new();
            let radius = crate::geometry::CornerRadius::new(radius_x, radius_y);
            path.add_round_rect(crate::geometry::RoundRect::new(rect, radius));
            self.draw_path(&path, paint);
        }
    }

    pub fn draw_oval(&mut self, rect: Rect, paint: Paint) {
        self.draw_arc(rect, 0.0, 360.0, false, paint);
    }

    pub fn draw_circle(&mut self, center: Offset, radius: f32, paint: Paint) {
        let oval =
            Rect::new(center.x - radius, center.y - radius, center.x + radius, center.y + radius);
        self.draw_oval(oval, paint);
    }

    /// An arc of the oval in `rect` from `start_angle` degrees over `sweep_angle`, closed
    /// through the center if `use_center`, as `drawArc`.
    pub fn draw_arc(
        &mut self,
        rect: Rect,
        start_angle: f32,
        sweep_angle: f32,
        use_center: bool,
        paint: Paint,
    ) {
        self.draw(DrawOp::DrawArc { oval: rect, start_angle, sweep_angle, use_center, paint });
    }

    pub fn draw_path(&mut self, path: &Path, paint: Paint) {
        let path = path.data();
        self.draw(DrawOp::DrawPath { path, offset: Offset::ZERO, scale: (1.0, 1.0), paint });
    }

    /// `image` with its top left at `top_left`, at the paint's alpha and through its color
    /// filter, as `drawImage`.
    pub fn draw_image(&mut self, image: &ImageBitmap, top_left: Offset, paint: Paint) {
        let (width, height) = (image.width() as f32, image.height() as f32);
        self.draw(DrawOp::DrawImage {
            image: image.clone(),
            src: Rect::new(0.0, 0.0, width, height),
            dst: Rect::new(top_left.x, top_left.y, top_left.x + width, top_left.y + height),
            alpha: paint.color.alpha(),
            color_filter: paint.color_filter(),
        });
    }

    /// The part of `image` at `src_offset` of `src_size` drawn into the part of the canvas at
    /// `dst_offset` of `dst_size`, as `drawImageRect`.
    pub fn draw_image_rect(
        &mut self,
        image: &ImageBitmap,
        src_offset: IntOffset,
        src_size: IntSize,
        dst_offset: IntOffset,
        dst_size: IntSize,
        paint: Paint,
    ) {
        let rect = |offset: IntOffset, size: IntSize| {
            Rect::new(
                offset.x as f32,
                offset.y as f32,
                (offset.x + size.width) as f32,
                (offset.y + size.height) as f32,
            )
        };
        self.draw(DrawOp::DrawImage {
            image: image.clone(),
            src: rect(src_offset, src_size),
            dst: rect(dst_offset, dst_size),
            alpha: paint.color.alpha(),
            color_filter: paint.color_filter(),
        });
    }

    /// Draws `op` now, or keeps it for the layer it's in.
    fn draw(&mut self, op: DrawOp) {
        if self.frames.iter().any(|frame| frame.layer) {
            self.frames.last_mut().expect("the root frame").ops.push(op);
        } else {
            self.rasterize(vec![op]);
        }
    }

    /// Draws `ops` into the bitmap after the open saves' transforms and clips.
    fn rasterize(&mut self, ops: Vec<DrawOp>) {
        let mut list = DisplayList {
            shaders: self.tables.shaders.clone(),
            color_filters: self.tables.color_filters.clone(),
            ..DisplayList::default()
        };
        for frame in &self.frames {
            list.ops.extend(frame.ops.iter().cloned());
        }
        list.ops.extend(ops);
        rasterize(&self.image, &list);
    }

    /// Draws `recording` (a `DrawScope`'s), its shaders and filters taken into the canvas's.
    pub(crate) fn draw_recording(&mut self, recording: DisplayList) {
        let (shaders, filters) =
            (self.tables.shaders.len() as u32, self.tables.color_filters.len() as u32);
        let mut ops = recording.ops;
        for op in &mut ops {
            if let Some(paint) = op.paint_mut()
                && let Some(shader) = paint.shader
            {
                paint.shader =
                    Some(crate::brush::ShaderIndex::new(shader.index() as u32 + shaders));
            }
            if let Some(filter) = op.color_filter_mut()
                && let Some(index) = *filter
            {
                *filter = Some(crate::color_filter::ColorFilterIndex::new(
                    index.index() as u32 + filters,
                ));
            }
        }
        self.tables.shaders.extend(recording.shaders);
        self.tables.color_filters.extend(recording.color_filters);
        if self.frames.iter().any(|frame| frame.layer) {
            self.frames.last_mut().expect("the root frame").ops.extend(ops);
        } else {
            self.rasterize(ops);
        }
    }
}

/// Draws with a `DrawScope` into a [`Canvas`], as Compose's `CanvasDrawScope`.
pub struct CanvasDrawScope;

impl CanvasDrawScope {
    /// Runs `block` with a `DrawScope` of `size`, `density` and `layout_direction`, what it
    /// draws drawn into `canvas` (after its transforms and clips) at once, as
    /// `CanvasDrawScope().draw(density, layoutDirection, canvas, size) { … }`.
    pub fn draw(
        canvas: &mut Canvas,
        density: Density,
        layout_direction: LayoutDirection,
        size: Size,
        block: impl FnOnce(&mut DrawScope<'_>),
    ) {
        let mut recording = DisplayList::default();
        let mut scope = DrawScope {
            list: &mut recording,
            origin: Offset::ZERO,
            size,
            density,
            layout_direction,
            locals: Default::default(),
        };
        block(&mut scope);
        canvas.draw_recording(recording);
    }
}
