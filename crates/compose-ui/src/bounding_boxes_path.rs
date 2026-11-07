//! A range's contour from its boxes, as Compose's `TextLayoutResult.fillPathForRange`
//! (`BoundingBoxesPath.kt`): each box of [`get_bounding_boxes_for_range`] a rectangle, or the
//! boxes of consecutive lines that overlap joined into one rectilinear outline, its corners
//! rounded (D2050).
//!
//! [`get_bounding_boxes_for_range`]: TextLayoutResult::get_bounding_boxes_for_range

use crate::geometry::{CornerRadius, RoundRect};
use crate::path::{Path, PathDirection};
use crate::text::TextLayoutResult;
use crate::unit::{Offset, Rect};

impl TextLayoutResult {
    /// Fills `target` with an outline of `start..end`, as Compose's `fillPathForRange`: with
    /// `merge_lines`, the boxes of consecutive lines with one box each that overlap are one
    /// outline; else (and on a line of many runs) each box is a rectangle of its own. Every
    /// corner, outer and inner, is rounded by `corner_radius`, scaled down (keeping its
    /// proportions) where a side is shorter than two radii. `target` is rewound first, its fill
    /// type kept. Panics on a range outside the text, as Compose's. Compose's defaults are
    /// [`CornerRadius::ZERO`] and `merge_lines` true.
    pub fn fill_path_for_range(
        &self,
        start: usize,
        end: usize,
        target: &Path,
        corner_radius: CornerRadius,
        merge_lines: bool,
    ) {
        // Rewinding unsets the fill type: set it back.
        let fill_type = target.fill_type();
        target.rewind();
        target.set_fill_type(fill_type);
        let boxes = self.get_bounding_boxes_for_range(start, end);
        let count = boxes.len();
        if count == 0 {
            return;
        }
        let rounded = corner_radius.x > 0.0 || corner_radius.y > 0.0;
        let add = |rect: &Rect| add_rect(target, rect, corner_radius, rounded);
        if !merge_lines {
            boxes.iter().for_each(add);
            return;
        }
        let mut index = 0;
        while index < count {
            let line_end = end_of_line(&boxes, index);
            if line_end - index > 1 {
                // A line of many runs: each its own rectangle.
                boxes[index..line_end].iter().for_each(add);
                index = line_end;
            } else {
                let stack_start = index;
                index += 1;
                while index < count
                    && end_of_line(&boxes, index) == index + 1
                    && self.can_merge(&boxes[index - 1], &boxes[index])
                {
                    index += 1;
                }
                add_stack(target, &boxes[stack_start..index], corner_radius, rounded);
            }
        }
    }

    /// Whether `current` is on the line after `previous`'s and overlaps it across.
    fn can_merge(&self, previous: &Rect, current: &Rect) -> bool {
        if self.multi_paragraph.line_count() == 0 {
            return false;
        }
        let previous_line = self.get_line_for_vertical_position(previous.center().y);
        let current_line = self.get_line_for_vertical_position(current.center().y);
        let overlap = previous.left.max(current.left) < previous.right.min(current.right);
        current_line == previous_line + 1 && overlap
    }
}

/// The end (exclusive) of the boxes from `start` on its line: the same top and bottom, within
/// half a pixel.
fn end_of_line(boxes: &[Rect], start: usize) -> usize {
    let first = boxes[start];
    let same = |rect: &Rect| {
        (rect.top - first.top).abs() < 0.5 && (rect.bottom - first.bottom).abs() < 0.5
    };
    start + 1 + boxes[start + 1..].iter().take_while(|rect| same(rect)).count()
}

/// `rect` as a clockwise rectangle, its corners rounded if `rounded`; nothing if it has no
/// height.
fn add_rect(path: &Path, rect: &Rect, corner_radius: CornerRadius, rounded: bool) {
    if rect.bottom <= rect.top {
        return;
    }
    if rounded {
        path.add_round_rect_with(RoundRect::new(*rect, corner_radius), PathDirection::Clockwise);
    } else {
        path.add_rect_with(*rect, PathDirection::Clockwise);
    }
}

/// The boxes of consecutive lines, one each, as one clockwise rectilinear outline: along the
/// top of the first, down their right sides stepping at each seam, along the bottom of the
/// last, up their left sides. Edges within half a pixel are one.
fn add_stack(path: &Path, boxes: &[Rect], corner_radius: CornerRadius, rounded: bool) {
    match boxes {
        [] => return,
        [only] => return add_rect(path, only, corner_radius, rounded),
        _ => {}
    }
    let last = boxes.len() - 1;
    let (min_top, max_bottom) = (boxes[0].top, boxes[last].bottom);
    if max_bottom <= min_top {
        return;
    }
    // At most two vertices at each seam on each side, and the four corners.
    let mut vertices: Vec<Offset> = Vec::with_capacity(4 * boxes.len() + 4);
    let first_left = boxes[0].left;
    let mut right = boxes[0].right;
    vertices.push(Offset::new(first_left, min_top));
    vertices.push(Offset::new(right, min_top));
    let seam =
        |upper: &Rect, lower: &Rect| ((upper.bottom + lower.top) / 2.0).clamp(min_top, max_bottom);
    for pair in boxes.windows(2) {
        let next_right = pair[1].right;
        if (right - next_right).abs() >= 0.5 {
            let y = seam(&pair[0], &pair[1]);
            vertices.push(Offset::new(right, y));
            vertices.push(Offset::new(next_right, y));
            right = next_right;
        }
    }
    let same_as_first = |left: f32| if (left - first_left).abs() < 0.5 { first_left } else { left };
    let mut left = same_as_first(boxes[last].left);
    vertices.push(Offset::new(right, max_bottom));
    vertices.push(Offset::new(left, max_bottom));
    for pair in boxes.windows(2).rev() {
        let previous_left = same_as_first(pair[0].left);
        if (left - previous_left).abs() >= 0.5 {
            let y = seam(&pair[0], &pair[1]);
            vertices.push(Offset::new(left, y));
            vertices.push(Offset::new(previous_left, y));
            left = previous_left;
        }
    }
    add_polygon(path, &vertices, corner_radius, rounded);
}

/// The closed rectilinear polygon through `vertices` (clockwise), each corner rounded by a
/// quadratic curve through it when `rounded`: both radii scaled down by one factor where either
/// is more than half the sides it joins.
fn add_polygon(path: &Path, vertices: &[Offset], corner_radius: CornerRadius, rounded: bool) {
    let count = vertices.len();
    if count < 3 {
        return;
    }
    if !rounded {
        path.move_to(vertices[0].x, vertices[0].y);
        for vertex in &vertices[1..] {
            path.line_to(vertex.x, vertex.y);
        }
        path.close();
        return;
    }
    for (index, &current) in vertices.iter().enumerate() {
        let previous = vertices[(index + count - 1) % count];
        let next = vertices[(index + 1) % count];
        let (dx_in, dy_in) = (current.x - previous.x, current.y - previous.y);
        let (dx_out, dy_out) = (next.x - current.x, next.y - current.y);
        let length_in = dx_in.abs() + dy_in.abs();
        let length_out = dx_out.abs() + dy_out.abs();
        if length_in < 0.01 || length_out < 0.01 {
            if index == 0 {
                path.move_to(current.x, current.y);
            } else {
                path.line_to(current.x, current.y);
            }
            continue;
        }
        let max_x = (dx_in.abs() + dx_out.abs()) / 2.0;
        let max_y = (dy_in.abs() + dy_out.abs()) / 2.0;
        let (raw_x, raw_y) = (corner_radius.x.max(0.0), corner_radius.y.max(0.0));
        let mut scale = 1.0f32;
        if raw_x > max_x {
            scale = scale.min(max_x / raw_x);
        }
        if raw_y > max_y {
            scale = scale.min(max_y / raw_y);
        }
        let (radius_x, radius_y) = (raw_x * scale, raw_y * scale);
        let entry_x = current.x - radius_x * dx_in / length_in;
        let entry_y = current.y - radius_y * dy_in / length_in;
        if index == 0 {
            path.move_to(entry_x, entry_y);
        } else {
            path.line_to(entry_x, entry_y);
        }
        let exit_x = current.x + radius_x * dx_out / length_out;
        let exit_y = current.y + radius_y * dy_out / length_out;
        path.quadratic_to(current.x, current.y, exit_x, exit_y);
    }
    path.close();
}
