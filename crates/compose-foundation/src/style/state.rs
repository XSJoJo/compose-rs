//! A styled component's state, as Compose's `StyleState.kt`: `StyleState`, `MutableStyleState`,
//! `StyleStateKey` and `rememberUpdatedStyleState`.

use std::any::Any;
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use compose_runtime::snapshots::Snapshot;
use compose_runtime::{ComposeArg, OwnedState, mutable_state_of, mutable_state_of_with};
use compose_ui::{
    FocusInteraction, HoverInteraction, Interaction, InteractionSource, PressInteraction,
    ToggleableState,
};

use super::property::Value;

const PRESSED: u32 = 1 << 0;
const HOVERED: u32 = 1 << 1;
const FOCUSED: u32 = 1 << 2;
const SELECTED: u32 = 1 << 3;
const ENABLED: u32 = 1 << 4;
const TOGGLE_SHIFT: u32 = 5;
const TOGGLE_OFF: u32 = 0;
const TOGGLE_ON: u32 = 1 << TOGGLE_SHIFT;
const TOGGLE_INDETERMINATE: u32 = 2 << TOGGLE_SHIFT;
const TOGGLE_MASK: u32 = 3 << TOGGLE_SHIFT;

/// What a key reads.
#[derive(Clone, Copy, PartialEq, Debug)]
enum KeyKind {
    /// A value of the state's own map.
    Custom,
    /// A predefined flag.
    Flag(u32),
    /// The predefined tri-state toggle.
    Toggle,
}

/// What a custom key does with an interaction its state's source emits, as an override of
/// `StyleStateKey.processInteraction`.
pub type ProcessInteraction = fn(&Interaction, &MutableStyleState);

/// A piece of a styled component's state, as Compose's `StyleStateKey<T>`: its default, and what
/// it does with the interactions the state's source emits (only once it's set in a state, as
/// Compose's, which asks the keys of its map). Declared as a `static` and identified by its name
/// and value type (D1584), as a composition local.
pub struct StyleStateKey<T: 'static> {
    name: &'static str,
    default_value: fn() -> T,
    kind: KeyKind,
    process_interaction: Option<ProcessInteraction>,
}

impl<T: 'static> StyleStateKey<T> {
    /// A key whose value is `default_value()` until set, as `StyleStateKey(defaultValue)`.
    pub const fn new(name: &'static str, default_value: fn() -> T) -> StyleStateKey<T> {
        StyleStateKey { name, default_value, kind: KeyKind::Custom, process_interaction: None }
    }

    /// A key that updates its state from the interactions its source emits (those that aren't
    /// a press, a hover or a focus), as a `StyleStateKey` overriding `processInteraction`.
    pub const fn with_interactions(
        name: &'static str,
        default_value: fn() -> T,
        process_interaction: ProcessInteraction,
    ) -> StyleStateKey<T> {
        StyleStateKey {
            name,
            default_value,
            kind: KeyKind::Custom,
            process_interaction: Some(process_interaction),
        }
    }

    /// Its name.
    pub fn name(&self) -> &'static str {
        self.name
    }

    fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = compose_runtime::KeyHasher::default();
        std::any::TypeId::of::<T>().hash(&mut hasher);
        compose_runtime::key_of(self.name) ^ hasher.finish().rotate_left(17)
    }
}

impl StyleStateKey<bool> {
    /// As `StyleStateKey.Pressed`.
    pub const PRESSED: StyleStateKey<bool> = StyleStateKey::flag("Pressed", PRESSED);
    /// As `StyleStateKey.Hovered`.
    pub const HOVERED: StyleStateKey<bool> = StyleStateKey::flag("Hovered", HOVERED);
    /// As `StyleStateKey.Focused`.
    pub const FOCUSED: StyleStateKey<bool> = StyleStateKey::flag("Focused", FOCUSED);
    /// As `StyleStateKey.Selected`.
    pub const SELECTED: StyleStateKey<bool> = StyleStateKey::flag("Selected", SELECTED);
    /// As `StyleStateKey.Enabled` (true until set).
    pub const ENABLED: StyleStateKey<bool> = StyleStateKey {
        name: "Enabled",
        default_value: || true,
        kind: KeyKind::Flag(ENABLED),
        process_interaction: None,
    };

    const fn flag(name: &'static str, mask: u32) -> StyleStateKey<bool> {
        StyleStateKey {
            name,
            default_value: || false,
            kind: KeyKind::Flag(mask),
            process_interaction: None,
        }
    }
}

impl StyleStateKey<ToggleableState> {
    /// As `StyleStateKey.Toggle`.
    pub const TOGGLE: StyleStateKey<ToggleableState> = StyleStateKey {
        name: "Toggle",
        default_value: || ToggleableState::Off,
        kind: KeyKind::Toggle,
        process_interaction: None,
    };
}

impl<T: 'static> fmt::Debug for StyleStateKey<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} style key", self.name)
    }
}

/// A custom key's value in a state.
#[derive(Clone)]
struct CustomEntry {
    key: u64,
    value: Value,
    process_interaction: Option<ProcessInteraction>,
}

struct StateInner {
    interaction_source: Option<InteractionSource>,
    /// The predefined flags, in one state, as Compose's `MutableStateFlagSet`.
    flags: OwnedState<u32>,
    /// The custom keys' values, as Compose's state map.
    custom: OwnedState<Vec<CustomEntry>>,
}

/// A styled component's state, which its style reads, as Compose's `StyleState`: pressed,
/// hovered, focused, selected, enabled, its toggle, and custom keys. A handle: cloned to share,
/// compared by identity. A [`MutableStyleState`] changes it.
#[derive(Clone)]
pub struct StyleState(Rc<StateInner>);

impl StyleState {
    /// Whether it's enabled, as `isEnabled` (true until set).
    pub fn is_enabled(&self) -> bool {
        self.flag(ENABLED)
    }

    /// As `isFocused`.
    pub fn is_focused(&self) -> bool {
        self.flag(FOCUSED)
    }

    /// As `isHovered`.
    pub fn is_hovered(&self) -> bool {
        self.flag(HOVERED)
    }

    /// As `isPressed`.
    pub fn is_pressed(&self) -> bool {
        self.flag(PRESSED)
    }

    /// As `isSelected`.
    pub fn is_selected(&self) -> bool {
        self.flag(SELECTED)
    }

    /// Whether its toggle is on, as `isChecked`.
    pub fn is_checked(&self) -> bool {
        self.tri_state_toggle() == ToggleableState::On
    }

    /// Its toggle, as `triStateToggle`.
    pub fn tri_state_toggle(&self) -> ToggleableState {
        match self.0.flags.get() & TOGGLE_MASK {
            TOGGLE_ON => ToggleableState::On,
            TOGGLE_OFF => ToggleableState::Off,
            _ => ToggleableState::Indeterminate,
        }
    }

    /// The value of `key`, as `get(key)`: its default until set.
    pub fn get<T: Clone + 'static>(&self, key: &StyleStateKey<T>) -> T {
        let value: Box<dyn Any> = match key.kind {
            KeyKind::Flag(mask) => Box::new(self.flag(mask)),
            KeyKind::Toggle => Box::new(self.tri_state_toggle()),
            KeyKind::Custom => {
                let id = key.key();
                let found = self.0.custom.with(|entries| {
                    entries.iter().find(|entry| entry.key == id).map(|entry| entry.value.clone())
                });
                match found.as_deref().and_then(|value| value.downcast_ref::<T>()) {
                    Some(value) => return value.clone(),
                    None => return (key.default_value)(),
                }
            }
        };
        match value.downcast::<T>() {
            Ok(value) => *value,
            Err(_) => (key.default_value)(),
        }
    }

    /// The source whose interactions set its pressed, hovered and focused flags (internal in
    /// Compose).
    pub(crate) fn interaction_source(&self) -> Option<&InteractionSource> {
        self.0.interaction_source.as_ref()
    }

    fn flag(&self, mask: u32) -> bool {
        self.0.flags.get() & mask != 0
    }

    /// Updates the flags from `source`'s interactions until the returned listener is dropped,
    /// as `processInteractions`: pressed, hovered and focused as long as one of each is under
    /// way (they start false; each written only by an interaction of its kind), the others given
    /// to the custom keys set in the state.
    pub(crate) fn listen(&self, source: &InteractionSource) -> Rc<compose_ui::Listener> {
        let state = MutableStyleState(self.clone());
        state.set_flags(PRESSED | HOVERED | FOCUSED, 0);
        let ongoing = RefCell::new(Ongoing::default());
        source.listen(move |interaction| {
            let mut ongoing = ongoing.borrow_mut();
            // Only the flag the interaction is about is written, as `processInteractions`: a
            // flag set by hand stays until an interaction of its kind (D1587).
            let (mask, on) = match interaction {
                Interaction::Press(PressInteraction::Press(press)) => {
                    ongoing.presses.push(*press);
                    (PRESSED, true)
                }
                Interaction::Press(
                    PressInteraction::Release(press) | PressInteraction::Cancel(press),
                ) => {
                    ongoing.presses.retain(|ongoing| ongoing != press);
                    (PRESSED, !ongoing.presses.is_empty())
                }
                Interaction::Hover(HoverInteraction::Enter(enter)) => {
                    ongoing.hovers.push(*enter);
                    (HOVERED, true)
                }
                Interaction::Hover(HoverInteraction::Exit(enter)) => {
                    ongoing.hovers.retain(|ongoing| ongoing != enter);
                    (HOVERED, !ongoing.hovers.is_empty())
                }
                Interaction::Focus(FocusInteraction::Focus(focus)) => {
                    ongoing.focuses.push(*focus);
                    (FOCUSED, true)
                }
                Interaction::Focus(FocusInteraction::Unfocus(focus)) => {
                    ongoing.focuses.retain(|ongoing| ongoing != focus);
                    (FOCUSED, !ongoing.focuses.is_empty())
                }
                _ => {
                    drop(ongoing);
                    let handlers: Vec<ProcessInteraction> = state.0.0.custom.with(|entries| {
                        entries.iter().filter_map(|entry| entry.process_interaction).collect()
                    });
                    for handler in handlers {
                        handler(interaction, &state);
                    }
                    return;
                }
            };
            drop(ongoing);
            state.set_flag(mask, on);
        })
    }
}

/// The interactions under way, by identity, as Compose's `InteractionSet`s.
#[derive(Default)]
struct Ongoing {
    presses: Vec<compose_ui::Press>,
    hovers: Vec<compose_ui::HoverEnter>,
    focuses: Vec<compose_ui::Focus>,
}

impl ComposeArg for StyleState {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for StyleState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StyleState")
            .field("flags", &format_args!("{:#b}", self.0.flags.get()))
            .finish()
    }
}

/// A [`StyleState`] its owner changes, as Compose's `MutableStyleState`: remember it (or use
/// [`remember_updated_style_state`]). It derefs to its state.
#[derive(Clone)]
pub struct MutableStyleState(StyleState);

impl MutableStyleState {
    /// A state enabled and nothing else, whose `interaction_source` (if any) sets its pressed,
    /// hovered and focused flags once a resolver node using it is attached, as
    /// `MutableStyleState(interactionSource)`.
    pub fn new(interaction_source: Option<InteractionSource>) -> MutableStyleState {
        MutableStyleState(StyleState(Rc::new(StateInner {
            interaction_source,
            flags: mutable_state_of(ENABLED),
            custom: mutable_state_of_with(Vec::new(), compose_runtime::NeverEqualPolicy),
        })))
    }

    /// The state it changes.
    pub fn state(&self) -> &StyleState {
        &self.0
    }

    /// As `isEnabled = value`.
    pub fn set_enabled(&self, value: bool) {
        self.set_flag(ENABLED, value);
    }

    /// As `isFocused = value`.
    pub fn set_focused(&self, value: bool) {
        self.set_flag(FOCUSED, value);
    }

    /// As `isHovered = value`.
    pub fn set_hovered(&self, value: bool) {
        self.set_flag(HOVERED, value);
    }

    /// As `isPressed = value`.
    pub fn set_pressed(&self, value: bool) {
        self.set_flag(PRESSED, value);
    }

    /// As `isSelected = value`.
    pub fn set_selected(&self, value: bool) {
        self.set_flag(SELECTED, value);
    }

    /// As `isChecked = value`: the toggle on or off.
    pub fn set_checked(&self, value: bool) {
        let toggle = if value { ToggleableState::On } else { ToggleableState::Off };
        self.set_tri_state_toggle(toggle);
    }

    /// As `triStateToggle = value`.
    pub fn set_tri_state_toggle(&self, value: ToggleableState) {
        let bits = match value {
            ToggleableState::On => TOGGLE_ON,
            ToggleableState::Off => TOGGLE_OFF,
            ToggleableState::Indeterminate => TOGGLE_INDETERMINATE,
        };
        self.set_flags(TOGGLE_MASK, bits);
    }

    /// Sets `key` to `value`, as `set(key, value)`.
    pub fn set<T: Clone + PartialEq + 'static>(&self, key: &StyleStateKey<T>, value: T) {
        match key.kind {
            KeyKind::Flag(mask) => {
                let value: &dyn Any = &value;
                self.set_flag(mask, value.downcast_ref::<bool>().copied().unwrap_or(false));
            }
            KeyKind::Toggle => {
                let value: &dyn Any = &value;
                if let Some(toggle) = value.downcast_ref::<ToggleableState>() {
                    self.set_tri_state_toggle(*toggle);
                }
            }
            KeyKind::Custom => {
                let id = key.key();
                let custom = &self.0.0.custom;
                let current = Snapshot::without_read_observation(|| {
                    custom.with(|entries| {
                        let found = entries.iter().find(|entry| entry.key == id);
                        found.map(|entry| entry.value.clone())
                    })
                });
                if current.as_deref().and_then(|current| current.downcast_ref::<T>())
                    == Some(&value)
                {
                    return;
                }
                let process_interaction = key.process_interaction;
                let entries = Snapshot::without_read_observation(|| custom.get());
                custom.set({
                    let mut entries = entries;
                    let value: Value = Rc::new(value);
                    match entries.iter_mut().find(|entry| entry.key == id) {
                        Some(entry) => entry.value = value,
                        None => entries.push(CustomEntry { key: id, value, process_interaction }),
                    }
                    entries
                });
            }
        }
    }

    /// Takes `key` out of the state (it reads its default again), as `remove(key)`.
    ///
    /// # Panics
    ///
    /// For a predefined key, as Compose's.
    pub fn remove<T: 'static>(&self, key: &StyleStateKey<T>) {
        assert!(key.kind == KeyKind::Custom, "Cannot remove an internal StyleStateKey");
        let id = key.key();
        let custom = &self.0.0.custom;
        let present = Snapshot::without_read_observation(|| {
            custom.with(|entries| entries.iter().any(|entry| entry.key == id))
        });
        if present {
            let entries = Snapshot::without_read_observation(|| custom.get());
            custom.set(entries.into_iter().filter(|entry| entry.key != id).collect());
        }
    }

    fn set_flag(&self, mask: u32, value: bool) {
        self.set_flags(mask, if value { mask } else { 0 });
    }

    /// The flags of `mask` set to `values`, written only if they change.
    fn set_flags(&self, mask: u32, values: u32) {
        let flags = &self.0.0.flags;
        let current = Snapshot::without_read_observation(|| flags.get());
        let new = (current & !mask) | values;
        if new != current {
            flags.set(new);
        }
    }
}

impl std::ops::Deref for MutableStyleState {
    type Target = StyleState;
    fn deref(&self) -> &StyleState {
        &self.0
    }
}

impl From<MutableStyleState> for StyleState {
    fn from(state: MutableStyleState) -> StyleState {
        state.0
    }
}

impl ComposeArg for MutableStyleState {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0.0, &other.0.0)
    }
}

impl fmt::Debug for MutableStyleState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A state remembered for `interaction_source` (a new one for another source), updated by
/// `block` on each composition, as `rememberUpdatedStyleState(interactionSource) { … }`.
pub fn remember_updated_style_state(
    interaction_source: Option<&InteractionSource>,
    block: impl FnOnce(&MutableStyleState),
) -> StyleState {
    let source = interaction_source.cloned();
    let state =
        compose_runtime::remember_keyed(source.clone(), || MutableStyleState::new(source.clone()));
    block(&state);
    state.0.clone()
}
