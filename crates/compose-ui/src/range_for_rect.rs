//! The text in an area, and the boxes of a range's characters: Compose's `getRangeForRect` (with
//! `TextGranularity` and `TextInclusionStrategy`), `fillBoundingBoxes` and
//! `getBoundingBoxesForRange` (D2048), computed from a [`Paragraph`]'s queries as Compose's
//! compatibility code computes them from Android's `Layout` below API 34, so every measurer
//! answers them alike.

use std::fmt;
use std::rc::Rc;

use unicode_segmentation::UnicodeSegmentation;

use crate::paragraph::Paragraph;
use crate::text_attributes::ResolvedTextDirection;
use crate::text_range::TextRange;
use crate::unit::Rect;

/// How a text is cut into pieces when finding the text in an area, as Compose's
/// `TextGranularity`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum TextGranularity {
    /// Grapheme clusters: what a user sees as one character.
    Character,
    /// Words (Compose's `WordIterator`'s breaks), spaces between them left out.
    Word,
}

/// When a piece of text is in an area, as Compose's `TextInclusionStrategy`: given the piece's
/// bounds and the area.
#[derive(Clone)]
pub enum TextInclusionStrategy {
    /// The piece overlaps the area.
    AnyOverlap,
    /// The piece is all inside a non-empty area.
    ContainsAll,
    /// The piece's center is inside the area.
    ContainsCenter,
    /// The answer of a function of the piece's bounds and the area.
    Custom(Rc<dyn Fn(Rect, Rect) -> bool>),
}

impl TextInclusionStrategy {
    /// A strategy that asks `is_included` with the piece's bounds and the area, as Compose's
    /// `fun interface`.
    pub fn new(is_included: impl Fn(Rect, Rect) -> bool + 'static) -> TextInclusionStrategy {
        TextInclusionStrategy::Custom(Rc::new(is_included))
    }

    /// Whether a piece of text within `text_bounds` is in `rect`.
    pub fn is_included(&self, text_bounds: Rect, rect: Rect) -> bool {
        match self {
            TextInclusionStrategy::AnyOverlap => text_bounds.overlaps(rect),
            TextInclusionStrategy::ContainsAll => {
                !rect.is_empty()
                    && text_bounds.left >= rect.left
                    && text_bounds.right <= rect.right
                    && text_bounds.top >= rect.top
                    && text_bounds.bottom <= rect.bottom
            }
            TextInclusionStrategy::ContainsCenter => rect.contains(text_bounds.center()),
            TextInclusionStrategy::Custom(is_included) => is_included(text_bounds, rect),
        }
    }
}

impl fmt::Debug for TextInclusionStrategy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TextInclusionStrategy::AnyOverlap => formatter.write_str("AnyOverlap"),
            TextInclusionStrategy::ContainsAll => formatter.write_str("ContainsAll"),
            TextInclusionStrategy::ContainsCenter => formatter.write_str("ContainsCenter"),
            TextInclusionStrategy::Custom(_) => formatter.write_str("Custom"),
        }
    }
}

/// Whitespace as Java's `Character.isWhitespace`: Unicode's space separators but the no-break
/// ones, the line and paragraph separators, and the ASCII controls tab to carriage return and
/// file to unit separator.
fn is_java_whitespace(c: char) -> bool {
    match c {
        '\u{00A0}' | '\u{2007}' | '\u{202F}' => false,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | '\u{1C}'..='\u{1F}' => true,
        ' '
        | '\u{1680}'
        | '\u{2000}'..='\u{200A}'
        | '\u{2028}'
        | '\u{2029}'
        | '\u{205F}'
        | '\u{3000}' => true,
        _ => false,
    }
}

/// The boundaries of a text's pieces, as Compose's `SegmentFinder`s: offsets, `None` for
/// Compose's `DONE`.
struct SegmentFinder<'a> {
    text: &'a str,
    granularity: TextGranularity,
    /// The text's grapheme cluster or word boundaries, its start and end included.
    boundaries: Vec<usize>,
}

impl<'a> SegmentFinder<'a> {
    fn new(text: &'a str, granularity: TextGranularity) -> SegmentFinder<'a> {
        let boundaries = match granularity {
            TextGranularity::Character => {
                let mut boundaries: Vec<usize> =
                    text.grapheme_indices(true).map(|(offset, _)| offset).collect();
                boundaries.push(text.len());
                boundaries
            }
            TextGranularity::Word => crate::word_boundary::word_breaks(text),
        };
        SegmentFinder { text, granularity, boundaries }
    }

    /// The last boundary before `offset` (a break iterator's `preceding`).
    fn preceding(&self, offset: usize) -> Option<usize> {
        let index = self.boundaries.partition_point(|&boundary| boundary < offset);
        index.checked_sub(1).map(|index| self.boundaries[index])
    }

    /// The first boundary after `offset` (a break iterator's `following`).
    fn following(&self, offset: usize) -> Option<usize> {
        let index = self.boundaries.partition_point(|&boundary| boundary <= offset);
        self.boundaries.get(index).copied()
    }

    fn whitespace_at(&self, offset: usize) -> bool {
        self.text[offset..].chars().next().is_some_and(is_java_whitespace)
    }

    fn whitespace_before(&self, offset: usize) -> bool {
        self.text[..offset].chars().next_back().is_some_and(is_java_whitespace)
    }

    fn previous_start_boundary(&self, offset: usize) -> Option<usize> {
        match self.granularity {
            TextGranularity::Character => self.preceding(offset),
            TextGranularity::Word => {
                let mut boundary = offset;
                loop {
                    boundary = self.preceding(boundary)?;
                    if !self.whitespace_at(boundary) {
                        return Some(boundary);
                    }
                }
            }
        }
    }

    fn previous_end_boundary(&self, offset: usize) -> Option<usize> {
        match self.granularity {
            TextGranularity::Character => {
                // Only a boundary with another one before it ends a cluster.
                let previous = self.preceding(offset)?;
                self.preceding(previous).map(|_| previous)
            }
            TextGranularity::Word => {
                let mut boundary = offset;
                loop {
                    boundary = self.preceding(boundary).filter(|&boundary| boundary != 0)?;
                    if !self.whitespace_before(boundary) {
                        return Some(boundary);
                    }
                }
            }
        }
    }

    fn next_start_boundary(&self, offset: usize) -> Option<usize> {
        match self.granularity {
            TextGranularity::Character => {
                // Only a boundary with another one after it starts a cluster.
                let next = self.following(offset)?;
                self.following(next).map(|_| next)
            }
            TextGranularity::Word => {
                let mut boundary = offset;
                loop {
                    boundary =
                        self.following(boundary).filter(|&boundary| boundary != self.text.len())?;
                    if !self.whitespace_at(boundary) {
                        return Some(boundary);
                    }
                }
            }
        }
    }

    fn next_end_boundary(&self, offset: usize) -> Option<usize> {
        match self.granularity {
            TextGranularity::Character => self.following(offset),
            TextGranularity::Word => {
                let mut boundary = offset;
                loop {
                    boundary = self.following(boundary)?;
                    if !self.whitespace_before(boundary) {
                        return Some(boundary);
                    }
                }
            }
        }
    }
}

/// A line's characters: their offsets and horizontal extents (Compose's
/// `fillLineHorizontalBounds`), and its bidi runs in the text's order.
struct LineBounds {
    top: f32,
    bottom: f32,
    /// Each character's offset, then the line's end.
    offsets: Vec<usize>,
    lefts: Vec<f32>,
    rights: Vec<f32>,
    /// Character index ranges of the same direction, and whether they run right to left.
    runs: Vec<(usize, usize, bool)>,
}

impl LineBounds {
    fn of<P: Paragraph + ?Sized>(paragraph: &P, line: usize) -> Option<LineBounds> {
        let start = paragraph.get_line_start(line);
        let end = paragraph.get_line_end(line, false);
        if start >= end {
            return None;
        }
        let text = paragraph.text();
        let mut offsets: Vec<usize> =
            text[start..end].char_indices().map(|(offset, _)| start + offset).collect();
        let (mut lefts, mut rights) =
            (Vec::with_capacity(offsets.len()), Vec::with_capacity(offsets.len()));
        let mut runs: Vec<(usize, usize, bool)> = Vec::new();
        for (index, &offset) in offsets.iter().enumerate() {
            let rect = paragraph.get_bounding_box(offset);
            lefts.push(rect.left);
            rights.push(rect.right);
            let rtl = paragraph.get_bidi_run_direction(offset) == ResolvedTextDirection::Rtl;
            match runs.last_mut() {
                Some(run) if run.2 == rtl => run.1 = index + 1,
                _ => runs.push((index, index + 1, rtl)),
            }
        }
        offsets.push(end);
        Some(LineBounds {
            top: paragraph.get_line_top(line),
            bottom: paragraph.get_line_bottom(line),
            offsets,
            lefts,
            rights,
            runs,
        })
    }

    /// The index of the character at `offset` (of a later one, inside a character).
    fn index_of(&self, offset: usize) -> usize {
        self.offsets.partition_point(|&at| at < offset)
    }

    fn left_of(&self, offset: usize) -> f32 {
        self.lefts[self.index_of(offset).min(self.lefts.len() - 1)]
    }

    fn right_of(&self, offset: usize) -> f32 {
        self.rights[self.index_of(offset).min(self.rights.len() - 1)]
    }

    /// The offset of the character before the one at `offset`.
    fn before(&self, offset: usize) -> usize {
        self.offsets[self.index_of(offset).saturating_sub(1)]
    }

    /// The bounds of a piece `start..end` of a run, as Compose's: from its first character's
    /// left to its last's right, the other way in a right-to-left run.
    fn piece(&self, start: usize, end: usize, rtl: bool) -> Rect {
        let last = self.before(end);
        let (left, right) = if rtl {
            (self.left_of(last), self.right_of(start))
        } else {
            (self.left_of(start), self.right_of(last))
        };
        Rect::new(left, self.top, right, self.bottom)
    }

    /// Where the first (or, with `!get_start`, the last) piece of the line in `rect` starts (or
    /// ends), as Compose's `getStartOrEndOffsetForRectWithinLine`.
    fn offset_in(
        &self,
        rect: Rect,
        finder: &SegmentFinder<'_>,
        strategy: &TextInclusionStrategy,
        get_start: bool,
    ) -> Option<usize> {
        let mut runs: Vec<&(usize, usize, bool)> = self.runs.iter().collect();
        if !get_start {
            runs.reverse();
        }
        runs.into_iter().find_map(|&(first, end, rtl)| {
            let run_left = if rtl { self.lefts[end - 1] } else { self.lefts[first] };
            let run_right = if rtl { self.rights[first] } else { self.rights[end - 1] };
            if !(run_right >= rect.left && run_left <= rect.right) {
                return None;
            }
            if get_start {
                self.start_in_run(first, end, rtl, run_left, run_right, rect, finder, strategy)
            } else {
                self.end_in_run(first, end, rtl, run_left, run_right, rect, finder, strategy)
            }
        })
    }

    /// The character index the area's `edge` is in, by the characters' left bounds.
    fn search(&self, first: usize, end: usize, rtl: bool, edge: f32) -> usize {
        let (mut low, mut high) = (first, end);
        while high - low > 1 {
            let guess = (high + low) / 2;
            let position = self.lefts[guess];
            if (!rtl && position > edge) || (rtl && position < edge) {
                high = guess;
            } else {
                low = guess;
            }
        }
        if rtl { high } else { low }
    }

    #[allow(clippy::too_many_arguments)]
    fn start_in_run(
        &self,
        first: usize,
        end: usize,
        rtl: bool,
        run_left: f32,
        run_right: f32,
        rect: Rect,
        finder: &SegmentFinder<'_>,
        strategy: &TextInclusionStrategy,
    ) -> Option<usize> {
        let (run_start, run_end) = (self.offsets[first], self.offsets[end]);
        let first_char = if (!rtl && rect.left <= run_left) || (rtl && rect.right >= run_right) {
            first
        } else {
            self.search(first, end, rtl, if rtl { rect.right } else { rect.left })
        };
        let mut segment_end = finder.next_end_boundary(self.offsets[first_char])?;
        let mut segment_start = match finder.previous_start_boundary(segment_end) {
            Some(start) if start >= run_end => return None,
            Some(start) => start.max(run_start),
            None => run_start,
        };
        segment_end = segment_end.min(run_end);
        loop {
            if strategy.is_included(self.piece(segment_start, segment_end, rtl), rect) {
                return Some(segment_start);
            }
            segment_start =
                finder.next_start_boundary(segment_start).filter(|&start| start < run_end)?;
            segment_end = finder.next_end_boundary(segment_start).unwrap_or(run_end).min(run_end);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn end_in_run(
        &self,
        first: usize,
        end: usize,
        rtl: bool,
        run_left: f32,
        run_right: f32,
        rect: Rect,
        finder: &SegmentFinder<'_>,
        strategy: &TextInclusionStrategy,
    ) -> Option<usize> {
        let (run_start, run_end) = (self.offsets[first], self.offsets[end]);
        let last_char = if (!rtl && rect.right >= run_right) || (rtl && rect.left <= run_left) {
            end - 1
        } else {
            self.search(first, end, rtl, if rtl { rect.left } else { rect.right })
        };
        let after_last = self.offsets[(last_char + 1).min(self.offsets.len() - 1)];
        let mut segment_start = finder.previous_start_boundary(after_last)?;
        let mut segment_end =
            finder.next_end_boundary(segment_start).filter(|&end| end > run_start)?;
        segment_start = segment_start.max(run_start);
        segment_end = segment_end.min(run_end);
        loop {
            if strategy.is_included(self.piece(segment_start, segment_end, rtl), rect) {
                return Some(segment_end);
            }
            segment_end =
                finder.previous_end_boundary(segment_end).filter(|&end| end > run_start)?;
            segment_start =
                finder.previous_start_boundary(segment_end).unwrap_or(run_start).max(run_start);
        }
    }
}

/// The range of `paragraph`'s text in `rect`, cut into pieces by `granularity`, a piece in it
/// as `strategy` says: from the first piece in it to the last, whole pieces (a piece across
/// lines or runs whole when part of it is in), or `TextRange::ZERO` when none is. Compose's
/// `getRangeForRect` below API 34 (`TextLayoutGetRangeForRectExtensions`).
pub(crate) fn range_for_rect<P: Paragraph + ?Sized>(
    paragraph: &P,
    rect: Rect,
    granularity: TextGranularity,
    strategy: &TextInclusionStrategy,
) -> TextRange {
    let line_count = paragraph.line_count();
    if line_count == 0 || paragraph.text().is_empty() {
        return TextRange::ZERO;
    }
    let finder = SegmentFinder::new(paragraph.text(), granularity);
    let mut start_line = paragraph.get_line_for_vertical_position(rect.top);
    // Below the last line, `getLineForVertical` answers it.
    if rect.top > paragraph.get_line_bottom(start_line) {
        start_line += 1;
        if start_line >= line_count {
            return TextRange::ZERO;
        }
    }
    let mut end_line = paragraph.get_line_for_vertical_position(rect.bottom);
    // Above the first line, it answers that.
    if end_line == 0 && rect.bottom < paragraph.get_line_top(0) {
        return TextRange::ZERO;
    }
    let offset_in = |line: usize, get_start: bool| {
        LineBounds::of(paragraph, line)
            .and_then(|bounds| bounds.offset_in(rect, &finder, strategy, get_start))
    };
    let mut start = offset_in(start_line, true);
    while start.is_none() && start_line < end_line {
        start_line += 1;
        start = offset_in(start_line, true);
    }
    let Some(start) = start else { return TextRange::ZERO };
    let mut end = offset_in(end_line, false);
    while end.is_none() && start_line < end_line {
        end_line -= 1;
        end = offset_in(end_line, false);
    }
    let Some(end) = end else { return TextRange::ZERO };
    // A piece across lines or runs is taken whole.
    let text = paragraph.text();
    let after_start = start + text[start..].chars().next().map_or(0, char::len_utf8);
    let before_end = end - text[..end].chars().next_back().map_or(0, char::len_utf8);
    let start = finder.previous_start_boundary(after_start).unwrap_or(start);
    let end = finder.next_end_boundary(before_end).unwrap_or(end);
    TextRange::new(start, end)
}

/// The boxes enclosing `start..end` of `paragraph`, as Compose's
/// `TextLayoutResult.getBoundingBoxesForRange` over its `TextLayout.fillRangeBoundingBoxes`: one
/// a line, or one a run of a direction where a line's are apart, left to right, a line's
/// trailing spaces and what an ellipsis or the maximum lines hide left out, boxes that touch
/// joined; each its line's height. Panics on a range outside the text (D2048).
pub(crate) fn range_bounding_boxes<P: Paragraph + ?Sized>(
    paragraph: &P,
    start: usize,
    end: usize,
) -> Vec<Rect> {
    let text = paragraph.text();
    let length = text.len();
    assert!(
        start <= end && end <= length,
        "Start({start}) or End({end}) is out of range [0..{length}], or start > end!"
    );
    let mut boxes: Vec<Rect> = Vec::new();
    let line_count = paragraph.line_count();
    if start == end || line_count == 0 {
        return boxes;
    }
    let shown_end = end.min(paragraph.get_line_end(line_count - 1, false));
    if start >= shown_end {
        return boxes;
    }
    let before_end = shown_end - text[..shown_end].chars().next_back().map_or(0, char::len_utf8);
    let (first, last) =
        (paragraph.get_line_for_offset(start), paragraph.get_line_for_offset(before_end));
    for line in first..=last {
        let line_start = paragraph.get_line_start(line);
        let visible_end = paragraph.get_line_end(line, true);
        let (from, to) = (start.max(line_start), shown_end.min(visible_end));
        if from >= to {
            continue;
        }
        let (top, bottom) = (paragraph.get_line_top(line), paragraph.get_line_bottom(line));
        // The whole line: its bounds.
        if from == line_start && to == visible_end && !paragraph.is_line_ellipsized(line) {
            let (left, right) = (paragraph.get_line_left(line), paragraph.get_line_right(line));
            if right - left > 0.01 {
                boxes.push(Rect::new(left, top, right, bottom));
            }
            continue;
        }
        // Else each run of a direction's extent.
        let first_box = boxes.len();
        let mut run: Option<(bool, f32, f32)> = None;
        let flush = |run: Option<(bool, f32, f32)>, boxes: &mut Vec<Rect>| {
            if let Some((_, left, right)) = run
                && right - left > 0.01
            {
                boxes.push(Rect::new(left, top, right, bottom));
            }
        };
        for (offset, _) in text[from..to].char_indices() {
            let offset = from + offset;
            let rect = paragraph.get_bounding_box(offset);
            let rtl = paragraph.get_bidi_run_direction(offset) == ResolvedTextDirection::Rtl;
            match &mut run {
                Some((direction, left, right)) if *direction == rtl => {
                    *left = left.min(rect.left);
                    *right = right.max(rect.right);
                }
                _ => {
                    flush(run.take(), &mut boxes);
                    run = Some((rtl, rect.left, rect.right));
                }
            }
        }
        flush(run, &mut boxes);
        coalesce(&mut boxes, first_box, top, bottom);
    }
    boxes
}

/// A line's boxes from `first` on, sorted left to right and those touching (within half a
/// pixel) joined, as Compose's `sortAndCoalesceLineBoxes`.
fn coalesce(boxes: &mut Vec<Rect>, first: usize, top: f32, bottom: f32) {
    if boxes.len() - first < 2 {
        return;
    }
    boxes[first..].sort_by(|a, b| a.left.total_cmp(&b.left));
    let mut write = first;
    for index in first + 1..boxes.len() {
        let next = boxes[index];
        if next.left <= boxes[write].right + 0.5 {
            let previous = boxes[write];
            boxes[write] = Rect::new(previous.left, top, previous.right.max(next.right), bottom);
        } else {
            write += 1;
            boxes[write] = next;
        }
    }
    boxes.truncate(write + 1);
}

/// Fills `array` from `array_start` with the box of each character in `range` (left, top,
/// right and bottom: four floats a character, in the text's order), as Compose's
/// `fillBoundingBoxes`: each [`Paragraph::get_bounding_box`], a character of a cluster after its
/// first zero wide; characters past the last line (cut by the maximum lines) leave theirs as
/// they were. Panics on an empty range, one past the text, or an array too small, as Compose's.
pub(crate) fn fill_bounding_boxes<P: Paragraph + ?Sized>(
    paragraph: &P,
    range: TextRange,
    array: &mut [f32],
    array_start: usize,
) {
    let text = paragraph.text();
    let (start, end) = (range.min(), range.max());
    let length = text.len();
    assert!(start < length, "startOffset must be less than text length");
    assert!(end > start, "endOffset must be greater than startOffset");
    assert!(end <= length, "endOffset must be smaller or equal to text length");
    let characters = text[start..end].chars().count();
    assert!(
        array.len().saturating_sub(array_start) >= characters * 4,
        "array.size - arrayStart must be greater or equal than (endOffset - startOffset) * 4"
    );
    let shown_end = match paragraph.line_count() {
        0 => length,
        lines => paragraph.get_line_end(lines - 1, false),
    };
    for (index, (offset, _)) in text[start..end].char_indices().enumerate() {
        let offset = start + offset;
        if offset >= shown_end {
            break;
        }
        let rect = paragraph.get_bounding_box(offset);
        let at = array_start + index * 4;
        array[at..at + 4].copy_from_slice(&[rect.left, rect.top, rect.right, rect.bottom]);
    }
}
