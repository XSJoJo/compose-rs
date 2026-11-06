//! Vector painters drawn by Skia, as Compose's `VectorTest` checks them on a device. Compose's
//! tests compose their vectors with `Path` and `Group` composables; compose-rs builds the same
//! vectors with `ImageVector::builder` (vector composition isn't here yet).

#![allow(non_snake_case)]

use std::cell::Cell;
use std::rc::Rc;

use compose::foundation::layout::*;
use compose::foundation::{PainterImage, VectorImage};
use compose::material3::Icon;
use compose::runtime::*;
use compose::test::*;
use compose::ui::{
    Alignment, BlendMode, Brush, Color, ColorFilter, CompositingStrategy, ContentScale, Dp,
    GraphicsLayerScope, ImageVector, IntSize, LOCAL_LAYOUT_DIRECTION, LayoutDirection, Modifier,
    Painter, StrokeCap, UnitExt, VectorGroup, VectorPainter, VectorPath, path_data,
    remember_vector_painter,
};

const TAG: &str = "vector";

fn painted(size: i32, content: impl Fn() + 'static) -> ComposeTestRule {
    set_capture_renderer(compose_skia::raster_colors);
    ComposeTestRule::sized(IntSize::new(size, size), content)
}

fn capture(ui: &mut ComposeTestRule) -> CapturedImage {
    ui.on_node_with_tag(TAG).capture_to_image()
}

/// The path data of a rectangle from (0, 0) to (`width`, `height`).
fn rectangle(width: f32, height: f32) -> Vec<compose::ui::PathNode> {
    path_data(|path| {
        path.line_to(width, 0.0).line_to(width, height).line_to(0.0, height).close();
    })
}

/// A `size` px vector filled with `fill` (density 1), as the test's `createTestVectorPainter`.
fn square(size: f32, fill: Brush, tint: Option<(Color, BlendMode)>) -> ImageVector {
    let mut builder = ImageVector::builder(Dp(size), Dp(size), size, size);
    if let Some((color, mode)) = tint {
        builder.tint_color(color).tint_blend_mode(mode);
    }
    builder
        .add_path(VectorPath {
            path_data: rectangle(size, size),
            fill: Some(fill),
            ..Default::default()
        })
        .build()
}

/// A node of `size` px painting `vector`, through `filter`, placed by `alignment`.
#[composable]
fn Painted(vector: ImageVector, size: f32, alignment: Alignment, filter: Option<ColorFilter>) {
    let painter = remember_vector_painter(&vector);
    Box!(
        modifier = Modifier.test_tag(TAG).size(Dp(size)).paint_with(
            painter,
            true,
            alignment,
            ContentScale::Inside,
            1.0,
            filter
        )
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testVectorTint, testVectorIntrinsicTint, testVectorAlignment
#[test]
fn a_vector_is_tinted_by_its_filter_or_its_own_tint() {
    let filter = Some(ColorFilter::tint(Color::CYAN));
    let mut ui = painted(450, move || {
        Painted(square(200.0, Brush::solid(Color::BLACK), None), 200.0, Alignment::CENTER, filter)
    });
    capture(&mut ui).assert_pixel_color(Color::CYAN, 100, 100);
    let mut ui = painted(450, || {
        let own = Some((Color::MAGENTA, BlendMode::SrcIn));
        Painted(square(200.0, Brush::solid(Color::BLACK), own), 200.0, Alignment::CENTER, None)
    });
    capture(&mut ui).assert_pixel_color(Color::MAGENTA, 100, 100);
    let mut ui = painted(450, move || {
        Painted(
            square(200.0, Brush::solid(Color::BLACK), None),
            450.0,
            Alignment::BOTTOM_END,
            filter,
        )
    });
    capture(&mut ui).assert_pixel_color(Color::CYAN, 430, 430);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testVectorClipPath
#[test]
fn a_group_is_clipped_then_rotated() {
    let size = 200.0;
    let vector = ImageVector::builder(Dp(size), Dp(size), size, size)
        .add_path(VectorPath {
            path_data: rectangle(size, size),
            fill: Some(Brush::solid(Color::CYAN)),
            ..Default::default()
        })
        .group(
            VectorGroup {
                // The top half, rotated a half turn: the bottom half black.
                clip_path_data: rectangle(size, size / 2.0),
                pivot_x: size / 2.0,
                pivot_y: size / 2.0,
                rotation: 180.0,
                ..Default::default()
            },
            |group| {
                group.add_path(VectorPath {
                    path_data: rectangle(size, size),
                    fill: Some(Brush::solid(Color::BLACK)),
                    ..Default::default()
                });
            },
        )
        .build();
    let mut ui = painted(200, move || Painted(vector.clone(), size, Alignment::CENTER, None));
    let image = capture(&mut ui);
    image.assert_pixel_color(Color::CYAN, 100, 50);
    image.assert_pixel_color(Color::BLACK, 100, 150);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testVectorZeroSizeDoesNotCrash, testVectorZeroWidthDoesNotCrash, testVectorZeroHeightDoesNotCrash
#[test]
fn a_vector_drawn_at_no_size_draws_nothing() {
    for (width, height) in [(0.0, 0.0), (0.0, 100.0), (50.0, 0.0)] {
        let mut ui = painted(200, move || {
            let painter = remember_vector_painter(&square(200.0, Brush::solid(Color::BLACK), None));
            Box!(modifier = Modifier.width(Dp(width)).height(Dp(height)).paint(painter));
        });
        ui.wait_for_idle();
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testImageVectorChangeOnStateChange
#[test]
fn a_vector_image_follows_the_vector_it_is_given() {
    let triangle = |first: (f32, f32)| {
        ImageVector::builder(48.dp(), 48.dp(), 24.0, 24.0)
            .add_path(VectorPath {
                path_data: path_data(|path| {
                    path.line_to(first.0, first.1).line_to(24.0, 24.0).line_to(0.0, 0.0).close();
                }),
                fill: Some(Brush::solid(Color::BLACK)),
                ..Default::default()
            })
            .build()
    };
    let (upper, lower) = (triangle((24.0, 0.0)), triangle((0.0, 24.0)));
    let which = mutable_state_of(false);
    let handle = which.handle();
    let mut ui = painted(100, move || {
        let vector = if handle.get() { upper.clone() } else { lower.clone() };
        VectorImage!(
            vector,
            None::<&str>,
            modifier = Modifier.test_tag(TAG).size(48.dp()).background(Color::RED),
            alignment = Alignment::TOP_START,
            content_scale = ContentScale::FillHeight
        );
    });
    let (red, black) = (Color::RED, Color::BLACK);
    let check = |ui: &mut ComposeTestRule, upper_right: Color, lower_left: Color| {
        let image = capture(ui);
        let (width, height) = (image.width(), image.height());
        image.assert_pixel_color(upper_right, width - 2, 0);
        image.assert_pixel_color(upper_right, width - 1, height - 4);
        image.assert_pixel_color(lower_left, 0, 2);
        image.assert_pixel_color(lower_left, width - 4, height - 2);
    };
    check(&mut ui, red, black);
    which.set(true);
    ui.wait_for_idle();
    check(&mut ui, black, red);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testDrawWithoutColorFilterAfterPreviouslyConfigured, testDrawWithColorFilterAfterNotPreviouslyConfigured
#[test]
fn a_filter_given_then_taken_away_and_back_tints_each_time() {
    let tint = mutable_state_of(Some(ColorFilter::tint(Color::GREEN)));
    let handle = tint.handle();
    let vector = square(24.0, Brush::solid(Color::BLUE), None);
    let mut ui = painted(100, move || {
        let painter = remember_vector_painter(&vector);
        PainterImage!(
            painter,
            None::<&str>,
            modifier = Modifier.test_tag(TAG).background(Color::RED),
            content_scale = ContentScale::FillBounds,
            color_filter = handle.get()
        );
    });
    for (filter, color) in [(None, Color::BLUE), (Some(Color::GREEN), Color::GREEN)] {
        capture(&mut ui).assert_pixels(Some(IntSize::new(24, 24)), |_| {
            Some(if tint.get().is_some() { Color::GREEN } else { Color::BLUE })
        });
        tint.set(filter.map(ColorFilter::tint));
        ui.wait_for_idle();
        capture(&mut ui).assert_pixels(None, |_| Some(color));
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testDrawWhenVectorHasOneColorWithAlpha
#[test]
fn a_translucent_vector_composites_over_what_is_under_it() {
    let brush = Color::BLUE.with_alpha(0.5);
    let mut ui = painted(100, move || {
        let painter = remember_vector_painter(&square(24.0, Brush::solid(brush), None));
        PainterImage!(
            painter,
            None::<&str>,
            modifier = Modifier.test_tag(TAG).background(Color::RED)
        );
    });
    let expected = brush.composite_over(Color::RED);
    capture(&mut ui).assert_pixels(Some(IntSize::new(24, 24)), |_| Some(expected));
}

/// The vector of a solid color brush and of a gradient of that one color, tinted by `own` (the
/// vector's tint mode) or `filter`, over a gradient background in an offscreen layer: the same
/// pixels, as the test's `verifyAlphaMaskWithBlendModes`.
fn same_pixels_for_a_solid_and_a_one_color_gradient(own: BlendMode, filter: Option<ColorFilter>) {
    let images: Vec<Vec<Color>> =
        [Brush::solid(Color::BLUE), Brush::horizontal_gradient([Color::BLUE, Color::BLUE])]
            .into_iter()
            .map(|brush| {
                let vector = square(24.0, brush, Some((Color::CYAN, own)));
                let mut ui = painted(100, move || {
                    let painter = remember_vector_painter(&vector);
                    let background = Brush::horizontal_gradient([
                        Color::TRANSPARENT,
                        Color::YELLOW,
                        Color::TRANSPARENT,
                    ]);
                    PainterImage!(
                        painter,
                        None::<&str>,
                        modifier = Modifier
                            .test_tag(TAG)
                            .background(background)
                            .graphics_layer_with(GraphicsLayerScope {
                                compositing_strategy: CompositingStrategy::Offscreen,
                                ..GraphicsLayerScope::default()
                            }),
                        content_scale = ContentScale::FillBounds,
                        color_filter = filter
                    );
                });
                capture(&mut ui).pixels().to_vec()
            })
            .collect();
    assert_eq!(images[0], images[1], "{own:?} {filter:?}");
}

const MODES: [BlendMode; 29] = [
    BlendMode::Clear,
    BlendMode::Src,
    BlendMode::Dst,
    BlendMode::SrcOver,
    BlendMode::DstOver,
    BlendMode::SrcIn,
    BlendMode::DstIn,
    BlendMode::SrcOut,
    BlendMode::DstOut,
    BlendMode::SrcAtop,
    BlendMode::DstAtop,
    BlendMode::Xor,
    BlendMode::Plus,
    BlendMode::Modulate,
    BlendMode::Screen,
    BlendMode::Overlay,
    BlendMode::Darken,
    BlendMode::Lighten,
    BlendMode::ColorDodge,
    BlendMode::ColorBurn,
    BlendMode::Hardlight,
    BlendMode::Softlight,
    BlendMode::Difference,
    BlendMode::Exclusion,
    BlendMode::Multiply,
    BlendMode::Hue,
    BlendMode::Saturation,
    BlendMode::Color,
    BlendMode::Luminosity,
];

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testAlphaMaskWithIntrinsicClearBlendMode and the other 28 intrinsic modes
#[test]
fn a_vectors_own_tint_draws_alike_whatever_its_brush() {
    for mode in MODES {
        same_pixels_for_a_solid_and_a_one_color_gradient(mode, None);
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testAlphaMaskWithDrawClearBlendMode and the other 28 drawn modes
#[test]
fn a_drawn_tint_draws_alike_whatever_the_brush() {
    for mode in MODES {
        let filter = ColorFilter::tint_with(Color::YELLOW, mode);
        same_pixels_for_a_solid_and_a_one_color_gradient(BlendMode::SrcIn, Some(filter));
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testVectorScaleNonUniformly
#[test]
fn a_vector_stretches_to_fill_the_bounds() {
    let mut ui = painted(300, || {
        let painter = remember_vector_painter(&square(24.0, Brush::solid(Color::BLUE), None));
        PainterImage!(
            painter,
            None::<&str>,
            modifier = Modifier.test_tag(TAG).size((192.dp(), 48.dp())).background(Color::RED),
            content_scale = ContentScale::FillBounds
        );
    });
    capture(&mut ui).assert_pixels(Some(IntSize::new(192, 48)), |_| Some(Color::BLUE));
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testVectorChangeSize
#[test]
fn a_new_vector_size_draws_at_that_size() {
    let state = mutable_state_of((200.0f32, Color::MAGENTA));
    let handle = state.handle();
    let mut ui = painted(400, move || {
        let (size, color) = handle.get();
        let painter = remember_vector_painter(&square(size, Brush::solid(color), None));
        Box!(
            modifier = Modifier.test_tag(TAG).size(400.dp()).background(Color::RED).paint_with(
                painter,
                true,
                Alignment::TOP_START,
                ContentScale::Inside,
                1.0,
                None::<ColorFilter>
            )
        );
    });
    let image = capture(&mut ui);
    image.assert_pixel_color(Color::MAGENTA, 100, 100);
    image.assert_pixel_color(Color::RED, 300, 300);
    state.set((400.0, Color::CYAN));
    ui.wait_for_idle();
    let image = capture(&mut ui);
    image.assert_pixel_color(Color::CYAN, 100, 100);
    image.assert_pixel_color(Color::CYAN, 300, 300);
    state.set((50.0, Color::YELLOW));
    ui.wait_for_idle();
    let image = capture(&mut ui);
    image.assert_pixel_color(Color::YELLOW, 10, 10);
    image.assert_pixel_color(Color::RED, 100, 100);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testVectorMirror
#[test]
fn an_auto_mirrored_vector_is_mirrored_right_to_left() {
    let size = 20.0;
    let vector = ImageVector::builder(Dp(size), Dp(size), size, size)
        .auto_mirror(true)
        .add_path(VectorPath {
            path_data: rectangle(size / 2.0, size),
            fill: Some(Brush::solid(Color::RED)),
            ..Default::default()
        })
        .path(VectorPath { fill: Some(Brush::solid(Color::BLUE)), ..Default::default() }, |path| {
            path.move_to(size / 2.0, 0.0).line_to(size, 0.0).line_to(size, size);
            path.line_to(size / 2.0, size).close();
        })
        .build();
    let mut ui = painted(100, move || {
        let vector = vector.clone();
        CompositionLocalProvider(
            LOCAL_LAYOUT_DIRECTION.provides(LayoutDirection::Rtl),
            move || {
                VectorImage!(vector.clone(), None::<&str>, modifier = Modifier.test_tag(TAG));
            },
        );
    });
    let image = capture(&mut ui);
    let (width, height) = (image.width(), image.height());
    for (x, y) in [(2, 2), (2, height - 3), (width / 2 - 3, 2), (width / 2 - 3, height - 3)] {
        image.assert_pixel_color(Color::BLUE, x, y);
    }
    let red =
        [(width - 3, 2), (width - 3, height - 3), (width / 2 + 3, 2), (width / 2 + 3, height - 3)];
    for (x, y) in red {
        image.assert_pixel_color(Color::RED, x, y);
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/vector/VectorTest.kt, testVectorStrokeWidth
#[test]
fn a_vectors_stroke_is_as_wide_as_it_says() {
    let width = mutable_state_of(100.0f32);
    let handle = width.handle();
    let mut ui = painted(200, move || {
        let stroke = handle.get();
        let vector = ImageVector::builder(200.dp(), 200.dp(), 200.0, 200.0)
            .add_path(VectorPath {
                path_data: rectangle(200.0, 200.0),
                fill: Some(Brush::solid(Color::BLUE)),
                ..Default::default()
            })
            .path(
                VectorPath {
                    stroke: Some(Brush::solid(Color::YELLOW)),
                    stroke_line_width: stroke,
                    stroke_line_cap: StrokeCap::Butt,
                    ..Default::default()
                },
                |path| {
                    path.move_to(0.0, 0.0).line_to(200.0, 0.0);
                },
            )
            .build();
        Painted(vector, 200.0, Alignment::CENTER, None);
    });
    let image = capture(&mut ui);
    image.assert_pixel_color(Color::YELLOW, 100, 25);
    image.assert_pixel_color(Color::BLUE, 100, 75);
    width.set(200.0);
    ui.wait_for_idle();
    let image = capture(&mut ui);
    image.assert_pixel_color(Color::YELLOW, 100, 25);
    image.assert_pixel_color(Color::YELLOW, 100, 75);
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/draw/PainterModifierTest.kt, testVectorPainterScalesContent
#[test]
fn a_vector_painter_scales_its_vector_to_fill_the_height() {
    let mut ui = painted(400, || {
        let vector = ImageVector::builder(50.dp(), 100.dp(), 50.0, 100.0)
            .path(
                VectorPath { fill: Some(Brush::solid(Color::RED)), ..Default::default() },
                |path| {
                    path.horizontal_line_to_relative(50.0)
                        .vertical_line_to_relative(100.0)
                        .horizontal_line_to_relative(-50.0)
                        .close();
                },
            )
            .build();
        let painter = remember_vector_painter(&vector);
        Box!(
            modifier = Modifier
                .test_tag(TAG)
                .background(Color::GRAY)
                .required_width(300.dp())
                .required_height(200.dp())
                .paint_with(
                    painter,
                    true,
                    Alignment::CENTER,
                    ContentScale::FillHeight,
                    1.0,
                    None::<ColorFilter>
                )
        );
    });
    let image = capture(&mut ui);
    assert_eq!(image.size(), IntSize::new(300, 200));
    image.assert_pixel_color(Color::GRAY, 150 - 50 - 5, 0);
    image.assert_pixel_color(Color::GRAY, 150 + 50 + 5, 198);
    image.assert_pixel_color(Color::RED, 150 - 50 + 5, 0);
    image.assert_pixel_color(Color::RED, 150 + 50 - 5, 198);
}

/// A vector painter records its drawing once per size, as Compose's `DrawCache` renders once:
/// drawing it again (its node drawn again for its background) copies the recording.
#[test]
fn a_vector_painter_records_its_drawing_once_per_size() {
    let background = mutable_state_of(Color::RED);
    let size = mutable_state_of(24.0f32);
    let (background_handle, size_handle) = (background.handle(), size.handle());
    let vector = square(24.0, Brush::solid(Color::BLUE), None);
    let kept: Rc<Cell<Option<Painter>>> = Rc::new(Cell::new(None));
    let keep = kept.clone();
    let mut ui = painted(100, move || {
        let painter = remember_vector_painter(&vector);
        keep.set(Some(painter.clone()));
        let side = Dp(size_handle.get());
        let background = background_handle.get();
        Box!(
            modifier = Modifier
                .test_tag(TAG)
                .size(side)
                .draw_behind(move |scope| {
                    let bounds = scope.bounds();
                    scope.draw_rect(bounds, compose::ui::Paint::fill(background));
                })
                .paint_with(
                    painter,
                    true,
                    Alignment::CENTER,
                    ContentScale::FillBounds,
                    1.0,
                    None::<ColorFilter>
                )
        );
    });
    let painter = kept.take().expect("a painter");
    let recordings = || painter.downcast_ref::<VectorPainter>().expect("a vector").recordings();
    assert_eq!(recordings(), 1);
    background.set(Color::GREEN);
    ui.wait_for_idle();
    assert_eq!(recordings(), 1, "drawn again at the same size");
    size.set(48.0);
    ui.wait_for_idle();
    assert_eq!(recordings(), 2, "a new size");
    capture(&mut ui);
}

/// A group's scale, rotation and translation apply around its pivot, in the viewport, and its
/// stroke and gradient are drawn by the painter; Material's `Icon` draws such a vector with it.
#[test]
fn a_groups_transform_applies_around_its_pivot() {
    let vector = ImageVector::builder(100.dp(), 100.dp(), 10.0, 10.0)
        .group(
            VectorGroup {
                // The left half, scaled to the whole width from the left, moved down a tenth.
                scale_x: 2.0,
                translation_y: 1.0,
                ..Default::default()
            },
            |group| {
                group.add_path(VectorPath {
                    path_data: rectangle(5.0, 5.0),
                    fill: Some(Brush::horizontal_gradient([Color::RED, Color::RED])),
                    ..Default::default()
                });
            },
        )
        .build();
    let mut ui = painted(200, move || {
        Icon!(
            vector.clone(),
            None::<&str>,
            modifier = Modifier.test_tag(TAG).size(100.dp()),
            tint = Color::UNSPECIFIED
        );
    });
    let image = capture(&mut ui);
    image.assert_pixel_color(Color::RED, 95, 15);
    image.assert_pixel_color(Color::RED, 5, 55);
    assert_ne!(image.pixel(5, 5), Color::RED, "moved down");
    assert_ne!(image.pixel(50, 65), Color::RED, "half as high as wide");
}

/// A rotated group in a vector stretched unevenly is drawn under the stretch: its square,
/// a half turn around the center, still fills the stretched bounds.
#[test]
fn a_rotated_group_is_stretched_with_its_vector() {
    let vector = ImageVector::builder(24.dp(), 24.dp(), 24.0, 24.0)
        .group(
            VectorGroup { rotation: 180.0, pivot_x: 12.0, pivot_y: 12.0, ..Default::default() },
            |group| {
                group.add_path(VectorPath {
                    path_data: rectangle(24.0, 12.0),
                    fill: Some(Brush::solid(Color::BLUE)),
                    ..Default::default()
                });
            },
        )
        .build();
    let mut ui = painted(300, move || {
        let painter = remember_vector_painter(&vector);
        PainterImage!(
            painter,
            None::<&str>,
            modifier = Modifier.test_tag(TAG).size((192.dp(), 48.dp())).background(Color::RED),
            content_scale = ContentScale::FillBounds
        );
    });
    let image = capture(&mut ui);
    image.assert_pixel_color(Color::RED, 96, 10);
    image.assert_pixel_color(Color::BLUE, 96, 38);
    image.assert_pixel_color(Color::BLUE, 2, 46);
}

/// A 10 × 10 viewport drawn at 100 px of `group` holding a blue square over all of it, over
/// red, stretched to `size` (`FillBounds`).
fn group_drawn(
    group: VectorGroup,
    size: (i32, i32),
    inner: Vec<compose::ui::PathNode>,
) -> CapturedImage {
    let vector = ImageVector::builder(Dp(100.0), Dp(100.0), 10.0, 10.0)
        .group(group, move |group| {
            group.add_path(VectorPath {
                path_data: inner,
                fill: Some(Brush::solid(Color::BLUE)),
                ..Default::default()
            });
        })
        .build();
    let mut ui = painted(300, move || {
        let painter = remember_vector_painter(&vector);
        PainterImage!(
            painter,
            None::<&str>,
            modifier = Modifier
                .test_tag(TAG)
                .size((Dp(size.0 as f32), Dp(size.1 as f32)))
                .background(Color::RED),
            content_scale = ContentScale::FillBounds
        );
    });
    capture(&mut ui)
}

/// A group's pivot is in its viewport: scaled with it to the drawing's pixels.
#[test]
fn a_groups_pivot_is_scaled_with_the_viewport() {
    let group = VectorGroup { scale_x: 2.0, pivot_x: 5.0, pivot_y: 5.0, ..Default::default() };
    // The middle fifth, doubled around the center: 30 px to 70 px.
    let image = group_drawn(
        group,
        (100, 100),
        path_data(|path| {
            path.move_to(4.0, 0.0).line_to(6.0, 0.0).line_to(6.0, 10.0).line_to(4.0, 10.0).close();
        }),
    );
    image.assert_pixel_color(Color::BLUE, 32, 50);
    image.assert_pixel_color(Color::BLUE, 68, 50);
    image.assert_pixel_color(Color::RED, 27, 50);
    image.assert_pixel_color(Color::RED, 73, 50);
}

/// A quarter turn in a vector stretched unevenly turns in the viewport, then stretches: the top
/// half turned to the right half.
#[test]
fn a_quarter_turn_under_a_stretch_turns_in_the_viewport() {
    let group = VectorGroup { rotation: 90.0, pivot_x: 5.0, pivot_y: 5.0, ..Default::default() };
    let image = group_drawn(group, (200, 50), rectangle(10.0, 5.0));
    image.assert_pixel_color(Color::BLUE, 150, 25);
    image.assert_pixel_color(Color::BLUE, 105, 5);
    image.assert_pixel_color(Color::RED, 50, 25);
    image.assert_pixel_color(Color::RED, 95, 45);
}

/// A clip path is in the viewport: scaled with it.
#[test]
fn a_clip_path_is_scaled_with_the_viewport() {
    let group = VectorGroup { clip_path_data: rectangle(5.0, 10.0), ..Default::default() };
    let image = group_drawn(group, (100, 100), rectangle(10.0, 10.0));
    image.assert_pixel_color(Color::BLUE, 45, 50);
    image.assert_pixel_color(Color::RED, 55, 50);
}

/// One painter drawn left to right, then right to left, is mirrored once it is: its recording
/// is kept per direction.
#[test]
fn a_painter_drawn_right_to_left_after_left_to_right_is_mirrored() {
    let vector = ImageVector::builder(20.dp(), 20.dp(), 20.0, 20.0)
        .auto_mirror(true)
        .add_path(VectorPath {
            path_data: rectangle(10.0, 20.0),
            fill: Some(Brush::solid(Color::BLUE)),
            ..Default::default()
        })
        .build();
    let rtl = mutable_state_of(false);
    let handle = rtl.handle();
    let mut ui = painted(100, move || {
        let painter = remember_vector_painter(&vector);
        let direction = if handle.get() { LayoutDirection::Rtl } else { LayoutDirection::Ltr };
        CompositionLocalProvider(LOCAL_LAYOUT_DIRECTION.provides(direction), move || {
            PainterImage!(
                painter.clone(),
                None::<&str>,
                modifier = Modifier.test_tag(TAG).background(Color::RED)
            );
        });
    });
    let image = capture(&mut ui);
    image.assert_pixel_color(Color::BLUE, 2, 10);
    image.assert_pixel_color(Color::RED, 17, 10);
    rtl.set(true);
    ui.wait_for_idle();
    let image = capture(&mut ui);
    image.assert_pixel_color(Color::RED, 2, 10);
    image.assert_pixel_color(Color::BLUE, 17, 10);
}
