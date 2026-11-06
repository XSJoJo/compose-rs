//! Lengths and positions along a path, as Compose's `PathMeasure` (D1908): Android's (Skia's
//! `SkContourMeasure`) algorithm in Rust, so both platforms measure the same.
//!
//! The first contour with a length is measured, as Android's `PathMeasure`: its curves are cut
//! in halves until each half is within half a pixel of its chord, and the chords' lengths summed.
//! A position at a distance is found on the chord's piece of curve, at the matching `t`.

use crate::geometry::Offset;
use crate::math::lerp;
use crate::path::{Path, PathCommand};

/// Skia's largest `t`, in its fixed point: a segment's end on its curve.
const MAX_T_VALUE: u32 = 0x3FFF_FFFF;
/// How far a curve's half may stray from its chord before it's cut again, as Skia's
/// `CHEAP_DIST_LIMIT` (at a scale of 1).
const TOLERANCE: f32 = 0.5;

/// One measured curve of the contour: a line, quadratic or cubic, as Skia's points with a type.
#[derive(Clone, Copy, Debug)]
enum Curve {
    Line([Offset; 2]),
    Quad([Offset; 3]),
    Cubic([Offset; 4]),
}

/// A chord of a curve: the contour's length at its end, and its end's `t` on the curve.
#[derive(Clone, Copy, Debug)]
struct Segment {
    distance: f32,
    curve: usize,
    t_value: u32,
}

impl Segment {
    fn t(&self) -> f32 {
        self.t_value as f32 * (1.0 / MAX_T_VALUE as f32)
    }
}

/// Measures a path's first contour, as Compose's `PathMeasure`: its length, the position and
/// direction at a distance along it, and the piece between two distances.
#[derive(Default)]
pub struct PathMeasure {
    curves: Vec<Curve>,
    segments: Vec<Segment>,
    length: f32,
}

fn tspan_big_enough(tspan: u32) -> bool {
    (tspan >> 10) != 0
}

fn cheap_dist_exceeds_limit(point: Offset, x: f32, y: f32) -> bool {
    (x - point.x).abs().max((y - point.y).abs()) > TOLERANCE
}

fn quad_too_curvy(points: &[Offset; 3]) -> bool {
    // The middle of the curve (a/4 + b/2 + c/4) less the middle of its chord (a/2 + c/2).
    let dx = points[1].x / 2.0 - (points[0].x + points[2].x) / 2.0 / 2.0;
    let dy = points[1].y / 2.0 - (points[0].y + points[2].y) / 2.0 / 2.0;
    dx.abs().max(dy.abs()) > TOLERANCE
}

fn cubic_too_curvy(points: &[Offset; 4]) -> bool {
    let at = |t: f32| (lerp(points[0].x, points[3].x, t), lerp(points[0].y, points[3].y, t));
    let (x1, y1) = at(1.0 / 3.0);
    let (x2, y2) = at(2.0 / 3.0);
    cheap_dist_exceeds_limit(points[1], x1, y1) || cheap_dist_exceeds_limit(points[2], x2, y2)
}

fn lerp_offset(a: Offset, b: Offset, t: f32) -> Offset {
    Offset::new(lerp(a.x, b.x, t), lerp(a.y, b.y, t))
}

/// A quadratic cut at `t`: the first half's points, then the second's (sharing the middle).
fn chop_quad_at(points: &[Offset; 3], t: f32) -> [Offset; 5] {
    let ab = lerp_offset(points[0], points[1], t);
    let bc = lerp_offset(points[1], points[2], t);
    [points[0], ab, lerp_offset(ab, bc, t), bc, points[2]]
}

/// A cubic cut at `t`: the first half's points, then the second's (sharing the middle).
fn chop_cubic_at(points: &[Offset; 4], t: f32) -> [Offset; 7] {
    let ab = lerp_offset(points[0], points[1], t);
    let bc = lerp_offset(points[1], points[2], t);
    let cd = lerp_offset(points[2], points[3], t);
    let abc = lerp_offset(ab, bc, t);
    let bcd = lerp_offset(bc, cd, t);
    [points[0], ab, abc, lerp_offset(abc, bcd, t), bcd, cd, points[3]]
}

fn distance(a: Offset, b: Offset) -> f32 {
    (b - a).get_distance()
}

impl PathMeasure {
    /// A measure of nothing, as `PathMeasure()`: [`set_path`](Self::set_path) gives it a path.
    pub fn new() -> PathMeasure {
        PathMeasure::default()
    }

    /// Measures `path`'s first contour with a length, as `setPath` (none for `None`), closed
    /// back to its start when `force_closed`. A later change to the path isn't seen.
    pub fn set_path(&mut self, path: Option<&Path>, force_closed: bool) {
        self.curves.clear();
        self.segments.clear();
        self.length = 0.0;
        let Some(path) = path else { return };
        let data = path.data();
        let commands = data.commands();
        let mut index = 0;
        while index < commands.len() {
            index = self.measure_contour(commands, index, force_closed);
            if self.length > 0.0 && self.length.is_finite() {
                return;
            }
            self.curves.clear();
            self.segments.clear();
            self.length = 0.0;
        }
    }

    /// Measures the contour starting at `commands[start]`: the index after it.
    fn measure_contour(
        &mut self,
        commands: &[PathCommand],
        start: usize,
        force_closed: bool,
    ) -> usize {
        let (mut first, mut current) = (Offset::ZERO, Offset::ZERO);
        let mut index = start;
        let mut closed = false;
        while let Some(command) = commands.get(index) {
            match *command {
                PathCommand::MoveTo(to) => {
                    if index > start {
                        break;
                    }
                    first = to;
                    current = to;
                }
                PathCommand::LineTo(to) => {
                    self.add_line(current, to);
                    current = to;
                }
                PathCommand::QuadTo(control, to) => {
                    let points = [current, control, to];
                    let curve = self.curves.len();
                    self.curves.push(Curve::Quad(points));
                    self.length = self.quad_segments(&points, self.length, 0, MAX_T_VALUE, curve);
                    current = to;
                }
                PathCommand::CubicTo(first_control, second_control, to) => {
                    let points = [current, first_control, second_control, to];
                    let curve = self.curves.len();
                    self.curves.push(Curve::Cubic(points));
                    self.length = self.cubic_segments(&points, self.length, 0, MAX_T_VALUE, curve);
                    current = to;
                }
                PathCommand::Close => {
                    // As Skia's iterator: a close draws the line back to the start.
                    self.add_line(current, first);
                    current = first;
                    closed = true;
                }
            }
            index += 1;
        }
        if force_closed && !closed {
            self.add_line(current, first);
        }
        index
    }

    fn add_line(&mut self, from: Offset, to: Offset) {
        let previous = self.length;
        let length = previous + distance(from, to);
        if length > previous {
            let curve = self.curves.len();
            self.curves.push(Curve::Line([from, to]));
            self.segments.push(Segment { distance: length, curve, t_value: MAX_T_VALUE });
            self.length = length;
        }
    }

    fn quad_segments(
        &mut self,
        points: &[Offset; 3],
        length: f32,
        min_t: u32,
        max_t: u32,
        curve: usize,
    ) -> f32 {
        if tspan_big_enough(max_t - min_t) && quad_too_curvy(points) {
            let halves = chop_quad_at(points, 0.5);
            let half_t = (min_t + max_t) >> 1;
            let first = [halves[0], halves[1], halves[2]];
            let second = [halves[2], halves[3], halves[4]];
            let length = self.quad_segments(&first, length, min_t, half_t, curve);
            return self.quad_segments(&second, length, half_t, max_t, curve);
        }
        self.add_segment(length, distance(points[0], points[2]), curve, max_t)
    }

    fn cubic_segments(
        &mut self,
        points: &[Offset; 4],
        length: f32,
        min_t: u32,
        max_t: u32,
        curve: usize,
    ) -> f32 {
        if tspan_big_enough(max_t - min_t) && cubic_too_curvy(points) {
            let halves = chop_cubic_at(points, 0.5);
            let half_t = (min_t + max_t) >> 1;
            let first = [halves[0], halves[1], halves[2], halves[3]];
            let second = [halves[3], halves[4], halves[5], halves[6]];
            let length = self.cubic_segments(&first, length, min_t, half_t, curve);
            return self.cubic_segments(&second, length, half_t, max_t, curve);
        }
        self.add_segment(length, distance(points[0], points[3]), curve, max_t)
    }

    fn add_segment(&mut self, previous: f32, chord: f32, curve: usize, t_value: u32) -> f32 {
        let length = previous + chord;
        if length > previous {
            self.segments.push(Segment { distance: length, curve, t_value });
        }
        length
    }

    /// The contour's length, as `length` (0 without a path).
    pub fn length(&self) -> f32 {
        self.length
    }

    /// The segment at `distance` and the `t` there on its curve, as Skia's `distanceToSegment`.
    fn segment_at(&self, distance: f32) -> (usize, f32) {
        let index = self.segments.partition_point(|segment| segment.distance < distance);
        let index = index.min(self.segments.len() - 1);
        let segment = self.segments[index];
        let (mut start_t, mut start_distance) = (0.0, 0.0);
        if index > 0 {
            let previous = self.segments[index - 1];
            start_distance = previous.distance;
            if previous.curve == segment.curve {
                start_t = previous.t();
            }
        }
        let t = start_t
            + (segment.t() - start_t) * (distance - start_distance)
                / (segment.distance - start_distance);
        (index, t.clamp(0.0, 1.0))
    }

    /// The position and unit tangent at `distance` (clamped to the contour), or `None` without
    /// a contour or for a NaN distance.
    fn position_and_tangent(&self, distance: f32) -> Option<(Offset, Offset)> {
        if self.segments.is_empty() || distance.is_nan() {
            return None;
        }
        let distance = distance.clamp(0.0, self.length);
        let (index, t) = self.segment_at(distance);
        Some(evaluate(&self.curves[self.segments[index].curve], t))
    }

    /// The point at `distance` along the contour (clamped to it), as `getPosition`:
    /// `Offset::UNSPECIFIED` without a contour.
    pub fn get_position(&self, distance: f32) -> Offset {
        self.position_and_tangent(distance).map_or(Offset::UNSPECIFIED, |(position, _)| position)
    }

    /// The unit tangent at `distance` along the contour (clamped to it), as `getTangent`:
    /// `Offset::UNSPECIFIED` without a contour.
    pub fn get_tangent(&self, distance: f32) -> Offset {
        self.position_and_tangent(distance).map_or(Offset::UNSPECIFIED, |(_, tangent)| tangent)
    }

    /// Adds the piece of the contour from `start_distance` to `stop_distance` (clamped to it) to
    /// `destination`, starting with a move there when `start_with_move_to`, as `getSegment`:
    /// `false`, adding nothing, when the start is after the stop or there's no contour.
    pub fn get_segment(
        &self,
        start_distance: f32,
        stop_distance: f32,
        destination: &Path,
        start_with_move_to: bool,
    ) -> bool {
        let start_distance = start_distance.max(0.0);
        let stop_distance = stop_distance.min(self.length);
        let reversed = start_distance > stop_distance;
        if reversed || start_distance.is_nan() || stop_distance.is_nan() || self.segments.is_empty()
        {
            return false;
        }
        let (mut index, mut start_t) = self.segment_at(start_distance);
        let (stop_index, stop_t) = self.segment_at(stop_distance);
        let stop_curve = self.segments[stop_index].curve;
        if start_with_move_to {
            let (position, _) = evaluate(&self.curves[self.segments[index].curve], start_t);
            destination.move_to(position.x, position.y);
        }
        let mut curve = self.segments[index].curve;
        if curve == stop_curve {
            segment_to(&self.curves[curve], start_t, stop_t, destination);
            return true;
        }
        while curve < stop_curve {
            segment_to(&self.curves[curve], start_t, 1.0, destination);
            while index < self.segments.len() && self.segments[index].curve == curve {
                index += 1;
            }
            curve = self.segments[index].curve;
            start_t = 0.0;
        }
        segment_to(&self.curves[stop_curve], 0.0, stop_t, destination);
        true
    }
}

/// The point at `t` on `curve` and its unit tangent.
fn evaluate(curve: &Curve, t: f32) -> (Offset, Offset) {
    let (position, tangent) = match *curve {
        Curve::Line([a, b]) => (lerp_offset(a, b, t), b - a),
        Curve::Quad([a, b, c]) => {
            let ab = lerp_offset(a, b, t);
            let bc = lerp_offset(b, c, t);
            let mut tangent = bc - ab;
            if tangent == Offset::ZERO {
                tangent = c - a;
            }
            (lerp_offset(ab, bc, t), tangent)
        }
        Curve::Cubic(points) => {
            let halves = chop_cubic_at(&points, t);
            let mut tangent = halves[4] - halves[2];
            if tangent == Offset::ZERO {
                // At an end whose control point is on it: toward the other control point.
                tangent = if t == 0.0 { points[2] - points[0] } else { points[3] - points[1] };
            }
            (halves[3], tangent)
        }
    };
    let length = tangent.get_distance();
    let tangent = if length > 0.0 { tangent / length } else { Offset::ZERO };
    (position, tangent)
}

/// Adds the piece of `curve` from `start_t` to `stop_t` to `destination`, as Skia's
/// `SkContourMeasure_segTo`: a zero-length line where they're equal.
fn segment_to(curve: &Curve, start_t: f32, stop_t: f32, destination: &Path) {
    if start_t == stop_t {
        if let Some(last) = destination.data().last_point() {
            destination.line_to(last.x, last.y);
        }
        return;
    }
    match *curve {
        Curve::Line([a, b]) => {
            let end = if stop_t == 1.0 { b } else { lerp_offset(a, b, stop_t) };
            destination.line_to(end.x, end.y);
        }
        Curve::Quad(points) => {
            let piece = if start_t == 0.0 {
                if stop_t == 1.0 { points } else { first_quad(&chop_quad_at(&points, stop_t)) }
            } else {
                let rest = chop_quad_at(&points, start_t);
                let rest = [rest[2], rest[3], rest[4]];
                if stop_t == 1.0 {
                    rest
                } else {
                    first_quad(&chop_quad_at(&rest, (stop_t - start_t) / (1.0 - start_t)))
                }
            };
            destination.quadratic_to(piece[1].x, piece[1].y, piece[2].x, piece[2].y);
        }
        Curve::Cubic(points) => {
            let piece = if start_t == 0.0 {
                if stop_t == 1.0 { points } else { first_cubic(&chop_cubic_at(&points, stop_t)) }
            } else {
                let rest = chop_cubic_at(&points, start_t);
                let rest = [rest[3], rest[4], rest[5], rest[6]];
                if stop_t == 1.0 {
                    rest
                } else {
                    first_cubic(&chop_cubic_at(&rest, (stop_t - start_t) / (1.0 - start_t)))
                }
            };
            let [_, first, second, end] = piece;
            destination.cubic_to(first.x, first.y, second.x, second.y, end.x, end.y);
        }
    }
}

fn first_quad(halves: &[Offset; 5]) -> [Offset; 3] {
    [halves[0], halves[1], halves[2]]
}

fn first_cubic(halves: &[Offset; 7]) -> [Offset; 4] {
    [halves[0], halves[1], halves[2], halves[3]]
}
