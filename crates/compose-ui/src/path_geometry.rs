//! Queries over a path's geometry, as Compose's `PathIterator`, `PathSegment`, `PathGeometry`
//! (`computeDirection`, `divide`, `reverse`), `PathSvg` (`addSvg`, `toSvg`), `PathHitTester`, the
//! path operators and `Path.copy` (D1908), computed in Rust from a path's commands.
//!
//! A compose-rs path keeps its arcs, ovals and round rectangles' corners as cubics, where Android
//! and Skia keep conics: its iterator never yields a conic, whatever its [`ConicEvaluation`]. A
//! path made by [`Path::op`] without a [`PathOperator`](crate::platform::PathOperator) (Android,
//! where the platform computes operations when it draws, D535) iterates its own commands only.

use std::ops::{Add, BitAnd, BitOr, BitXor, Sub};
use std::sync::Arc;

use crate::geometry::{Offset, Rect};
use crate::math::lerp;
use crate::path::{
    Path, PathCommand, PathData, PathDirection, PathError, PathFillType, PathOperation,
};

/// What a [`PathSegment`] is, as Compose's `PathSegment.Type`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum PathSegmentType {
    /// Starts a contour: one point.
    Move,
    /// A line: its start and end.
    Line,
    /// A quadratic Bézier curve: its start, control point and end.
    Quadratic,
    /// A conic: its start, control point and end, and a weight. compose-rs's paths have none.
    Conic,
    /// A cubic Bézier curve: its start, two control points and end.
    Cubic,
    /// Closes the contour: no points.
    Close,
    /// After the last segment: no points.
    Done,
}

impl PathSegmentType {
    /// The points a segment of this type has, as Compose's `expectedPointCount`.
    pub const fn point_count(self) -> usize {
        match self {
            PathSegmentType::Move => 1,
            PathSegmentType::Line => 2,
            PathSegmentType::Quadratic | PathSegmentType::Conic => 3,
            PathSegmentType::Cubic => 4,
            PathSegmentType::Close | PathSegmentType::Done => 0,
        }
    }
}

/// One segment of a path, as Compose's `PathSegment`: its type, its points as floats (`x0, y0,
/// x1, y1, …`, as many as the type has) and, for a conic, its weight.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PathSegment {
    segment_type: PathSegmentType,
    points: [f32; 8],
    weight: f32,
}

impl PathSegment {
    /// The segment after the last, as Compose's `DoneSegment`.
    pub const DONE: PathSegment =
        PathSegment { segment_type: PathSegmentType::Done, points: [0.0; 8], weight: 0.0 };
    /// A contour's close, as Compose's `CloseSegment`.
    pub const CLOSE: PathSegment =
        PathSegment { segment_type: PathSegmentType::Close, points: [0.0; 8], weight: 0.0 };

    /// As `PathSegment(type, points, weight)`: panics, as Compose's `require`, unless `points`
    /// holds the type's points and the weight is 0 for anything but a conic.
    pub fn new(segment_type: PathSegmentType, points: &[f32], weight: f32) -> PathSegment {
        let expected = segment_type.point_count() * 2;
        assert!(
            points.len() == expected,
            "The number of points for {segment_type:?} must be {expected} (got {})",
            points.len()
        );
        assert!(
            segment_type == PathSegmentType::Conic || weight == 0.0,
            "The weight for {segment_type:?} must be 0.0f (got {weight})"
        );
        let mut stored = [0.0; 8];
        stored[..expected].copy_from_slice(points);
        PathSegment { segment_type, points: stored, weight }
    }

    /// As `type`.
    pub fn segment_type(&self) -> PathSegmentType {
        self.segment_type
    }

    /// Its points' coordinates, as `points`.
    pub fn points(&self) -> &[f32] {
        &self.points[..self.segment_type.point_count() * 2]
    }

    /// Its `index`th point.
    pub fn point(&self, index: usize) -> Offset {
        let points = self.points();
        Offset::new(points[index * 2], points[index * 2 + 1])
    }

    /// A conic's weight, 0 for the rest, as `weight`.
    pub fn weight(&self) -> f32 {
        self.weight
    }
}

/// How a [`PathIterator`] returns conics, as Compose's `PathIterator.ConicEvaluation`.
/// compose-rs's paths hold no conics, so both return the same segments.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Default)]
pub enum ConicEvaluation {
    /// Conics as conics.
    AsConic,
    /// Conics as quadratics approximating them within the iterator's tolerance.
    #[default]
    AsQuadratics,
}

/// The segments of a path, as Compose's `PathIterator`: the path's contents when the iterator
/// was made, segment by segment, each line and curve with its start. As an `Iterator` it stops
/// at the end; [`next_segment`](Self::next_segment) gives [`PathSegment::DONE`] there, as
/// Compose's `next()`.
pub struct PathIterator {
    path: Path,
    data: Arc<PathData>,
    conic_evaluation: ConicEvaluation,
    tolerance: f32,
    index: usize,
    current: Offset,
}

impl PathIterator {
    /// As `PathIterator(path, conicEvaluation, tolerance)` (Compose's defaults:
    /// [`ConicEvaluation::AsQuadratics`] and 0.25).
    pub fn new(path: &Path, conic_evaluation: ConicEvaluation, tolerance: f32) -> PathIterator {
        PathIterator {
            path: path.clone(),
            data: path.data(),
            conic_evaluation,
            tolerance,
            index: 0,
            current: Offset::ZERO,
        }
    }

    /// The path iterated, as `path`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn conic_evaluation(&self) -> ConicEvaluation {
        self.conic_evaluation
    }

    pub fn tolerance(&self) -> f32 {
        self.tolerance
    }

    /// The number of segments, as `calculateSize`: the same with conics converted or not, as
    /// there are none.
    pub fn calculate_size(&self, _include_converted_conics: bool) -> usize {
        self.data.commands().len()
    }

    /// Whether a segment is left, as `hasNext`.
    pub fn has_next(&self) -> bool {
        self.index < self.data.commands().len()
    }

    /// The next segment's type, its points written into `points` from `offset` (a conic's
    /// weight after them), as Compose's `next(outPoints, offset)`.
    pub fn next_into(&mut self, points: &mut [f32], offset: usize) -> PathSegmentType {
        let segment = self.next_segment();
        let values = segment.points();
        points[offset..offset + values.len()].copy_from_slice(values);
        segment.segment_type
    }

    /// The next segment, [`PathSegment::DONE`] after the last, as Compose's `next()`.
    pub fn next_segment(&mut self) -> PathSegment {
        let Some(command) = self.data.commands().get(self.index) else {
            return PathSegment::DONE;
        };
        self.index += 1;
        let start = self.current;
        let (segment_type, points, end): (_, &[Offset], _) = match *command {
            PathCommand::MoveTo(to) => (PathSegmentType::Move, &[to], to),
            PathCommand::LineTo(to) => (PathSegmentType::Line, &[start, to], to),
            PathCommand::QuadTo(control, to) => {
                (PathSegmentType::Quadratic, &[start, control, to], to)
            }
            PathCommand::CubicTo(first, second, to) => {
                (PathSegmentType::Cubic, &[start, first, second, to], to)
            }
            PathCommand::Close => return PathSegment::CLOSE,
        };
        self.current = end;
        let mut stored = [0.0; 8];
        for (index, point) in points.iter().enumerate() {
            stored[index * 2] = point.x;
            stored[index * 2 + 1] = point.y;
        }
        PathSegment { segment_type, points: stored, weight: 0.0 }
    }
}

impl Iterator for PathIterator {
    type Item = PathSegment;

    fn next(&mut self) -> Option<PathSegment> {
        self.has_next().then(|| self.next_segment())
    }
}

impl IntoIterator for &Path {
    type Item = PathSegment;
    type IntoIter = PathIterator;

    /// `for segment in &path`, as Kotlin's `for (segment in path)`.
    fn into_iter(self) -> PathIterator {
        self.iterator()
    }
}

/// Within Compose's `closeTo` of each other.
fn close_to(a: f32, b: f32) -> bool {
    (a - b).abs() < 1.05e-6
}

/// The signed area under a cubic, as Compose's `cubicArea`: positive clockwise (y down).
#[allow(clippy::too_many_arguments)]
fn cubic_area(x0: f32, y0: f32, x1: f32, y1: f32, x2: f32, y2: f32, x3: f32, y3: f32) -> f32 {
    ((y3 - y0) * (x1 + x2) - (x3 - x0) * (y1 + y2) + y1 * (x0 - x2) - x1 * (y0 - y2)
        + y3 * (x2 + x0 / 3.0)
        - x3 * (y2 + y0 / 3.0))
        * 3.0
        / 20.0
}

impl Path {
    /// Its segments, as `iterator()`: conics as quadratics, a tolerance of 0.25.
    pub fn iterator(&self) -> PathIterator {
        PathIterator::new(self, ConicEvaluation::AsQuadratics, 0.25)
    }

    /// Its segments, as `iterator(conicEvaluation, tolerance)`.
    pub fn iterator_with(&self, conic_evaluation: ConicEvaluation, tolerance: f32) -> PathIterator {
        PathIterator::new(self, conic_evaluation, tolerance)
    }

    /// The direction its first contour winds, by the sign of its area (an open contour closed by
    /// a line), as `computeDirection`: clockwise for an empty or flat one.
    pub fn compute_direction(&self) -> PathDirection {
        let mut iterator = self.iterator();
        let mut points = [0.0; 8];
        let (mut first, mut area) = (true, 0.0);
        let (mut start, mut end) = ((0.0, 0.0), (0.0, 0.0));
        loop {
            let p = &mut points;
            match iterator.next_into(p, 0) {
                PathSegmentType::Move => {
                    if !first {
                        break;
                    }
                    first = false;
                    start = (p[0], p[1]);
                }
                PathSegmentType::Line => {
                    area += cubic_area(p[0], p[1], p[0], p[1], p[2], p[3], p[2], p[3]);
                    end = (p[2], p[3]);
                }
                PathSegmentType::Quadratic => {
                    let (x0, y0, x1, y1, x2, y2) = (p[0], p[1], p[2], p[3], p[4], p[5]);
                    let c1x = x0 + 2.0 / 3.0 * (x1 - x0);
                    let c1y = y0 + 2.0 / 3.0 * (y1 - y0);
                    let c2x = x2 + 2.0 / 3.0 * (x1 - x2);
                    let c2y = y2 + 2.0 / 3.0 * (y1 - y2);
                    area += cubic_area(x0, y0, c1x, c1y, c2x, c2y, x2, y2);
                    end = (x2, y2);
                }
                PathSegmentType::Cubic => {
                    area += cubic_area(p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7]);
                    end = (p[6], p[7]);
                }
                PathSegmentType::Close => {
                    if !close_to(end.0, start.0) || !close_to(end.1, start.1) {
                        area += cubic_area(
                            end.0, end.1, end.0, end.1, start.0, start.1, start.0, start.1,
                        );
                        end = start;
                    }
                }
                PathSegmentType::Conic => {}
                PathSegmentType::Done => break,
            }
        }
        if area >= 0.0 { PathDirection::Clockwise } else { PathDirection::CounterClockwise }
    }

    /// Its contours as paths of their own, appended to `contours`, as `divide(contours)`: a
    /// contour that's only a move is dropped.
    pub fn divide_into(&self, contours: &mut Vec<Path>) {
        let mut path = Path::new();
        let (mut first, mut empty) = (true, true);
        let mut iterator = self.iterator();
        let mut p = [0.0; 8];
        loop {
            match iterator.next_into(&mut p, 0) {
                PathSegmentType::Move => {
                    if !first && !empty {
                        contours.push(std::mem::take(&mut path));
                    }
                    first = false;
                    empty = true;
                    path.move_to(p[0], p[1]);
                }
                PathSegmentType::Line => {
                    path.line_to(p[2], p[3]);
                    empty = false;
                }
                PathSegmentType::Quadratic => {
                    path.quadratic_to(p[2], p[3], p[4], p[5]);
                    empty = false;
                }
                PathSegmentType::Cubic => {
                    path.cubic_to(p[2], p[3], p[4], p[5], p[6], p[7]);
                    empty = false;
                }
                PathSegmentType::Close => path.close(),
                PathSegmentType::Conic => {}
                PathSegmentType::Done => break,
            }
        }
        if !first && !empty {
            contours.push(path);
        }
    }

    /// Its contours as paths of their own, as `divide()`.
    pub fn divide(&self) -> Vec<Path> {
        let mut contours = Vec::new();
        self.divide_into(&mut contours);
        contours
    }

    /// Its segments in reverse order, each drawn backwards, appended to `destination`, as
    /// `reverse(destination)`: the last contour first, closed if it was.
    pub fn reverse_into(&self, destination: &Path) {
        let mut segments = Vec::with_capacity(self.iterator().calculate_size(false));
        let mut data: Vec<PathSegment> = Vec::with_capacity(segments.capacity());
        for segment in self {
            segments.push(segment.segment_type);
            if segment.segment_type != PathSegmentType::Close {
                data.push(segment);
            }
        }
        let (mut insert_move, mut insert_close) = (true, false);
        let mut data_index = data.len();
        for segment_type in segments.iter().rev() {
            let points;
            if insert_move {
                data_index -= 1;
                points = data[data_index];
                let last = points.point(segment_type_point_count(points.segment_type) - 1);
                destination.move_to(last.x, last.y);
                insert_move = false;
            } else {
                points = data[data_index];
            }
            let p = points.points();
            match segment_type {
                PathSegmentType::Move => {
                    if insert_close {
                        destination.close();
                        insert_close = false;
                    }
                    insert_move = true;
                }
                PathSegmentType::Line => {
                    destination.line_to(p[0], p[1]);
                    data_index -= 1;
                }
                PathSegmentType::Quadratic => {
                    destination.quadratic_to(p[2], p[3], p[0], p[1]);
                    data_index -= 1;
                }
                PathSegmentType::Cubic => {
                    destination.cubic_to(p[4], p[5], p[2], p[3], p[0], p[1]);
                    data_index -= 1;
                }
                PathSegmentType::Close => insert_close = true,
                PathSegmentType::Conic | PathSegmentType::Done => {}
            }
        }
        if insert_close {
            destination.close();
        }
    }

    /// A new path of its segments reversed, as `reverse()`.
    pub fn reverse(&self) -> Path {
        let destination = Path::new();
        self.reverse_into(&destination);
        destination
    }

    /// A new path with its contents, as `Path.copy` (`Path().apply { addPath(this) }`: the fill
    /// type isn't copied).
    pub fn copy(&self) -> Path {
        let copy = Path::new();
        copy.add_path(self, Offset::ZERO);
        copy
    }

    /// Adds the contours of SVG path data, as `addSvg`: an error for data that isn't (where
    /// Compose throws).
    pub fn add_svg(&self, path_data: &str) -> Result<(), PathError> {
        let parsed = Path::from_svg(path_data)?;
        self.add_path(&parsed, Offset::ZERO);
        Ok(())
    }

    /// Its contents as SVG path data, as `toSvg`, or as a whole SVG document (its bounds the view
    /// box, its fill type the fill rule) when `as_document`.
    pub fn to_svg(&self, as_document: bool) -> String {
        let mut svg = String::new();
        if as_document {
            let bounds = self.get_bounds();
            svg.push_str(&format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{:?} {:?} {:?} {:?}\">\n",
                bounds.left,
                bounds.top,
                bounds.width(),
                bounds.height()
            ));
        }
        let mut iterator = self.iterator();
        if iterator.has_next() {
            if as_document {
                svg.push_str(match self.fill_type() {
                    PathFillType::EvenOdd => "  <path fill-rule=\"evenodd\" d=\"",
                    PathFillType::NonZero => "  <path d=\"",
                });
            }
            let mut last = PathSegmentType::Done;
            let mut p = [0.0; 8];
            while iterator.has_next() {
                let segment_type = iterator.next_into(&mut p, 0);
                let values: &[f32] = match segment_type {
                    PathSegmentType::Move => &p[0..2],
                    PathSegmentType::Line => &p[2..4],
                    PathSegmentType::Quadratic => &p[2..6],
                    PathSegmentType::Cubic => &p[2..8],
                    PathSegmentType::Close => &[],
                    PathSegmentType::Conic | PathSegmentType::Done => continue,
                };
                svg.push_str(svg_command(segment_type, last));
                let numbers: Vec<String> =
                    values.iter().map(|value| format!("{value:?}")).collect();
                svg.push_str(&numbers.join(" "));
                last = segment_type;
            }
            if as_document {
                svg.push_str("\"/>\n");
            }
        }
        if as_document {
            svg.push_str("</svg>\n");
        }
        svg
    }
}

fn segment_type_point_count(segment_type: PathSegmentType) -> usize {
    segment_type.point_count()
}

/// The letter starting a segment, or a space when it repeats the last one's.
fn svg_command(segment_type: PathSegmentType, last: PathSegmentType) -> &'static str {
    if segment_type == last {
        return " ";
    }
    match segment_type {
        PathSegmentType::Move => "M",
        PathSegmentType::Line => "L",
        PathSegmentType::Quadratic => "Q",
        PathSegmentType::Cubic => "C",
        PathSegmentType::Close => "Z",
        PathSegmentType::Conic | PathSegmentType::Done => "",
    }
}

/// The union of two paths, as Compose's `plus` (and `or`).
impl Add<&Path> for &Path {
    type Output = Path;
    fn add(self, other: &Path) -> Path {
        Path::combine(PathOperation::Union, self, other)
    }
}

/// The first path less the second, as Compose's `minus`.
impl Sub<&Path> for &Path {
    type Output = Path;
    fn sub(self, other: &Path) -> Path {
        Path::combine(PathOperation::Difference, self, other)
    }
}

/// The union of two paths, as Compose's `or`.
impl BitOr<&Path> for &Path {
    type Output = Path;
    fn bitor(self, other: &Path) -> Path {
        Path::combine(PathOperation::Union, self, other)
    }
}

/// Where both paths are, as Compose's `and`.
impl BitAnd<&Path> for &Path {
    type Output = Path;
    fn bitand(self, other: &Path) -> Path {
        Path::combine(PathOperation::Intersect, self, other)
    }
}

/// Where exactly one path is, as Compose's `xor`.
impl BitXor<&Path> for &Path {
    type Output = Path;
    fn bitxor(self, other: &Path) -> Path {
        Path::combine(PathOperation::Xor, self, other)
    }
}

/// A line or curve kept by a [`PathHitTester`], with its vertical extent.
#[derive(Clone, Copy)]
struct Edge {
    segment: PathSegment,
    min_y: f32,
    max_y: f32,
}

/// Tests whether points are inside a path, as Compose's `PathHitTester`: the path's lines and
/// curves kept with their vertical extents, so a test looks at the ones crossing the point's
/// height and sums their windings exactly (curves split where they turn vertically and solved
/// for the point's height). Made for many tests of one path; [`update_path`](Self::update_path)
/// takes another.
pub struct PathHitTester {
    fill_type: PathFillType,
    empty: bool,
    bounds: Rect,
    edges: Vec<Edge>,
}

impl Default for PathHitTester {
    fn default() -> PathHitTester {
        PathHitTester {
            fill_type: PathFillType::NonZero,
            empty: true,
            bounds: Rect::ZERO,
            edges: Vec::new(),
        }
    }
}

impl PathHitTester {
    /// As `PathHitTester(path, tolerance)` (Compose's default tolerance: 0.5).
    pub fn new(path: &Path, tolerance: f32) -> PathHitTester {
        let mut tester = PathHitTester::default();
        tester.update_path(path, tolerance);
        tester
    }

    /// Tests `path` from now on, as `updatePath`: its contents now (a later change to it isn't
    /// seen until it's given again). `tolerance` is how closely conics are converted; compose-rs's
    /// paths have none.
    pub fn update_path(&mut self, path: &Path, tolerance: f32) {
        self.fill_type = path.fill_type();
        self.empty = path.is_empty();
        self.bounds = path.get_bounds();
        self.edges.clear();
        for segment in path.iterator_with(ConicEvaluation::AsQuadratics, tolerance) {
            if matches!(
                segment.segment_type,
                PathSegmentType::Line | PathSegmentType::Quadratic | PathSegmentType::Cubic
            ) {
                let (min_y, max_y) = vertical_bounds(&segment);
                self.edges.push(Edge { segment, min_y, max_y });
            }
        }
    }

    /// Whether `position` is inside the path, by its fill type, as `contains`.
    pub fn contains(&self, position: Offset) -> bool {
        if self.empty || !self.bounds.contains(position) {
            return false;
        }
        let (x, y) = (position.x, position.y);
        let mut winding = 0;
        for edge in &self.edges {
            if y < edge.min_y || y > edge.max_y {
                continue;
            }
            let points = edge.segment.points();
            winding += match edge.segment.segment_type {
                PathSegmentType::Line => line_winding(points, x, y),
                PathSegmentType::Quadratic => quadratic_winding(points, x, y),
                PathSegmentType::Cubic => cubic_winding(points, x, y),
                _ => 0,
            };
        }
        if self.fill_type == PathFillType::EvenOdd {
            winding &= 1;
        }
        winding != 0
    }
}

/// `r` if it's in [0, 1] (within Compose's epsilon, then clamped), else NaN.
fn clamp_valid_root_in_unit_range(r: f32) -> f32 {
    let s = r.clamp(0.0, 1.0);
    if (s - r).abs() > 1.05e-6 { f32::NAN } else { s }
}

/// Writes `r` at `roots[index]` if it's a valid root in [0, 1]: the roots written.
fn write_valid_root(r: f32, roots: &mut [f32], index: usize) -> usize {
    let value = clamp_valid_root_in_unit_range(r);
    roots[index] = value;
    usize::from(!value.is_nan())
}

fn find_line_root(p0: f32, p1: f32, roots: &mut [f32], index: usize) -> usize {
    write_valid_root(-p0 / (p1 - p0), roots, index)
}

/// The roots in [0, 1] of the quadratic Bézier polynomial of `p0`, `p1`, `p2`, sorted, written
/// from `index`, as Compose's `findQuadraticRoots`.
fn find_quadratic_roots(p0: f32, p1: f32, p2: f32, roots: &mut [f32], index: usize) -> usize {
    let (a, b, c) = (f64::from(p0), f64::from(p1), f64::from(p2));
    let d = a - 2.0 * b + c;
    let mut count = 0;
    if d != 0.0 {
        let v1 = -(b * b - a * c).sqrt();
        let v2 = -a + b;
        count += write_valid_root((-(v1 + v2) / d) as f32, roots, index);
        count += write_valid_root(((v1 - v2) / d) as f32, roots, index + count);
        if count > 1 {
            let (s, t) = (roots[index], roots[index + 1]);
            if s > t {
                roots.swap(index, index + 1);
            } else if s == t {
                count -= 1;
            }
        }
    } else if b != c {
        count += write_valid_root(((2.0 * b - c) / (2.0 * b - 2.0 * c)) as f32, roots, index);
    }
    count
}

fn evaluate_quadratic(p0: f32, p1: f32, p2: f32, t: f32) -> f32 {
    let by = 2.0 * (p1 - p0);
    let ay = p2 - 2.0 * p1 + p0;
    (ay * t + by) * t + p0
}

fn evaluate_cubic(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let a = p3 + 3.0 * (p1 - p2) - p0;
    let b = 3.0 * (p2 - 2.0 * p1 + p0);
    let c = 3.0 * (p1 - p0);
    ((a * t + b) * t + c) * t + p0
}

/// The lowest and highest y a line or curve reaches, as Compose's `computeVerticalBounds`.
fn vertical_bounds(segment: &PathSegment) -> (f32, f32) {
    let p = segment.points();
    let mut roots = [0.0; 4];
    let (count, end) = match segment.segment_type {
        PathSegmentType::Quadratic => {
            (find_line_root(2.0 * (p[3] - p[1]), 2.0 * (p[5] - p[3]), &mut roots, 0), p[5])
        }
        PathSegmentType::Cubic => {
            let d0 = 3.0 * (p[3] - p[1]);
            let d1 = 3.0 * (p[5] - p[3]);
            let d2 = 3.0 * (p[7] - p[5]);
            let count = find_quadratic_roots(d0, d1, d2, &mut roots, 0);
            (count + find_line_root(2.0 * (d1 - d0), 2.0 * (d2 - d1), &mut roots, count), p[7])
        }
        _ => (0, p[3]),
    };
    let (mut min, mut max) = (p[1].min(end), p[1].max(end));
    for &t in &roots[..count] {
        let y = match segment.segment_type {
            PathSegmentType::Quadratic => evaluate_quadratic(p[1], p[3], p[5], t),
            _ => evaluate_cubic(p[1], p[3], p[5], p[7], t),
        };
        min = min.min(y);
        max = max.max(y);
    }
    (min, max)
}

/// A line's winding about a ray from (`x`, `y`) to the left, as Compose's `lineWinding`.
fn line_winding(points: &[f32], x: f32, y: f32) -> i32 {
    let (x0, y_start, x1, y_end) = (points[0], points[1], points[2], points[3]);
    let dy = y_end - y_start;
    let (mut direction, low, high) =
        if y_start > y_end { (-1, y_end, y_start) } else { (1, y_start, y_end) };
    if y < low || y >= high {
        return 0;
    }
    let cross_product = (x1 - x0) * (y - y_start) - dy * (x - x0);
    if cross_product == 0.0 || cross_product.signum() as i32 == direction {
        direction = 0;
    }
    direction
}

fn is_quadratic_monotonic(y0: f32, y1: f32, y2: f32) -> bool {
    sign(y0 - y1) + sign(y1 - y2) != 0.0
}

/// Kotlin's `sign`: 0 for 0.
fn sign(value: f32) -> f32 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        value
    }
}

fn quadratic_winding(points: &[f32], x: f32, y: f32) -> i32 {
    if is_quadratic_monotonic(points[1], points[3], points[5]) {
        return monotonic_quadratic_winding(points, x, y);
    }
    let mut split = [0.0; 10];
    let count = quadratic_to_monotonic_quadratics(points, &mut split);
    let mut winding = monotonic_quadratic_winding(&split[0..6], x, y);
    if count > 0 {
        winding += monotonic_quadratic_winding(&split[4..10], x, y);
    }
    winding
}

fn monotonic_quadratic_winding(points: &[f32], x: f32, y: f32) -> i32 {
    let (y0, y1, y2) = (points[1], points[3], points[5]);
    let (direction, low, high) = if y0 > y2 { (-1, y2, y0) } else { (1, y0, y2) };
    if y < low || y >= high {
        return 0;
    }
    let mut roots = [0.0; 2];
    let count = find_quadratic_roots(y0 - 2.0 * y1 + y2, 2.0 * (y1 - y0), y0 - y, &mut roots, 0);
    let xt = if count == 0 {
        points[((1 - direction) * 2) as usize]
    } else {
        evaluate_quadratic(points[0], points[2], points[4], roots[0])
    };
    if close_to(xt, x) && (x != points[4] || y != y2) {
        return 0;
    }
    if xt < x { direction } else { 0 }
}

/// `x / y` if it's in (0, 1), else NaN, as Compose's `unitDivide`.
fn unit_divide(x: f32, y: f32) -> f32 {
    let (n, d) = if x < 0.0 { (-x, -y) } else { (x, y) };
    if d == 0.0 || n == 0.0 || n >= d {
        return f32::NAN;
    }
    let r = n / d;
    if r == 0.0 { f32::NAN } else { r }
}

fn quadratic_to_monotonic_quadratics(quadratic: &[f32], destination: &mut [f32; 10]) -> usize {
    let (y0, y1, y2) = (quadratic[1], quadratic[3], quadratic[5]);
    let mut flat_y1 = y1;
    if !is_quadratic_monotonic(y0, y1, y2) {
        let t = unit_divide(y0 - y1, y0 - y1 - y1 + y2);
        if !t.is_nan() {
            let (p0x, p0y, p1x, p1y, p2x, p2y) = (
                quadratic[0],
                quadratic[1],
                quadratic[2],
                quadratic[3],
                quadratic[4],
                quadratic[5],
            );
            let (abx, aby) = (lerp(p0x, p1x, t), lerp(p0y, p1y, t));
            let (bcx, bcy) = (lerp(p1x, p2x, t), lerp(p1y, p2y, t));
            *destination =
                [p0x, p0y, abx, aby, lerp(abx, bcx, t), lerp(aby, bcy, t), bcx, bcy, p2x, p2y];
            return 1;
        }
        flat_y1 = if (y0 - y1).abs() < (y1 - y2).abs() { y0 } else { y2 };
    }
    destination[..6].copy_from_slice(&quadratic[..6]);
    destination[3] = flat_y1;
    0
}

/// The first root in [0, 1] of the cubic Bézier polynomial of `p0`…`p3`, as Compose's
/// `findFirstCubicRoot` (Cardano's method; Rust's `cbrt` where Compose has `fastCbrt`).
fn find_first_cubic_root(p0: f32, p1: f32, p2: f32, p3: f32) -> f32 {
    let close = |value: f64| value.abs() < 1e-7;
    let (p0, p1, p2, p3) = (f64::from(p0), f64::from(p1), f64::from(p2), f64::from(p3));
    let mut a = 3.0 * (p0 - 2.0 * p1 + p2);
    let mut b = 3.0 * (p1 - p0);
    let mut c = p0;
    let d = -p0 + 3.0 * (p1 - p2) + p3;
    if close(d) {
        if close(a) {
            if close(b) {
                return f32::NAN;
            }
            return clamp_valid_root_in_unit_range((-c / b) as f32);
        }
        let q = (b * b - 4.0 * a * c).sqrt();
        let a2 = 2.0 * a;
        let root = clamp_valid_root_in_unit_range(((q - b) / a2) as f32);
        if !root.is_nan() {
            return root;
        }
        return clamp_valid_root_in_unit_range(((-b - q) / a2) as f32);
    }
    a /= d;
    b /= d;
    c /= d;
    let o3 = (3.0 * b - a * a) / 9.0;
    let q2 = (2.0 * a * a * a - 9.0 * a * b + 27.0 * c) / 54.0;
    let discriminant = q2 * q2 + o3 * o3 * o3;
    let a3 = a / 3.0;
    if discriminant < 0.0 {
        let r = (-(o3 * o3 * o3)).sqrt();
        let phi = (-q2 / r).clamp(-1.0, 1.0).acos();
        let t1 = f64::from(2.0 * (r as f32).cbrt());
        let tau = std::f64::consts::TAU;
        for angle in [phi, phi + tau, phi + 2.0 * tau] {
            let root = clamp_valid_root_in_unit_range((t1 * (angle / 3.0).cos() - a3) as f32);
            if !root.is_nan() {
                return root;
            }
        }
        return f32::NAN;
    } else if discriminant == 0.0 {
        let u1 = -(q2 as f32).cbrt();
        let root = clamp_valid_root_in_unit_range(2.0 * u1 - a3 as f32);
        if !root.is_nan() {
            return root;
        }
        return clamp_valid_root_in_unit_range(-u1 - a3 as f32);
    }
    let sd = discriminant.sqrt();
    let u1 = ((-q2 + sd) as f32).cbrt();
    let v1 = ((q2 + sd) as f32).cbrt();
    clamp_valid_root_in_unit_range(u1 - v1 - a3 as f32)
}

fn cubic_winding(points: &[f32], x: f32, y: f32) -> i32 {
    let mut split = [0.0; 20];
    let splits = cubic_to_monotonic_cubics(points, &mut split);
    (0..=splits).map(|index| monotonic_cubic_winding(&split[index * 6..index * 6 + 8], x, y)).sum()
}

fn monotonic_cubic_winding(points: &[f32], x: f32, y: f32) -> i32 {
    let (y0, y3) = (points[1], points[7]);
    let (direction, low, high) = if y0 > y3 { (-1, y3, y0) } else { (1, y0, y3) };
    if y < low || y >= high {
        return 0;
    }
    let (x0, x1, x2, x3) = (points[0], points[2], points[4], points[6]);
    if x < x0.min(x1.min(x2.min(x3))) {
        return 0;
    }
    if x > x0.max(x1.max(x2.max(x3))) {
        return direction;
    }
    let root = find_first_cubic_root(y0 - y, points[3] - y, points[5] - y, y3 - y);
    if root.is_nan() {
        return 0;
    }
    let xt = evaluate_cubic(x0, x1, x2, x3, root);
    if close_to(xt, x) && (x != x3 || y != y3) {
        return 0;
    }
    if xt < x { direction } else { 0 }
}

/// Splits a cubic where it turns vertically: the number of splits, the cubics written one after
/// the other sharing their ends (6 floats apart), as Compose's `cubicToMonotonicCubics`.
fn cubic_to_monotonic_cubics(cubic: &[f32], destination: &mut [f32; 20]) -> usize {
    let (a, b, c, d) = (cubic[1], cubic[3], cubic[5], cubic[7]);
    let mut roots = [0.0; 2];
    let count =
        find_quadratic_roots(d - a + 3.0 * (b - c), 2.0 * (a - b - b - c), b - a, &mut roots, 0);
    destination[..8].copy_from_slice(&cubic[..8]);
    let mut last_t = 0.0;
    let mut offset = 0;
    for &root in &roots[..count] {
        let t = ((root - last_t) / (1.0 - last_t)).clamp(0.0, 1.0);
        last_t = t;
        split_cubic_at(destination, offset, t);
        offset += 6;
    }
    count
}

/// Splits the cubic at `offset` in `cubics` at `t`, in place: the two halves from `offset`.
fn split_cubic_at(cubics: &mut [f32; 20], offset: usize, t: f32) {
    let source: [f32; 8] = cubics[offset..offset + 8].try_into().unwrap_or([0.0; 8]);
    let [p0x, p0y, p1x, p1y, p2x, p2y, p3x, p3y] = source;
    if t >= 1.0 {
        cubics[offset + 8..offset + 14].copy_from_slice(&[p3x, p3y, p3x, p3y, p3x, p3y]);
        return;
    }
    let (abx, aby) = (lerp(p0x, p1x, t), lerp(p0y, p1y, t));
    let (bcx, bcy) = (lerp(p1x, p2x, t), lerp(p1y, p2y, t));
    let (cdx, cdy) = (lerp(p2x, p3x, t), lerp(p2y, p3y, t));
    let (abcx, abcy) = (lerp(abx, bcx, t), lerp(aby, bcy, t));
    let (bcdx, bcdy) = (lerp(bcx, cdx, t), lerp(bcy, cdy, t));
    let (abcdx, abcdy) = (lerp(abcx, bcdx, t), lerp(abcy, bcdy, t));
    cubics[offset..offset + 14].copy_from_slice(&[
        p0x, p0y, abx, aby, abcx, abcy, abcdx, abcdy, bcdx, bcdy, cdx, cdy, p3x, p3y,
    ]);
}
