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
use std::rc::Rc;

use compose_runtime::CompositionLocal;

use crate::modifier::Modifier;
use crate::node_chain::NodeKinds;
use crate::platform_services::of_composing_tree;
use crate::semantics::{
    PART_OF_PARENT, PARTS_HOST, Role, SemanticsActions, SemanticsConfiguration,
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
}

impl Snapshot {
    fn of(config: &SemanticsConfiguration) -> Snapshot {
        Snapshot {
            content_data_type: config.get(&SemanticsProperties::CONTENT_DATA_TYPE).copied(),
            input_text: config.get(&SemanticsProperties::INPUT_TEXT).cloned(),
            toggleable_state: config.get(&SemanticsProperties::TOGGLEABLE_STATE).copied(),
            fillable_data: config.get(&SemanticsProperties::FILLABLE_DATA).cloned(),
            has_content_type: config.contains(&SemanticsProperties::CONTENT_TYPE),
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
    }

    fn is_empty(&self) -> bool {
        *self == Snapshot::default()
    }
}

/// The tree's autofill bookkeeping while a platform manager is installed (D1680).
pub(crate) struct AutofillTracking {
    platform: Rc<dyn PlatformAutofillManager>,
    manager: Rc<dyn AutofillManager>,
    /// The ids of the nodes in use (attached, not deactivated) at the last sync.
    present: HashSet<i32>,
    /// Their layout nodes and snapshots, those not empty.
    known: HashMap<i32, (NodeId, Snapshot)>,
    /// The nodes with a content type on screen, as Compose's `currentlyDisplayedIDs`.
    displayed: HashSet<i32>,
    /// Some were displayed since the last commit, as `pendingAutofillCommit`.
    pending_commit: bool,
    /// The focus's moves since the last sync: the layout nodes of the targets.
    focus_changes: Vec<(Option<NodeId>, Option<NodeId>)>,
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
                present: HashSet::new(),
                known: HashMap::new(),
                displayed: HashSet::new(),
                pending_commit: false,
                focus_changes: Vec::new(),
            })
        });
        // The next frame tells it of what's there.
        self.semantics_changed = true;
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

    /// The focus moved from one target's layout node to another's (`activeFocusTargetNode`'s
    /// setter telling its listeners): told to the platform at the next sync.
    #[inline]
    pub(crate) fn note_autofill_focus(
        &mut self,
        previous: Option<NodeId>,
        current: Option<NodeId>,
    ) {
        if let Some(tracking) = &mut self.autofill {
            tracking.focus_changes.push((previous, current));
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
        // The nearest host above, else the nearest node with semantics.
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
        let Some(owner) = self.autofill_owner(node) else { return };
        let Some(bounds) = self.autofill_bounds(owner) else { return };
        let semantics_id = self.node(owner).semantics_id;
        self.pending_actions.push(crate::input::Action::Run(Box::new(move || {
            platform.call(PlatformAutofillCall::RequestAutofill { semantics_id, bounds });
        })));
    }

    /// What the platform's autofill is told of since the last sync, as Compose's
    /// `AndroidAutofillManager` hears of each change: the focus's moves, the nodes come, changed,
    /// deactivated and gone, then the commit once every node with a content type is gone
    /// (`onEndApplyChanges`). A frame that may have changed the semantics syncs (D1680).
    #[doc(hidden)]
    pub fn sync_autofill(&mut self) -> Vec<PlatformAutofillCall> {
        let Some(mut tracking) = self.autofill.take() else { return Vec::new() };
        let mut calls = Vec::new();
        self.sync_autofill_focus(&mut tracking, &mut calls);
        self.sync_autofill_nodes(&mut tracking, &mut calls);
        if tracking.displayed.is_empty() && tracking.pending_commit {
            calls.push(PlatformAutofillCall::Commit);
            tracking.pending_commit = false;
        }
        if !tracking.displayed.is_empty() {
            tracking.pending_commit = true;
        }
        self.autofill = Some(tracking);
        calls
    }

    /// The platform's manager, if installed, and what `sync_autofill` tells it.
    #[doc(hidden)]
    pub fn take_autofill_sync(
        &mut self,
    ) -> Option<(Rc<dyn PlatformAutofillManager>, Vec<PlatformAutofillCall>)> {
        let platform = self.autofill.as_ref()?.platform.clone();
        Some((platform, self.sync_autofill()))
    }

    /// Compose's `onFocusChanged`: the node losing the focus exits, the one taking it enters,
    /// each if autofillable.
    fn sync_autofill_focus(
        &mut self,
        tracking: &mut AutofillTracking,
        calls: &mut Vec<PlatformAutofillCall>,
    ) {
        for (previous, current) in std::mem::take(&mut tracking.focus_changes) {
            if let Some((_, id)) = previous.and_then(|node| self.autofillable_owner(node)) {
                calls.push(PlatformAutofillCall::NotifyViewExited { semantics_id: id });
            }
            if let Some((owner, id)) = current.and_then(|node| self.autofillable_owner(node))
                && let Some(bounds) = self.autofill_bounds(owner)
            {
                calls.push(PlatformAutofillCall::NotifyViewEntered { semantics_id: id, bounds });
            }
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

    /// Each node in use with its layout node and snapshot, in tree order (a part's taken into its
    /// owner's). Deactivated content is left out.
    fn autofill_snapshots(&mut self) -> Vec<(i32, NodeId, Snapshot)> {
        let mut out: Vec<(i32, NodeId, Snapshot)> = Vec::new();
        // (node, the nearest semantics node's index, the nearest host's)
        let mut roots = vec![self.root_id()];
        roots.extend(self.overlays.iter().copied());
        let mut stack: Vec<(NodeId, Option<usize>, Option<usize>)> =
            roots.into_iter().rev().map(|root| (root, None, None)).collect();
        let mut visited = HashSet::new();
        while let Some((node, nearest, host)) = stack.pop() {
            let Some(value) = self.get(node) else { continue };
            if !value.attached || value.deactivated || !visited.insert(node) {
                continue;
            }
            let id = value.semantics_id;
            let (nearest, host) = if value.overlay { (None, None) } else { (nearest, host) };
            let children: Vec<NodeId> = value.children.iter().rev().map(|c| c.node).collect();
            let own =
                if value.kinds.any(NodeKinds::SEMANTICS) { self.own_semantics(node) } else { None };
            let index = out.len();
            let (mut child_nearest, mut child_host) = (nearest, host);
            match own {
                Some(own) if own.config.contains(&PART_OF_PARENT) => {
                    out.push((id, node, Snapshot::default()));
                    let part = Snapshot::of(&own.config);
                    if let Some(owner) = host.or(nearest) {
                        out[owner].2.apply(&part);
                    }
                }
                Some(own) => {
                    out.push((id, node, Snapshot::of(&own.config)));
                    if !own.config.is_empty() {
                        child_nearest = Some(index);
                    }
                    if own.config.contains(&PARTS_HOST) {
                        child_host = Some(index);
                    }
                }
                None => out.push((id, node, Snapshot::default())),
            }
            stack.extend(children.into_iter().map(|child| (child, child_nearest, child_host)));
        }
        out
    }

    /// Compose's `onSemanticsChanged`, `onPostAttach`, `onLayoutNodeDeactivated` and `onDetach`,
    /// from the nodes' snapshots against the last sync's.
    fn sync_autofill_nodes(
        &mut self,
        tracking: &mut AutofillTracking,
        calls: &mut Vec<PlatformAutofillCall>,
    ) {
        let nodes = self.autofill_snapshots();
        let previous_present = std::mem::take(&mut tracking.present);
        let mut previous_known = std::mem::take(&mut tracking.known);
        let empty = Snapshot::default();
        for (id, node, snapshot) in nodes {
            tracking.present.insert(id);
            let previous = previous_known.remove(&id).map(|(_, snapshot)| snapshot);
            let previous = previous.as_ref().unwrap_or(&empty);
            if *previous != snapshot {
                semantics_changed(tracking, calls, id, previous, &snapshot);
            }
            // Attached (or reused, with a new id) since: `onPostAttach`.
            if !previous_present.contains(&id) && snapshot.has_content_type {
                tracking.displayed.insert(id);
                calls.push(PlatformAutofillCall::NotifyViewVisibilityChanged {
                    semantics_id: id,
                    is_visible: true,
                });
            }
            if !snapshot.is_empty() {
                tracking.known.insert(id, (node, snapshot));
            }
        }
        // Those no longer in use, in the order of their ids.
        let mut gone: Vec<(i32, (NodeId, Snapshot))> = previous_known.into_iter().collect();
        gone.sort_by_key(|(id, _)| *id);
        for (id, (node, previous)) in gone {
            if tracking.displayed.remove(&id) {
                calls.push(PlatformAutofillCall::NotifyViewVisibilityChanged {
                    semantics_id: id,
                    is_visible: false,
                });
            }
            // Detached, its configuration gone (`notifySemanticsChange` with none); a node still
            // attached was deactivated (or reused since, with a new id), its configuration gone
            // without telling (`onDeactivate`).
            if !self.get(node).is_some_and(|value| value.attached) {
                semantics_changed(tracking, calls, id, &previous, &empty);
            }
        }
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
            for child in self.z_sorted_children(parent) {
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
    tracking: &mut AutofillTracking,
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
