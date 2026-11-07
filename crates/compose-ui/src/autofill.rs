//! Autofill, as Compose's `androidx.compose.ui.autofill` (D1680): what a node says an autofill
//! service may fill it with (`ContentType`, its hints), the kind and value it holds
//! (`ContentDataType`, `FillableData`), the semantics carrying them, and `AutofillManager`
//! (`LOCAL_AUTOFILL_MANAGER`), with what Compose's `AndroidAutofillManager` does for the platform:
//! telling it of the autofillable nodes as they come, change, take the focus and go, the virtual
//! structure it asks for, and filling the nodes with its values.
//!
//! The platform side is a host's: it installs a [`PlatformAutofillManager`]
//! (`UiTree::set_platform_autofill_manager`) and passes on what it asks (`UiHost::perform_autofill`,
//! `UiTree::populate_autofill_structure`). Android's does (compose-android); desktop has none, as
//! Compose Desktop, and `LOCAL_AUTOFILL_MANAGER` is then `None`. Without one, the tree pays a
//! check per frame.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::BuildHasherDefault;
use std::rc::Rc;

use smallvec::SmallVec;

use compose_runtime::CompositionLocal;

use crate::modifier::Modifier;
use crate::node_chain::NodeKinds;
use crate::platform_services::of_composing_tree;
use crate::semantics::{
    OwnSemantics, PART_OF_PARENT, PARTS_HOST, Role, SemanticsActions, SemanticsConfiguration,
    SemanticsProperties, ToggleableState,
};
use crate::text::SharedString;
use crate::tree::{NodeId, UiTree};
use crate::unit::IntRect;

// ---- Content types -----------------------------------------------------------------------------

/// What a node holds, for an autofill service to fill it with, as Compose's `ContentType`: a set
/// of Android's autofill hints (`androidx.autofill.HintConstants`). The predefined ones are
/// constants; [`ContentType::new`] takes any hint, and [`plus`](ContentType::plus) combines them.
#[derive(Clone)]
pub struct ContentType {
    hints: Hints,
}

#[derive(Clone)]
enum Hints {
    One(SharedString),
    Many(Rc<[SharedString]>),
}

impl ContentType {
    const fn of(hint: &'static str) -> ContentType {
        ContentType { hints: Hints::One(SharedString::from_static(hint)) }
    }

    pub const EMAIL_ADDRESS: ContentType = ContentType::of("emailAddress");
    pub const USERNAME: ContentType = ContentType::of("username");
    pub const PASSWORD: ContentType = ContentType::of("password");
    pub const NEW_USERNAME: ContentType = ContentType::of("newUsername");
    pub const NEW_PASSWORD: ContentType = ContentType::of("newPassword");
    pub const POSTAL_ADDRESS: ContentType = ContentType::of("postalAddress");
    pub const POSTAL_CODE: ContentType = ContentType::of("postalCode");
    pub const CREDIT_CARD_NUMBER: ContentType = ContentType::of("creditCardNumber");
    pub const CREDIT_CARD_SECURITY_CODE: ContentType = ContentType::of("creditCardSecurityCode");
    pub const CREDIT_CARD_EXPIRATION_DATE: ContentType =
        ContentType::of("creditCardExpirationDate");
    pub const CREDIT_CARD_EXPIRATION_MONTH: ContentType =
        ContentType::of("creditCardExpirationMonth");
    pub const CREDIT_CARD_EXPIRATION_YEAR: ContentType =
        ContentType::of("creditCardExpirationYear");
    pub const CREDIT_CARD_EXPIRATION_DAY: ContentType = ContentType::of("creditCardExpirationDay");
    pub const ADDRESS_COUNTRY: ContentType = ContentType::of("addressCountry");
    pub const ADDRESS_REGION: ContentType = ContentType::of("addressRegion");
    pub const ADDRESS_LOCALITY: ContentType = ContentType::of("addressLocality");
    pub const ADDRESS_STREET: ContentType = ContentType::of("streetAddress");
    pub const ADDRESS_AUXILIARY_DETAILS: ContentType = ContentType::of("extendedAddress");
    pub const POSTAL_CODE_EXTENDED: ContentType = ContentType::of("extendedPostalCode");
    pub const PERSON_FULL_NAME: ContentType = ContentType::of("personName");
    pub const PERSON_FIRST_NAME: ContentType = ContentType::of("personGivenName");
    pub const PERSON_LAST_NAME: ContentType = ContentType::of("personFamilyName");
    pub const PERSON_MIDDLE_NAME: ContentType = ContentType::of("personMiddleName");
    pub const PERSON_MIDDLE_INITIAL: ContentType = ContentType::of("personMiddleInitial");
    pub const PERSON_NAME_PREFIX: ContentType = ContentType::of("personNamePrefix");
    pub const PERSON_NAME_SUFFIX: ContentType = ContentType::of("personNameSuffix");
    pub const PHONE_NUMBER: ContentType = ContentType::of("phoneNumber");
    pub const PHONE_NUMBER_DEVICE: ContentType = ContentType::of("phoneNumberDevice");
    pub const PHONE_COUNTRY_CODE: ContentType = ContentType::of("phoneCountryCode");
    pub const PHONE_NUMBER_NATIONAL: ContentType = ContentType::of("phoneNational");
    pub const GENDER: ContentType = ContentType::of("gender");
    pub const BIRTH_DATE_FULL: ContentType = ContentType::of("birthDateFull");
    pub const BIRTH_DATE_DAY: ContentType = ContentType::of("birthDateDay");
    pub const BIRTH_DATE_MONTH: ContentType = ContentType::of("birthDateMonth");
    pub const BIRTH_DATE_YEAR: ContentType = ContentType::of("birthDateYear");
    pub const SMS_OTP_CODE: ContentType = ContentType::of("smsOTPCode");

    /// A content type of one hint, any string the platform's services know, as
    /// `ContentType(contentHint)`.
    pub fn new(content_hint: impl Into<SharedString>) -> ContentType {
        ContentType { hints: Hints::One(content_hint.into()) }
    }

    /// Both content types' hints, this one's first, each once, as `plus`.
    pub fn plus(&self, other: &ContentType) -> ContentType {
        let mut hints: Vec<SharedString> = self.content_hints().to_vec();
        for hint in other.content_hints() {
            if !hints.contains(hint) {
                hints.push(hint.clone());
            }
        }
        if hints.len() == 1 {
            return ContentType { hints: Hints::One(hints.pop().expect("one")) };
        }
        ContentType { hints: Hints::Many(hints.into()) }
    }

    /// Its hints, as Android's `contentHints`.
    pub fn content_hints(&self) -> &[SharedString] {
        match &self.hints {
            Hints::One(hint) => std::slice::from_ref(hint),
            Hints::Many(hints) => hints,
        }
    }
}

impl std::ops::Add for ContentType {
    type Output = ContentType;
    fn add(self, other: ContentType) -> ContentType {
        self.plus(&other)
    }
}

/// Equal with the same hints, in any order (Compose's are sets).
impl PartialEq for ContentType {
    fn eq(&self, other: &Self) -> bool {
        let (mine, theirs) = (self.content_hints(), other.content_hints());
        mine.len() == theirs.len() && mine.iter().all(|hint| theirs.contains(hint))
    }
}

impl Eq for ContentType {}

impl fmt::Debug for ContentType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ContentType")?;
        formatter.debug_list().entries(self.content_hints()).finish()
    }
}

impl compose_runtime::ComposeArg for ContentType {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// The kind of value a node holds for autofill, as Compose's `ContentDataType`: one of Android's
/// `View.AUTOFILL_TYPE_*`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentDataType(i32);

impl ContentDataType {
    /// Not autofillable: Android's `AUTOFILL_TYPE_NONE`.
    pub const NONE: ContentDataType = ContentDataType(0);
    pub const TEXT: ContentDataType = ContentDataType(1);
    pub const TOGGLE: ContentDataType = ContentDataType(2);
    pub const LIST: ContentDataType = ContentDataType(3);
    pub const DATE: ContentDataType = ContentDataType(4);

    /// One of the platform's types, as `ContentDataType(dataType)`.
    pub const fn new(data_type: i32) -> ContentDataType {
        ContentDataType(data_type)
    }

    /// The platform's type, as `dataType`.
    pub const fn data_type(self) -> i32 {
        self.0
    }
}

impl fmt::Debug for ContentDataType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ContentDataType::NONE => formatter.write_str("None"),
            ContentDataType::TEXT => formatter.write_str("Text"),
            ContentDataType::TOGGLE => formatter.write_str("Toggle"),
            ContentDataType::LIST => formatter.write_str("List"),
            ContentDataType::DATE => formatter.write_str("Date"),
            ContentDataType(other) => write!(formatter, "ContentDataType({other})"),
        }
    }
}

// ---- Values ------------------------------------------------------------------------------------

/// The most UTF-16 units of text a node's autofill value holds, as Compose's
/// `MAX_AUTOFILL_TEXT_LENGTH`: more crashes the platform's service.
pub const MAX_AUTOFILL_TEXT_LENGTH: usize = 5000;

/// `text` kept to `MAX_AUTOFILL_TEXT_LENGTH` UTF-16 units, not splitting a character (a surrogate
/// pair), as Compose's `trimToSafeLength`.
pub fn trim_to_safe_length(text: &str) -> &str {
    // A byte is at most one unit: short texts fit.
    if text.len() <= MAX_AUTOFILL_TEXT_LENGTH {
        return text;
    }
    let mut units = 0;
    for (index, character) in text.char_indices() {
        units += character.len_utf16();
        if units > MAX_AUTOFILL_TEXT_LENGTH {
            return &text[..index];
        }
    }
    text
}

/// A value an autofill service fills a node with, or the node holds, as Compose's `FillableData`
/// over Android's `AutofillValue`: text, a toggle, a list's index or a date (D1680).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FillableData(Value);

#[derive(Clone, PartialEq, Eq, Debug)]
enum Value {
    Text(SharedString),
    Toggle(bool),
    List(i32),
    Date(i64),
}

impl FillableData {
    /// Text, kept to `MAX_AUTOFILL_TEXT_LENGTH`, as `createFromText`. `Option` as Compose's
    /// (`null` before Android 8): always a value here.
    pub fn create_from_text(text: impl Into<SharedString>) -> Option<FillableData> {
        let text: SharedString = text.into();
        let kept = trim_to_safe_length(text.as_str()).len();
        let text = if kept == text.as_str().len() {
            text
        } else {
            SharedString::from(text.as_str()[..kept].to_string())
        };
        Some(FillableData(Value::Text(text)))
    }

    /// As `createFromBoolean`.
    pub fn create_from_boolean(value: bool) -> Option<FillableData> {
        Some(FillableData(Value::Toggle(value)))
    }

    /// As `createFromListIndex`.
    pub fn create_from_list_index(index: i32) -> Option<FillableData> {
        Some(FillableData(Value::List(index)))
    }

    /// As `createFromDateMillis`.
    pub fn create_from_date_millis(millis: i64) -> Option<FillableData> {
        Some(FillableData(Value::Date(millis)))
    }

    /// A platform's text as it gave it, not trimmed (`createFromAutofillValue`): for hosts.
    #[doc(hidden)]
    pub fn from_platform_text(text: impl Into<SharedString>) -> FillableData {
        FillableData(Value::Text(text.into()))
    }

    /// As `textValue`.
    pub fn text_value(&self) -> Option<&SharedString> {
        match &self.0 {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }

    /// As `booleanValue`.
    pub fn boolean_value(&self) -> Option<bool> {
        match self.0 {
            Value::Toggle(value) => Some(value),
            _ => None,
        }
    }

    /// As `listIndexValue`.
    pub fn list_index_value(&self) -> Option<i32> {
        match self.0 {
            Value::List(index) => Some(index),
            _ => None,
        }
    }

    /// As `getListIndexOrDefault`.
    pub fn get_list_index_or_default(&self, default_value: i32) -> i32 {
        self.list_index_value().unwrap_or(default_value)
    }

    /// As `dateMillisValue`.
    pub fn date_millis_value(&self) -> Option<i64> {
        match self.0 {
            Value::Date(millis) => Some(millis),
            _ => None,
        }
    }

    /// As `getDateMillisOrDefault`.
    pub fn get_date_millis_or_default(&self, default_value: i64) -> i64 {
        self.date_millis_value().unwrap_or(default_value)
    }

    /// The platform's type of the value (`AutofillValue.isText`…), for hosts.
    pub fn data_type(&self) -> ContentDataType {
        match self.0 {
            Value::Text(_) => ContentDataType::TEXT,
            Value::Toggle(_) => ContentDataType::TOGGLE,
            Value::List(_) => ContentDataType::LIST,
            Value::Date(_) => ContentDataType::DATE,
        }
    }
}

// ---- The manager -------------------------------------------------------------------------------

/// The platform's autofill, as Compose's `AutofillManager`: `LOCAL_AUTOFILL_MANAGER`.
pub trait AutofillManager {
    /// Ends the autofill session, telling the service to save what was entered (a form
    /// submitted), as `commit`.
    fn commit(&self);
    /// Ends the autofill session without saving (a form left), as `cancel`.
    fn cancel(&self);
}

/// The tree's autofill manager, as Compose's `LocalAutofillManager`: the host's
/// (`UiTree::set_platform_autofill_manager`), `None` where there's no autofill (desktop, a tree
/// without a platform).
pub static LOCAL_AUTOFILL_MANAGER: CompositionLocal<Option<Rc<dyn AutofillManager>>> =
    CompositionLocal::new("LocalAutofillManager", || {
        of_composing_tree(|tree| tree.autofill_manager()).flatten()
    });

/// What the tree tells the platform's autofill, as Compose's `PlatformAutofillManager`'s calls
/// (the view implied). Ids are semantics ids; bounds are in the window, in pixels.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum PlatformAutofillCall {
    /// `notifyViewEntered`: an autofillable node took the focus.
    NotifyViewEntered {
        semantics_id: i32,
        bounds: IntRect,
    },
    /// `notifyViewExited`: an autofillable node lost it.
    NotifyViewExited {
        semantics_id: i32,
    },
    /// `notifyValueChanged`.
    NotifyValueChanged {
        semantics_id: i32,
        value: FillableData,
    },
    /// `notifyViewVisibilityChanged`.
    NotifyViewVisibilityChanged {
        semantics_id: i32,
        is_visible: bool,
    },
    Commit,
    Cancel,
    /// `requestAutofill`: a node asked for the service (a text field's toolbar's "Autofill").
    RequestAutofill {
        semantics_id: i32,
        bounds: IntRect,
    },
}

/// The platform's autofill as a host gives it, as Compose's `PlatformAutofillManager` over
/// Android's `AutofillManager`. Told outside the tree's passes.
pub trait PlatformAutofillManager {
    fn call(&self, call: PlatformAutofillCall);
}

/// `LOCAL_AUTOFILL_MANAGER`'s value: commit and cancel go to the platform.
struct PlatformAutofill(Rc<dyn PlatformAutofillManager>);

impl AutofillManager for PlatformAutofill {
    fn commit(&self) {
        self.0.call(PlatformAutofillCall::Commit);
    }
    fn cancel(&self) {
        self.0.call(PlatformAutofillCall::Cancel);
    }
}

/// What the autofill bookkeeping reads of a node's semantics, as Compose's `AndroidAutofillManager`
/// compares a node's configuration with its last.
#[derive(Clone, PartialEq, Default, Debug)]
struct Snapshot {
    content_data_type: Option<ContentDataType>,
    input_text: Option<SharedString>,
    toggleable_state: Option<ToggleableState>,
    fillable_data: Option<FillableData>,
    /// It has a content type: it counts for the commit when every such node is gone
    /// (`isRelatedToAutoCommit`).
    has_content_type: bool,
    /// It fills (`OnFillData`): with its data type, whether it's autofillable, for the focus's
    /// moves (`isAutofillable`).
    fills: bool,
}

impl Snapshot {
    fn of(config: &SemanticsConfiguration) -> Snapshot {
        Snapshot {
            content_data_type: config.get(&SemanticsProperties::CONTENT_DATA_TYPE).copied(),
            input_text: config.get(&SemanticsProperties::INPUT_TEXT).cloned(),
            toggleable_state: config.get(&SemanticsProperties::TOGGLEABLE_STATE).copied(),
            fillable_data: config.get(&SemanticsProperties::FILLABLE_DATA).cloned(),
            has_content_type: config.contains(&SemanticsProperties::CONTENT_TYPE),
            fills: config.contains(&SemanticsActions::ON_FILL_DATA),
        }
    }

    /// A part's values over its host's, as `SemanticsConfiguration::apply`.
    fn apply(&mut self, part: &Snapshot) {
        if part.content_data_type.is_some() {
            self.content_data_type = part.content_data_type;
        }
        if part.input_text.is_some() {
            self.input_text = part.input_text.clone();
        }
        if part.toggleable_state.is_some() {
            self.toggleable_state = part.toggleable_state;
        }
        if part.fillable_data.is_some() {
            self.fillable_data = part.fillable_data.clone();
        }
        self.has_content_type |= part.has_content_type;
        self.fills |= part.fills;
    }

    fn is_empty(&self) -> bool {
        *self == Snapshot::default()
    }

    /// Compose's `isAutofillable`: it fills, and its data type isn't `NONE`.
    fn is_autofillable(&self) -> bool {
        self.fills && self.content_data_type != Some(ContentDataType::NONE)
    }
}

/// Whether two configurations hold the same for autofill: what it reads, and whether each is a
/// part or a parts' host (`apply_autofill_semantics` against `apply_semantics`, checked in tests).
pub(crate) fn same_for_autofill(a: &SemanticsConfiguration, b: &SemanticsConfiguration) -> bool {
    Snapshot::of(a) == Snapshot::of(b)
        && a.contains(&PART_OF_PARENT) == b.contains(&PART_OF_PARENT)
        && a.contains(&PARTS_HOST) == b.contains(&PARTS_HOST)
}

/// Whether `config` has something the bookkeeping reads (`Snapshot`): a node measured again is
/// looked at only then, and a node noted told of only then (D1688).
fn is_autofill_related(config: &SemanticsConfiguration) -> bool {
    config.contains(&SemanticsProperties::CONTENT_DATA_TYPE)
        || config.contains(&SemanticsProperties::INPUT_TEXT)
        || config.contains(&SemanticsProperties::TOGGLEABLE_STATE)
        || config.contains(&SemanticsProperties::FILLABLE_DATA)
        || config.contains(&SemanticsProperties::CONTENT_TYPE)
        || config.contains(&SemanticsActions::ON_FILL_DATA)
}

type NodeMap<V> = HashMap<NodeId, V, BuildHasherDefault<compose_runtime::KeyHasher>>;

/// The platform's autofill a host installed (D1680), and what's tracked for it while it's
/// enabled (D1688).
pub(crate) struct AutofillTracking {
    platform: Rc<dyn PlatformAutofillManager>,
    manager: Rc<dyn AutofillManager>,
    /// `None` while the platform's autofill is off (`set_platform_autofill_enabled`).
    live: Option<Live>,
}

/// What the platform was told, and what may have changed since, noted as it happened (D1688):
/// Compose hears of each node's change, attach and detach; compose-rs notes the node, and the
/// sync after the frame looks at the nodes noted only.
#[derive(Default)]
struct Live {
    /// The nodes whose semantics were forgotten since the last sync (invalidated, a chain changed,
    /// attached, detached, deactivated, reused; measured, if what they had was autofill's), each
    /// maybe more than once.
    dirty: Vec<NodeId>,
    /// Those attached since (or reused, with a new id): `onPostAttach`.
    attached: Vec<NodeId>,
    /// Every node in use is to be looked at: installed or enabled since.
    scan: bool,
    /// The focus's moves since the last sync, in order.
    focus_moves: Vec<FocusMove>,
    /// The nodes that aren't parts with a snapshot not empty at the last sync: their ids then and
    /// snapshots (a part's taken into its owner's).
    known: NodeMap<(i32, Snapshot)>,
    /// The parts (`PART_OF_PARENT`) in use at the last sync: their owners and own snapshots.
    parts: NodeMap<Part>,
    /// The nodes with a content type on screen, as Compose's `currentlyDisplayedIDs`.
    displayed: HashSet<i32>,
    /// Some were displayed since the last commit, as `pendingAutofillCommit`.
    pending_commit: bool,
    /// How many noted nodes the syncs looked at, for tests.
    looked_at: usize,
    /// The sync's owners to look at, kept for its capacity.
    owners: Vec<NodeId>,
    /// Those of them found in use as their parts were routed, in this sync.
    in_use: Vec<NodeId>,
    /// The owners with parts: their own snapshots, for a sync where only a part was noted (a
    /// field's text at each key: its host's semantics not computed again).
    hosts: NodeMap<Snapshot>,
}

impl Live {
    fn new() -> Live {
        Live { scan: true, ..Live::default() }
    }

    /// The focus moved (`note_autofill_focus`).
    fn note_focus(&mut self, previous: Option<NodeId>, current: Option<NodeId>) {
        if let Some(previous) = previous {
            // What the last sync knew of it: its owner's id and whether it was autofillable.
            let known = match self.parts.get(&previous) {
                Some(part) => Some(part.owner.and_then(|owner| self.known.get(&owner))),
                None => self.known.get(&previous).map(Some),
            };
            match known {
                Some(Some((id, snapshot))) => {
                    if snapshot.is_autofillable() {
                        self.focus_moves.push(FocusMove::Exited(*id));
                    }
                }
                // A part of no owner, or of one not autofillable.
                Some(None) => {}
                // Not known: looked at at the sync if it may have changed since.
                None => {
                    if self.scan || self.dirty.contains(&previous) {
                        self.focus_moves.push(FocusMove::ExitedIfAutofillable(previous));
                    }
                }
            }
        }
        if let Some(current) = current {
            self.focus_moves.push(FocusMove::Entered(current));
        }
    }
}

/// A part known at the last sync.
struct Part {
    /// The node whose semantics it's part of (`part_owner`).
    owner: Option<NodeId>,
    /// Its semantics id, for its order among its owner's parts.
    id: i32,
    snapshot: Snapshot,
}

/// A move of the focus, as `onFocusChanged` hears it.
enum FocusMove {
    /// A node autofillable at the last sync lost the focus: its id, found as it lost it (it may
    /// be gone by the sync).
    Exited(i32),
    /// A node not looked at since it changed lost it: told at the sync if it's autofillable then.
    ExitedIfAutofillable(NodeId),
    /// A node took it.
    Entered(NodeId),
}

impl UiTree {
    /// Installs the platform's autofill (`None`: none, as on desktop): `LOCAL_AUTOFILL_MANAGER`'s
    /// value, told of the autofillable nodes from the next frame on. Before composing.
    pub fn set_platform_autofill_manager(
        &mut self,
        platform: Option<Rc<dyn PlatformAutofillManager>>,
    ) {
        self.autofill = platform.map(|platform| {
            Box::new(AutofillTracking {
                manager: Rc::new(PlatformAutofill(platform.clone())),
                platform,
                live: Some(Live::new()),
            })
        });
    }

    /// Says whether the platform's autofill is on (Android's `AutofillManager.isEnabled`; on
    /// when a manager is installed). While it's off nothing is tracked nor told, and frames pay
    /// nothing for it; the manager stays `LOCAL_AUTOFILL_MANAGER`'s value and the platform has
    /// autofill still (a field's menu offers it). On again, it's told of what's there, as when
    /// installed (D1688).
    pub fn set_platform_autofill_enabled(&mut self, enabled: bool) {
        let Some(autofill) = &mut self.autofill else { return };
        if enabled != autofill.live.is_some() {
            autofill.live = enabled.then(Live::new);
        }
    }

    /// Whether a platform's autofill is installed and on.
    pub fn is_platform_autofill_enabled(&self) -> bool {
        self.autofill.as_ref().is_some_and(|autofill| autofill.live.is_some())
    }

    /// `LOCAL_AUTOFILL_MANAGER`'s value: the platform's, if installed.
    pub fn autofill_manager(&self) -> Option<Rc<dyn AutofillManager>> {
        self.autofill.as_ref().map(|tracking| tracking.manager.clone())
    }

    /// Says the platform has autofill, as Compose's `isAutofillAvailable` (Android 8 and up): a
    /// text field's toolbar and menu offer it then. Installing a manager says so too; a test
    /// rule says so without one, as Compose's device tests run where autofill is (D1682).
    pub fn set_autofill_available(&mut self, available: bool) {
        self.autofill_available = available;
    }

    /// Whether the platform has autofill (`set_autofill_available`, or a manager installed).
    pub fn is_autofill_available(&self) -> bool {
        self.autofill_available || self.autofill.is_some()
    }

    /// What's tracked, if a platform's autofill is installed and on.
    #[inline]
    fn autofill_live(&mut self) -> Option<&mut Live> {
        self.autofill.as_deref_mut()?.live.as_mut()
    }

    /// `node`'s semantics were forgotten: looked at at the next sync, if tracked (D1688).
    #[inline]
    pub(crate) fn note_autofill_dirty(&mut self, node: NodeId) {
        if let Some(live) = self.autofill_live() {
            live.dirty.push(node);
        }
    }

    /// `node` was attached: looked at at the next sync, which tells its visibility as
    /// `onPostAttach` (D1688).
    #[inline]
    pub(crate) fn note_autofill_attached(&mut self, node: NodeId) {
        // A node without semantics has nothing autofill reads.
        if self.autofill.is_some()
            && self.get(node).is_some_and(|value| value.kinds.any(NodeKinds::SEMANTICS))
        {
            self.note_autofill_reused(node);
        }
    }

    /// `node` was reused for new content, with a new id: looked at at the next sync, as attached
    /// (`onPostLayoutNodeReused`), whatever its content (its new modifiers are to come).
    #[inline]
    pub(crate) fn note_autofill_reused(&mut self, node: NodeId) {
        if let Some(live) = self.autofill_live() {
            live.dirty.push(node);
            live.attached.push(node);
        }
    }

    /// `node` was measured, its kept semantics `dropped`: looked at at the next sync only if they
    /// had something the bookkeeping reads (a field's text, a toggle's state). Compose doesn't
    /// compute semantics again for a measurement; compose-rs forgets them (D1257), and what
    /// autofill reads changes with one only where it was there (D1688).
    #[inline]
    pub(crate) fn note_autofill_measured(&mut self, node: NodeId, dropped: &OwnSemantics) {
        if let Some(live) = self.autofill_live()
            && is_autofill_related(&dropped.config)
        {
            live.dirty.push(node);
        }
    }

    /// The focus moved from one target's layout node to another's (`activeFocusTargetNode`'s
    /// setter telling its listeners). The node losing it is resolved now, from what the last
    /// sync knew of it, as Compose reads its configuration then: it may be gone by the sync (a
    /// focused field removed, D1688). The node taking it is told at the sync, with its bounds.
    #[inline]
    pub(crate) fn note_autofill_focus(
        &mut self,
        previous: Option<NodeId>,
        current: Option<NodeId>,
    ) {
        if let Some(live) = self.autofill_live() {
            live.note_focus(previous, current);
        }
    }

    /// The node whose semantics `node`'s are, for autofill: `node`, or for a part
    /// (`PART_OF_PARENT`) the node it belongs to (D2003). `None` if it's not in use.
    fn autofill_owner(&mut self, node: NodeId) -> Option<NodeId> {
        let value = self.get(node)?;
        if !value.attached || self.is_deactivated(node) {
            return None;
        }
        let is_part =
            self.own_semantics(node).is_some_and(|own| own.config.contains(&PART_OF_PARENT));
        if !is_part {
            return Some(node);
        }
        self.part_owner(node)
    }

    /// The node part `node` belongs to: the nearest host above, else the nearest node with
    /// semantics, short of an overlay's root.
    fn part_owner(&mut self, node: NodeId) -> Option<NodeId> {
        let mut nearest = None;
        let mut at = node;
        loop {
            let value = self.node(at);
            match value.parent {
                Some(parent) if !value.overlay => at = parent,
                _ => return nearest,
            }
            let Some(own) = self.own_semantics(at) else { continue };
            if own.config.contains(&PARTS_HOST) {
                return Some(at);
            }
            if nearest.is_none() && !own.config.is_empty() && !own.config.contains(&PART_OF_PARENT)
            {
                nearest = Some(at);
            }
        }
    }

    /// `node`'s bounds for the platform, as Compose's `RectManager.withRect`: its outer bounds in
    /// the window, unclipped; `None` while it's not placed.
    pub(crate) fn autofill_bounds(&mut self, node: NodeId) -> Option<IntRect> {
        if !self.get(node)?.placed {
            return None;
        }
        let rect = self.outer_bounds_in_root(node);
        Some(IntRect {
            left: rect.left.round() as i32,
            top: rect.top.round() as i32,
            right: rect.right.round() as i32,
            bottom: rect.bottom.round() as i32,
        })
    }

    /// A node asked for the platform's autofill, as `DelegatableNode.requestAutofill`: told
    /// once the tree is free.
    pub(crate) fn request_autofill_of(&mut self, node: NodeId) {
        let Some(platform) = self.autofill.as_ref().map(|tracking| tracking.platform.clone())
        else {
            return;
        };
        // Its owner as the tracking knows it: the node asking runs its own code, out of its
        // chain, so its semantics can't be computed now. While the platform's autofill is off,
        // nothing (the platform would drop it).
        let Some(live) = self.autofill.as_ref().and_then(|autofill| autofill.live.as_ref()) else {
            return;
        };
        let owner = match live.parts.get(&node) {
            Some(part) => part.owner,
            None => live.known.contains_key(&node).then_some(node),
        };
        let Some(owner) = owner else { return };
        let Some(bounds) = self.autofill_bounds(owner) else { return };
        let semantics_id = self.node(owner).semantics_id;
        self.pending_actions.push(crate::input::Action::Run(Box::new(move || {
            platform.call(PlatformAutofillCall::RequestAutofill { semantics_id, bounds });
        })));
    }

    /// Whether the platform's autofill has something to hear of (a node noted, the focus moved):
    /// a frame with nothing of the sort syncs nothing (D1688).
    #[doc(hidden)]
    pub fn autofill_pending(&self) -> bool {
        self.autofill
            .as_ref()
            .and_then(|autofill| autofill.live.as_ref())
            .is_some_and(|live| live.scan || !live.dirty.is_empty() || !live.focus_moves.is_empty())
    }

    /// What the platform's autofill is told of since the last sync, as Compose's
    /// `AndroidAutofillManager` hears of each change: the focus's moves, the nodes noted that
    /// came, changed, were deactivated or went, then the commit once every node with a content
    /// type is gone (`onEndApplyChanges`). Only the nodes noted since are looked at (D1688).
    #[doc(hidden)]
    pub fn sync_autofill(&mut self) -> Vec<PlatformAutofillCall> {
        let Some(mut live) = self.autofill.as_deref_mut().and_then(|autofill| autofill.live.take())
        else {
            return Vec::new();
        };
        let mut calls = Vec::new();
        self.sync_autofill_live(&mut live, &mut calls);
        if let Some(autofill) = self.autofill.as_deref_mut() {
            autofill.live = Some(live);
        }
        calls
    }

    /// How many noted nodes the syncs looked at since the platform's autofill was installed or
    /// turned on: what the frames cost it. For tests.
    #[doc(hidden)]
    pub fn autofill_nodes_looked_at(&self) -> usize {
        let live = self.autofill.as_ref().and_then(|autofill| autofill.live.as_ref());
        live.map_or(0, |live| live.looked_at)
    }

    /// The platform's manager, if installed, and what `sync_autofill` tells it.
    #[doc(hidden)]
    pub fn take_autofill_sync(
        &mut self,
    ) -> Option<(Rc<dyn PlatformAutofillManager>, Vec<PlatformAutofillCall>)> {
        let platform = self.autofill.as_ref()?.platform.clone();
        Some((platform, self.sync_autofill()))
    }

    fn sync_autofill_live(&mut self, live: &mut Live, calls: &mut Vec<PlatformAutofillCall>) {
        if std::mem::take(&mut live.scan) {
            self.autofill_scan(live);
        }
        // Its buffers, back with their capacity at the end: a frame allocates nothing for them.
        let mut dirty = std::mem::take(&mut live.dirty);
        dirty.sort_unstable_by_key(|node| node.to_bits());
        dirty.dedup();
        let mut attached = std::mem::take(&mut live.attached);
        attached.sort_unstable_by_key(|node| node.to_bits());
        attached.dedup();
        // The owners to look at: each node noted, a part's owner for a part.
        let mut owners = std::mem::take(&mut live.owners);
        live.looked_at += dirty.len();
        for &node in &dirty {
            let newly = attached.binary_search_by_key(&node.to_bits(), |each| each.to_bits());
            self.route_autofill_node(live, node, newly.is_ok(), &mut owners);
        }
        owners.sort_unstable_by_key(|node| node.to_bits());
        owners.dedup();
        let newly = |node: &NodeId| {
            attached.binary_search_by_key(&node.to_bits(), |each| each.to_bits()).is_ok()
        };
        // A host noted, not just its parts: its own semantics looked at again.
        let noted = |node: &NodeId| {
            dirty.binary_search_by_key(&node.to_bits(), |each| each.to_bits()).is_ok()
        };
        // Compose's `onFocusChanged`, each move as it came. A node taking the focus that came
        // since is looked at first: Compose shows it (`onPostAttach`) before anything can focus
        // it.
        let mut gone = Vec::new();
        for focus_move in std::mem::take(&mut live.focus_moves) {
            match focus_move {
                FocusMove::Exited(id) => {
                    calls.push(PlatformAutofillCall::NotifyViewExited { semantics_id: id })
                }
                FocusMove::ExitedIfAutofillable(node) => {
                    if let Some((_, id)) = self.autofillable_owner(node) {
                        calls.push(PlatformAutofillCall::NotifyViewExited { semantics_id: id });
                    }
                }
                FocusMove::Entered(node) => {
                    // Its owner as the tracking knows it, the parts in use all routed by now.
                    let owner = match live.parts.get(&node) {
                        Some(part) => part.owner,
                        None => Some(node),
                    };
                    let Some(owner) = owner else { continue };
                    if let Ok(index) =
                        owners.binary_search_by_key(&owner.to_bits(), |each| each.to_bits())
                    {
                        owners.remove(index);
                        self.sync_autofill_owner(
                            live,
                            owner,
                            newly(&owner),
                            noted(&owner),
                            calls,
                            &mut gone,
                        );
                    }
                    let id = self.get(owner).map(|value| value.semantics_id);
                    if let Some((known_id, snapshot)) = live.known.get(&owner)
                        && Some(*known_id) == id
                        && snapshot.is_autofillable()
                        && let Some(bounds) = self.autofill_bounds(owner)
                    {
                        calls.push(PlatformAutofillCall::NotifyViewEntered {
                            semantics_id: *known_id,
                            bounds,
                        });
                    }
                }
            }
        }
        // In the order of their ids (the order they came in), those gone told after.
        let id_of = |node: &NodeId| {
            let id = self.get(*node).map(|value| value.semantics_id);
            id.or_else(|| live.known.get(node).map(|known| known.0)).unwrap_or(0)
        };
        owners.sort_unstable_by_key(id_of);
        for &owner in &owners {
            self.sync_autofill_owner(live, owner, newly(&owner), noted(&owner), calls, &mut gone);
        }
        calls.append(&mut gone);
        live.in_use.clear();
        for (buffer, into) in
            [(dirty, &mut live.dirty), (attached, &mut live.attached), (owners, &mut live.owners)]
        {
            let mut buffer = buffer;
            buffer.clear();
            *into = buffer;
        }
        if live.displayed.is_empty() && live.pending_commit {
            calls.push(PlatformAutofillCall::Commit);
            live.pending_commit = false;
        }
        if !live.displayed.is_empty() {
            live.pending_commit = true;
        }
    }

    /// Notes every node in use, as attached: installed or enabled.
    fn autofill_scan(&mut self, live: &mut Live) {
        let mut stack = vec![self.root_id()];
        stack.extend(self.overlays.iter().copied());
        while let Some(node) = stack.pop() {
            let Some(value) = self.get(node) else { continue };
            if !value.attached || value.deactivated {
                continue;
            }
            live.dirty.push(node);
            live.attached.push(node);
            stack.extend(value.children.iter().map(|child| child.node));
        }
    }

    /// Whether `node` is in use: attached, not deactivated (each node of deactivated content is
    /// noted, another composition's under it too).
    fn autofill_in_use(&self, node: NodeId) -> bool {
        self.get(node).is_some_and(|value| value.attached) && !self.is_deactivated(node)
    }

    /// The owners noting `node` asks to look at: itself, or for a part its owner, and the owner
    /// it had if that changed.
    fn route_autofill_node(
        &mut self,
        live: &mut Live,
        node: NodeId,
        newly: bool,
        owners: &mut Vec<NodeId>,
    ) {
        // (Most trees have no node autofill reads: no lookup then.)
        let known = !live.known.is_empty() && live.known.contains_key(&node);
        let tracked = known || (!live.parts.is_empty() && live.parts.contains_key(&node));
        let in_use = match self.get(node) {
            None => false,
            Some(value) if !value.attached => false,
            // Nothing autofill reads, now or at the last sync.
            Some(value) if !value.kinds.any(NodeKinds::SEMANTICS) && !tracked => return,
            Some(_) => self.autofill_in_use(node),
        };
        if !in_use {
            if known {
                owners.push(node);
            }
            if tracked && let Some(Part { owner: Some(owner), .. }) = live.parts.remove(&node) {
                owners.push(owner);
            }
            return;
        }
        match self.autofill_semantics(node) {
            Some(own) if own.config.contains(&PART_OF_PARENT) => {
                // The owner it had, unless it came since (its place in the tree the same).
                let owner = match live.parts.get(&node) {
                    Some(part) if !newly => part.owner,
                    _ => self.part_owner(node),
                };
                let part = Part {
                    owner,
                    id: self.node(node).semantics_id,
                    snapshot: Snapshot::of(&own.config),
                };
                if let Some(Part { owner: Some(previous), .. }) = live.parts.insert(node, part)
                    && Some(previous) != owner
                {
                    owners.push(previous);
                }
                owners.extend(owner);
                // In use: it's above a part in use (no walk up again for it).
                live.in_use.extend(owner);
                // An owner that became a part.
                if known {
                    owners.push(node);
                }
            }
            own => {
                if tracked
                    && let Some(Part { owner: Some(previous), .. }) = live.parts.remove(&node)
                {
                    owners.push(previous);
                }
                // Nothing autofill reads, now or at the last sync: nothing to tell (its parts, if
                // any, are noted themselves when they change).
                if known || own.is_some_and(|own| is_autofill_related(&own.config)) {
                    owners.push(node);
                }
            }
        }
    }

    /// Compose's `onSemanticsChanged` and `onPostAttach` for `owner` (`newly` attached), its
    /// snapshot against the last sync's; `onDetach` or `onLayoutNodeDeactivated` if it's gone,
    /// told in `gone`.
    fn sync_autofill_owner(
        &mut self,
        live: &mut Live,
        owner: NodeId,
        newly: bool,
        noted: bool,
        calls: &mut Vec<PlatformAutofillCall>,
        gone: &mut Vec<PlatformAutofillCall>,
    ) {
        if !live.in_use.contains(&owner) && !self.autofill_in_use(owner) {
            live.hosts.remove(&owner);
            self.autofill_gone(live, owner, gone);
            return;
        }
        let id = self.node(owner).semantics_id;
        // A host not noted itself (a part was) has its own snapshot of the last sync.
        let kept = if noted { None } else { live.hosts.get(&owner).cloned() };
        let own = match kept {
            Some(own) => Some(own),
            None => match self.autofill_semantics(owner) {
                // A part's values are its owner's.
                Some(own) if own.config.contains(&PART_OF_PARENT) => None,
                own => Some(own.map(|own| Snapshot::of(&own.config)).unwrap_or_default()),
            },
        };
        let snapshot = match own {
            None => {
                live.hosts.remove(&owner);
                Snapshot::default()
            }
            Some(own) => {
                let mut snapshot = own.clone();
                // Its parts' values over its own, in their order.
                let mut parts: SmallVec<[(i32, &Snapshot); 2]> = live
                    .parts
                    .values()
                    .filter(|part| part.owner == Some(owner))
                    .map(|part| (part.id, &part.snapshot))
                    .collect();
                parts.sort_unstable_by_key(|(id, _)| *id);
                let has_parts = !parts.is_empty();
                for (_, part) in parts {
                    snapshot.apply(part);
                }
                if has_parts {
                    live.hosts.insert(owner, own);
                } else if !live.hosts.is_empty() {
                    live.hosts.remove(&owner);
                }
                snapshot
            }
        };
        let known_id = live.known.get(&owner).map(|known| known.0);
        let previous = match live.known.remove(&owner) {
            Some((known_id, previous)) if known_id == id => previous,
            // Reused, with a new id: the content that was there is gone, told only of its
            // visibility (`onPostLayoutNodeReused`).
            Some((known_id, _)) => {
                if live.displayed.remove(&known_id) {
                    calls.push(PlatformAutofillCall::NotifyViewVisibilityChanged {
                        semantics_id: known_id,
                        is_visible: false,
                    });
                }
                Snapshot::default()
            }
            None => Snapshot::default(),
        };
        if previous != snapshot {
            semantics_changed(live, calls, id, &previous, &snapshot);
        }
        // `onPostAttach`: attached, or reused, since (a node moved, detached and attached again
        // in one pass, isn't told).
        if newly && known_id != Some(id) && snapshot.has_content_type {
            live.displayed.insert(id);
            calls.push(PlatformAutofillCall::NotifyViewVisibilityChanged {
                semantics_id: id,
                is_visible: true,
            });
        }
        if !snapshot.is_empty() {
            live.known.insert(owner, (id, snapshot));
        }
    }

    /// `node`, known, is no longer in use: `onDetach` (its configuration gone,
    /// `notifySemanticsChange` with none) if it's detached, `onLayoutNodeDeactivated` if it's
    /// still attached (deactivated: its configuration gone untold).
    fn autofill_gone(
        &mut self,
        live: &mut Live,
        node: NodeId,
        gone: &mut Vec<PlatformAutofillCall>,
    ) {
        let Some((id, previous)) = live.known.remove(&node) else { return };
        if live.displayed.remove(&id) {
            gone.push(PlatformAutofillCall::NotifyViewVisibilityChanged {
                semantics_id: id,
                is_visible: false,
            });
        }
        if !self.get(node).is_some_and(|value| value.attached) {
            semantics_changed(live, gone, id, &previous, &Snapshot::default());
        }
    }

    /// The node whose semantics `node`'s are and its id, if they're autofillable
    /// (`isAutofillable`: it fills, and its data type isn't `NONE`).
    fn autofillable_owner(&mut self, node: NodeId) -> Option<(NodeId, i32)> {
        let owner = self.autofill_owner(node)?;
        let config = self.own_with_parts(owner)?;
        if config.get(&SemanticsProperties::CONTENT_DATA_TYPE) == Some(&ContentDataType::NONE) {
            return None;
        }
        config
            .contains(&SemanticsActions::ON_FILL_DATA)
            .then(|| (owner, self.node(owner).semantics_id))
    }

    /// The virtual structure the platform's autofill asks for, as Compose's
    /// `populateViewStructure`: the root, and under it the nodes related to autofill (a content
    /// type or data type, filling), placed and in use, each under the nearest such ancestor, in
    /// Compose's order. `package_name` is the application's.
    pub fn populate_autofill_structure(&mut self, package_name: Option<&str>) -> ViewStructure {
        let root = self.root_id();
        // Each structure and its children's indices, the root's first.
        let mut arena: Vec<(ViewStructure, Vec<usize>)> = Vec::new();
        let mut structure = ViewStructure::default();
        self.populate_autofill_node(&mut structure, root, true, package_name);
        arena.push((structure, Vec::new()));
        let mut stack: Vec<(NodeId, usize)> = vec![(root, 0)];
        while let Some((parent, parent_structure)) = stack.pop() {
            // In the children's order, as Compose's `childrenInfo` (not their drawing order).
            let children: SmallVec<[NodeId; 8]> =
                self.node(parent).children.iter().map(|child| child.node).collect();
            for child in children {
                let value = self.node(child);
                if !value.attached || !value.placed || self.is_deactivated(child) {
                    continue;
                }
                let related = self.autofill_config(child).is_some_and(|config| {
                    config.contains(&SemanticsActions::ON_FILL_DATA)
                        || config.contains(&SemanticsProperties::CONTENT_TYPE)
                        || config.contains(&SemanticsProperties::CONTENT_DATA_TYPE)
                });
                if !related {
                    stack.push((child, parent_structure));
                    continue;
                }
                let mut structure = ViewStructure::default();
                self.populate_autofill_node(&mut structure, child, false, package_name);
                let index = arena.len();
                arena.push((structure, Vec::new()));
                arena[parent_structure].1.push(index);
                stack.push((child, index));
            }
        }
        fn build(arena: &mut [(ViewStructure, Vec<usize>)], index: usize) -> ViewStructure {
            let children = std::mem::take(&mut arena[index].1);
            let mut structure = std::mem::take(&mut arena[index].0);
            structure.children = children.into_iter().map(|child| build(arena, child)).collect();
            structure
        }
        build(&mut arena, 0)
    }

    /// `node`'s own semantics with its parts', `None` for a part or a node without.
    fn autofill_config(&mut self, node: NodeId) -> Option<SemanticsConfiguration> {
        let own = self.own_semantics(node)?;
        if own.config.contains(&PART_OF_PARENT) {
            return None;
        }
        self.own_with_parts(node)
    }

    /// Compose's `ViewStructure.populate`.
    fn populate_autofill_node(
        &mut self,
        structure: &mut ViewStructure,
        node: NodeId,
        is_root: bool,
        package_name: Option<&str>,
    ) {
        use SemanticsActions as A;
        use SemanticsProperties as P;
        let config = self.autofill_config(node).unwrap_or_default();
        if let Some(description) = config.get(&P::CONTENT_DESCRIPTION).and_then(|it| it.first()) {
            structure.content_description = Some(description.to_string());
        }
        if let Some(&focused) = config.get(&P::FOCUSED) {
            structure.is_focused = focused;
        }
        if config.contains(&A::ON_CLICK) {
            structure.is_clickable = true;
        }
        if config.contains(&A::ON_LONG_CLICK) {
            structure.is_long_clickable = true;
        }
        if config.contains(&A::REQUEST_FOCUS) {
            structure.is_focusable = true;
        }
        let content_data_type = config.get(&P::CONTENT_DATA_TYPE).copied();
        let content_type = config.get(&P::CONTENT_TYPE).cloned();
        let toggleable_state = config.get(&P::TOGGLEABLE_STATE).copied();
        let selected = config.get(&P::SELECTED).copied();
        let role = config.get(&P::ROLE).copied();
        let has_set_text = config.contains(&A::SET_TEXT);
        let is_sensitive_data = config.get(&P::IS_SENSITIVE_DATA).copied().unwrap_or(true);

        let merged = self.merged_config_of(node).unwrap_or_default();
        if merged.contains(&P::DISABLED) {
            structure.is_enabled = false;
        }

        // The id: the host's for the root.
        let id = if is_root { HOST_VIEW_ID } else { self.node(node).semantics_id };
        structure.virtual_id = id;
        structure.package_name = package_name.map(str::to_string);

        let autofill_type = content_data_type.map(ContentDataType::data_type).or_else(|| {
            if has_set_text {
                Some(ContentDataType::TEXT.data_type())
            } else if toggleable_state.is_some() {
                Some(ContentDataType::TOGGLE.data_type())
            } else {
                None
            }
        });
        if let Some(autofill_type) = autofill_type {
            structure.autofill_type = autofill_type;
        }
        if let Some(text) = config.get(&P::EDITABLE_TEXT) {
            structure.autofill_value = FillableData::create_from_text(text.clone());
        }
        if let Some(data) = config.get(&P::FILLABLE_DATA) {
            structure.autofill_value = Some(data.clone());
        }
        if let Some(content_type) = &content_type {
            structure.autofill_hints =
                content_type.content_hints().iter().map(|hint| hint.to_string()).collect();
        }
        if let Some(bounds) = self.autofill_bounds(node) {
            structure.bounds = Some(bounds);
        }
        if let Some(selected) = selected {
            structure.is_selected = selected;
        }
        if let Some(state) = toggleable_state {
            structure.is_checkable = true;
            structure.is_checked = state == ToggleableState::On;
        } else if let Some(selected) = selected
            && role != Some(Role::Tab)
        {
            structure.is_checkable = true;
            structure.is_checked = selected;
        }
        let password_hint = ContentType::PASSWORD.content_hints()[0].clone();
        let is_password = config.contains(&P::PASSWORD)
            || content_type.is_some_and(|it| it.content_hints().contains(&password_hint));
        structure.data_is_sensitive = is_password || is_sensitive_data;
        let transparent = self.alpha_around(node, Some(0)) <= 0.0;
        structure.visibility = if transparent { VIEW_INVISIBLE } else { VIEW_VISIBLE };
        if let Some(texts) = merged.get(&P::TEXT) {
            let mut text = String::new();
            for each in texts {
                text.push_str(each.as_str());
                text.push('\n');
            }
            structure.text = text;
            structure.class_name = Some(TEXT_CLASS_NAME.to_string());
        }
        if self.node(node).children.is_empty()
            && let Some(class_name) = role.and_then(legacy_class_name)
        {
            structure.class_name = Some(class_name.to_string());
        }
        if has_set_text {
            structure.class_name = Some(TEXT_FIELD_CLASS_NAME.to_string());
            if let Some(&length) = config.get(&P::MAX_TEXT_LENGTH) {
                structure.max_text_length = length;
            }
            if is_password {
                structure.input_type = INPUT_TYPE_TEXT_PASSWORD;
            }
        }
    }
}

/// Compose's `Role.toLegacyClassName`.
fn legacy_class_name(role: Role) -> Option<&'static str> {
    match role {
        Role::Button => Some("android.widget.Button"),
        Role::Checkbox => Some("android.widget.CheckBox"),
        Role::RadioButton => Some("android.widget.RadioButton"),
        Role::Image => Some("android.widget.ImageView"),
        Role::DropdownList => Some("android.widget.Spinner"),
        Role::ValuePicker => Some("android.widget.NumberPicker"),
        _ => None,
    }
}

/// Compose's `onSemanticsChanged` for one node (`notifyAutofillManager`, then whether it counts
/// for the commit).
fn semantics_changed(
    tracking: &mut Live,
    calls: &mut Vec<PlatformAutofillCall>,
    id: i32,
    previous: &Snapshot,
    current: &Snapshot,
) {
    notify_autofill_manager(calls, id, previous, current);
    if previous.has_content_type != current.has_content_type {
        if current.has_content_type {
            tracking.displayed.insert(id);
        } else {
            tracking.displayed.remove(&id);
        }
    }
}

/// Compose's `notifyAutofillManager`. Values are compared by value where Compose compares the
/// input text by identity (D1680).
fn notify_autofill_manager(
    calls: &mut Vec<PlatformAutofillCall>,
    id: i32,
    previous: &Snapshot,
    current: &Snapshot,
) {
    let visibility = |is_visible| PlatformAutofillCall::NotifyViewVisibilityChanged {
        semantics_id: id,
        is_visible,
    };
    let none = Some(ContentDataType::NONE);
    let data_type = current.content_data_type;
    if data_type == none {
        if previous.content_data_type != none {
            calls.push(visibility(false));
        }
        // Not autofillable any more: nothing else.
        return;
    }
    if previous.content_data_type == none {
        calls.push(visibility(true));
    }
    if previous.input_text != current.input_text {
        match (&previous.input_text, &current.input_text) {
            (None, _) => calls.push(visibility(true)),
            (_, None) => calls.push(visibility(false)),
            (_, Some(text)) => {
                if data_type == Some(ContentDataType::TEXT) {
                    let value = FillableData::create_from_text(text.clone()).expect("a value");
                    calls
                        .push(PlatformAutofillCall::NotifyValueChanged { semantics_id: id, value });
                }
            }
        }
    }
    if previous.toggleable_state != current.toggleable_state {
        match (previous.toggleable_state, current.toggleable_state) {
            (None, _) => calls.push(visibility(true)),
            (_, None) => calls.push(visibility(false)),
            (_, Some(state)) => {
                let toggled = match state {
                    ToggleableState::On => Some(true),
                    ToggleableState::Off => Some(false),
                    ToggleableState::Indeterminate => None,
                };
                if data_type == Some(ContentDataType::TOGGLE)
                    && let Some(toggled) = toggled
                {
                    let value = FillableData::create_from_boolean(toggled).expect("a value");
                    calls
                        .push(PlatformAutofillCall::NotifyValueChanged { semantics_id: id, value });
                }
            }
        }
    }
    if previous.fillable_data != current.fillable_data {
        match (&previous.fillable_data, &current.fillable_data) {
            (None, _) => calls.push(visibility(true)),
            (_, None) => calls.push(visibility(false)),
            (_, Some(value)) => calls.push(PlatformAutofillCall::NotifyValueChanged {
                semantics_id: id,
                value: value.clone(),
            }),
        }
    }
}

// ---- The virtual structure ---------------------------------------------------------------------

/// The root's virtual id, as `AccessibilityNodeProviderCompat.HOST_VIEW_ID`.
pub const HOST_VIEW_ID: i32 = -1;
/// `View.VISIBLE`.
pub const VIEW_VISIBLE: i32 = 0;
/// `View.INVISIBLE`.
pub const VIEW_INVISIBLE: i32 = 4;
/// `InputType.TYPE_CLASS_TEXT | TYPE_TEXT_VARIATION_PASSWORD`.
pub const INPUT_TYPE_TEXT_PASSWORD: i32 = 0x81;
/// A field's class name, as Compose's `TextFieldClassName`.
pub const TEXT_FIELD_CLASS_NAME: &str = "android.widget.EditText";
/// A text's, as `TextClassName`.
pub const TEXT_CLASS_NAME: &str = "android.widget.TextView";

/// A node of the virtual structure an autofill service reads, as what Compose writes into
/// Android's `ViewStructure` (the fields of its tests' `FakeViewStructure` it sets, with their
/// defaults): the Android host writes it into the platform's.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ViewStructure {
    /// `setAutofillId(root, virtualId)` and `setId`: the semantics id, `HOST_VIEW_ID` for the root.
    pub virtual_id: i32,
    pub package_name: Option<String>,
    pub children: Vec<ViewStructure>,
    /// `setDimens`: left, top, then the size, in the window's pixels.
    pub bounds: Option<IntRect>,
    pub is_checkable: bool,
    pub is_focusable: bool,
    /// One of `ContentDataType`'s values (`View.AUTOFILL_TYPE_*`).
    pub autofill_type: i32,
    pub autofill_hints: Vec<String>,
    pub autofill_value: Option<FillableData>,
    pub class_name: Option<String>,
    pub content_description: Option<String>,
    pub data_is_sensitive: bool,
    pub input_type: i32,
    pub is_enabled: bool,
    pub is_checked: bool,
    pub is_clickable: bool,
    pub is_focused: bool,
    pub is_long_clickable: bool,
    pub is_selected: bool,
    pub text: String,
    pub visibility: i32,
    /// `setMaxTextLength`; -1 for none.
    pub max_text_length: i32,
}

impl Default for ViewStructure {
    fn default() -> ViewStructure {
        ViewStructure {
            virtual_id: 0,
            package_name: None,
            children: Vec::new(),
            bounds: None,
            is_checkable: false,
            is_focusable: false,
            autofill_type: ContentDataType::NONE.data_type(),
            autofill_hints: Vec::new(),
            autofill_value: None,
            class_name: None,
            content_description: None,
            data_is_sensitive: false,
            input_type: 0,
            is_enabled: true,
            is_checked: false,
            is_clickable: false,
            is_focused: false,
            is_long_clickable: false,
            is_selected: false,
            text: String::new(),
            visibility: VIEW_VISIBLE,
            max_text_length: -1,
        }
    }
}

// ---- Modifiers and hosts -----------------------------------------------------------------------

impl Modifier {
    /// The hints an autofill service fills this node by, as `Modifier.contentType`.
    pub fn content_type(self, content_type: ContentType) -> Modifier {
        self.semantics(|config| config.set_content_type(content_type))
    }
}

impl crate::host::UiHost {
    /// Fills the nodes of these semantics ids with an autofill service's values, as Compose's
    /// `AndroidComposeView.autofill`: each node's `OnFillData`, run with the tree free. Ids no
    /// node has any more, or whose node doesn't fill, are skipped.
    pub fn perform_autofill(&mut self, values: Vec<(i32, FillableData)>) {
        type Fill = Rc<crate::semantics::FillDataFn>;
        let actions: Vec<(Fill, FillableData)> = self.with_tree(|tree| {
            values
                .into_iter()
                .filter_map(|(id, value)| {
                    let node = tree.node_with_semantics_id(id)?;
                    let config = tree.unmerged_semantics_config(node)?;
                    let action = config.get(&SemanticsActions::ON_FILL_DATA)?.action.clone()?;
                    Some((action, value))
                })
                .collect()
        });
        for (action, value) in actions {
            action(value);
        }
    }
}
