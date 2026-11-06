//! What a state-based text field shows of its state, as Compose's `OutputTransformation`, with
//! the offset mapping its edits make (`OffsetMappingCalculator`) and `TransformedTextFieldState`'s
//! rules for a cursor and a selection across them (D1134).

use std::fmt;
use std::rc::Rc;

use compose_runtime::ComposeArg;
use compose_ui::TextRange;

use crate::text_field_buffer::{TextFieldBuffer, TextFieldCharSequence};

/// An application's output transformation, as Compose's `OutputTransformation` interface: given a
/// buffer of the state's value, it changes what's shown (a phone number's spaces, say); the state
/// stays as it was, and the cursor never goes inside what it replaced or inserted. Make it an
/// [`OutputTransformation`] with [`OutputTransformation::new`] or
/// [`OutputTransformation::from_fn`].
pub trait OutputTransform {
    fn transform_output(&self, buffer: &mut TextFieldBuffer);
}

#[derive(Clone)]
enum Kind {
    Custom(Rc<dyn OutputTransform>),
    /// A single line's: a line feed shows as a space, a carriage return as nothing (U+FEFF), as
    /// Compose's `SingleLineOutputTransformation`; after the field's own, if any.
    SingleLine(Option<Rc<dyn OutputTransform>>),
}

/// A field's output transformation, as Compose's `OutputTransformation`: an application's, compared
/// by identity.
#[derive(Clone)]
pub struct OutputTransformation(Kind);

impl OutputTransformation {
    pub fn new(transform: impl OutputTransform + 'static) -> OutputTransformation {
        OutputTransformation(Kind::Custom(Rc::new(transform)))
    }

    /// A transformation of a function, as Compose's lambda form.
    pub fn from_fn(transform: impl Fn(&mut TextFieldBuffer) + 'static) -> OutputTransformation {
        struct Function<F>(F);
        impl<F: Fn(&mut TextFieldBuffer)> OutputTransform for Function<F> {
            fn transform_output(&self, buffer: &mut TextFieldBuffer) {
                (self.0)(buffer)
            }
        }
        OutputTransformation::new(Function(transform))
    }

    /// A single line's, after `inner`, as Compose's `SingleLineOutputTransformation.wrap`.
    pub(crate) fn single_line(inner: Option<&OutputTransformation>) -> OutputTransformation {
        let inner = inner.map(|inner| match &inner.0 {
            Kind::Custom(transform) => transform.clone(),
            Kind::SingleLine(_) => unreachable!("a single line's isn't the field's own"),
        });
        OutputTransformation(Kind::SingleLine(inner))
    }

    /// Changes what `buffer` shows, as `transformOutput`.
    pub fn transform_output(&self, buffer: &mut TextFieldBuffer) {
        match &self.0 {
            Kind::Custom(transform) => transform.transform_output(buffer),
            Kind::SingleLine(inner) => {
                if let Some(inner) = inner {
                    inner.transform_output(buffer);
                }
                single_line(buffer);
            }
        }
    }
}

/// A line feed as a space, a carriage return as a zero-width no-break space (whose bytes are
/// neither).
fn single_line(buffer: &mut TextFieldBuffer) {
    let mut index = 0;
    while index < buffer.length() {
        match buffer.as_str().as_bytes()[index] {
            b'\n' => buffer.replace(index, index + 1, " "),
            b'\r' => buffer.replace(index, index + 1, "\u{FEFF}"),
            _ => {}
        }
        index += 1;
    }
}

impl PartialEq for OutputTransformation {
    fn eq(&self, other: &OutputTransformation) -> bool {
        match (&self.0, &other.0) {
            (Kind::Custom(a), Kind::Custom(b)) => Rc::ptr_eq(a, b),
            (Kind::SingleLine(a), Kind::SingleLine(b)) => match (a, b) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            },
            _ => false,
        }
    }
}

impl ComposeArg for OutputTransformation {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl fmt::Debug for OutputTransformation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Kind::Custom(_) => formatter.write_str("OutputTransformation"),
            Kind::SingleLine(_) => formatter.write_str("SingleLineOutputTransformation"),
        }
    }
}

/// One edit an output transformation made: `source_length` bytes at `offset` became
/// `destination_length`.
#[derive(Clone, Copy, Debug)]
struct Operation {
    offset: usize,
    source_length: usize,
    destination_length: usize,
}

/// Maps offsets both ways across the edits made to a text, as Compose's `OffsetMappingCalculator`:
/// an offset maps to one offset where it can, and to the range it could be in where it can't (an
/// insertion point to the inserted text, an offset inside a replacement to the replacement).
#[derive(Clone, Default, Debug)]
pub(crate) struct OffsetMappingCalculator {
    operations: Vec<Operation>,
}

impl OffsetMappingCalculator {
    /// Records that `source_start..source_end` of the text as it was then became `new_length`
    /// bytes, as `recordEditOperation`.
    pub(crate) fn record_edit_operation(
        &mut self,
        source_start: usize,
        source_end: usize,
        new_length: usize,
    ) {
        let (min, max) = (source_start.min(source_end), source_start.max(source_end));
        let source_length = max - min;
        // Nothing, or one byte for one: the identity.
        if source_length < 2 && source_length == new_length {
            return;
        }
        self.operations.push(Operation {
            offset: min,
            source_length,
            destination_length: new_length,
        });
    }

    /// Where `offset` of the original text is in the edited one, as `mapFromSource`.
    pub(crate) fn map_from_source(&self, offset: usize) -> TextRange {
        self.map(offset, true)
    }

    /// Where `offset` of the edited text is in the original one, as `mapFromDest`.
    pub(crate) fn map_from_destination(&self, offset: usize) -> TextRange {
        self.map(offset, false)
    }

    fn map(&self, offset: usize, from_source: bool) -> TextRange {
        let (mut start, mut end) = (offset, offset);
        let mut step = |operation: &Operation| {
            let new_start = map_step(start, operation, from_source);
            let new_end = map_step(end, operation, from_source);
            start = new_start.start.min(new_end.start);
            end = new_start.end.max(new_end.end);
        };
        if from_source {
            self.operations.iter().for_each(&mut step);
        } else {
            self.operations.iter().rev().for_each(&mut step);
        }
        TextRange::new(start, end)
    }
}

fn map_step(offset: usize, operation: &Operation, from_source: bool) -> TextRange {
    let (source, destination) = if from_source {
        (operation.source_length, operation.destination_length)
    } else {
        (operation.destination_length, operation.source_length)
    };
    let at = operation.offset;
    if offset < at {
        TextRange::at(offset)
    } else if offset == at {
        // An insertion point maps to what's inserted; a replacement's start to its start.
        if source == 0 { TextRange::new(at, at + destination) } else { TextRange::at(at) }
    } else if offset < at + source {
        // Inside what was deleted, its start; inside what was replaced, the replacement.
        if destination == 0 { TextRange::at(at) } else { TextRange::new(at, at + destination) }
    } else {
        TextRange::at(offset - source + destination)
    }
}

/// Which side of an insertion (or a replacement) a collapsed end of the selection sits on, as
/// Compose's `WedgeAffinity`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum WedgeAffinity {
    #[default]
    Start,
    End,
}

/// The wedge affinities of a selection's ends, as Compose's `SelectionWedgeAffinity`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) struct SelectionWedgeAffinity {
    pub(crate) start: WedgeAffinity,
    pub(crate) end: WedgeAffinity,
}

impl SelectionWedgeAffinity {
    pub(crate) const fn both(affinity: WedgeAffinity) -> SelectionWedgeAffinity {
        SelectionWedgeAffinity { start: affinity, end: affinity }
    }
}

/// `range` of the original text in the shown one, as `mapToTransformed`: an end that maps to a range
/// goes to the side its wedge affinity says, if one is given.
pub(crate) fn map_to_transformed(
    range: TextRange,
    mapping: &OffsetMappingCalculator,
    affinity: Option<SelectionWedgeAffinity>,
) -> TextRange {
    let mut start = mapping.map_from_source(range.start);
    let mut end = if range.collapsed() { start } else { mapping.map_from_source(range.end) };
    let start_affinity = affinity.map(|it| it.start);
    let end_affinity = if range.collapsed() { start_affinity } else { affinity.map(|it| it.end) };
    let collapse = |mapped: TextRange, affinity: Option<WedgeAffinity>| match affinity {
        Some(_) if mapped.collapsed() => mapped,
        Some(WedgeAffinity::Start) => TextRange::at(mapped.start),
        Some(WedgeAffinity::End) => TextRange::at(mapped.end),
        None => mapped,
    };
    start = collapse(start, start_affinity);
    end = collapse(end, end_affinity);
    let (min, max) = (start.min().min(end.min()), start.max().max(end.max()));
    if range.reversed() { TextRange::new(max, min) } else { TextRange::new(min, max) }
}

/// `range` of the shown text in the original one, as `mapFromTransformed`: what it touches.
pub(crate) fn map_from_transformed(
    range: TextRange,
    mapping: &OffsetMappingCalculator,
) -> TextRange {
    let start = mapping.map_from_destination(range.start);
    let end = if range.collapsed() { start } else { mapping.map_from_destination(range.end) };
    let (min, max) = (start.min().min(end.min()), start.max().max(end.max()));
    if range.reversed() { TextRange::new(max, min) } else { TextRange::new(min, max) }
}

/// What a shown offset is in, as Compose's `IndexTransformationType`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum IndexTransformationType {
    Untransformed,
    Insertion,
    Replacement,
    Deletion,
}

/// What shown offset `index` is in, with its range in the original text and that range shown
/// again, as Compose's `getIndexTransformationType`.
pub(crate) fn index_transformation_type(
    index: usize,
    mapping: &OffsetMappingCalculator,
) -> (IndexTransformationType, TextRange, TextRange) {
    let untransformed = map_from_transformed(TextRange::at(index), mapping);
    let retransformed = map_to_transformed(untransformed, mapping, None);
    let kind = match (untransformed.collapsed(), retransformed.collapsed()) {
        (true, true) => IndexTransformationType::Untransformed,
        (false, false) => IndexTransformationType::Replacement,
        (true, false) => IndexTransformationType::Insertion,
        (false, true) => IndexTransformationType::Deletion,
    };
    (kind, untransformed, retransformed)
}

/// Where a cursor moving from shown offset `cursor` to `proposed` goes, and the wedge affinity it
/// takes, as Compose's `calculateNextCursorPositionAndWedgeAffinity`: never inside a replacement or
/// an insertion, a step into one going to its far edge.
pub(crate) fn next_cursor_position(
    proposed: usize,
    cursor: usize,
    mapping: &OffsetMappingCalculator,
) -> (usize, Option<WedgeAffinity>) {
    let forward = proposed > cursor;
    let (kind, _, retransformed) = index_transformation_type(proposed, mapping);
    match kind {
        IndexTransformationType::Untransformed => {
            (proposed, Some(if forward { WedgeAffinity::Start } else { WedgeAffinity::End }))
        }
        IndexTransformationType::Deletion => (proposed, None),
        IndexTransformationType::Replacement if forward => {
            (retransformed.end, Some(WedgeAffinity::Start))
        }
        IndexTransformationType::Replacement => (retransformed.start, Some(WedgeAffinity::End)),
        IndexTransformationType::Insertion if forward => {
            if proposed == retransformed.start {
                (proposed, Some(WedgeAffinity::Start))
            } else {
                (retransformed.end, Some(WedgeAffinity::End))
            }
        }
        IndexTransformationType::Insertion => {
            if proposed == retransformed.end {
                (proposed, Some(WedgeAffinity::End))
            } else {
                (retransformed.start, Some(WedgeAffinity::Start))
            }
        }
    }
}

/// What an input method was shown of a state-based field: the shown text's offsets mapped to the
/// state's, as Compose's `ImeEditCommandScope` maps them (D1135).
pub(crate) struct ShownToInputMethod {
    pub(crate) mapping: Rc<OffsetMappingCalculator>,
    pub(crate) length: usize,
}

impl compose_ui::ShownText for ShownToInputMethod {
    fn shown_length(&self) -> usize {
        self.length
    }

    /// Without a wedge affinity: a cursor at an insertion is the whole insertion, as Compose's
    /// `TransformedTextFieldState.mapToTransformed(range)`.
    fn map_to_shown(&self, range: TextRange) -> TextRange {
        map_to_transformed(range, &self.mapping, None)
    }

    fn map_from_shown(&self, range: TextRange) -> TextRange {
        map_from_transformed(range, &self.mapping)
    }
}

/// What a field shows of `value` through `transformation`, with the mapping its edits make, as
/// Compose's `calculateTransformedText`; `None` if it changed nothing.
pub(crate) fn transformed_text(
    value: &TextFieldCharSequence,
    transformation: &OutputTransformation,
) -> Option<(String, OffsetMappingCalculator)> {
    let mut buffer = TextFieldBuffer::new(value.clone());
    buffer.track_offset_mapping();
    transformation.transform_output(&mut buffer);
    if buffer.changes().change_count() == 0 {
        return None;
    }
    let mapping = buffer.take_offset_mapping().unwrap_or_default();
    Some((buffer.as_str().to_string(), mapping))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A text and the mapping its edits make, as Compose's test's `TestEditBuffer`.
    struct Edited {
        text: String,
        mapping: OffsetMappingCalculator,
    }

    impl Edited {
        fn new(text: &str) -> Edited {
            Edited { text: text.to_string(), mapping: OffsetMappingCalculator::default() }
        }
        fn replace(&mut self, start: usize, end: usize, with: &str) -> &mut Self {
            self.mapping.record_edit_operation(start, end, with.len());
            self.text.replace_range(start.min(end)..start.max(end), with);
            self
        }
        fn insert(&mut self, at: usize, with: &str) -> &mut Self {
            self.replace(at, at, with)
        }
        fn append(&mut self, with: &str) -> &mut Self {
            self.insert(self.text.len(), with)
        }
        fn delete(&mut self, start: usize, end: usize) -> &mut Self {
            self.replace(start, end, "")
        }
        fn delete_one(&mut self, at: usize) -> &mut Self {
            self.delete(at, at + 1)
        }
        fn replace_one(&mut self, at: usize, with: &str) -> &mut Self {
            self.replace(at, at + 1, with)
        }
        fn replace_all(&mut self, with: &str) -> &mut Self {
            self.replace(0, self.text.len(), with)
        }

        /// Checks the mappings of offsets 0, 1, … from the source, then from the destination.
        fn assert(
            &self,
            expected: &str,
            from_source: &[(usize, usize)],
            from_destination: &[(usize, usize)],
        ) {
            assert_eq!(self.text, expected);
            for (offset, &(start, end)) in from_source.iter().enumerate() {
                let range = TextRange::new(start, end);
                assert_eq!(
                    self.mapping.map_from_source(offset),
                    range,
                    "{expected}: from source {offset}"
                );
            }
            for (offset, &(start, end)) in from_destination.iter().enumerate() {
                let range = TextRange::new(start, end);
                assert_eq!(
                    self.mapping.map_from_destination(offset),
                    range,
                    "{expected}: from destination {offset}"
                );
            }
        }

        fn assert_identity(&self, expected: &str) {
            assert_eq!(self.text, expected);
            for offset in 0..self.text.len() + 2 {
                assert_eq!(self.mapping.map_from_source(offset), TextRange::at(offset));
                assert_eq!(self.mapping.map_from_destination(offset), TextRange::at(offset));
            }
        }
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, noChanges
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, replaceCharInMiddle
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleContinuousOneToOneReplacements
    #[test]
    fn nothing_or_one_for_one_maps_as_it_is() {
        Edited::new("").assert_identity("");
        Edited::new("abc").replace_one(1, "d").assert_identity("adc");
        Edited::new("abc")
            .replace_one(0, "f")
            .replace_one(1, "f")
            .replace_one(2, "f")
            .assert_identity("fff");
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, insertCharIntoEmpty
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, insertCharIntoMiddle
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, insertStringIntoEmpty
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, insertStringIntoMiddle
    #[test]
    fn an_insertion_point_maps_to_what_was_inserted() {
        Edited::new("").append("a").assert(
            "a",
            &[(0, 1), (2, 2), (3, 3), (4, 4)],
            &[(0, 0), (0, 0), (1, 1)],
        );
        Edited::new("ab").insert(1, "c").assert(
            "acb",
            &[(0, 0), (1, 2), (3, 3)],
            &[(0, 0), (1, 1), (1, 1), (2, 2)],
        );
        Edited::new("").append("ab").assert(
            "ab",
            &[(0, 2), (3, 3), (4, 4), (5, 5)],
            &[(0, 0), (0, 0), (0, 0), (1, 1)],
        );
        Edited::new("ab").insert(1, "cd").assert(
            "acdb",
            &[(0, 0), (1, 3), (4, 4), (5, 5)],
            &[(0, 0), (1, 1), (1, 1), (1, 1), (2, 2), (3, 3)],
        );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, deleteCharFromMiddle
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, deleteStringFromMiddle
    #[test]
    fn a_deleted_range_maps_to_where_it_was() {
        Edited::new("abc").delete_one(1).assert(
            "ac",
            &[(0, 0), (1, 1), (1, 1), (2, 2)],
            &[(0, 0), (1, 2), (3, 3), (4, 4)],
        );
        Edited::new("abcd").delete(1, 3).assert(
            "ad",
            &[(0, 0), (1, 1), (1, 1), (1, 1), (2, 2)],
            &[(0, 0), (1, 3), (4, 4), (5, 5)],
        );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, replaceStringWithEqualLengthInMiddle
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, replaceStringWithLongerInMiddle
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, replaceStringWithShorterInMiddle
    #[test]
    fn inside_a_replacement_maps_to_the_replacement() {
        Edited::new("abcd").replace(1, 3, "ef").assert(
            "aefd",
            &[(0, 0), (1, 1), (1, 3), (3, 3), (4, 4), (5, 5)],
            &[(0, 0), (1, 1), (1, 3), (3, 3), (4, 4), (5, 5)],
        );
        Edited::new("abcd").replace(1, 3, "efg").assert(
            "aefgd",
            &[(0, 0), (1, 1), (1, 4), (4, 4), (5, 5), (6, 6), (7, 7)],
            &[(0, 0), (1, 1), (1, 3), (1, 3), (3, 3), (4, 4)],
        );
        Edited::new("abcd").replace(1, 3, "e").assert(
            "aed",
            &[(0, 0), (1, 1), (1, 2), (2, 2), (3, 3), (4, 4)],
            &[(0, 0), (1, 1), (3, 3), (4, 4), (5, 5), (6, 6)],
        );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, replaceAllWithEqualLength
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, replaceAllWithLonger
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, replaceAllWithShorter
    #[test]
    fn replacing_everything_maps_its_inside_to_all() {
        Edited::new("abcd").replace_all("efgh").assert(
            "efgh",
            &[(0, 0), (0, 4), (0, 4), (0, 4), (4, 4), (5, 5)],
            &[(0, 0), (0, 4), (0, 4), (0, 4), (4, 4), (5, 5)],
        );
        Edited::new("abcd").replace_all("efghi").assert(
            "efghi",
            &[(0, 0), (0, 5), (0, 5), (0, 5), (5, 5), (6, 6)],
            &[(0, 0), (0, 4), (0, 4), (0, 4), (0, 4), (4, 4), (5, 5)],
        );
        Edited::new("abcd").replace_all("ef").assert(
            "ef",
            &[(0, 0), (0, 2), (0, 2), (0, 2), (2, 2), (3, 3)],
            &[(0, 0), (0, 4), (4, 4), (5, 5), (6, 6), (7, 7)],
        );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, prependCharToString
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, prependStringToString
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, appendCharToString
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, appendStringToString
    #[test]
    fn text_added_at_either_end() {
        Edited::new("a").insert(0, "b").assert(
            "ba",
            &[(0, 1), (2, 2), (3, 3), (4, 4)],
            &[(0, 0), (0, 0), (1, 1)],
        );
        Edited::new("a").insert(0, "bc").assert(
            "bca",
            &[(0, 2), (3, 3), (4, 4), (5, 5), (6, 6)],
            &[(0, 0), (0, 0), (0, 0), (1, 1)],
        );
        Edited::new("a").append("b").assert(
            "ab",
            &[(0, 0), (1, 2), (3, 3), (4, 4)],
            &[(0, 0), (1, 1), (1, 1), (2, 2)],
        );
        Edited::new("a").append("bc").assert(
            "abc",
            &[(0, 0), (1, 3), (4, 4), (5, 5), (6, 6)],
            &[(0, 0), (1, 1), (1, 1), (1, 1), (2, 2)],
        );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multiplePrepends
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleAppends
    #[test]
    fn insertions_one_after_another_map_as_one() {
        Edited::new("ab").insert(0, "c").insert(0, "d").insert(0, "ef").assert(
            "efdcab",
            &[(0, 4), (5, 5), (6, 6), (7, 7), (8, 8)],
            &[(0, 0), (0, 0), (0, 0), (0, 0), (0, 0), (1, 1), (2, 2), (3, 3)],
        );
        Edited::new("ab").append("c").append("d").append("ef").assert(
            "abcdef",
            &[(0, 0), (1, 1), (2, 6), (7, 7), (8, 8)],
            &[(0, 0), (1, 1), (2, 2), (2, 2), (2, 2), (2, 2), (2, 2), (3, 3)],
        );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multiplePrependsThenDeletesCancellingOut
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleAppendsThenDeletesCancellingOut
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleInsertsThenDeletesCancellingOut
    #[test]
    fn edits_that_cancel_out_map_as_it_is() {
        Edited::new("ab")
            .insert(0, "cde")
            .delete_one(2)
            .delete_one(0)
            .insert(0, "f")
            .delete(0, 2)
            .assert_identity("ab");
        Edited::new("ab")
            .append("cde")
            .delete_one(2)
            .delete_one(3)
            .append("f")
            .delete(2, 4)
            .assert_identity("ab");
        Edited::new("ab")
            .insert(1, "c")
            .insert(2, "de")
            .insert(1, "f")
            .delete(1, 3)
            .delete_one(1)
            .delete_one(1)
            .assert_identity("ab");
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleContinuousDeletesAtStartInOrder
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleContinuousDeletesAtStartOutOfOrder
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleContinuousDeletesAtEndInOrder
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleContinuousDeletesAtEndOutOfOrder
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleContinuousDeletesInMiddleInOrder
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleContinuousDeletesInMiddleOutOfOrder
    #[test]
    fn deletions_next_to_each_other_map_as_one_in_any_order() {
        let start_source = [(0, 0), (0, 0), (0, 0), (0, 0), (1, 1), (2, 2), (3, 3), (4, 4)];
        let start_destination = [(0, 3), (4, 4), (5, 5), (6, 6), (7, 7)];
        Edited::new("abcdef").delete_one(0).delete_one(0).delete_one(0).assert(
            "def",
            &start_source,
            &start_destination,
        );
        Edited::new("abcdef").delete_one(1).delete_one(1).delete_one(0).assert(
            "def",
            &start_source,
            &start_destination,
        );
        let end_source = [(0, 0), (1, 1), (2, 2), (3, 3), (3, 3), (3, 3), (3, 3), (4, 4)];
        let end_destination = [(0, 0), (1, 1), (2, 2), (3, 6), (7, 7), (8, 8)];
        Edited::new("abcdef").delete_one(5).delete_one(4).delete_one(3).assert(
            "abc",
            &end_source,
            &end_destination,
        );
        Edited::new("abcdef").delete_one(4).delete_one(3).delete_one(3).assert(
            "abc",
            &end_source,
            &end_destination,
        );
        let middle_source = [(0, 0), (1, 1), (1, 1), (1, 1), (1, 1), (1, 1), (2, 2), (3, 3)];
        let middle_destination = [(0, 0), (1, 5), (6, 6), (7, 7), (8, 8), (9, 9)];
        Edited::new("abcdef").delete_one(1).delete(1, 3).delete_one(1).assert(
            "af",
            &middle_source,
            &middle_destination,
        );
        Edited::new("abcdef").delete_one(2).delete(2, 4).delete_one(1).assert(
            "af",
            &middle_source,
            &middle_destination,
        );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, discontinuousInsertsAndDeletes
    #[test]
    fn insertions_and_deletions_apart() {
        Edited::new("ab")
            .insert(1, "cde")
            .delete_one(2)
            .append("fgh")
            .delete_one(4)
            .insert(0, "ijk")
            .delete_one(2)
            .assert(
                "ijacebgh",
                &[(0, 2), (3, 5), (6, 8), (9, 9), (10, 10)],
                &[(0, 0), (0, 0), (0, 0), (1, 1), (1, 1), (1, 1), (2, 2), (2, 2), (2, 2), (3, 3)],
            );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleContinuousOneToManyReplacements
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleContinuousOneToManyReplacementsReversed
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleContinuousManyToOneReplacements
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, multipleContinuousManyToOneReplacementsReversed
    #[test]
    fn character_by_character_replacements_map_each() {
        let one_to_many_source = [(0, 0), (2, 2), (4, 4), (6, 6), (7, 7)];
        let one_to_many_destination =
            [(0, 0), (0, 1), (1, 1), (1, 2), (2, 2), (2, 3), (3, 3), (4, 4)];
        Edited::new("abc").replace_one(0, "dd").replace_one(2, "ee").replace_one(4, "ff").assert(
            "ddeeff",
            &one_to_many_source,
            &one_to_many_destination,
        );
        Edited::new("abc").replace_one(2, "dd").replace_one(1, "ee").replace_one(0, "ff").assert(
            "ffeedd",
            &one_to_many_source,
            &one_to_many_destination,
        );
        Edited::new("abcdef").replace(0, 2, "g").replace(1, 3, "h").replace(2, 4, "i").assert(
            "ghi",
            &one_to_many_destination,
            &one_to_many_source,
        );
        Edited::new("abcdef").replace(4, 6, "g").replace(2, 4, "h").replace(0, 2, "i").assert(
            "ihg",
            &one_to_many_destination,
            &one_to_many_source,
        );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, twoOverlappingReplacements
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/OffsetMappingCalculatorTest.kt, fourOverlappingReplacementsReversed
    #[test]
    fn overlapping_replacements_map_consistently() {
        Edited::new("abc").replace(0, 2, "wx").replace(1, 3, "yz").assert(
            "wyz",
            &[(0, 0), (0, 3), (1, 3), (3, 3), (4, 4)],
            &[(0, 0), (0, 2), (0, 3), (3, 3), (4, 4)],
        );
        Edited::new("abcde")
            .replace(1, 3, "fg")
            .replace(2, 4, "hi")
            .replace(0, 2, "jk")
            .replace(3, 5, "lm")
            .assert(
                "jkhlm",
                &[(0, 0), (0, 2), (0, 5), (2, 5), (3, 5), (5, 5), (6, 6), (7, 7)],
                &[(0, 0), (0, 3), (1, 3), (1, 4), (1, 5), (5, 5), (6, 6), (7, 7)],
            );
    }

    /// What `transform` shows of `text`, with its mapping.
    fn shown(
        text: &str,
        transform: impl Fn(&mut TextFieldBuffer) + 'static,
    ) -> (String, OffsetMappingCalculator) {
        let value = TextFieldCharSequence::new(Rc::from(text), TextRange::ZERO, None);
        transformed_text(&value, &OutputTransformation::from_fn(transform)).expect("transformed")
    }

    fn mapped_to(
        mapping: &OffsetMappingCalculator,
        offsets: std::ops::Range<usize>,
    ) -> Vec<TextRange> {
        offsets.map(|offset| map_to_transformed(TextRange::at(offset), mapping, None)).collect()
    }

    fn mapped_from(
        mapping: &OffsetMappingCalculator,
        offsets: std::ops::Range<usize>,
    ) -> Vec<TextRange> {
        offsets.map(|offset| map_from_transformed(TextRange::at(offset), mapping)).collect()
    }

    fn ranges(pairs: &[(usize, usize)]) -> Vec<TextRange> {
        pairs.iter().map(|&(start, end)| TextRange::new(start, end)).collect()
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, outputTransformationAffectsPresentedAndVisualText
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, mapToTransformed_insertions
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, mapFromTransformed_insertions
    #[test]
    fn insertions_map_both_ways() {
        assert_eq!(shown("hello", |buffer| buffer.append("world")).0, "helloworld");
        let (text, mapping) = shown("zzzz", |buffer| {
            buffer.insert(2, "bb");
            buffer.insert(0, "aa");
            buffer.append("cc");
        });
        assert_eq!(text, "aazzbbzzcc");
        assert_eq!(
            mapped_to(&mapping, 0..6),
            ranges(&[(0, 2), (3, 3), (4, 6), (7, 7), (8, 10), (11, 11)])
        );
        assert_eq!(
            mapped_from(&mapping, 0..12),
            ranges(&[
                (0, 0),
                (0, 0),
                (0, 0),
                (1, 1),
                (2, 2),
                (2, 2),
                (2, 2),
                (3, 3),
                (4, 4),
                (4, 4),
                (4, 4),
                (5, 5)
            ])
        );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, mapToTransformed_deletions
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, mapFromTransformed_deletions
    #[test]
    fn deletions_map_both_ways() {
        let (text, mapping) = shown("aazzbbzzcc", |buffer| {
            buffer.delete(8, 10);
            buffer.delete(4, 6);
            buffer.delete(0, 2);
        });
        assert_eq!(text, "zzzz");
        assert_eq!(
            mapped_to(&mapping, 0..12),
            ranges(&[
                (0, 0),
                (0, 0),
                (0, 0),
                (1, 1),
                (2, 2),
                (2, 2),
                (2, 2),
                (3, 3),
                (4, 4),
                (4, 4),
                (4, 4),
                (5, 5)
            ])
        );
        assert_eq!(
            mapped_from(&mapping, 0..6),
            ranges(&[(0, 2), (3, 3), (4, 6), (7, 7), (8, 10), (11, 11)])
        );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, mapToTransformed_replacements
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, mapFromTransformed_replacements
    #[test]
    fn replacements_map_both_ways() {
        let (text, mapping) = shown("aabb", |buffer| {
            buffer.replace(2, 4, "ddd");
            buffer.replace(0, 2, "c");
        });
        assert_eq!(text, "cddd");
        assert_eq!(
            mapped_to(&mapping, 0..6),
            ranges(&[(0, 0), (0, 1), (1, 1), (1, 4), (4, 4), (5, 5)])
        );
        assert_eq!(
            mapped_from(&mapping, 0..6),
            ranges(&[(0, 0), (2, 2), (2, 4), (2, 4), (4, 4), (5, 5)])
        );
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextFieldStateTest.kt, textFieldStateSelection_isTransformed_toPresentedText
    #[test]
    fn a_selection_covers_what_was_inserted_inside_it() {
        let (text, mapping) = shown("hello", |buffer| buffer.insert(0, "aa"));
        assert_eq!(text, "aahello");
        let affinity = Some(SelectionWedgeAffinity::default());
        assert_eq!(
            map_to_transformed(TextRange::new(0, 2), &mapping, affinity),
            TextRange::new(0, 4)
        );
    }

    /// A state's selection, cursor or not, and its wedge affinity, moved over what `mapping` shows
    /// as Compose's test's `calculateNextCursorPosition` and `calculatePreviousCursorPosition`.
    struct Cursor<'a> {
        text: &'a str,
        mapping: &'a OffsetMappingCalculator,
        selection: TextRange,
        affinity: SelectionWedgeAffinity,
    }

    impl Cursor<'_> {
        fn shown(&self) -> TextRange {
            map_to_transformed(self.selection, self.mapping, Some(self.affinity))
        }

        fn step(&mut self, forward: bool) {
            let cursor = self.shown().end;
            let proposed = if forward { cursor + 1 } else { cursor - 1 };
            assert!(proposed <= self.text.len());
            let (cursor, affinity) = next_cursor_position(proposed, cursor, self.mapping);
            self.selection = map_from_transformed(TextRange::at(cursor), self.mapping);
            if let Some(affinity) = affinity {
                self.affinity = SelectionWedgeAffinity::both(affinity);
            }
        }
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextSelectionMovementTest.kt, calculateNextCursorPosition_aroundReplacement
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextSelectionMovementTest.kt, calculatePreviousCursorPosition_aroundReplacement
    #[test]
    fn a_cursor_steps_over_a_replacement() {
        let (text, mapping) = shown("abc", |buffer| buffer.replace(1, 2, "zz"));
        let affinity = SelectionWedgeAffinity::default();
        let mut cursor =
            Cursor { text: &text, mapping: &mapping, selection: TextRange::at(1), affinity };
        cursor.step(true);
        assert_eq!((cursor.selection, cursor.shown()), (TextRange::at(2), TextRange::at(3)));
        let mut cursor =
            Cursor { text: &text, mapping: &mapping, selection: TextRange::at(2), affinity };
        assert_eq!(cursor.shown(), TextRange::at(3));
        cursor.step(false);
        assert_eq!((cursor.selection, cursor.shown()), (TextRange::at(1), TextRange::at(1)));
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextSelectionMovementTest.kt, calculateNextCursorPosition_aroundInsertion
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextSelectionMovementTest.kt, calculatePreviousCursorPosition_aroundInsertion
    #[test]
    fn a_cursor_stops_at_both_sides_of_an_insertion() {
        use WedgeAffinity::{End, Start};
        let (text, mapping) = shown("ab", |buffer| buffer.insert(1, "zz"));
        let mut cursor = Cursor {
            text: &text,
            mapping: &mapping,
            selection: TextRange::at(0),
            affinity: SelectionWedgeAffinity::default(),
        };
        for (selection, shown, affinity) in [(1, 1, Start), (1, 3, End), (2, 4, Start)] {
            cursor.step(true);
            assert_eq!(
                (cursor.selection, cursor.shown()),
                (TextRange::at(selection), TextRange::at(shown))
            );
            assert_eq!(cursor.affinity, SelectionWedgeAffinity::both(affinity));
        }
        let mut cursor = Cursor {
            text: &text,
            mapping: &mapping,
            selection: TextRange::at(2),
            affinity: SelectionWedgeAffinity::default(),
        };
        assert_eq!(cursor.shown(), TextRange::at(4));
        for (selection, shown, affinity) in [(1, 3, End), (1, 1, Start), (0, 0, End)] {
            cursor.step(false);
            assert_eq!(
                (cursor.selection, cursor.shown()),
                (TextRange::at(selection), TextRange::at(shown))
            );
            assert_eq!(cursor.affinity, SelectionWedgeAffinity::both(affinity));
        }
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextSelectionMovementTest.kt, calculateNextCursorPosition_aroundDeletion
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/internal/TransformedTextSelectionMovementTest.kt, calculatePreviousCursorPosition_aroundDeletion
    #[test]
    fn a_cursor_on_a_deletion_selects_what_was_deleted() {
        let (text, mapping) = shown("abcd", |buffer| buffer.delete(1, 3));
        let affinity = SelectionWedgeAffinity::default();
        let mut cursor =
            Cursor { text: &text, mapping: &mapping, selection: TextRange::at(0), affinity };
        cursor.step(true);
        assert_eq!((cursor.selection, cursor.shown()), (TextRange::new(1, 3), TextRange::at(1)));
        cursor.step(true);
        assert_eq!((cursor.selection, cursor.shown()), (TextRange::at(4), TextRange::at(2)));
        let mut cursor =
            Cursor { text: &text, mapping: &mapping, selection: TextRange::at(4), affinity };
        assert_eq!(cursor.shown(), TextRange::at(2));
        cursor.step(false);
        assert_eq!((cursor.selection, cursor.shown()), (TextRange::new(1, 3), TextRange::at(1)));
        cursor.step(false);
        assert_eq!((cursor.selection, cursor.shown()), (TextRange::at(0), TextRange::at(0)));
    }

    // Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/SingleLineOutputTransformationTest.kt, lineFeed_isReplacedWithWhitespace
    // Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/SingleLineOutputTransformationTest.kt, carriageReturn_isReplacedWithZeroWidthSpace
    // Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/SingleLineOutputTransformationTest.kt, lineFeed_isReplaced_aroundEmojis
    // Compose: compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/text/input/internal/SingleLineOutputTransformationTest.kt, wrapAppliesOtherOutputTransformationFirst
    #[test]
    fn a_single_line_shows_line_feeds_as_spaces_and_returns_as_nothing() {
        let single = |text: &str, inner: Option<&OutputTransformation>| {
            let value = TextFieldCharSequence::new(Rc::from(text), TextRange::ZERO, None);
            transformed_text(&value, &OutputTransformation::single_line(inner))
                .map(|(shown, _)| shown)
        };
        assert_eq!(single("hello\nworld", None).as_deref(), Some("hello world"));
        assert_eq!(single("hello\rworld", None).as_deref(), Some("hello\u{FEFF}world"));
        let (complex, regular) = ("\u{1F3F3}\u{FE0F}\u{200D}\u{1F308}", "\u{1F600}");
        assert_eq!(
            single(&format!("{complex}\n{regular}"), None),
            Some(format!("{complex} {regular}"))
        );
        let other = OutputTransformation::from_fn(|buffer| buffer.insert(3, "\n"));
        assert_eq!(single("abcdef", Some(&other)).as_deref(), Some("abc def"));
        assert_eq!(single("plain", None), None, "nothing changed");
    }

    /// A transformation going character by character, each for one as long, leaves nothing to
    /// map, as Compose's skips one unit for one: its maps stay short.
    #[test]
    fn one_character_for_one_as_long_is_not_mapped() {
        let each = |with: &'static str| {
            move |buffer: &mut TextFieldBuffer| {
                let mut index = 0;
                while index < buffer.length() {
                    let length = buffer.as_str()[index..].chars().next().map_or(1, char::len_utf8);
                    buffer.replace(index, index + length, with);
                    index += with.len();
                }
            }
        };
        let (text, mapping) = shown("abc", each("x"));
        assert_eq!((text.as_str(), mapping.operations.len()), ("xxx", 0));
        let (text, mapping) = shown("\u{e9}\u{e9}", each("\u{fc}"));
        assert_eq!((text.as_str(), mapping.operations.len()), ("\u{fc}\u{fc}", 0));
        // A byte for three: offsets move, so it's mapped.
        let (text, mapping) = shown("ab", each("\u{2022}"));
        assert_eq!((text.as_str(), mapping.operations.len()), ("\u{2022}\u{2022}", 2));
        assert_eq!(mapping.map_from_source(1), TextRange::at(3));
    }
}
