//! A text of many paragraphs, as Compose's `MultiParagraph` and `MultiParagraphIntrinsics`
//! (D2047): the runs of a text's paragraph styles each laid out on the platform in its own
//! style, one below the other, with Compose's queries across them (an offset, line or height
//! found in its paragraph, and the answer shifted back).
//!
//! A text without paragraph styles is one paragraph: [`MultiParagraph::of_paragraph`] wraps it,
//! asking nothing more of the platform than the paragraph itself.

use std::cell::{OnceCell, RefCell};
use std::cmp::Ordering;
use std::fmt;
use std::rc::Rc;

use crate::alignment::LayoutDirection;
use crate::annotated::{AnnotatedRange, AnnotatedString, intersect};
use crate::constraints::Constraints;
use crate::font::FontFamilyResolver;
use crate::intl::{Locale, LocaleList};
use crate::paragraph::Paragraph;
use crate::path::Path;
use crate::placeholder::{Placeholder, check_placeholders};
use crate::text::TextLayoutOptions;
use crate::text_attributes::{ResolvedTextDirection, TextDirection, TextOverflow};
use crate::text_measurer::{LaidOutParagraph, MeasuredParagraph, ParagraphSource};
use crate::text_range::TextRange;
use crate::text_style::TextStyle;
use crate::unit::{Density, Offset, Rect};

/// One paragraph of a [`MultiParagraph`] and where it is in the text, as Compose's
/// `ParagraphInfo`: its offsets (`start_index..end_index`), lines
/// (`start_line_index..end_line_index`) and extent (`top..bottom`) in the whole text's.
#[derive(Clone)]
pub struct ParagraphInfo {
    pub paragraph: Rc<dyn Paragraph>,
    pub start_index: usize,
    pub end_index: usize,
    pub start_line_index: usize,
    pub end_line_index: usize,
    pub top: f32,
    pub bottom: f32,
}

impl ParagraphInfo {
    /// `paragraph` over `start_index..end_index`, its lines and extent left at zero.
    pub fn new(paragraph: Rc<dyn Paragraph>, start_index: usize, end_index: usize) -> Self {
        ParagraphInfo {
            paragraph,
            start_index,
            end_index,
            start_line_index: 0,
            end_line_index: 0,
            top: 0.0,
            bottom: 0.0,
        }
    }

    /// How much of the text it covers.
    pub fn length(&self) -> usize {
        self.end_index - self.start_index
    }

    /// An offset of the whole text as the paragraph's, kept in it.
    pub fn to_local_index(&self, index: usize) -> usize {
        index.clamp(self.start_index, self.end_index) - self.start_index
    }

    /// An offset of the paragraph as the whole text's.
    pub fn to_global_index(&self, index: usize) -> usize {
        index + self.start_index
    }

    /// A line of the whole text as the paragraph's.
    pub fn to_local_line_index(&self, line: usize) -> usize {
        line - self.start_line_index
    }

    /// A line of the paragraph as the whole text's.
    pub fn to_global_line_index(&self, line: usize) -> usize {
        line + self.start_line_index
    }

    /// A height in the paragraph as the whole text's.
    pub fn to_global_y_position(&self, y: f32) -> f32 {
        y + self.top
    }

    /// A height in the whole text as the paragraph's.
    pub fn to_local_y_position(&self, y: f32) -> f32 {
        y - self.top
    }

    /// A position in the whole text as the paragraph's.
    pub fn to_local_offset(&self, position: Offset) -> Offset {
        Offset::new(position.x, position.y - self.top)
    }

    /// A rectangle in the paragraph as the whole text's.
    pub fn to_global_rect(&self, rect: Rect) -> Rect {
        rect.translate(Offset::new(0.0, self.top))
    }

    /// A rectangle in the whole text as the paragraph's.
    pub fn to_local_rect(&self, rect: Rect) -> Rect {
        rect.translate(Offset::new(0.0, -self.top))
    }

    /// A range of the paragraph as the whole text's; with `treat_zero_as_null`, an empty range
    /// at 0 is no range and stays as it is, as Compose's.
    pub fn to_global_range(&self, range: TextRange, treat_zero_as_null: bool) -> TextRange {
        if treat_zero_as_null && range == TextRange::ZERO {
            return TextRange::ZERO;
        }
        TextRange::new(self.to_global_index(range.start), self.to_global_index(range.end))
    }
}

impl fmt::Debug for ParagraphInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ParagraphInfo")
            .field("start_index", &self.start_index)
            .field("end_index", &self.end_index)
            .field("start_line_index", &self.start_line_index)
            .field("end_line_index", &self.end_line_index)
            .field("top", &self.top)
            .field("bottom", &self.bottom)
            .finish()
    }
}

/// The paragraph of `infos` covering offset `index` of the text (the last at the end of the
/// last one), as Compose's `findParagraphByIndex`. Panics past the last one's end.
pub fn find_paragraph_by_index(infos: &[ParagraphInfo], index: usize) -> usize {
    let last_end = infos.last().map_or(0, |last| last.end_index);
    assert!(
        index <= last_end,
        "Index {index} should be less or equal than last line's end {last_end}"
    );
    if index == last_end {
        return infos.len() - 1;
    }
    let found = infos.binary_search_by(|info| {
        if info.start_index > index {
            Ordering::Greater
        } else if info.end_index <= index {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    });
    found.unwrap_or_else(|_| panic!("No paragraph covers index {index}"))
}

/// The paragraph of `infos` at height `y` of the text (the first above them, the last below), as
/// Compose's `findParagraphByY`.
pub fn find_paragraph_by_y(infos: &[ParagraphInfo], y: f32) -> usize {
    if y <= 0.0 {
        return 0;
    }
    let last = infos.len() - 1;
    if y >= infos[last].bottom {
        return last;
    }
    let found = infos.binary_search_by(|info| {
        if info.top > y {
            Ordering::Greater
        } else if info.bottom <= y {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    });
    found.unwrap_or_else(|insertion| insertion.min(last))
}

/// The paragraph of `infos` holding line `line` of the text, as Compose's
/// `findParagraphByLineIndex`. Panics when none holds it.
pub fn find_paragraph_by_line_index(infos: &[ParagraphInfo], line: usize) -> usize {
    let found = infos.binary_search_by(|info| {
        if info.start_line_index > line {
            Ordering::Greater
        } else if info.end_line_index <= line {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    });
    found.unwrap_or_else(|_| panic!("No paragraph holds line {line}"))
}

/// The paragraphs of `infos` with text in `range`, from the one covering its start, as Compose's
/// `findParagraphsByRange`: empty paragraphs left out.
fn find_paragraphs_by_range(
    infos: &[ParagraphInfo],
    range: TextRange,
) -> impl Iterator<Item = &ParagraphInfo> {
    let first = find_paragraph_by_index(infos, range.min());
    infos[first..]
        .iter()
        .take_while(move |info| info.start_index < range.max())
        .filter(|info| info.start_index != info.end_index)
}

/// Whether a locale's text runs right to left, as Android's `TextUtils.getLayoutDirectionFromLocale`
/// tells by its script (written, or its language's usual one).
fn is_right_to_left(locale: &Locale) -> bool {
    match locale.script().as_str() {
        "Arab" | "Hebr" | "Syrc" | "Thaa" | "Nkoo" | "Adlm" | "Rohg" | "Mand" | "Samr" => true,
        "" => matches!(
            locale.language().as_str(),
            "ar" | "fa"
                | "he"
                | "iw"
                | "ur"
                | "yi"
                | "ji"
                | "ps"
                | "sd"
                | "ug"
                | "dv"
                | "ckb"
                | "ks"
                | "ku"
                | "syr"
                | "nqo"
                | "arc"
        ),
        _ => false,
    }
}

/// One paragraph of a [`MultiParagraphIntrinsics`]: its text, in its style.
pub(crate) struct ParagraphIntrinsicInfo {
    /// Its part of the text, without the paragraph styles.
    pub(crate) text: AnnotatedString,
    /// The text's style with the paragraph's over it, its direction resolved.
    pub(crate) style: TextStyle,
    /// The placeholders in it, at its own offsets.
    pub(crate) placeholders: Vec<AnnotatedRange<Placeholder>>,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl ParagraphIntrinsicInfo {
    /// The direction its text goes when it has no strong character.
    fn layout_direction(&self) -> LayoutDirection {
        match self.style.text_direction {
            Some(TextDirection::Rtl | TextDirection::ContentOrRtl) => LayoutDirection::Rtl,
            _ => LayoutDirection::Ltr,
        }
    }
}

/// A text split into its paragraphs, and their intrinsic widths, as Compose's
/// `MultiParagraphIntrinsics`: a paragraph for each run of the text's paragraph styles (and
/// for each stretch between them), each in the text's style with its own over it.
pub struct MultiParagraphIntrinsics {
    annotated_string: AnnotatedString,
    placeholders: Vec<AnnotatedRange<Placeholder>>,
    pub(crate) infos: Vec<ParagraphIntrinsicInfo>,
    density: Density,
    font_family_resolver: FontFamilyResolver,
    soft_wrap: bool,
    /// The narrowest and widest, measured when first asked.
    widths: OnceCell<(f32, f32)>,
}

impl MultiParagraphIntrinsics {
    /// `annotated_string`'s paragraphs in `style`, as Compose's constructor. Panics when a
    /// placeholder is outside the text or crosses a paragraph's boundary. A paragraph whose
    /// direction is unset or `Content` goes by its first strong character, else by its locale's
    /// direction (the style's first locale, or `default_locale_list`'s), as Android's.
    pub fn new(
        annotated_string: AnnotatedString,
        style: &TextStyle,
        placeholders: &[AnnotatedRange<Placeholder>],
        density: Density,
        font_family_resolver: FontFamilyResolver,
        soft_wrap: bool,
        default_locale_list: LocaleList,
    ) -> MultiParagraphIntrinsics {
        check_placeholders(annotated_string.as_str(), placeholders);
        let paragraph_style = style.to_paragraph_style();
        let infos = annotated_string
            .normalized_paragraph_styles(&paragraph_style)
            .into_iter()
            .map(|range| {
                let (start, end) = (range.start, range.end);
                let mut style = style.merge_paragraph_style(&range.item);
                if matches!(style.text_direction, None | Some(TextDirection::Content)) {
                    let locale = style
                        .locale_list
                        .as_ref()
                        .and_then(|list| list.get(0))
                        .or_else(|| default_locale_list.get(0));
                    style.text_direction = Some(if locale.is_some_and(is_right_to_left) {
                        TextDirection::ContentOrRtl
                    } else {
                        TextDirection::ContentOrLtr
                    });
                }
                let placeholders = placeholders
                    .iter()
                    .filter(|placeholder| intersect(start, end, placeholder.start, placeholder.end))
                    .map(|placeholder| {
                        assert!(
                            start <= placeholder.start && placeholder.end <= end,
                            "placeholder can not overlap with paragraph."
                        );
                        AnnotatedRange::new(
                            placeholder.item,
                            placeholder.start - start,
                            placeholder.end - start,
                        )
                    })
                    .collect();
                ParagraphIntrinsicInfo {
                    text: annotated_string.substring_without_paragraph_styles(start, end),
                    style,
                    placeholders,
                    start,
                    end,
                }
            })
            .collect();
        MultiParagraphIntrinsics {
            annotated_string,
            placeholders: placeholders.to_vec(),
            infos,
            density,
            font_family_resolver,
            soft_wrap,
            widths: OnceCell::new(),
        }
    }

    /// The text.
    pub fn annotated_string(&self) -> &AnnotatedString {
        &self.annotated_string
    }

    /// The placeholders, at the text's offsets.
    pub fn placeholders(&self) -> &[AnnotatedRange<Placeholder>] {
        &self.placeholders
    }

    /// The widest paragraph's narrowest width, with every soft line break taken.
    pub fn min_intrinsic_width(&self) -> f32 {
        self.widths().0
    }

    /// The widest paragraph's width without soft line breaks.
    pub fn max_intrinsic_width(&self) -> f32 {
        self.widths().1
    }

    /// Each paragraph measured once, unconstrained, as Compose's `ParagraphIntrinsics` measure
    /// the text: only when asked (a text laid out at a width it's given never is).
    fn widths(&self) -> (f32, f32) {
        *self.widths.get_or_init(|| {
            let platform = self.font_family_resolver.platform();
            platform.release_dropped();
            let mut measurer = platform.borrow_mut();
            let (mut min, mut max) = (0.0f32, 0.0f32);
            for info in &self.infos {
                let direction = info.layout_direction();
                let options = TextLayoutOptions {
                    soft_wrap: self.soft_wrap,
                    max_lines: u32::MAX,
                    direction,
                    overflow: TextOverflow::Clip,
                };
                let style = TextStyle::resolve_defaults(&info.style, direction);
                let constraints = Constraints::unbounded();
                let handle = if info.placeholders.is_empty() {
                    measurer.layout_probe(
                        None,
                        &info.text,
                        &style,
                        options,
                        constraints,
                        &self.density,
                    )
                } else {
                    measurer.layout_with_placeholders(
                        &info.text,
                        &style,
                        &info.placeholders,
                        options,
                        constraints,
                        &self.density,
                    )
                };
                let widest = measurer
                    .max_intrinsic_width(handle)
                    .unwrap_or_else(|| measurer.metrics(handle).size.width as f32);
                let narrowest = measurer
                    .min_intrinsic_width(handle)
                    .unwrap_or_else(|| measurer.paragraph(handle).min_intrinsic_width());
                measurer.release(handle);
                min = min.max(narrowest);
                max = max.max(widest);
            }
            (min, max)
        })
    }
}

impl fmt::Debug for MultiParagraphIntrinsics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MultiParagraphIntrinsics")
            .field("text", &self.annotated_string.as_str())
            .field("paragraphs", &self.infos.len())
            .finish()
    }
}

/// A paragraph of a [`MultiParagraph`] as the platform laid it out, and what it was laid out
/// from: what a draw scope draws (D2047).
pub(crate) struct StackedParagraph {
    /// Where it is in the text.
    pub(crate) top: f32,
    pub(crate) measured: LaidOutParagraph,
    /// Its style, which a paint goes over.
    pub(crate) style: TextStyle,
    text: AnnotatedString,
    placeholders: Vec<AnnotatedRange<Placeholder>>,
    options: TextLayoutOptions,
    constraints: Constraints,
    density: Density,
    /// It laid out on another host's platform, when drawn there.
    pub(crate) elsewhere: RefCell<Option<LaidOutParagraph>>,
}

impl StackedParagraph {
    /// What it was laid out from, to lay it out again.
    pub(crate) fn source(&self) -> ParagraphSource<'_> {
        ParagraphSource {
            text: &self.text,
            placeholders: &self.placeholders,
            options: self.options,
            constraints: self.constraints,
            density: &self.density,
        }
    }
}

/// A laid-out text of many paragraphs, as Compose's `MultiParagraph`: each paragraph of its
/// [`MultiParagraphIntrinsics`] laid out at the width given, below the one before, until the
/// maximum lines are reached. Its queries are [`Paragraph`]'s, over the whole text.
pub struct MultiParagraph {
    intrinsics: Option<Rc<MultiParagraphIntrinsics>>,
    text: AnnotatedString,
    max_lines: u32,
    infos: Vec<ParagraphInfo>,
    parts: Vec<StackedParagraph>,
    width: f32,
    height: f32,
    line_count: usize,
    did_exceed_max_lines: bool,
    /// One paragraph, the whole text's: its own numbers are asked of it when wanted.
    single: bool,
}

impl MultiParagraph {
    /// `intrinsics`' paragraphs laid out within `constraints`, at most `max_lines` lines in all,
    /// as Compose's constructor: each paragraph is as wide as the maximum width (as wide as its
    /// own text when it's unbounded), and gets the lines and height the paragraphs above it
    /// left. A paragraph that reaches the maximum lines is the last. Panics on a minimum width
    /// or height, as Compose's, or without a line.
    pub fn new(
        intrinsics: Rc<MultiParagraphIntrinsics>,
        constraints: Constraints,
        max_lines: u32,
        overflow: TextOverflow,
    ) -> MultiParagraph {
        assert!(
            constraints.min_width == 0 && constraints.min_height == 0,
            "Setting Constraints.minWidth and Constraints.minHeight is not supported, these \
             should be the default zero values instead."
        );
        assert!(max_lines >= 1, "maxLines should be greater than 0");
        let platform = intrinsics.font_family_resolver.platform().clone();
        let (min_width, max_width) = if constraints.has_bounded_width() {
            (constraints.max_width, constraints.max_width)
        } else {
            (0, Constraints::INFINITY)
        };
        let mut current_height = 0.0f32;
        let mut current_lines = 0usize;
        let mut did_exceed_max_lines = false;
        let mut infos = Vec::with_capacity(intrinsics.infos.len());
        let mut parts = Vec::with_capacity(intrinsics.infos.len());
        let last = intrinsics.infos.len() - 1;
        for (index, info) in intrinsics.infos.iter().enumerate() {
            let max_height = if constraints.has_bounded_height() {
                (constraints.max_height - current_height.ceil() as i32).max(0)
            } else {
                constraints.max_height
            };
            let options = TextLayoutOptions {
                soft_wrap: intrinsics.soft_wrap,
                max_lines: max_lines - current_lines as u32,
                direction: info.layout_direction(),
                overflow,
            };
            let item_constraints = Constraints::new(min_width, max_width, 0, max_height);
            let source = ParagraphSource {
                text: &info.text,
                placeholders: &info.placeholders,
                options,
                constraints: item_constraints,
                density: &intrinsics.density,
            };
            let laid_out = LaidOutParagraph::lay_out(&platform, source, &info.style);
            let height = platform.borrow().height(laid_out.handle());
            let exceeded = laid_out.did_exceed_max_lines();
            let top = current_height;
            current_height += height;
            let start_line_index = current_lines;
            current_lines += laid_out.metrics().line_count as usize;
            infos.push(ParagraphInfo {
                paragraph: Rc::new(MeasuredParagraph::new(laid_out.clone())),
                start_index: info.start,
                end_index: info.end,
                start_line_index,
                end_line_index: current_lines,
                top,
                bottom: current_height,
            });
            parts.push(StackedParagraph {
                top,
                measured: laid_out,
                style: info.style.clone(),
                text: info.text.clone(),
                placeholders: info.placeholders.clone(),
                options,
                constraints: item_constraints,
                density: intrinsics.density,
                elsewhere: RefCell::new(None),
            });
            if exceeded || (current_lines == max_lines as usize && index != last) {
                did_exceed_max_lines = true;
                break;
            }
        }
        let width = if constraints.has_bounded_width() {
            constraints.max_width as f32
        } else {
            parts.iter().map(|part| part.measured.metrics().size.width as f32).fold(0.0, f32::max)
        };
        MultiParagraph {
            text: intrinsics.annotated_string.clone(),
            intrinsics: Some(intrinsics),
            max_lines,
            infos,
            parts,
            width,
            height: current_height,
            line_count: current_lines,
            did_exceed_max_lines,
            single: false,
        }
    }

    /// `text` laid out as one `paragraph`, the whole text's (a text without paragraph styles):
    /// its numbers are the paragraph's, and nothing is asked of it until a query is.
    pub fn of_paragraph(text: AnnotatedString, paragraph: Rc<dyn Paragraph>) -> MultiParagraph {
        let length = text.len();
        MultiParagraph {
            intrinsics: None,
            text,
            max_lines: u32::MAX,
            infos: vec![ParagraphInfo {
                paragraph,
                start_index: 0,
                end_index: length,
                start_line_index: 0,
                end_line_index: usize::MAX,
                top: 0.0,
                bottom: f32::INFINITY,
            }],
            parts: Vec::new(),
            width: 0.0,
            height: 0.0,
            line_count: 0,
            did_exceed_max_lines: false,
            single: true,
        }
    }

    /// The paragraphs and texts it was laid out from (none for a single paragraph wrapped).
    pub fn intrinsics(&self) -> Option<&Rc<MultiParagraphIntrinsics>> {
        self.intrinsics.as_ref()
    }

    /// The text.
    pub fn annotated_string(&self) -> &AnnotatedString {
        &self.text
    }

    /// The most lines it may have.
    pub fn max_lines(&self) -> u32 {
        self.max_lines
    }

    /// The paragraphs laid out, in order: those the maximum lines left out aren't.
    pub fn paragraph_info_list(&self) -> &[ParagraphInfo] {
        &self.infos
    }

    /// The platform's paragraphs, for drawing (none for a single paragraph wrapped).
    pub(crate) fn parts(&self) -> &[StackedParagraph] {
        &self.parts
    }

    /// A font one of its paragraphs asked for arrived since it was laid out.
    pub(crate) fn has_stale_resolved_fonts(&self) -> bool {
        self.parts.iter().any(|part| part.measured.has_stale_resolved_fonts())
    }

    fn check_index(&self, offset: usize) {
        let length = self.text.len();
        assert!(offset < length, "offset({offset}) is out of bounds [0, {length})");
    }

    fn check_index_inclusive(&self, offset: usize) {
        let length = self.text.len();
        assert!(offset <= length, "offset({offset}) is out of bounds [0, {length}]");
    }

    fn check_line(&self, line: usize) {
        let count = self.line_count();
        assert!(line < count, "lineIndex({line}) is out of bounds [0, {count})");
    }

    /// The paragraph with offset `offset`, the last one at the text's end.
    fn info_at(&self, offset: usize) -> &ParagraphInfo {
        let index = if offset == self.text.len() {
            self.infos.len() - 1
        } else {
            find_paragraph_by_index(&self.infos, offset)
        };
        &self.infos[index]
    }

    /// The paragraph holding line `line`, checked.
    fn info_of_line(&self, line: usize) -> &ParagraphInfo {
        self.check_line(line);
        &self.infos[find_paragraph_by_line_index(&self.infos, line)]
    }

    fn first(&self) -> &ParagraphInfo {
        &self.infos[0]
    }
}

impl fmt::Debug for MultiParagraph {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MultiParagraph")
            .field("text", &self.text.as_str())
            .field("paragraphs", &self.infos)
            .finish()
    }
}

impl MultiParagraph {
    pub fn text(&self) -> &str {
        self.text.as_str()
    }

    pub fn width(&self) -> f32 {
        if self.single { self.first().paragraph.width() } else { self.width }
    }

    pub fn height(&self) -> f32 {
        if self.single { self.first().paragraph.height() } else { self.height }
    }

    pub fn min_intrinsic_width(&self) -> f32 {
        match &self.intrinsics {
            Some(intrinsics) => intrinsics.min_intrinsic_width(),
            None => self.first().paragraph.min_intrinsic_width(),
        }
    }

    pub fn max_intrinsic_width(&self) -> f32 {
        match &self.intrinsics {
            Some(intrinsics) => intrinsics.max_intrinsic_width(),
            None => self.first().paragraph.max_intrinsic_width(),
        }
    }

    pub fn first_baseline(&self) -> f32 {
        self.first().paragraph.first_baseline()
    }

    pub fn last_baseline(&self) -> f32 {
        let last = &self.infos[self.infos.len() - 1];
        last.to_global_y_position(last.paragraph.last_baseline())
    }

    pub fn did_exceed_max_lines(&self) -> bool {
        if self.single {
            self.first().paragraph.did_exceed_max_lines()
        } else {
            self.did_exceed_max_lines
        }
    }

    pub fn line_count(&self) -> usize {
        if self.single { self.first().paragraph.line_count() } else { self.line_count }
    }

    pub fn get_line_left(&self, line: usize) -> f32 {
        let info = self.info_of_line(line);
        info.paragraph.get_line_left(info.to_local_line_index(line))
    }

    pub fn get_line_right(&self, line: usize) -> f32 {
        let info = self.info_of_line(line);
        info.paragraph.get_line_right(info.to_local_line_index(line))
    }

    pub fn get_line_top(&self, line: usize) -> f32 {
        let info = self.info_of_line(line);
        info.to_global_y_position(info.paragraph.get_line_top(info.to_local_line_index(line)))
    }

    pub fn get_line_bottom(&self, line: usize) -> f32 {
        let info = self.info_of_line(line);
        info.to_global_y_position(info.paragraph.get_line_bottom(info.to_local_line_index(line)))
    }

    pub fn get_line_baseline(&self, line: usize) -> f32 {
        let info = self.info_of_line(line);
        info.to_global_y_position(info.paragraph.get_line_baseline(info.to_local_line_index(line)))
    }

    pub fn get_line_height(&self, line: usize) -> f32 {
        let info = self.info_of_line(line);
        info.paragraph.get_line_height(info.to_local_line_index(line))
    }

    pub fn get_line_width(&self, line: usize) -> f32 {
        let info = self.info_of_line(line);
        info.paragraph.get_line_width(info.to_local_line_index(line))
    }

    pub fn get_line_start(&self, line: usize) -> usize {
        let info = self.info_of_line(line);
        info.to_global_index(info.paragraph.get_line_start(info.to_local_line_index(line)))
    }

    pub fn get_line_end(&self, line: usize, visible_end: bool) -> usize {
        let info = self.info_of_line(line);
        let local = info.paragraph.get_line_end(info.to_local_line_index(line), visible_end);
        info.to_global_index(local)
    }

    pub fn is_line_ellipsized(&self, line: usize) -> bool {
        let info = self.info_of_line(line);
        info.paragraph.is_line_ellipsized(info.to_local_line_index(line))
    }

    pub fn get_line_for_offset(&self, offset: usize) -> usize {
        let index = if offset >= self.text.len() {
            self.infos.len() - 1
        } else {
            find_paragraph_by_index(&self.infos, offset)
        };
        let info = &self.infos[index];
        info.to_global_line_index(info.paragraph.get_line_for_offset(info.to_local_index(offset)))
    }

    pub fn get_line_for_vertical_position(&self, vertical: f32) -> usize {
        let info = &self.infos[find_paragraph_by_y(&self.infos, vertical)];
        if info.length() == 0 {
            return info.start_line_index;
        }
        let local =
            info.paragraph.get_line_for_vertical_position(info.to_local_y_position(vertical));
        info.to_global_line_index(local)
    }

    pub fn get_horizontal_position(&self, offset: usize, use_primary_direction: bool) -> f32 {
        self.check_index_inclusive(offset);
        let info = self.info_at(offset);
        info.paragraph.get_horizontal_position(info.to_local_index(offset), use_primary_direction)
    }

    pub fn get_paragraph_direction(&self, offset: usize) -> ResolvedTextDirection {
        self.check_index_inclusive(offset);
        let info = self.info_at(offset);
        info.paragraph.get_paragraph_direction(info.to_local_index(offset))
    }

    pub fn get_bidi_run_direction(&self, offset: usize) -> ResolvedTextDirection {
        self.check_index_inclusive(offset);
        let info = self.info_at(offset);
        info.paragraph.get_bidi_run_direction(info.to_local_index(offset))
    }

    pub fn get_offset_for_position(&self, position: Offset) -> usize {
        let info = &self.infos[find_paragraph_by_y(&self.infos, position.y)];
        if info.length() == 0 {
            return info.start_index;
        }
        info.to_global_index(info.paragraph.get_offset_for_position(info.to_local_offset(position)))
    }

    pub fn get_bounding_box(&self, offset: usize) -> Rect {
        self.check_index(offset);
        let last = &self.infos[self.infos.len() - 1];
        // Cut by the maximum lines: where the text stops.
        if offset >= last.end_index {
            return last.to_global_rect(last.paragraph.get_cursor_rect(last.length()));
        }
        let info = &self.infos[find_paragraph_by_index(&self.infos, offset)];
        info.to_global_rect(info.paragraph.get_bounding_box(info.to_local_index(offset)))
    }

    /// The boxes of the characters in `start..end`, each as [`get_bounding_box`]'s.
    ///
    /// [`get_bounding_box`]: Self::get_bounding_box
    pub fn get_bounding_boxes_for_range(&self, start: usize, end: usize) -> Vec<Rect> {
        if self.single {
            return self.first().paragraph.get_bounding_boxes_for_range(start, end);
        }
        self.text
            .as_str()
            .char_indices()
            .map(|(offset, _)| offset)
            .filter(|&offset| offset >= start && offset < end)
            .map(|offset| self.get_bounding_box(offset))
            .collect()
    }

    pub fn get_cursor_rect(&self, offset: usize) -> Rect {
        self.check_index_inclusive(offset);
        let info = self.info_at(offset);
        info.to_global_rect(info.paragraph.get_cursor_rect(info.to_local_index(offset)))
    }

    pub fn get_word_boundary(&self, offset: usize) -> TextRange {
        self.check_index_inclusive(offset);
        let info = self.info_at(offset);
        info.to_global_range(info.paragraph.get_word_boundary(info.to_local_index(offset)), false)
    }

    pub fn get_path_for_range(&self, start: usize, end: usize) -> Path {
        let length = self.text.len();
        assert!(
            start <= end && end <= length,
            "Start({start}) or End({end}) is out of range [0..{length}), or start > end!"
        );
        let path = Path::new();
        if start == end {
            return path;
        }
        for info in find_paragraphs_by_range(&self.infos, TextRange::new(start, end)) {
            let local = info
                .paragraph
                .get_path_for_range(info.to_local_index(start), info.to_local_index(end));
            path.add_path(&local, Offset::new(0.0, info.top));
        }
        path
    }

    pub fn placeholder_rects(&self) -> Vec<Option<Rect>> {
        if self.single {
            return self.first().paragraph.placeholder_rects();
        }
        let mut rects: Vec<Option<Rect>> = self
            .infos
            .iter()
            .flat_map(|info| {
                info.paragraph
                    .placeholder_rects()
                    .into_iter()
                    .map(|rect| rect.map(|rect| info.to_global_rect(rect)))
                    .collect::<Vec<_>>()
            })
            .collect();
        // The paragraphs the maximum lines left out: their placeholders are cut.
        let count = self.intrinsics.as_ref().map_or(0, |intrinsics| intrinsics.placeholders.len());
        if rects.len() < count {
            rects.resize(count, None);
        }
        rects
    }
}

/// Its queries as a [`Paragraph`]'s: the inherent ones, Compose's `MultiParagraph`'s.
impl Paragraph for MultiParagraph {
    fn text(&self) -> &str {
        MultiParagraph::text(self)
    }
    fn width(&self) -> f32 {
        MultiParagraph::width(self)
    }
    fn height(&self) -> f32 {
        MultiParagraph::height(self)
    }
    fn min_intrinsic_width(&self) -> f32 {
        MultiParagraph::min_intrinsic_width(self)
    }
    fn max_intrinsic_width(&self) -> f32 {
        MultiParagraph::max_intrinsic_width(self)
    }
    fn first_baseline(&self) -> f32 {
        MultiParagraph::first_baseline(self)
    }
    fn last_baseline(&self) -> f32 {
        MultiParagraph::last_baseline(self)
    }
    fn did_exceed_max_lines(&self) -> bool {
        MultiParagraph::did_exceed_max_lines(self)
    }
    fn line_count(&self) -> usize {
        MultiParagraph::line_count(self)
    }
    fn get_line_left(&self, line: usize) -> f32 {
        MultiParagraph::get_line_left(self, line)
    }
    fn get_line_right(&self, line: usize) -> f32 {
        MultiParagraph::get_line_right(self, line)
    }
    fn get_line_top(&self, line: usize) -> f32 {
        MultiParagraph::get_line_top(self, line)
    }
    fn get_line_bottom(&self, line: usize) -> f32 {
        MultiParagraph::get_line_bottom(self, line)
    }
    fn get_line_baseline(&self, line: usize) -> f32 {
        MultiParagraph::get_line_baseline(self, line)
    }
    fn get_line_height(&self, line: usize) -> f32 {
        MultiParagraph::get_line_height(self, line)
    }
    fn get_line_width(&self, line: usize) -> f32 {
        MultiParagraph::get_line_width(self, line)
    }
    fn get_line_start(&self, line: usize) -> usize {
        MultiParagraph::get_line_start(self, line)
    }
    fn get_line_end(&self, line: usize, visible_end: bool) -> usize {
        MultiParagraph::get_line_end(self, line, visible_end)
    }
    fn is_line_ellipsized(&self, line: usize) -> bool {
        MultiParagraph::is_line_ellipsized(self, line)
    }
    fn get_line_for_offset(&self, offset: usize) -> usize {
        MultiParagraph::get_line_for_offset(self, offset)
    }
    fn get_line_for_vertical_position(&self, vertical: f32) -> usize {
        MultiParagraph::get_line_for_vertical_position(self, vertical)
    }
    fn get_horizontal_position(&self, offset: usize, use_primary_direction: bool) -> f32 {
        MultiParagraph::get_horizontal_position(self, offset, use_primary_direction)
    }
    fn get_paragraph_direction(&self, offset: usize) -> ResolvedTextDirection {
        MultiParagraph::get_paragraph_direction(self, offset)
    }
    fn get_bidi_run_direction(&self, offset: usize) -> ResolvedTextDirection {
        MultiParagraph::get_bidi_run_direction(self, offset)
    }
    fn get_offset_for_position(&self, position: Offset) -> usize {
        MultiParagraph::get_offset_for_position(self, position)
    }
    fn get_bounding_box(&self, offset: usize) -> Rect {
        MultiParagraph::get_bounding_box(self, offset)
    }
    fn get_bounding_boxes_for_range(&self, start: usize, end: usize) -> Vec<Rect> {
        MultiParagraph::get_bounding_boxes_for_range(self, start, end)
    }
    fn get_cursor_rect(&self, offset: usize) -> Rect {
        MultiParagraph::get_cursor_rect(self, offset)
    }
    fn get_word_boundary(&self, offset: usize) -> TextRange {
        MultiParagraph::get_word_boundary(self, offset)
    }
    fn get_path_for_range(&self, start: usize, end: usize) -> Path {
        MultiParagraph::get_path_for_range(self, start, end)
    }
    fn placeholder_rects(&self) -> Vec<Option<Rect>> {
        MultiParagraph::placeholder_rects(self)
    }
}
