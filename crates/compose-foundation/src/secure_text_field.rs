//! `BasicSecureTextField`, as Compose's: a single-line state-based field for passwords, its text
//! shown as a mask but for the last character typed, for a moment, as the obfuscation mode says
//! (D1142).

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use compose_runtime::{
    Callback, ComposeArg, CoroutineScope, Job, MutableState, OwnedState, State, composable,
    mutable_state_of,
};
use compose_ui::{Modifier, TextRange, TextStyle};
use unicode_segmentation::UnicodeSegmentation;

use crate::text_input::{KeyboardOptions, KeyboardType};
use crate::{
    InputTransform, InputTransformation, KeyboardActionHandler, OutputTransform,
    OutputTransformation, TextFieldBuffer, TextFieldLineLimits, TextFieldState,
};

/// How a secure field hides its text, as Compose's `TextObfuscationMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TextObfuscationMode {
    /// Nothing hidden: the text shows (a "show password" toggle).
    Visible,
    /// Each character hidden but the last one typed, for a moment, whatever the platform says.
    RevealLastTyped,
    /// Each character hidden.
    Hidden,
    /// The platform's choice ([`LOCAL_PASSWORD_VISIBILITY`]): the last one typed shown for a
    /// moment where it says so for the kind of keyboard that typed it, else hidden.
    #[default]
    System,
}

impl ComposeArg for TextObfuscationMode {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// Whether the platform shows a password's last typed character, typed on a screen and on a
/// physical keyboard, as Compose's `SplitVisibilitySettings` (Android's "Show passwords").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct PasswordVisibility {
    pub touch: bool,
    pub physical: bool,
}

impl ComposeArg for PasswordVisibility {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// What the platform says of revealing a password's last typed character, read by
/// [`TextObfuscationMode::System`]: by default, the host's "Show passwords" setting
/// (`compose_ui::platform_show_password`, Android's, read as Compose's `PasswordVisibilitySetting`,
/// D1146); where the host has none, shown for a screen's keyboard and hidden for a physical one,
/// as Android's defaults (D1142).
pub static LOCAL_PASSWORD_VISIBILITY: compose_runtime::CompositionLocal<PasswordVisibility> =
    compose_runtime::CompositionLocal::new("LocalPasswordVisibility", || {
        match compose_ui::platform_show_password() {
            Some((touch, physical)) => PasswordVisibility { touch, physical },
            None => PasswordVisibility { touch: true, physical: false },
        }
    });

/// How long the last typed character shows, as Android's `PasswordTransformationMethod`.
pub(crate) const LAST_TYPED_CHARACTER_REVEAL_DURATION: Duration = Duration::from_millis(1500);

/// The mask's default character, a bullet.
pub const DEFAULT_OBFUSCATION_CHARACTER: char = '\u{2022}';

/// The keyboard options a secure field asks for by default, as Compose's
/// `KeyboardOptions.SecureTextField`: a password keyboard that corrects nothing.
impl KeyboardOptions {
    pub const SECURE_TEXT_FIELD: KeyboardOptions = KeyboardOptions {
        auto_correct_enabled: Some(false),
        keyboard_type: KeyboardType::Password,
        ..KeyboardOptions::DEFAULT
    };
}

/// A field for a password over `state`, as Compose's `BasicSecureTextField`: always a single
/// line, its text hidden by `text_obfuscation_mode` behind `text_obfuscation_character`, its
/// semantics a password's, and its text never copied or cut. In `RevealLastTyped`, and in `System`
/// where the platform says so, a character typed alone shows for 1.5 s, until another edit or the
/// focus leaves. The other parameters are [`BasicTextFieldOfState`](crate::BasicTextFieldOfState)'s
/// (D1142).
#[composable]
pub fn BasicSecureTextField(
    state: TextFieldState,
    #[default(crate::Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(false)] read_only: bool,
    #[default(None)] input_transformation: Option<InputTransformation>,
    #[default(crate::TextStyle::DEFAULT)] text_style: TextStyle,
    #[default(crate::KeyboardOptions::SECURE_TEXT_FIELD)] keyboard_options: KeyboardOptions,
    #[default(None)] on_keyboard_action: Option<KeyboardActionHandler>,
    #[default(None)] on_text_layout: Option<impl Fn(compose_ui::TextLayoutResult) + 'static>,
    #[default(None)] interaction_source: Option<compose_ui::MutableInteractionSource>,
    #[default(crate::Brush::solid(crate::Color::BLACK))] cursor_brush: compose_ui::Brush,
    #[default(crate::inner_text_field_only)] decorator: impl Fn(Callback) + 'static,
    #[default(crate::TextObfuscationMode::System)] text_obfuscation_mode: TextObfuscationMode,
    #[default(crate::DEFAULT_OBFUSCATION_CHARACTER)] text_obfuscation_character: char,
    #[default(None)] scroll_state: Option<crate::ScrollState>,
) {
    let mask = compose_runtime::remember_updated_state(text_obfuscation_character);
    let mode = compose_runtime::remember_updated_state(text_obfuscation_mode);
    let visibility = compose_runtime::remember_updated_state(LOCAL_PASSWORD_VISIBILITY.current());
    let scope = compose_runtime::remember_coroutine_scope();
    let controller = compose_runtime::remember(|| {
        Rc::new(SecureTextFieldController::new(mask, mode, visibility, scope))
    });
    // Revealing gone (`Hidden`, `Visible`): nothing stays revealed.
    let reveals = matches!(
        text_obfuscation_mode,
        TextObfuscationMode::RevealLastTyped | TextObfuscationMode::System
    );
    let hiding = controller.clone();
    compose_runtime::LaunchedEffect(reveals, async move {
        if !reveals {
            hiding.password.hide();
        }
    });
    let output_transformation = match text_obfuscation_mode {
        TextObfuscationMode::RevealLastTyped | TextObfuscationMode::System => {
            Some(controller.revealing.clone())
        }
        TextObfuscationMode::Hidden => Some(controller.hidden.clone()),
        TextObfuscationMode::Visible => None,
    };
    let input_transformation = match input_transformation {
        Some(first) => Some(first.then(controller.password_transformation.clone())),
        None => Some(controller.password_transformation.clone()),
    };
    let on_focus = controller.clone();
    let modifier = modifier.on_focus_changed(move |focus: compose_ui::FocusState| {
        if !focus.is_focused() {
            on_focus.password.hide();
        }
    });
    crate::BasicTextFieldOfState!(
        state,
        modifier = modifier,
        enabled = enabled,
        read_only = read_only,
        input_transformation = input_transformation,
        text_style = text_style,
        keyboard_options = keyboard_options,
        on_keyboard_action = on_keyboard_action,
        line_limits = TextFieldLineLimits::SingleLine,
        interaction_source = interaction_source,
        cursor_brush = cursor_brush,
        output_transformation = output_transformation,
        decorator = decorator,
        on_text_layout = on_text_layout,
        scroll_state = scroll_state,
        password = Some(text_obfuscation_mode != TextObfuscationMode::Visible),
    );
}

/// A secure field's transformations and its timer, as Compose's `SecureTextFieldController`.
struct SecureTextFieldController {
    password: Rc<PasswordInputTransformation>,
    password_transformation: InputTransformation,
    /// Each glyph masked but the one revealed.
    revealing: OutputTransformation,
    /// Each glyph masked.
    hidden: OutputTransformation,
}

impl SecureTextFieldController {
    fn new(
        mask: State<char>,
        mode: State<TextObfuscationMode>,
        visibility: State<PasswordVisibility>,
        scope: CoroutineScope,
    ) -> SecureTextFieldController {
        let timer: Rc<Cell<Option<Job>>> = Rc::default();
        let owner = mutable_state_of(None::<usize>);
        let reveal = owner.handle();
        // Each new reveal starts the timer again, as Compose's `collectLatest` over its channel.
        let schedule_hide = move || {
            if let Some(job) = timer.take() {
                job.cancel();
            }
            timer.set(Some(scope.launch(async move {
                compose_runtime::delay(LAST_TYPED_CHARACTER_REVEAL_DURATION).await;
                reveal.set(None);
            })));
        };
        let password = Rc::new(PasswordInputTransformation {
            schedule_hide: Box::new(schedule_hide),
            mode: Box::new(move || mode.get()),
            visibility: Box::new(move || visibility.get()),
            reveal,
            _owner: owner,
        });
        let revealed = password.clone();
        let revealing = OutputTransformation::new(PerGlyph(Box::new(move |range: TextRange| {
            (Some(range.start) != revealed.reveal.get()).then(|| mask.get().to_string())
        })));
        let hidden = OutputTransformation::new(PerGlyph(Box::new(move |_: TextRange| {
            Some(mask.get().to_string())
        })));
        SecureTextFieldController {
            password_transformation: InputTransformation::new(SharedPassword(password.clone())),
            password,
            revealing,
            hidden,
        }
    }
}

/// The password transformation the field runs, shared with the controller that hides it.
struct SharedPassword(Rc<PasswordInputTransformation>);

impl InputTransform for SharedPassword {
    fn transform_input(&self, buffer: &mut TextFieldBuffer) {
        self.0.transform_input(buffer);
    }
}

/// Marks a character typed alone to be shown, as Compose's `PasswordInputTransformation`: an edit
/// inserting one glyph (a grapheme cluster: an emoji of several characters is one), with nothing
/// selected after it, reveals it where the mode, and in `System` the platform for the keyboard
/// that typed it, says so; any other edit hides it. A new reveal starts the timer that hides it.
#[doc(hidden)]
pub struct PasswordInputTransformation {
    schedule_hide: Box<dyn Fn()>,
    mode: Box<dyn Fn() -> TextObfuscationMode>,
    visibility: Box<dyn Fn() -> PasswordVisibility>,
    /// Where the glyph shown starts, in bytes, as Compose's `revealCodepointIndex`.
    reveal: MutableState<Option<usize>>,
    _owner: OwnedState<Option<usize>>,
}

impl PasswordInputTransformation {
    /// One for tests: `schedule_hide` called at each new reveal.
    #[doc(hidden)]
    pub fn for_test(
        schedule_hide: impl Fn() + 'static,
        mode: TextObfuscationMode,
        visibility: PasswordVisibility,
    ) -> PasswordInputTransformation {
        let owner = mutable_state_of(None);
        PasswordInputTransformation {
            schedule_hide: Box::new(schedule_hide),
            mode: Box::new(move || mode),
            visibility: Box::new(move || visibility),
            reveal: owner.handle(),
            _owner: owner,
        }
    }

    /// Where the glyph shown starts, if one is.
    #[doc(hidden)]
    pub fn revealed(&self) -> Option<usize> {
        self.reveal.get()
    }

    /// Everything hidden again.
    fn hide(&self) {
        self.reveal.set(None);
    }

    #[doc(hidden)]
    pub fn transform_input(&self, buffer: &mut TextFieldBuffer) {
        let changes = buffer.changes();
        let inserted = (changes.change_count() == 1).then(|| changes.get_range(0));
        let from_hardware = changes.change_count() == 1 && changes.is_from_hardware_source(0);
        let one_glyph = inserted.is_some_and(|range| {
            !range.collapsed()
                && buffer.as_str()[range.start..range.end].graphemes(true).nth(1).is_none()
        });
        let Some(range) = inserted.filter(|_| one_glyph && !buffer.has_selection()) else {
            self.hide();
            return;
        };
        let reveals = match (self.mode)() {
            TextObfuscationMode::RevealLastTyped => true,
            TextObfuscationMode::System => {
                let visibility = (self.visibility)();
                if from_hardware { visibility.physical } else { visibility.touch }
            }
            TextObfuscationMode::Hidden | TextObfuscationMode::Visible => false,
        };
        if !reveals {
            self.hide();
            return;
        }
        if self.reveal.get() != Some(range.start) {
            (self.schedule_hide)();
            self.reveal.set(Some(range.start));
        }
    }
}

/// Replaces each glyph (a grapheme cluster) by what `transform` gives for its range, or keeps
/// it, as Compose's `OutputTransformationPerGlyph`.
#[doc(hidden)]
pub struct PerGlyph(pub Box<dyn Fn(TextRange) -> Option<String>>);

impl OutputTransform for PerGlyph {
    fn transform_output(&self, buffer: &mut TextFieldBuffer) {
        // From the end: a replacement leaves the ranges before it where they are.
        let ranges: Vec<_> = buffer
            .as_str()
            .grapheme_indices(true)
            .map(|(start, glyph)| TextRange::new(start, start + glyph.len()))
            .collect();
        for range in ranges.into_iter().rev() {
            if let Some(replacement) = (self.0)(range) {
                buffer.replace(range.start, range.end, &replacement);
            }
        }
    }
}

#[cfg(test)]
#[path = "secure_text_field_tests.rs"]
mod tests;
