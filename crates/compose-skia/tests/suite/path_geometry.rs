//! Path geometry against Skia (Android's `android.graphics.Path` and Compose Desktop are both
//! Skia): Compose's `PathReversionTest.pixelComparison`, and compose-rs's `PathMeasure` (Skia's
//! `SkContourMeasure` ported) measuring as Skia's does. "Ours" is our commands replayed into a
//! Skia path, as Replayer.kt replays them into an `android.graphics.Path`. And `PathHitTester`
//! against Skia's `contains`.

use compose::ui::{Offset, Path, PathCommand, PathFillType, PathHitTester, PathMeasure};
use compose_skia::skia_safe::{self, ContourMeasureIter, PathBuilder};

#[path = "../../../compose-ui/tests/suite/path_shapes.rs"]
mod shapes;

fn replay(path: &Path) -> skia_safe::Path {
    let mut skia = PathBuilder::new();
    for command in path.data().commands() {
        match *command {
            PathCommand::MoveTo(p) => {
                skia.move_to((p.x, p.y));
            }
            PathCommand::LineTo(p) => {
                skia.line_to((p.x, p.y));
            }
            PathCommand::QuadTo(c, p) => {
                skia.quad_to((c.x, c.y), (p.x, p.y));
            }
            PathCommand::CubicTo(a, b, p) => {
                skia.cubic_to((a.x, a.y), (b.x, b.y), (p.x, p.y));
            }
            PathCommand::Close => {
                skia.close();
            }
        }
    }
    let mut skia = skia.detach();
    if path.fill_type() == PathFillType::EvenOdd {
        skia.set_fill_type(skia_safe::PathFillType::EvenOdd);
    }
    skia
}

/// As Compose's `createSvgPath`: the shape moved to the origin, filled even-odd.
fn svg_path(data: &str) -> Path {
    let path = Path::new();
    path.add_svg(data).unwrap();
    let bounds = path.get_bounds();
    path.translate(Offset::new(-bounds.left, -bounds.top));
    path.set_fill_type(PathFillType::EvenOdd);
    path
}

/// The path filled red, without antialiasing, in an image of `width` by `height`.
fn pixels(path: &Path, width: i32, height: i32) -> Vec<u8> {
    let mut surface = skia_safe::surfaces::raster_n32_premul((width, height)).unwrap();
    let mut paint = skia_safe::Paint::default();
    paint.set_color(skia_safe::Color::RED);
    paint.set_anti_alias(false);
    surface.canvas().draw_path(&replay(path), &paint);
    let info = skia_safe::ImageInfo::new_n32_premul((width, height), None);
    let mut bytes = vec![0; (width * height * 4) as usize];
    assert!(surface.read_pixels(&info, &mut bytes, (width * 4) as usize, (0, 0)));
    bytes
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathReversionTest.kt, pixelComparison
#[test]
fn a_reversed_path_fills_the_same_pixels() {
    for data in [shapes::CUBICS, shapes::QUADS, shapes::HEART, shapes::LINES] {
        let path = svg_path(data);
        let reversed = path.reverse();
        reversed.set_fill_type(path.fill_type());
        let bounds = path.get_bounds().round_to_int_rect();
        assert_eq!(reversed.get_bounds().round_to_int_rect(), bounds);
        let (width, height) = (bounds.width(), bounds.height());
        assert!(pixels(&path, width, height) == pixels(&reversed, width, height));
    }
}

fn assert_near(ours: f32, theirs: f32, what: &str) {
    assert!(
        (ours - theirs).abs() <= 1e-3 * theirs.abs().max(1.0),
        "{what}: ours {ours}, Skia's {theirs}"
    );
}

/// Our measure of `path`'s first contour against Skia's: length, and position and tangent at
/// distances along it.
fn assert_measures_as_skia(path: &Path, force_closed: bool) {
    let mut ours = PathMeasure::new();
    ours.set_path(Some(path), force_closed);
    let theirs = ContourMeasureIter::new(&replay(path), force_closed, 1.0).next().unwrap();
    assert_near(ours.length(), theirs.length(), "length");
    for step in 0..=20 {
        let distance = theirs.length() * step as f32 / 20.0;
        let (position, tangent) = theirs.pos_tan(distance).unwrap();
        let (our_position, our_tangent) = (ours.get_position(distance), ours.get_tangent(distance));
        assert_near(our_position.x, position.x, "x");
        assert_near(our_position.y, position.y, "y");
        assert_near(our_tangent.x, tangent.x, "tangent x");
        assert_near(our_tangent.y, tangent.y, "tangent y");
    }
}

#[test]
fn measures_lines_quadratics_and_cubics_as_skia() {
    for data in [shapes::CUBICS, shapes::QUADS, shapes::HEART, shapes::LINES, shapes::FILL_TYPES] {
        assert_measures_as_skia(&svg_path(data), false);
    }
    let open = Path::new();
    open.move_to(0.0, 0.0);
    open.quadratic_to(50.0, 100.0, 100.0, 0.0);
    open.cubic_to(120.0, -40.0, 160.0, 40.0, 200.0, 0.0);
    assert_measures_as_skia(&open, false);
    assert_measures_as_skia(&open, true);
}

#[test]
fn pieces_end_where_skia_ends_them() {
    let path = svg_path(shapes::HEART);
    let mut ours = PathMeasure::new();
    ours.set_path(Some(&path), false);
    let theirs = ContourMeasureIter::new(&replay(&path), false, 1.0).next().unwrap();
    for (start, stop) in [(0.0, 100.0), (150.0, 900.0), (400.0, 401.0), (1000.0, 5000.0)] {
        let piece = Path::new();
        assert!(ours.get_segment(start, stop, &piece, true));
        let mut builder = PathBuilder::new();
        assert!(theirs.get_segment(start, stop, &mut builder, true));
        let skia_piece = builder.detach();
        let ours_bounds = replay(&piece).compute_tight_bounds();
        let their_bounds = skia_piece.compute_tight_bounds();
        for (a, b) in [
            (ours_bounds.left, their_bounds.left),
            (ours_bounds.top, their_bounds.top),
            (ours_bounds.right, their_bounds.right),
            (ours_bounds.bottom, their_bounds.bottom),
        ] {
            assert_near(a, b, "piece bounds");
        }
    }
}

/// The points of a 200 × 200 grid around `path` where our hit tester and Skia's `contains`
/// disagree.
fn disagreements_with_skia(path: &Path) -> Vec<Offset> {
    let tester = PathHitTester::new(path, 0.5);
    let skia = replay(path);
    let bounds = path.get_bounds().inflate(10.0);
    let mut disagreements = Vec::new();
    for row in 0..200 {
        for column in 0..200 {
            // Off the grid of the path's own coordinates, so no point is on an edge.
            let x = bounds.left + bounds.width() * (column as f32 + 0.37) / 200.0;
            let y = bounds.top + bounds.height() * (row as f32 + 0.61) / 200.0;
            if tester.contains(Offset::new(x, y)) != skia.contains((x, y)) {
                disagreements.push(Offset::new(x, y));
            }
        }
    }
    disagreements
}

// Not one of Compose's tests: a cubic turning vertically twice, which Compose's own hit tester
// gets wrong (its `findCubicExtremaY` finds no turn; D1908), hits as Skia's.
#[test]
fn a_cubic_turning_twice_hits_as_skia() {
    let path = Path::new();
    path.move_to(50.0, 0.0);
    path.cubic_to(120.0, -40.0, -20.0, 140.0, 50.0, 0.0);
    path.close();
    assert_eq!(disagreements_with_skia(&path), []);
    assert!(PathHitTester::new(&path, 0.5).contains(Offset::new(64.13, -5.83)));
    for data in [shapes::CUBICS, shapes::HEART, shapes::FILL_TYPES] {
        assert_eq!(disagreements_with_skia(&svg_path(data)), []);
    }
}

// Not one of Compose's tests: a horizontal ray through a vertex crosses its two edges once.
#[test]
fn a_ray_through_a_vertex_hits_as_skia() {
    let diamond = Path::new();
    diamond.move_to(50.0, 0.0);
    diamond.line_to(100.0, 50.0);
    diamond.line_to(50.0, 100.0);
    diamond.line_to(0.0, 50.0);
    diamond.close();
    diamond.set_fill_type(PathFillType::EvenOdd);
    let skia = replay(&diamond);
    let tester = PathHitTester::new(&diamond, 0.5);
    for x in [20.0, 50.0, 80.0] {
        assert!(skia.contains((x, 50.0)));
        assert!(tester.contains(Offset::new(x, 50.0)), "{x}");
    }
}

// Not one of Compose's tests: a contour is filled as if closed by a line back to its start,
// whether it's closed (a close has no points of its own) or left open, so it's hit so too
// (where Compose's hit tester leaves that line out; D1908).
#[test]
fn contours_hit_as_closed_as_skia() {
    let closed = Path::new();
    closed.move_to(50.0, 0.0);
    closed.line_to(100.0, 50.0);
    closed.line_to(50.0, 100.0);
    closed.line_to(0.0, 50.0);
    closed.close();
    assert_eq!(disagreements_with_skia(&closed), []);
    let open = Path::new();
    open.move_to(50.0, 0.0);
    open.cubic_to(120.0, -40.0, -20.0, 140.0, 20.0, 30.0);
    open.move_to(200.0, 200.0);
    open.line_to(300.0, 200.0);
    open.line_to(250.0, 300.0);
    assert_eq!(disagreements_with_skia(&open), []);
}
