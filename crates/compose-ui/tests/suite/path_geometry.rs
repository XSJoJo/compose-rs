//! Compose's path geometry tests (ui-graphics device tests): `PathIteratorTest`,
//! `PathDirectionTest`, `PathDivisionTest`, `PathReversionTest`, `PathSvgTest`, `PathHitTest`,
//! `PathMeasureTest`, `PathTest`'s copy, and ui's `AndroidPathMeasureTest`. Compose's paths are
//! Skia's, whose round rectangles have conic corners; compose-rs's have cubic ones, so the tests
//! counting a round rectangle's segments count compose-rs's. The tests comparing pixels are in
//! compose-skia's `path_geometry.rs`.

use compose_ui::{
    ConicEvaluation, CornerRadius, Offset, Path, PathDirection, PathFillType, PathHitTester,
    PathMeasure, PathSegment, PathSegmentType, Rect, RoundRect,
};

use crate::path_shapes as shapes;

/// As Compose's `createSvgPath`: the shape moved to the origin, filled even-odd as SVG's.
pub(crate) fn svg_path(data: &str) -> Path {
    let path = Path::new();
    path.add_svg(data).unwrap();
    let bounds = path.get_bounds();
    path.translate(Offset::new(-bounds.left, -bounds.top));
    path.set_fill_type(PathFillType::EvenOdd);
    path
}

fn assert_point(expected: Offset, points: &[f32], index: usize) {
    assert!((expected.x - points[index * 2]).abs() <= 1e-6, "{expected:?} vs {points:?}");
    assert!((expected.y - points[index * 2 + 1]).abs() <= 1e-6, "{expected:?} vs {points:?}");
}

/// As Compose's `assertPathEquals`: the same sizes, then the same segments, point by point.
fn assert_path_equals(expected: &Path, actual: &Path) {
    let (mut first, mut second) = (expected.iterator(), actual.iterator());
    assert_eq!(first.calculate_size(true), second.calculate_size(true));
    while first.has_next() && second.has_next() {
        let (a, b) = (first.next_segment(), second.next_segment());
        assert_eq!(a.segment_type(), b.segment_type());
        for (x, y) in a.points().iter().zip(b.points()) {
            assert!((x - y).abs() <= 1e-7, "{a:?} vs {b:?}");
        }
    }
}

fn rect_path(rects: &[Rect]) -> Path {
    let path = Path::new();
    for &rect in rects {
        path.add_rect_with(rect, PathDirection::Clockwise);
    }
    path
}

const TWO_RECTS: [Rect; 2] = [Rect::new(0.0, 0.0, 10.0, 10.0), Rect::new(20.0, 20.0, 50.0, 50.0)];

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathIteratorTest.kt, emptyIterator
#[test]
fn an_empty_path_has_no_segments() {
    let path = Path::new();
    assert!(!path.iterator().has_next());
    assert_eq!((&path).into_iter().count(), 0);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathIteratorTest.kt, nonEmptyIterator
#[test]
fn segments_come_in_order_each_line_with_its_start() {
    let path = Path::new();
    path.move_to(1.0, 1.0);
    path.line_to(2.0, 2.0);
    path.close();
    assert!(path.iterator().has_next());
    let segments: Vec<PathSegment> = (&path).into_iter().collect();
    let types: Vec<_> = segments.iter().map(|segment| segment.segment_type()).collect();
    assert_eq!(types, [PathSegmentType::Move, PathSegmentType::Line, PathSegmentType::Close]);
    assert_eq!(segments[0].point(0), Offset::new(1.0, 1.0));
    assert_eq!(segments[1].point(0), Offset::new(1.0, 1.0));
    assert_eq!(segments[1].point(1), Offset::new(2.0, 2.0));
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathIteratorTest.kt, iteratorStyles
#[test]
fn the_three_ways_to_iterate_agree() {
    let path = Path::new();
    path.move_to(1.0, 1.0);
    path.line_to(2.0, 2.0);
    path.cubic_to(3.0, 3.0, 4.0, 4.0, 5.0, 5.0);
    path.quadratic_to(7.0, 7.0, 8.0, 8.0);
    path.move_to(10.0, 10.0);
    path.add_round_rect(RoundRect::from_ltrb(12.0, 12.0, 36.0, 36.0, CornerRadius::circular(8.0)));
    path.close();

    let mut first = path.iterator_with(ConicEvaluation::AsConic, 0.25);
    let mut second = path.iterator_with(ConicEvaluation::AsConic, 0.25);
    let mut third = path.iterator_with(ConicEvaluation::AsConic, 0.25);
    let (mut points, mut points2) = ([0.0; 8], [0.0; 16]);
    while first.has_next() || second.has_next() || third.has_next() {
        let segment = first.next_segment();
        let segment_type = second.next_into(&mut points, 0);
        let segment_type2 = third.next_into(&mut points2, 8);
        assert_eq!(segment_type, segment.segment_type());
        assert_eq!(segment_type2, segment.segment_type());
        for index in 0..segment.segment_type().point_count() {
            assert_point(segment.point(index), &points, index);
            assert_point(segment.point(index), &points2, index + 4);
        }
    }
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathIteratorTest.kt, done
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathIteratorTest.kt, close
#[test]
fn done_and_close_have_no_points() {
    let path = Path::new();
    path.close();
    let segment = path.iterator().next_segment();
    assert_eq!(segment.segment_type(), PathSegmentType::Done);
    assert!(segment.points().is_empty());
    assert_eq!(segment.weight(), 0.0);

    let path = Path::new();
    path.line_to(10.0, 12.0);
    path.close();
    let mut iterator = path.iterator();
    iterator.next_segment();
    iterator.next_segment();
    let segment = iterator.next_segment();
    assert_eq!(segment.segment_type(), PathSegmentType::Close);
    assert!(segment.points().is_empty());
    assert_eq!(segment.weight(), 0.0);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathIteratorTest.kt, moveTo
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathIteratorTest.kt, lineTo
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathIteratorTest.kt, quadraticTo
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathIteratorTest.kt, cubicTo
#[test]
fn each_segment_has_its_points() {
    let path = Path::new();
    path.move_to(10.0, 12.0);
    let segment = path.iterator().next_segment();
    assert_eq!(segment.segment_type(), PathSegmentType::Move);
    assert_eq!(segment.points(), [10.0, 12.0]);
    assert_eq!(segment.weight(), 0.0);

    let second_segment = |build: &dyn Fn(&Path)| {
        let path = Path::new();
        path.move_to(4.0, 6.0);
        build(&path);
        let mut iterator = path.iterator();
        iterator.next_segment();
        iterator.next_segment()
    };
    let line = second_segment(&|path| path.line_to(10.0, 12.0));
    assert_eq!(line.segment_type(), PathSegmentType::Line);
    assert_eq!(line.points(), [4.0, 6.0, 10.0, 12.0]);
    let quadratic = second_segment(&|path| path.quadratic_to(10.0, 12.0, 20.0, 24.0));
    assert_eq!(quadratic.segment_type(), PathSegmentType::Quadratic);
    assert_eq!(quadratic.points(), [4.0, 6.0, 10.0, 12.0, 20.0, 24.0]);
    let cubic = second_segment(&|path| path.cubic_to(10.0, 12.0, 20.0, 24.0, 30.0, 36.0));
    assert_eq!(cubic.segment_type(), PathSegmentType::Cubic);
    assert_eq!(cubic.points(), [4.0, 6.0, 10.0, 12.0, 20.0, 24.0, 30.0, 36.0]);
    assert_eq!(cubic.weight(), 0.0);
}

// compose-rs's round rectangles have no conics to convert or keep (Compose's `conicTo` needs
// Android's conic corners, n/a): neither way of iterating yields one.
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathIteratorTest.kt, conicAsQuadratics
#[test]
fn a_round_rect_yields_no_conic() {
    let path = Path::new();
    path.add_round_rect(RoundRect::from_ltrb(12.0, 12.0, 24.0, 24.0, CornerRadius::circular(8.0)));
    for evaluation in [ConicEvaluation::AsQuadratics, ConicEvaluation::AsConic] {
        for segment in path.iterator_with(evaluation, 0.25) {
            assert_ne!(segment.segment_type(), PathSegmentType::Conic, "{segment:?}");
        }
    }
}

// Android counts a round rect's move, four lines, four conics and close (10), and two
// quadratics a conic once converted (14); compose-rs's has a move, four cubics, three lines (the
// fourth is the close's) and a close, the same counted either way.
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathIteratorTest.kt, sizes
#[test]
fn sizes_count_the_segments() {
    let path = Path::new();
    assert_eq!(path.iterator().calculate_size(true), 0);
    path.add_round_rect(RoundRect::from_ltrb(12.0, 12.0, 64.0, 64.0, CornerRadius::circular(8.0)));
    let as_conic = path.iterator_with(ConicEvaluation::AsConic, 0.25);
    assert_eq!(as_conic.calculate_size(true), 9);
    assert_eq!(as_conic.calculate_size(false), as_conic.calculate_size(true));
    let as_quadratics = path.iterator();
    assert_eq!(as_quadratics.calculate_size(false), 9);
    assert_eq!(as_quadratics.calculate_size(true), 9);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDirectionTest.kt, directionForEmptyPath
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDirectionTest.kt, directionForMoves
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDirectionTest.kt, directionForSingleLine
#[test]
fn a_path_without_area_is_clockwise() {
    let path = Path::new();
    assert_eq!(path.compute_direction(), PathDirection::Clockwise);
    path.move_to(10.0, 10.0);
    assert_eq!(path.compute_direction(), PathDirection::Clockwise);
    path.move_to(20.0, 20.0);
    assert_eq!(path.compute_direction(), PathDirection::Clockwise);
    let line = Path::new();
    line.move_to(10.0, 10.0);
    line.line_to(30.0, 30.0);
    assert_eq!(line.compute_direction(), PathDirection::Clockwise);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDirectionTest.kt, directionForLines
#[test]
fn open_lines_wind_as_their_closed_triangle() {
    let path = Path::new();
    path.move_to(0.0, 0.0);
    path.line_to(30.0, 30.0);
    path.line_to(60.0, 0.0);
    assert_eq!(path.compute_direction(), PathDirection::CounterClockwise);
    path.rewind();
    path.move_to(60.0, 0.0);
    path.line_to(30.0, 30.0);
    path.line_to(0.0, 0.0);
    assert_eq!(path.compute_direction(), PathDirection::Clockwise);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDirectionTest.kt, directionForClosedLines
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDirectionTest.kt, directionForMultipleContours
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDirectionTest.kt, directionForQuadratics
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDirectionTest.kt, directionForCubics
#[test]
fn shapes_wind_as_they_were_added_and_only_the_first_contour_counts() {
    for direction in [PathDirection::Clockwise, PathDirection::CounterClockwise] {
        let path = Path::new();
        path.add_rect_with(Rect::new(10.0, 10.0, 40.0, 40.0), direction);
        assert_eq!(path.compute_direction(), direction);
        let round = Path::new();
        let round_rect = RoundRect::from_ltrb(10.0, 10.0, 40.0, 40.0, CornerRadius::circular(6.0));
        round.add_round_rect_with(round_rect, direction);
        assert_eq!(round.compute_direction(), direction);
    }
    let path = Path::new();
    path.add_rect_with(Rect::new(10.0, 10.0, 40.0, 40.0), PathDirection::Clockwise);
    path.add_rect_with(Rect::new(100.0, 10.0, 140.0, 40.0), PathDirection::CounterClockwise);
    assert_eq!(path.compute_direction(), PathDirection::Clockwise);

    let cubics = Path::new();
    cubics.add_svg(shapes::CUBICS).unwrap();
    assert_eq!(cubics.compute_direction(), PathDirection::Clockwise);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDivisionTest.kt, emptyPath
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDivisionTest.kt, singleMove
#[test]
fn nothing_but_moves_divides_into_nothing() {
    assert!(Path::new().divide().is_empty());
    let path = Path::new();
    path.move_to(10.0, 10.0);
    assert!(path.divide().is_empty());
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDivisionTest.kt, divide
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathDivisionTest.kt, divideWithEmptyContour
#[test]
fn each_contour_becomes_a_path() {
    let paths = rect_path(&TWO_RECTS).divide();
    assert_eq!(paths.len(), 2);
    for (index, rect) in TWO_RECTS.iter().enumerate() {
        assert_path_equals(&rect_path(&[*rect]), &paths[index]);
    }

    let path = rect_path(&TWO_RECTS[..1]);
    path.move_to(10.0, 10.0);
    path.move_to(100.0, 100.0);
    path.add_rect_with(TWO_RECTS[1], PathDirection::Clockwise);
    assert_eq!(path.divide().len(), 2);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathReversionTest.kt, emptyPath
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathReversionTest.kt, singleMove
#[test]
fn reversing_nothing_or_a_move() {
    assert!(Path::new().reverse().is_empty());
    let path = Path::new();
    path.move_to(10.0, 10.0);
    let mut iterator = path.reverse().iterator();
    assert!(iterator.has_next());
    let segment = iterator.next_segment();
    assert_eq!(segment.segment_type(), PathSegmentType::Move);
    assert_point(Offset::new(10.0, 10.0), segment.points(), 0);
}

fn assert_segment(
    iterator: &mut compose_ui::PathIterator,
    segment_type: PathSegmentType,
    end: (f32, f32),
) {
    let segment = iterator.next_segment();
    assert_eq!(segment.segment_type(), segment_type);
    let index = if segment_type == PathSegmentType::Move { 0 } else { 1 };
    assert_point(Offset::new(end.0, end.1), segment.points(), index);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathReversionTest.kt, lineTo
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathReversionTest.kt, close
#[test]
fn a_reversed_contour_starts_at_its_end() {
    let path = Path::new();
    path.move_to(10.0, 10.0);
    path.line_to(20.0, 20.0);
    let mut iterator = path.reverse().iterator();
    assert_eq!(iterator.calculate_size(false), 2);
    assert_segment(&mut iterator, PathSegmentType::Move, (20.0, 20.0));
    assert_segment(&mut iterator, PathSegmentType::Line, (10.0, 10.0));

    let path = Path::new();
    path.move_to(10.0, 10.0);
    path.line_to(20.0, 20.0);
    path.line_to(10.0, 30.0);
    path.close();
    let mut iterator = path.reverse().iterator();
    assert_eq!(iterator.calculate_size(false), 4);
    assert_segment(&mut iterator, PathSegmentType::Move, (10.0, 30.0));
    assert_segment(&mut iterator, PathSegmentType::Line, (20.0, 20.0));
    assert_segment(&mut iterator, PathSegmentType::Line, (10.0, 10.0));
    assert_eq!(iterator.next_segment().segment_type(), PathSegmentType::Close);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathReversionTest.kt, multipleContours
#[test]
fn reversed_contours_come_last_first() {
    let path = Path::new();
    path.move_to(10.0, 10.0);
    path.line_to(20.0, 20.0);
    path.line_to(10.0, 30.0);
    path.close();
    path.move_to(50.0, 50.0);
    path.line_to(70.0, 70.0);
    path.line_to(50.0, 90.0);
    let mut iterator = path.reverse().iterator();
    assert_eq!(iterator.calculate_size(false), 7);
    assert_segment(&mut iterator, PathSegmentType::Move, (50.0, 90.0));
    assert_segment(&mut iterator, PathSegmentType::Line, (70.0, 70.0));
    assert_segment(&mut iterator, PathSegmentType::Line, (50.0, 50.0));
    assert_segment(&mut iterator, PathSegmentType::Move, (10.0, 30.0));
    assert_segment(&mut iterator, PathSegmentType::Line, (20.0, 20.0));
    assert_segment(&mut iterator, PathSegmentType::Line, (10.0, 10.0));
    assert_eq!(iterator.next_segment().segment_type(), PathSegmentType::Close);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathReversionTest.kt, directionIsReversed
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathReversionTest.kt, contoursDirectionIsReversed
#[test]
fn reversing_turns_every_contour_the_other_way() {
    let heart = svg_path(shapes::HEART);
    assert_eq!(heart.compute_direction(), PathDirection::Clockwise);
    assert_eq!(heart.reverse().compute_direction(), PathDirection::CounterClockwise);

    let cubics = svg_path(shapes::CUBICS);
    let contours = cubics.divide();
    let reversed: Vec<Path> = cubics.reverse().divide().into_iter().rev().collect();
    assert_eq!(contours.len(), reversed.len());
    for (contour, reversed) in contours.iter().zip(&reversed) {
        assert_ne!(contour.compute_direction(), reversed.compute_direction());
    }
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathSvgTest.kt, emptyPath
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathSvgTest.kt, singleMove
#[test]
fn an_empty_path_or_a_move_as_svg() {
    assert_eq!(
        Path::new().to_svg(true),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0.0 0.0 0.0 0.0\">\n</svg>\n"
    );
    assert!(Path::new().to_svg(false).is_empty());

    let path = Path::new();
    path.move_to(10.0, 10.0);
    assert_eq!(
        path.to_svg(true),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"10.0 10.0 0.0 0.0\">\n  \
         <path d=\"M10.0 10.0\"/>\n</svg>\n"
    );
}

const TWO_RECTS_SVG: &str =
    "M0.0 0.0L10.0 0.0 10.0 10.0 0.0 10.0ZM20.0 20.0L50.0 20.0 50.0 50.0 20.0 50.0Z";

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathSvgTest.kt, twoPaths
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathSvgTest.kt, dataOnly
#[test]
fn two_rects_as_svg() {
    let path = rect_path(&TWO_RECTS);
    assert_eq!(
        path.to_svg(true),
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0.0 0.0 50.0 50.0\">\n  \
             <path d=\"{TWO_RECTS_SVG}\"/>\n</svg>\n"
        )
    );
    assert_eq!(path.to_svg(false), TWO_RECTS_SVG);
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathSvgTest.kt, bezier
#[test]
fn curves_as_svg() {
    let path = Path::new();
    path.move_to(10.0, 10.0);
    path.cubic_to(20.0, 20.0, 30.0, 30.0, 40.0, 40.0);
    path.quadratic_to(50.0, 50.0, 60.0, 60.0);
    assert_eq!(
        path.to_svg(true),
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"10.0 10.0 50.0 50.0\">\n  \
         <path d=\"M10.0 10.0C20.0 20.0 30.0 30.0 40.0 40.0Q50.0 50.0 60.0 60.0\"/>\n</svg>\n"
    );
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathSvgTest.kt, hole
#[test]
fn the_fill_type_is_the_fill_rule() {
    let hole = rect_path(&[Rect::new(0.0, 0.0, 80.0, 80.0), Rect::new(20.0, 20.0, 50.0, 50.0)]);
    let data = "M0.0 0.0L80.0 0.0 80.0 80.0 0.0 80.0ZM20.0 20.0L50.0 20.0 50.0 50.0 20.0 50.0Z";
    let header = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0.0 0.0 80.0 80.0\">\n";
    assert_eq!(hole.to_svg(true), format!("{header}  <path d=\"{data}\"/>\n</svg>\n"));
    hole.set_fill_type(PathFillType::EvenOdd);
    assert_eq!(
        hole.to_svg(true),
        format!("{header}  <path fill-rule=\"evenodd\" d=\"{data}\"/>\n</svg>\n")
    );
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathSvgTest.kt, addSvg
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathSvgTest.kt, addInvalidSvg
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathSvgTest.kt, roundTrip
#[test]
fn svg_data_adds_contours_and_round_trips() {
    let two_rects = Path::new();
    two_rects.add_svg(TWO_RECTS_SVG).unwrap();
    let reference = rect_path(&TWO_RECTS);
    assert_eq!(
        (&reference).into_iter().collect::<Vec<_>>(),
        (&two_rects).into_iter().collect::<Vec<_>>()
    );

    // 'K' isn't an SVG command: an error, where Compose throws.
    assert!(Path::new().add_svg("M0.0 0.0K10.0 0.0 10.0 10.0 0.0 10.0Z").is_err());

    let path = Path::new();
    path.add_svg(&reference.to_svg(false)).unwrap();
    assert_eq!(
        (&reference).into_iter().collect::<Vec<_>>(),
        (&path).into_iter().collect::<Vec<_>>()
    );
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathHitTest.kt, linesHit
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathHitTest.kt, cubicsHit
// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathHitTest.kt, quadsHit
#[test]
fn hits_inside_lines_curves_and_not_their_cutouts() {
    let lines = PathHitTester::new(&svg_path(shapes::LINES), 0.5);
    assert!(lines.contains(Offset::new(400.0, 370.0)));
    assert!(!lines.contains(Offset::new(180.0, 160.0)));

    let cubics = PathHitTester::new(&svg_path(shapes::CUBICS), 0.5);
    assert!(cubics.contains(Offset::new(100.0, 275.0)));
    assert!(cubics.contains(Offset::new(370.0, 40.0)));
    assert!(!cubics.contains(Offset::new(370.0, 280.0)));
    assert!(!cubics.contains(Offset::new(380.0, 600.0)));

    let quads = PathHitTester::new(&svg_path(shapes::QUADS), 0.5);
    assert!(quads.contains(Offset::new(50.0, 180.0)));
    assert!(quads.contains(Offset::new(225.0, 15.0)));
    assert!(!quads.contains(Offset::new(270.0, 175.0)));
    assert!(!quads.contains(Offset::new(275.0, 425.0)));
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathHitTest.kt, fillTypesHit
#[test]
fn the_fill_type_decides_a_hit_in_a_hole() {
    let non_zero = svg_path(shapes::FILL_TYPES);
    non_zero.set_fill_type(PathFillType::NonZero);
    let even_odd = svg_path(shapes::FILL_TYPES);
    assert!(PathHitTester::new(&non_zero, 0.5).contains(Offset::new(350.0, 350.0)));
    assert!(!PathHitTester::new(&even_odd, 0.5).contains(Offset::new(350.0, 350.0)));
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathMeasureTest.kt, testGetPositionAndTangent
#[test]
fn the_middle_of_a_diagonal_and_its_direction() {
    use std::f32::consts::FRAC_1_SQRT_2;
    let path = Path::new();
    path.line_to(100.0, 100.0);
    let mut measure = PathMeasure::new();
    measure.set_path(Some(&path), false);
    let distance = measure.length();
    let position = measure.get_position(distance * 0.5);
    let tangent = measure.get_tangent(distance * 0.5);
    assert!((position.x - 50.0).abs() <= 0.0001 && (position.y - 50.0).abs() <= 0.0001);
    assert!(
        (tangent.x - FRAC_1_SQRT_2).abs() <= 0.0001 && (tangent.y - FRAC_1_SQRT_2).abs() <= 0.0001
    );
}

// Compose draws the result and reads its pixels; here its segments are read: the two pieces,
// each a contour of its own.
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/AndroidPathMeasureTest.kt, potentialWorkaround_writeParts
#[test]
fn pieces_of_a_line_add_up_as_contours() {
    let path = Path::new();
    path.move_to(0.0, 0.0);
    path.line_to(20.0, 0.0);
    let mut measure = PathMeasure::new();
    measure.set_path(Some(&path), false);
    let length = measure.length();
    assert_eq!(length, 20.0);

    let piece = Path::new();
    let target = Path::new();
    assert!(measure.get_segment(10.0, length, &piece, true));
    target.add_path(&piece, Offset::ZERO);
    piece.reset();
    assert!(measure.get_segment(0.0, 5.0, &piece, true));
    target.add_path(&piece, Offset::ZERO);
    assert_eq!(target.to_svg(false), "M10.0 0.0L20.0 0.0M0.0 0.0L5.0 0.0");
}

// Compose: compose/ui/ui-graphics/src/androidDeviceTest/kotlin/androidx/compose/ui/graphics/PathTest.kt, testPathCopy
#[test]
fn a_copy_changes_apart_from_its_original() {
    let original = rect_path(&[Rect::new(0.0, 0.0, 10.0, 10.0)]);
    let copy = original.copy();
    copy.add_rect_with(Rect::new(5.0, 5.0, 15.0, 15.0), PathDirection::Clockwise);
    assert_eq!(original.get_bounds(), Rect::new(0.0, 0.0, 10.0, 10.0));
    assert_eq!(copy.get_bounds(), Rect::new(0.0, 0.0, 15.0, 15.0));
}

// Not one of Compose's tests: a measure's ends, pieces of curves, a closed contour and the
// operators.
#[test]
fn measures_clamp_curves_split_and_operators_combine() {
    let mut measure = PathMeasure::new();
    assert_eq!(measure.length(), 0.0);
    assert!(measure.get_position(1.0).is_unspecified());
    assert!(!measure.get_segment(0.0, 1.0, &Path::new(), true));

    let square = rect_path(&[Rect::new(0.0, 0.0, 10.0, 10.0)]);
    measure.set_path(Some(&square), false);
    assert_eq!(measure.length(), 40.0);
    assert_eq!(measure.get_position(35.0), Offset::new(0.0, 5.0));
    assert_eq!(measure.get_position(-5.0), Offset::new(0.0, 0.0));
    assert_eq!(measure.get_position(100.0), Offset::new(0.0, 0.0));
    assert!(!measure.get_segment(20.0, 10.0, &Path::new(), true));

    let open = Path::new();
    open.move_to(0.0, 0.0);
    open.line_to(10.0, 0.0);
    open.line_to(10.0, 10.0);
    measure.set_path(Some(&open), true);
    assert_eq!(measure.length(), 20.0 + 200.0_f32.sqrt());

    let curve = Path::new();
    curve.move_to(0.0, 0.0);
    curve.cubic_to(0.0, 50.0, 100.0, 50.0, 100.0, 0.0);
    measure.set_path(Some(&curve), false);
    let half = Path::new();
    assert!(measure.get_segment(0.0, measure.length() / 2.0, &half, true));
    let middle = measure.get_position(measure.length() / 2.0);
    assert!((middle.x - 50.0).abs() < 0.5 && (middle.y - 37.5).abs() < 0.5, "{middle:?}");
    let end = half.data().last_point().unwrap();
    assert!((end.x - 50.0).abs() < 0.5 && (end.y - 37.5).abs() < 0.5, "{end:?}");

    let first = rect_path(&[Rect::new(0.0, 0.0, 10.0, 10.0)]);
    let second = rect_path(&[Rect::new(5.0, 5.0, 15.0, 15.0)]);
    for combined in
        [&first + &second, &first - &second, &first | &second, &first & &second, &first ^ &second]
    {
        assert!(!combined.is_empty());
    }
}
