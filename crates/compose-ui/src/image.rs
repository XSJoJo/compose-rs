//! Bitmaps, as Compose's `ImageBitmap`: pixels drawn by the display list's `DrawImage`, decoded
//! by the platform's [`ImageDecoder`] (Skia on desktop, `BitmapFactory` on Android), which apps
//! reach through [`decode_image_bytes`] and [`remember_image_resource`], or made to draw into
//! ([`ImageBitmap::new`], with a [`Canvas`](crate::canvas::Canvas)).

use std::sync::{Arc, Mutex, RwLock};

use crate::colorspace::{ColorSpace, ColorSpaces};
use crate::graphics::Color;

/// How a bitmap's pixels are kept, as Compose's `ImageBitmapConfig`. compose-rs keeps every
/// bitmap's pixels as premultiplied 8-bit RGBA: the config is what the bitmap was made with.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ImageBitmapConfig {
    #[default]
    Argb8888,
    Alpha8,
    Rgb565,
    F16,
    Gpu,
}

/// A bitmap, as Compose's `ImageBitmap`: its size in pixels and its pixels, premultiplied RGBA,
/// a row after another. Shared (`Arc`): a clone copies nothing, and two clones are the same
/// image. A decoded or given bitmap's pixels never change; one made to draw into
/// ([`ImageBitmap::new`]) keeps what's drawn into it until its pixels are read, then draws it
/// all in one pass, into a copy if a renderer holds the pixels it drew; every clone sees it.
#[derive(Clone)]
pub struct ImageBitmap {
    width: u32,
    height: u32,
    pixels: Pixels,
}

#[derive(Clone)]
enum Pixels {
    Fixed(Arc<[u8]>),
    Drawable(Arc<Drawable>),
}

struct Drawable {
    /// Its pixels and the drawings not in them yet, behind one lock: a drawing waits there,
    /// drawn with the others at the next read, one canvas or thread at a time.
    drawing: Mutex<Drawing>,
    config: ImageBitmapConfig,
    has_alpha: bool,
    color_space: ColorSpace,
}

/// A drawable bitmap's pixels and what's to be drawn into them.
pub(crate) struct Drawing {
    /// The pixels, as last drawn: replaced by a changed copy when a renderer may hold them.
    pixels: Arc<[u8]>,
    /// What canvases drew since, waiting to be drawn at the next read ([`ImageBitmap::pixels`]):
    /// each canvas's run of drawings in a save of its own, after the canvas's transforms and
    /// clips. It names no drawable bitmap (a canvas draws a snapshot of one), so drawing it
    /// reads no other bitmap's lock.
    pub(crate) pending: crate::display_list::DisplayList,
    /// The canvas (its id, and its state's version) whose run `pending` ends in, and the saves
    /// to restore to close it: the next drawing of that canvas in that state joins the run.
    pub(crate) open: Option<(u64, u64, usize)>,
}

impl Drawing {
    /// Closes the open run: restores its saves.
    pub(crate) fn close(&mut self) {
        if let Some((_, _, saves)) = self.open.take() {
            for _ in 0..saves {
                self.pending.ops.push(crate::display_list::DrawOp::Restore);
            }
        }
    }
}

impl ImageBitmap {
    /// A bitmap of `width` by `height` premultiplied RGBA pixels (4 bytes each).
    ///
    /// # Panics
    ///
    /// If `pixels` isn't `width × height × 4` bytes.
    pub fn from_rgba_premultiplied(width: u32, height: u32, pixels: Vec<u8>) -> ImageBitmap {
        assert_eq!(
            pixels.len(),
            width as usize * height as usize * 4,
            "compose-ui: an image of {width} × {height} pixels needs {} bytes",
            width as usize * height as usize * 4
        );
        ImageBitmap { width, height, pixels: Pixels::Fixed(pixels.into()) }
    }

    /// A transparent sRGB bitmap of `width` by `height` to draw into, as
    /// `ImageBitmap(width, height)`.
    pub fn new(width: u32, height: u32) -> ImageBitmap {
        ImageBitmap::new_with(width, height, ImageBitmapConfig::Argb8888, true, &ColorSpaces::SRGB)
    }

    /// A transparent bitmap to draw into, as
    /// `ImageBitmap(width, height, config, hasAlpha, colorSpace)`: the config, alpha and space
    /// are kept (its pixels are 8-bit RGBA, drawn as sRGB, whatever they are).
    pub fn new_with(
        width: u32,
        height: u32,
        config: ImageBitmapConfig,
        has_alpha: bool,
        color_space: &ColorSpace,
    ) -> ImageBitmap {
        let pixels = vec![0; width as usize * height as usize * 4].into();
        let drawing = Drawing { pixels, pending: Default::default(), open: None };
        let drawable = Drawable {
            drawing: Mutex::new(drawing),
            config,
            has_alpha,
            color_space: color_space.clone(),
        };
        ImageBitmap { width, height, pixels: Pixels::Drawable(Arc::new(drawable)) }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// What it was made with, as `config`: `Argb8888` for a decoded bitmap.
    pub fn config(&self) -> ImageBitmapConfig {
        match &self.pixels {
            Pixels::Fixed(_) => ImageBitmapConfig::Argb8888,
            Pixels::Drawable(drawable) => drawable.config,
        }
    }

    /// Whether it has alpha, as `hasAlpha`.
    pub fn has_alpha(&self) -> bool {
        match &self.pixels {
            Pixels::Fixed(_) => true,
            Pixels::Drawable(drawable) => drawable.has_alpha,
        }
    }

    /// Its color space, as `colorSpace`: sRGB for a decoded bitmap.
    pub fn color_space(&self) -> &ColorSpace {
        match &self.pixels {
            Pixels::Fixed(_) => &ColorSpaces::SRGB,
            Pixels::Drawable(drawable) => &drawable.color_space,
        }
    }

    /// The pixels now, premultiplied RGBA: what was drawn into it is drawn first.
    pub fn pixels(&self) -> Arc<[u8]> {
        match &self.pixels {
            Pixels::Fixed(pixels) => pixels.clone(),
            Pixels::Drawable(drawable) => {
                let mut drawing = drawable.lock();
                self.draw_pending(&mut drawing);
                drawing.pixels.clone()
            }
        }
    }

    /// Draws what waits in `drawing` into its pixels, with the platform's rasterizer: in place
    /// if nothing else holds them, else into a copy (a renderer may hold the ones it drew).
    fn draw_pending(&self, drawing: &mut Drawing) {
        if drawing.pending.ops.is_empty() {
            return;
        }
        drawing.close();
        let list = std::mem::take(&mut drawing.pending);
        let Some(rasterizer) = crate::canvas::rasterizer() else {
            log::warn!(
                "compose-ui: no bitmap rasterizer: drawing into a bitmap draws nothing here"
            );
            return;
        };
        if Arc::get_mut(&mut drawing.pixels).is_none() {
            drawing.pixels = Arc::from(&drawing.pixels[..]);
        }
        let pixels = Arc::get_mut(&mut drawing.pixels).expect("compose-ui: pixels of its own");
        rasterizer.draw(&list, self.width, self.height, pixels);
    }

    /// Its pixels as they are now, as a bitmap that doesn't change: what a drawing of it into a
    /// bitmap reads, as a canvas reads a bitmap when it draws it.
    pub(crate) fn snapshot(&self) -> ImageBitmap {
        match &self.pixels {
            Pixels::Fixed(_) => self.clone(),
            Pixels::Drawable(_) => ImageBitmap {
                width: self.width,
                height: self.height,
                pixels: Pixels::Fixed(self.pixels()),
            },
        }
    }

    /// Runs `block` on what waits to be drawn into it, its lock held (one drawing at a time):
    /// `None`, and a warning, for a decoded bitmap, whose pixels don't change.
    pub(crate) fn with_drawing<R>(&self, block: impl FnOnce(&mut Drawing) -> R) -> Option<R> {
        let Pixels::Drawable(drawable) = &self.pixels else {
            log::warn!("compose-ui: a decoded bitmap can't be drawn into");
            return None;
        };
        Some(block(&mut drawable.lock()))
    }

    /// An identity for caches of its pixels: the same for clones of one image while its pixels
    /// don't change.
    pub fn id(&self) -> usize {
        match &self.pixels {
            Pixels::Fixed(pixels) => pixels.as_ptr() as usize,
            Pixels::Drawable(_) => self.pixels().as_ptr() as usize,
        }
    }

    /// A weak reference to the pixels now, for caches that mustn't keep the image alive.
    pub fn downgrade(&self) -> std::sync::Weak<[u8]> {
        match &self.pixels {
            Pixels::Fixed(pixels) => Arc::downgrade(pixels),
            Pixels::Drawable(_) => Arc::downgrade(&self.pixels()),
        }
    }

    /// Whether `other` is a clone of this image.
    pub fn same_image(&self, other: &ImageBitmap) -> bool {
        match (&self.pixels, &other.pixels) {
            (Pixels::Fixed(a), Pixels::Fixed(b)) => Arc::ptr_eq(a, b),
            (Pixels::Drawable(a), Pixels::Drawable(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }

    /// Reads a part of its pixels as unpremultiplied ARGB, as `readPixels`: the `width` by
    /// `height` pixels from `(start_x, start_y)` into `buffer` from `buffer_offset`, a row every
    /// `stride` values.
    ///
    /// # Panics
    ///
    /// If the part isn't in the bitmap or doesn't fit in the buffer.
    #[allow(clippy::too_many_arguments)]
    pub fn read_pixels(
        &self,
        buffer: &mut [u32],
        start_x: u32,
        start_y: u32,
        width: u32,
        height: u32,
        buffer_offset: usize,
        stride: usize,
    ) {
        assert!(
            start_x + width <= self.width && start_y + height <= self.height,
            "compose-ui: pixels to read past the bitmap's {} × {}",
            self.width,
            self.height
        );
        let pixels = self.pixels();
        for row in 0..height as usize {
            for column in 0..width as usize {
                let x = start_x as usize + column;
                let at = ((start_y as usize + row) * self.width as usize + x) * 4;
                let alpha = u32::from(pixels[at + 3]);
                let unpremultiply = |channel: u8| {
                    (u32::from(channel) * 255 + alpha / 2)
                        .checked_div(alpha)
                        .map_or(0, |value| value.min(255))
                };
                buffer[buffer_offset + row * stride + column] = alpha << 24
                    | unpremultiply(pixels[at]) << 16
                    | unpremultiply(pixels[at + 1]) << 8
                    | unpremultiply(pixels[at + 2]);
            }
        }
    }

    /// All its pixels as a [`PixelMap`], as `toPixelMap()`.
    pub fn to_pixel_map(&self) -> PixelMap {
        let size = self.width as usize * self.height as usize;
        self.to_pixel_map_with(0, 0, self.width, self.height, vec![0; size], 0, self.width as usize)
    }

    /// A part of its pixels read into `buffer`, as
    /// `toPixelMap(startX, startY, width, height, buffer, bufferOffset, stride)`.
    #[allow(clippy::too_many_arguments)]
    pub fn to_pixel_map_with(
        &self,
        start_x: u32,
        start_y: u32,
        width: u32,
        height: u32,
        mut buffer: Vec<u32>,
        buffer_offset: usize,
        stride: usize,
    ) -> PixelMap {
        self.read_pixels(&mut buffer, start_x, start_y, width, height, buffer_offset, stride);
        PixelMap { buffer, width, height, buffer_offset, stride }
    }

    /// Readies it to be drawn, as `prepareToDraw`: nothing to do here.
    pub fn prepare_to_draw(&self) {}
}

/// Pixels read from a bitmap, as Compose's `PixelMap`: unpremultiplied ARGB, a row every
/// `stride` values from `buffer_offset`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PixelMap {
    buffer: Vec<u32>,
    width: u32,
    height: u32,
    buffer_offset: usize,
    stride: usize,
}

impl PixelMap {
    /// The color at `(x, y)` of the part read, as `get(x, y)`.
    pub fn get(&self, x: i32, y: i32) -> Color {
        Color::from_argb(self.buffer[self.buffer_offset + y as usize * self.stride + x as usize])
    }

    pub fn buffer(&self) -> &[u32] {
        &self.buffer
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn buffer_offset(&self) -> usize {
        self.buffer_offset
    }

    pub fn stride(&self) -> usize {
        self.stride
    }
}

impl Drawable {
    fn lock(&self) -> std::sync::MutexGuard<'_, Drawing> {
        self.drawing.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl PartialEq for ImageBitmap {
    fn eq(&self, other: &Self) -> bool {
        self.same_image(other)
    }
}

impl std::fmt::Debug for ImageBitmap {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "ImageBitmap({} × {})", self.width, self.height)
    }
}

impl compose_runtime::ComposeArg for ImageBitmap {
    fn same(&self, other: &Self) -> bool {
        self.same_image(other)
    }
}

/// Decodes encoded images (PNG, JPEG, WebP, …) into bitmaps: the platform's, as the text
/// measurer is (Skia's codecs on desktop, `BitmapFactory` on Android), set once by the host
/// ([`set_image_decoder`]).
pub trait ImageDecoder: Send + Sync {
    /// The image `bytes` encode, or `None` if they're not one the platform decodes.
    fn decode(&self, bytes: &[u8]) -> Option<ImageBitmap>;
}

/// The platform's decoder, for every thread (images may be decoded off the main thread).
static DECODER: RwLock<Option<Arc<dyn ImageDecoder>>> = RwLock::new(None);

/// Sets the decoder [`decode_image_bytes`] uses: the platform host's, at startup.
pub fn set_image_decoder(decoder: Arc<dyn ImageDecoder>) {
    *DECODER.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(decoder);
}

/// Decodes an encoded image with the platform's decoder, as Android's
/// `BitmapFactory.decodeByteArray(…).asImageBitmap()`: `None` if the bytes aren't an image the
/// platform decodes, or no platform set a decoder (headless tests).
pub fn decode_image_bytes(bytes: &[u8]) -> Option<ImageBitmap> {
    let decoder = DECODER.read().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
    decoder?.decode(bytes)
}

/// An image bundled with the app (`include_bytes!("icon.png")`), decoded once where it's
/// composed, as Compose's `ImageBitmap.imageResource(id)`: `None` if the platform can't decode
/// it. Named `remember_…` as it takes a slot: `#[composable]` puts a group around a call by that
/// name in a branch, so calling it conditionally keeps later state in place.
pub fn remember_image_resource(bytes: &'static [u8]) -> Option<ImageBitmap> {
    compose_runtime::remember_keyed((bytes.as_ptr() as usize, bytes.len()), || {
        decode_image_bytes(bytes)
    })
}
