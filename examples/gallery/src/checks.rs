//! Two screens to check a platform against the desktop by eye and by screenshot: drawing into
//! bitmaps (D1914, D1915), the canvas's operations no `DrawScope` records (D1916) and wide colors
//! (D1911); and the secure field's "Show passwords" (D1146) with touch selection's handles in a
//! field and a `SelectionContainer` scrolled out of their container.

use compose::foundation::layout::*;
use compose::foundation::{
    BasicSecureTextField, BasicTextFieldOfState, DEFAULT_OBFUSCATION_CHARACTER, PainterImage,
    ScrollModifiers, SelectionContainer, TextObfuscationMode, remember_scroll_state,
    remember_text_field_state,
};
use compose::material3::*;
use compose::runtime::*;
use compose::ui::canvas::{
    Canvas as BitmapCanvas, CanvasDrawScope, ClipOp, PointMode, VertexMode, Vertices,
};
use compose::ui::colorspace::ColorSpaces;
use compose::ui::*;

/// The side of each bitmap's square, in pixels: shown a bitmap pixel to a screen pixel.
const SIDE: u32 = 400;

/// Shapes, gradients, content turned and points: drawn into a bitmap and on the screen alike.
fn shapes(scope: &mut DrawScope<'_>) {
    let side = scope.size().width;
    scope.draw_rect(scope.bounds(), Paint::fill(Color::from_rgb(240, 240, 240)));
    scope.draw_circle(Offset::new(70.0, 70.0), 50.0, Paint::fill(Color::RED));
    scope.draw_round_rect(
        Rect::new(140.0, 20.0, 260.0, 120.0),
        [24.0; 4],
        Paint::stroke(Color::BLUE, 8.0),
    );
    scope.draw_oval(Rect::new(280.0, 30.0, 380.0, 110.0), Paint::fill(Color::from_rgb(0, 160, 0)));
    // A gradient across a band.
    let gradient = Brush::horizontal_gradient([Color::MAGENTA, Color::YELLOW, Color::CYAN]);
    let paint = scope.brush_paint(&gradient, 1.0, PaintStyle::Fill);
    scope.draw_rect(Rect::new(20.0, 140.0, side - 20.0, 190.0), paint);
    // A square turned 30 degrees, filled with a radial gradient.
    scope.rotate(30.0, Offset::new(100.0, 280.0), |scope| {
        let radial = Brush::radial_gradient([Color::WHITE, Color::BLUE]);
        let paint = scope.brush_paint(&radial, 1.0, PaintStyle::Fill);
        scope.draw_rect(Rect::new(50.0, 230.0, 150.0, 330.0), paint);
    });
    // A path: a curve stroked.
    let path = Path::new();
    path.move_to(200.0, 360.0);
    path.cubic_to(240.0, 220.0, 300.0, 380.0, 380.0, 230.0);
    scope.draw_path(&path, Paint::stroke(Color::BLACK, 6.0));
    // Points: dots (square, then round), pairs as lines, a polygon.
    let dots: Vec<Offset> = (0..6).map(|i| Offset::new(200.0 + i as f32 * 30.0, 220.0)).collect();
    scope.draw_points(&dots, PointMode::Points, Paint::stroke(Color::RED, 12.0));
    let round = Paint::stroke_with(Color::BLUE, 12.0, StrokeCap::Round);
    let below: Vec<Offset> = dots.iter().map(|dot| *dot + Offset::new(0.0, 25.0)).collect();
    scope.draw_points(&below, PointMode::Points, round);
    let lines = [Offset::new(20.0, 370.0), Offset::new(80.0, 350.0), Offset::new(100.0, 390.0)];
    scope.draw_points(&lines, PointMode::Polygon, Paint::stroke(Color::from_rgb(255, 128, 0), 4.0));
    let pairs = [Offset::new(120.0, 350.0), Offset::new(180.0, 390.0)];
    scope.draw_points(&pairs, PointMode::Lines, Paint::stroke(Color::DARK_GRAY, 4.0));
}

/// Skew, a matrix concatenated, clips out and vertices: what only a bitmap's canvas draws.
fn canvas_ops(canvas: &mut BitmapCanvas) {
    let side = SIDE as f32;
    canvas.draw_rect(Rect::new(0.0, 0.0, side, side), Paint::fill(Color::from_rgb(240, 240, 240)));
    // Top left: a square skewed along x.
    canvas.with_save(|canvas| {
        canvas.translate(20.0, 20.0);
        canvas.skew(0.5, 0.0);
        canvas.draw_rect(Rect::new(0.0, 0.0, 100.0, 100.0), Paint::fill(Color::BLUE));
    });
    // Top right: a square through a matrix, turned 20 degrees and scaled half again on x.
    canvas.with_save(|canvas| {
        let mut matrix = Matrix::rotation(20.0);
        matrix.post_concat(&Matrix::scaling(1.5, 1.0));
        matrix.post_concat(&Matrix::translation(240.0, 20.0));
        canvas.concat(&matrix);
        canvas.draw_rect(Rect::new(0.0, 0.0, 80.0, 80.0), Paint::fill(Color::from_rgb(0, 160, 0)));
    });
    // Bottom left: a disc with a square clipped out, and a disc clipped out of a square.
    canvas.with_save(|canvas| {
        canvas.clip_rect_with(Rect::new(50.0, 230.0, 90.0, 270.0), ClipOp::Difference);
        canvas.draw_circle(Offset::new(70.0, 250.0), 50.0, Paint::fill(Color::RED));
    });
    canvas.with_save(|canvas| {
        let hole = Path::new();
        hole.add_oval(Rect::new(140.0, 220.0, 200.0, 280.0));
        canvas.clip_path_with(&hole, ClipOp::Difference);
        canvas.draw_rect(Rect::new(130.0, 210.0, 210.0, 290.0), Paint::fill(Color::MAGENTA));
    });
    // Bottom right: triangles colored by their vertices, and a fan.
    let triangles = std::sync::Arc::new(Vertices::new(
        VertexMode::Triangles,
        &[Offset::new(240.0, 300.0), Offset::new(380.0, 300.0), Offset::new(310.0, 180.0)],
        &[Offset::ZERO; 3],
        &[Color::RED, Color::GREEN, Color::BLUE],
        &[],
    ));
    canvas.draw_vertices(&triangles, BlendMode::SrcOver, Paint::fill(Color::BLACK));
    let fan = std::sync::Arc::new(Vertices::new(
        VertexMode::TriangleFan,
        &[
            Offset::new(80.0, 350.0),
            Offset::new(20.0, 320.0),
            Offset::new(140.0, 320.0),
            Offset::new(140.0, 390.0),
            Offset::new(20.0, 390.0),
        ],
        &[Offset::ZERO; 5],
        &[Color::WHITE, Color::CYAN, Color::MAGENTA, Color::YELLOW, Color::CYAN],
        &[],
    ));
    canvas.draw_vertices(&fan, BlendMode::SrcOver, Paint::fill(Color::BLACK));
    // Points through the canvas: dots along the bottom.
    let dots: Vec<Offset> = (0..5).map(|i| Offset::new(200.0 + i as f32 * 40.0, 370.0)).collect();
    canvas.draw_points(PointMode::Points, &dots, Paint::stroke(Color::BLACK, 10.0));
}

/// Display P3's red and green, and sRGB's: a solid swatch of each and a gradient of P3's.
fn wide_colors(scope: &mut DrawScope<'_>) {
    let p3 = &ColorSpaces::DISPLAY_P3;
    let p3_red = Color::new_with(1.0, 0.0, 0.0, 1.0, p3);
    let p3_green = Color::new_with(0.0, 1.0, 0.0, 1.0, p3);
    let third = scope.size().width / 4.0;
    let height = scope.size().height / 2.0;
    scope.draw_rect(Rect::new(0.0, 0.0, third, height), Paint::fill(Color::RED));
    scope.draw_rect(Rect::new(third, 0.0, 2.0 * third, height), Paint::fill(p3_red));
    scope.draw_rect(Rect::new(2.0 * third, 0.0, 3.0 * third, height), Paint::fill(Color::GREEN));
    scope.draw_rect(Rect::new(3.0 * third, 0.0, 4.0 * third, height), Paint::fill(p3_green));
    // A gradient of P3's colors, and one mixing P3 and sRGB (drawn in extended sRGB).
    let quarter = height / 2.0;
    let wide = Brush::horizontal_gradient([p3_red, p3_green]);
    let paint = scope.brush_paint(&wide, 1.0, PaintStyle::Fill);
    scope.draw_rect(Rect::new(0.0, height, scope.size().width, height + quarter), paint);
    let mixed = Brush::horizontal_gradient([p3_red, Color::BLUE]);
    let paint = scope.brush_paint(&mixed, 1.0, PaintStyle::Fill);
    scope.draw_rect(Rect::new(0.0, height + quarter, scope.size().width, 2.0 * height), paint);
}

/// A bitmap of `width` × `height` with `draw`'s drawing in it.
fn bitmap_of(width: u32, height: u32, draw: impl FnOnce(&mut BitmapCanvas)) -> ImageBitmap {
    let image = ImageBitmap::new(width, height);
    let mut canvas = BitmapCanvas::new(&image);
    draw(&mut canvas);
    image
}

/// A bitmap shown a pixel to a screen pixel.
#[composable]
fn Bitmap(image: ImageBitmap, description: &'static str) {
    let density = LOCAL_DENSITY.current();
    let size =
        DpSize::new(density.to_dp(image.width() as f32), density.to_dp(image.height() as f32));
    PainterImage!(
        Painter::new(BitmapPainter::new(image)),
        Some(description),
        modifier = Modifier.size(size),
        content_scale = ContentScale::FillBounds
    );
}

/// Drawing into bitmaps beside the same drawing on the screen, the canvas's own operations, and
/// wide colors. `android-demo`'s "drawing".
#[composable]
pub fn Drawing() {
    let density = LOCAL_DENSITY.current();
    let side = density.to_dp(SIDE as f32);
    let shapes_bitmap = remember(move || {
        bitmap_of(SIDE, SIDE, |canvas| {
            let size = Size::new(SIDE as f32, SIDE as f32);
            CanvasDrawScope::draw(canvas, density, LayoutDirection::Ltr, size, shapes);
        })
    });
    let ops_bitmap = remember(|| bitmap_of(SIDE, SIDE, canvas_ops));
    let wide_bitmap = remember(move || {
        bitmap_of(2 * SIDE, SIDE / 2, |canvas| {
            let size = Size::new(2.0 * SIDE as f32, SIDE as f32 / 2.0);
            CanvasDrawScope::draw(canvas, density, LayoutDirection::Ltr, size, wide_colors);
        })
    });
    let scroll = remember_scroll_state!();
    Column!(
        modifier = Modifier.fill_max_size().vertical_scroll(scroll).padding(8.dp()),
        vertical_arrangement = Arrangement::spaced_by(8.dp()),
        {
            Text!("Into a bitmap, then on the screen");
            Row!({
                Bitmap(shapes_bitmap.clone(), "Shapes drawn into a bitmap");
                Canvas(Modifier.size((side, side)), shapes);
            });
            Text!("Into a bitmap: skew, concat, clips out, vertices, points");
            Bitmap(ops_bitmap.clone(), "The canvas's operations drawn into a bitmap");
            Text!("Wide colors: sRGB red, P3 red, sRGB green, P3 green; P3 gradients");
            Canvas(Modifier.size((2.0 * side, side / 2.0)), wide_colors);
            Text!("The same into an sRGB bitmap");
            Bitmap(wide_bitmap.clone(), "Wide colors drawn into a bitmap");
        }
    );
}

/// The "Show passwords" setting as read, password fields (Material's and the basic one), a field
/// of a state, and in a short scrolling box fields of a state and of a value and a selectable
/// text (to scroll them out with their handles showing). A switch runs an animation, so frames
/// keep coming. `android-demo`'s "fields".
#[composable]
pub fn Fields() {
    let password = remember_text_field_state!();
    let basic_password = remember_text_field_state!();
    let note = remember_text_field_state!(
        "Long press a word here, then drag the handles. Scroll this box to hide the field."
    );
    let basic = remember_text_field_state!("A basic field of a state, outside the box");
    let value = remember_saveable(|| mutable_state_of(String::from("A field of a value")));
    let scroll = remember_scroll_state!();
    Column!(
        modifier = Modifier.fill_max_size().padding(8.dp()),
        vertical_arrangement = Arrangement::spaced_by(8.dp()),
        {
            // An animation keeping frames coming: a field's long press waits for a frame on
            // Android (phone-check, 2026-11-07), so this tells the two cases apart.
            let spin = remember_saveable(|| mutable_state_of(false));
            Row!(vertical_alignment = Alignment::CENTER_VERTICALLY, {
                Switch!(spin.get(), Some(move |on: bool| spin.set(on)));
                Text!("An animation", modifier = Modifier.padding((8.dp(), 0.dp())));
                if spin.get() {
                    IndeterminateCircularProgressIndicator!();
                }
            });
            match platform_show_password() {
                Some((touch, physical)) => {
                    Text!("Show passwords, as read: touch {touch}, physical {physical}")
                }
                None => Text!("Show passwords: this host has no setting"),
            }
            SecureTextField!(
                password,
                modifier = Modifier.fill_max_width(),
                label = Some(|| Text!("Password")),
                // Material's defaults name `compose_foundation`, which the facade's users can't.
                text_obfuscation_mode = TextObfuscationMode::System,
                text_obfuscation_character = DEFAULT_OBFUSCATION_CHARACTER
            );
            Text!("A basic secure field:");
            BasicSecureTextField!(
                basic_password,
                modifier = Modifier
                    .fill_max_width()
                    .background(Color::from_rgb(230, 230, 230))
                    .padding(12.dp())
            );
            BasicTextFieldOfState!(
                basic,
                modifier = Modifier
                    .fill_max_width()
                    .background(Color::from_rgb(230, 230, 230))
                    .padding(12.dp())
            );
            Text!("A box 220 dp tall that scrolls:");
            Column!(
                modifier = Modifier
                    .fill_max_width()
                    .height(220.dp())
                    .background(Color::from_rgb(245, 235, 220))
                    .vertical_scroll(scroll)
                    .padding(8.dp()),
                vertical_arrangement = Arrangement::spaced_by(8.dp()),
                {
                    TextFieldOfState!(
                        note,
                        modifier = Modifier.fill_max_width(),
                        label = Some(|| Text!("A field"))
                    );
                    TextField!(
                        value.get(),
                        move |text| value.set(text),
                        modifier = Modifier.fill_max_width(),
                        label = Some(|| Text!("A field of a value"))
                    );
                    SelectionContainer!({
                        Column!({
                            Text!("Selectable: long press a word to select it.");
                            Text!("A second line of selectable text, to drag a handle across.");
                        });
                    });
                    Spacer(Modifier.height(400.dp()));
                    Text!("The bottom of the box.");
                }
            );
        }
    );
}
