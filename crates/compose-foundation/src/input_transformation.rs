//! What a state-based text field does with the user's edits before they reach its state, as
//! Compose's `InputTransformation`, and what its keyboard's action key does, as Compose's
//! `KeyboardActionHandler` (D1133).

use std::borrow::Cow;
use std::fmt;
use std::rc::Rc;

use compose_runtime::ComposeArg;
use compose_ui::{KeyboardCapitalization, Locale, SemanticsConfiguration};

use crate::KeyboardOptions;
use crate::text_field_buffer::{TextFieldBuffer, floor_boundary};

/// An application's transformation, as Compose's `InputTransformation` interface: given every
/// edit a user makes (keys, the input method, semantics, a paste), in a buffer whose original is
/// the text before it and whose changes are the edit's, it changes or rejects
/// (`revert_all_changes`) what was proposed. Make it an [`InputTransformation`] with
/// [`InputTransformation::new`], or a closure with [`InputTransformation::from_fn`].
pub trait InputTransform {
    fn transform_input(&self, buffer: &mut TextFieldBuffer);

    /// The keyboard it asks for, as `keyboardOptions`: the field's own options say first.
    fn keyboard_options(&self) -> Option<KeyboardOptions> {
        None
    }

    /// What it says of the field to accessibility (its longest text, say), as `applySemantics`.
    fn apply_semantics(&self, _config: &mut SemanticsConfiguration) {}
}

/// A function deciding the accepted text from the current and the proposed ones, as
/// `byValue`'s: returning `current` itself rejects the edit, `proposed` itself keeps it.
type ByValue = Rc<dyn for<'a> Fn(&'a str, &'a str) -> Cow<'a, str>>;

#[derive(Clone)]
enum Kind {
    Custom(Rc<dyn InputTransform>),
    Chain(Rc<(InputTransformation, InputTransformation)>),
    ByValue(ByValue),
    AllCaps(Locale),
    MaxLengthTrim(usize),
    MaxLengthReject(usize),
}

/// A field's input transformation, as Compose's `InputTransformation`: an application's
/// ([`new`](Self::new), [`from_fn`](Self::from_fn)) or one of Compose's
/// ([`by_value`](Self::by_value), [`all_caps`](Self::all_caps),
/// [`max_length_trim`](Self::max_length_trim), [`max_length_reject`](Self::max_length_reject)),
/// chained with [`then`](Self::then). Equal as Compose's: a chain by its parts, Compose's own by
/// their parameters, an application's by identity (D1133).
#[derive(Clone)]
pub struct InputTransformation(Kind);

impl InputTransformation {
    pub fn new(transform: impl InputTransform + 'static) -> InputTransformation {
        InputTransformation(Kind::Custom(Rc::new(transform)))
    }

    /// A transformation of only `transform_input`, as Compose's lambda form.
    pub fn from_fn(transform: impl Fn(&mut TextFieldBuffer) + 'static) -> InputTransformation {
        struct Function<F>(F);
        impl<F: Fn(&mut TextFieldBuffer)> InputTransform for Function<F> {
            fn transform_input(&self, buffer: &mut TextFieldBuffer) {
                (self.0)(buffer)
            }
        }
        InputTransformation::new(Function(transform))
    }

    /// This, then `next` on what this left, as `then`: `next`'s keyboard options say first, and
    /// its semantics come last.
    pub fn then(self, next: InputTransformation) -> InputTransformation {
        InputTransformation(Kind::Chain(Rc::new((self, next))))
    }

    /// Decides the text from the current and proposed ones, as `byValue`: returning `current`
    /// itself rejects the edit, `proposed` itself keeps it, and another text replaces what
    /// differs (the borrow tells them apart, as Compose's reference comparison).
    pub fn by_value(
        transformation: impl for<'a> Fn(&'a str, &'a str) -> Cow<'a, str> + 'static,
    ) -> InputTransformation {
        InputTransformation(Kind::ByValue(Rc::new(transformation)))
    }

    /// Typed text in capitals, as `allCaps(locale)`: what an edit inserted, the rest left alone;
    /// `locale` gives Turkish and Azeri their dotted capital I. Asks for a keyboard capitalizing
    /// every character.
    pub fn all_caps(locale: Locale) -> InputTransformation {
        InputTransformation(Kind::AllCaps(locale))
    }

    /// At most `max_length` characters, what an edit inserted past them cut from its end, as
    /// `maxLengthTrim`. Characters are Unicode scalar values (D1133).
    pub fn max_length_trim(max_length: usize) -> InputTransformation {
        InputTransformation(Kind::MaxLengthTrim(max_length))
    }

    /// At most `max_length` characters, an edit past them rejected, as `maxLengthReject`.
    pub fn max_length_reject(max_length: usize) -> InputTransformation {
        InputTransformation(Kind::MaxLengthReject(max_length))
    }

    /// Changes or rejects what `buffer` was given, as `transformInput`.
    pub fn transform_input(&self, buffer: &mut TextFieldBuffer) {
        match &self.0 {
            Kind::Custom(transform) => transform.transform_input(buffer),
            Kind::Chain(chain) => {
                chain.0.transform_input(buffer);
                chain.1.transform_input(buffer);
            }
            Kind::ByValue(transformation) => by_value(transformation, buffer),
            Kind::AllCaps(locale) => all_caps(locale, buffer),
            Kind::MaxLengthTrim(max) => max_length_trim(*max, buffer),
            Kind::MaxLengthReject(max) => {
                if buffer.as_str().chars().count() > *max {
                    buffer.revert_all_changes();
                }
            }
        }
    }

    /// The keyboard it asks for, as `keyboardOptions`.
    pub fn keyboard_options(&self) -> Option<KeyboardOptions> {
        match &self.0 {
            Kind::Custom(transform) => transform.keyboard_options(),
            Kind::Chain(chain) => match (chain.0.keyboard_options(), chain.1.keyboard_options()) {
                (first, Some(second)) => Some(second.fill_unspecified_values_with(first.as_ref())),
                (first, None) => first,
            },
            Kind::AllCaps(_) => Some(KeyboardOptions {
                capitalization: KeyboardCapitalization::Characters,
                ..KeyboardOptions::DEFAULT
            }),
            _ => None,
        }
    }

    /// What it says of the field to accessibility, as `applySemantics`.
    pub fn apply_semantics(&self, config: &mut SemanticsConfiguration) {
        match &self.0 {
            Kind::Custom(transform) => transform.apply_semantics(config),
            Kind::Chain(chain) => {
                chain.0.apply_semantics(config);
                chain.1.apply_semantics(config);
            }
            Kind::MaxLengthTrim(max) | Kind::MaxLengthReject(max) => {
                config.set_max_text_length(i32::try_from(*max).unwrap_or(i32::MAX));
            }
            _ => {}
        }
    }
}

fn by_value(transformation: &ByValue, buffer: &mut TextFieldBuffer) {
    let original = buffer.original_text().to_string();
    let proposed = buffer.as_str().to_string();
    let accepted = transformation(&original, &proposed);
    let same = |text: &str, other: &str| std::ptr::eq(text, other);
    match &accepted {
        Cow::Borrowed(text) if same(text, proposed.as_str()) => {}
        Cow::Borrowed(text) if same(text, original.as_str()) => buffer.revert_all_changes(),
        _ => {
            let accepted = accepted.into_owned();
            buffer.set_text_if_changed(&accepted);
        }
    }
}

/// `text` in capitals, a Turkish or Azeri `i` a dotted capital I, as Java's `toUpperCase(locale)`
/// for them.
fn to_upper_case(text: &str, locale: &Locale) -> String {
    let dotted = matches!(locale.language().as_str(), "tr" | "az");
    let mut upper = String::with_capacity(text.len());
    for c in text.chars() {
        if dotted && c == 'i' {
            upper.push('\u{130}');
        } else {
            upper.extend(c.to_uppercase());
        }
    }
    upper
}

fn all_caps(locale: &Locale, buffer: &mut TextFieldBuffer) {
    buffer.for_each_change(|buffer, range, _| {
        if !range.collapsed() {
            let upper = to_upper_case(range.substring(buffer.as_str()), locale);
            buffer.replace(range.min(), range.max(), &upper);
        }
    });
}

/// The byte offset `count` characters before `end` in `text`, not before `start`.
fn back_by_characters(text: &str, start: usize, end: usize, count: usize) -> usize {
    if count == 0 {
        return end;
    }
    text[start..end].char_indices().rev().nth(count - 1).map_or(start, |(at, _)| start + at)
}

fn max_length_trim(max: usize, buffer: &mut TextFieldBuffer) {
    let length = buffer.as_str().chars().count();
    if length <= max {
        return;
    }
    let mut excess = length - max;
    buffer.for_each_change_reversed(|buffer, range, _| {
        if excess == 0 {
            return;
        }
        let inserted = range.substring(buffer.as_str()).chars().count();
        if inserted > 0 {
            let delete = excess.min(inserted);
            let from = back_by_characters(buffer.as_str(), range.min(), range.max(), delete);
            buffer.delete(from, range.max());
            excess -= delete;
        }
    });
    if buffer.as_str().chars().count() > max {
        let from = buffer.as_str().char_indices().nth(max).map_or(buffer.length(), |(at, _)| at);
        buffer.delete(floor_boundary(buffer.as_str(), from), buffer.length());
    }
}

impl PartialEq for InputTransformation {
    fn eq(&self, other: &InputTransformation) -> bool {
        match (&self.0, &other.0) {
            (Kind::Custom(a), Kind::Custom(b)) => Rc::ptr_eq(a, b),
            (Kind::Chain(a), Kind::Chain(b)) => Rc::ptr_eq(a, b) || (a.0 == b.0 && a.1 == b.1),
            (Kind::ByValue(a), Kind::ByValue(b)) => Rc::ptr_eq(a, b),
            (Kind::AllCaps(a), Kind::AllCaps(b)) => a == b,
            (Kind::MaxLengthTrim(a), Kind::MaxLengthTrim(b)) => a == b,
            (Kind::MaxLengthReject(a), Kind::MaxLengthReject(b)) => a == b,
            _ => false,
        }
    }
}

impl ComposeArg for InputTransformation {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// As Compose's `toString`s: `InputTransformation.maxLengthTrim(5)`, a chain as `a.then(b)`.
impl fmt::Debug for InputTransformation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            Kind::Custom(_) => formatter.write_str("InputTransformation"),
            Kind::Chain(chain) => write!(formatter, "{:?}.then({:?})", chain.0, chain.1),
            Kind::ByValue(_) => formatter.write_str("InputTransformation.byValue"),
            Kind::AllCaps(locale) => {
                write!(formatter, "InputTransformation.allCaps(locale={locale:?})")
            }
            Kind::MaxLengthTrim(max) => {
                write!(formatter, "InputTransformation.maxLengthTrim({max})")
            }
            Kind::MaxLengthReject(max) => {
                write!(formatter, "InputTransformation.maxLengthReject({max})")
            }
        }
    }
}

/// What a state-based field's keyboard action key (and a single line's Enter) does, as Compose's
/// `KeyboardActionHandler`: called with the default action (`Next` and `Previous` move the
/// focus, `Done` hides the on-screen keyboard), which it may call or not. It runs after the event,
/// as every application callback a field calls (D1133).
#[derive(Clone)]
pub struct KeyboardActionHandler(Rc<OnKeyboardAction>);

/// A handler's function, given the default action.
type OnKeyboardAction = dyn Fn(&dyn Fn());

impl KeyboardActionHandler {
    pub fn new(on_keyboard_action: impl Fn(&dyn Fn()) + 'static) -> KeyboardActionHandler {
        KeyboardActionHandler(Rc::new(on_keyboard_action))
    }

    /// As `onKeyboardAction(performDefaultAction)`.
    pub fn on_keyboard_action(&self, perform_default_action: &dyn Fn()) {
        (self.0)(perform_default_action)
    }
}

impl ComposeArg for KeyboardActionHandler {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for KeyboardActionHandler {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KeyboardActionHandler")
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;
    use std::rc::Rc;

    use compose_ui::{
        KeyboardCapitalization, Locale, SemanticsConfiguration, SemanticsProperties, TextRange,
    };

    use super::*;
    use crate::text_field_buffer::TextFieldCharSequence;

    fn sequence(text: &str, selection: TextRange) -> TextFieldCharSequence {
        TextFieldCharSequence::new(Rc::from(text), selection, None)
    }

    /// A buffer of `original`, edited by `edit`, as Compose's tests' `TextFieldBuffer(original)`.
    fn edited(
        original: &str,
        selection: TextRange,
        edit: impl FnOnce(&mut TextFieldBuffer),
    ) -> TextFieldBuffer {
        let mut buffer = TextFieldBuffer::new(sequence(original, selection));
        edit(&mut buffer);
        buffer
    }

    /// A buffer whose original is `current` and whose text is `proposed`, without changes, as
    /// Compose's `TextFieldBuffer(originalValue = current, initialValue = proposed)`.
    fn proposing(current: &str, proposed: &str) -> TextFieldBuffer {
        TextFieldBuffer::with_original(
            sequence(proposed, TextRange::ZERO),
            sequence(current, TextRange::ZERO),
            None,
        )
    }

    /// The longest text, the original, its cursor, the edit, the text expected, the cursor after.
    type Trimming<'a> = (usize, &'a str, usize, (usize, usize, &'a str), &'a str, Option<usize>);

    fn max_text_length(transformation: &InputTransformation) -> Option<i32> {
        let mut config = SemanticsConfiguration::default();
        transformation.apply_semantics(&mut config);
        config.get(&SemanticsProperties::MAX_TEXT_LENGTH).copied()
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, byValue_reverts_whenReturnsCurrent
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, byValue_appliesChanges_whenReturnsSameContentAsCurrent
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, byValue_noops_whenReturnsProposed
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/InputTransformationTest.kt, byValue_appliesChanges_whenDifferentCharSequenceReturned
    #[test]
    fn by_value_rejects_keeps_or_replaces_by_what_it_returns() {
        let cases: [(InputTransformation, usize, &str); 4] = [
            (InputTransformation::by_value(|current, _| Cow::Borrowed(current)), 0, "a"),
            (InputTransformation::by_value(|_, _| Cow::Owned("a".to_string())), 1, "a"),
            (InputTransformation::by_value(|_, _| Cow::Owned("ab".to_string())), 0, "ab"),
            (InputTransformation::by_value(|_, _| Cow::Owned("c".to_string())), 1, "c"),
        ];
        for (index, (transformation, changes, text)) in cases.into_iter().enumerate() {
            let mut buffer = proposing("a", "ab");
            transformation.transform_input(&mut buffer);
            assert_eq!(buffer.changes().change_count(), changes, "case {index}");
            assert_eq!(buffer.as_str(), text, "case {index}");
        }
        // The proposed text itself keeps the edit.
        let keep = InputTransformation::by_value(|_, proposed| Cow::Borrowed(proposed));
        let mut buffer = proposing("a", "ab");
        keep.transform_input(&mut buffer);
        assert_eq!((buffer.as_str(), buffer.changes().change_count()), ("ab", 0));
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/AllCapsTransformationTest.kt, allCapsTransformation_definesCharacterCapitalizationKeyboardOption
    #[test]
    fn all_caps_asks_for_a_capitalizing_keyboard() {
        let transformation = InputTransformation::all_caps(Locale::new("en"));
        let options = transformation.keyboard_options().expect("options");
        assert_eq!(options.capitalization, KeyboardCapitalization::Characters);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/AllCapsTransformationTest.kt, allNewTypedCharacters_convertedToUppercase
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/AllCapsTransformationTest.kt, oldCharacters_areNotConverted
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/AllCapsTransformationTest.kt, localeDifference_turkishI
    #[test]
    fn all_caps_capitalizes_what_was_typed_by_its_locale() {
        for (locale, original, appended, expected) in [
            ("en-US", "", "hello", "HELLO"),
            ("en-US", "hello", " world", "hello WORLD"),
            ("tr", "", "i", "\u{130}"),
        ] {
            let mut buffer = edited(original, TextRange::ZERO, |buffer| buffer.append(appended));
            InputTransformation::all_caps(Locale::new(locale)).transform_input(&mut buffer);
            assert_eq!(buffer.as_str(), expected, "{locale}");
        }
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/AllCapsTransformationTest.kt, multipleEdits
    #[test]
    fn all_caps_capitalizes_each_edits_insertions() {
        let transformation = InputTransformation::all_caps(Locale::new("en-US"));
        let mut buffer = edited("hello", TextRange::ZERO, |buffer| {
            buffer.delete(0, 3);
            buffer.replace(1, 1, "abc");
        });
        transformation.transform_input(&mut buffer);
        let mut buffer = edited(buffer.as_str(), buffer.selection(), |buffer| {
            buffer.delete(2, 3);
            buffer.append("xyz");
        });
        transformation.transform_input(&mut buffer);
        assert_eq!(buffer.as_str(), "lACoXYZ");
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthTrim_definesMaxTextLengthSemantics
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthReject_definesMaxTextLengthSemantics
    #[test]
    fn a_max_length_says_so_to_accessibility() {
        assert_eq!(max_text_length(&InputTransformation::max_length_trim(10)), Some(10));
        assert_eq!(max_text_length(&InputTransformation::max_length_reject(10)), Some(10));
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthTrim_doesNotTruncate_whenLengthIsLessThanOrEqualToMaxLength
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthTrim_truncatesInsertedText_atEnd
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthTrim_truncatesInsertedText_inMiddle
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthTrim_truncatesReplacedText
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthTrim_handlesZeroMaxLength
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthTrim_truncatesExistingLongText_whenFurtherEditsMade
    #[test]
    fn trimming_cuts_what_was_inserted_past_the_length() {
        let cases: [Trimming; 6] = [
            (5, "", 0, (0, 0, "abc"), "abc", None),
            (5, "abc", 3, (3, 3, "12345"), "abc12", Some(5)),
            (3, "ac", 1, (1, 1, "1234"), "a1c", Some(2)),
            (10, "hello world", 11, (6, 11, "everyone!"), "hello ever", None),
            (0, "", 0, (0, 0, "abc"), "", None),
            (5, "abcdef", 6, (6, 6, "g"), "abcde", None),
        ];
        for (max, original, cursor, (start, end, text), expected, selection) in cases {
            let mut buffer =
                edited(original, TextRange::at(cursor), |buffer| buffer.replace(start, end, text));
            InputTransformation::max_length_trim(max).transform_input(&mut buffer);
            assert_eq!(buffer.as_str(), expected, "{original} + {text}");
            if let Some(selection) = selection {
                assert_eq!(buffer.selection(), TextRange::at(selection), "{original} + {text}");
            }
        }
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthTrim_multipleEdits
    #[test]
    fn trimming_follows_each_edit() {
        let transformation = InputTransformation::max_length_trim(3);
        let mut buffer = edited("a", TextRange::at(1), |buffer| buffer.insert(1, "bcde"));
        transformation.transform_input(&mut buffer);
        assert_eq!(buffer.as_str(), "abc");
        let mut buffer = edited(buffer.as_str(), buffer.selection(), |buffer| buffer.delete(1, 3));
        transformation.transform_input(&mut buffer);
        assert_eq!(buffer.as_str(), "a");
        let mut buffer =
            edited(buffer.as_str(), buffer.selection(), |buffer| buffer.insert(1, "xyz"));
        transformation.transform_input(&mut buffer);
        assert_eq!(buffer.as_str(), "axy");
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthTrim_multipleEditsInSingleBuffer_commitDeleteCommit
    #[test]
    fn trimming_cuts_from_the_last_insertion_first() {
        let mut buffer = edited("", TextRange::ZERO, |buffer| {
            buffer.insert(0, "1234567890");
            buffer.delete(0, 7);
            buffer.insert(3, "123");
        });
        InputTransformation::max_length_trim(5).transform_input(&mut buffer);
        assert_eq!(buffer.as_str(), "89012");
    }

    /// Lengths are characters, not bytes (D1133).
    #[test]
    fn a_max_length_counts_characters() {
        let mut buffer = edited("", TextRange::ZERO, |buffer| buffer.append("héllo!"));
        InputTransformation::max_length_trim(5).transform_input(&mut buffer);
        assert_eq!(buffer.as_str(), "héllo");
        let mut buffer = edited("", TextRange::ZERO, |buffer| buffer.append("héllo"));
        InputTransformation::max_length_reject(5).transform_input(&mut buffer);
        assert_eq!(buffer.as_str(), "héllo");
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthReject_allowsInput_whenLengthIsLessThanOrEqualToMaxLength
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthReject_revertsAllChanges_whenLengthExceedsMaxLength
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/text/input/MaxLengthTransformationTest.kt, maxLengthReject_handlesZeroMaxLength
    #[test]
    fn rejecting_reverts_an_edit_past_the_length() {
        for (max, original, cursor, inserted, expected) in
            [(5, "", 0, "abc", "abc"), (5, "abc", 3, "12345", "abc"), (0, "", 0, "abc", "")]
        {
            let mut buffer =
                edited(original, TextRange::at(cursor), |buffer| buffer.insert(cursor, inserted));
            InputTransformation::max_length_reject(max).transform_input(&mut buffer);
            assert_eq!(buffer.as_str(), expected);
            if expected == original {
                assert_eq!(buffer.selection(), TextRange::at(cursor));
            }
        }
    }
}
