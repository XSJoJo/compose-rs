//! Android's autofill (D1687): compose-ui's `PlatformAutofillManager` over the platform's
//! `AutofillManager`, as Compose's `PlatformAutofillManagerImpl`, and the virtual structure and
//! values crossing to `ComposeRsAutofill.kt`.
//!
//! Rust tells the platform through `ComposeRsView.autofillCall(kind, id, left, top, right,
//! bottom, valueType, text, number)`, after a frame or an event, the host free
//! (`call` below). The view asks for the structure (`ComposeRsNative.autofillStructure`, written by
//! `write_structure`) when the framework does (`onProvideAutofillVirtualStructure`) and hands
//! the service's values back (`ComposeRsNative.autofill`, read by `fillable_data`).

use std::rc::Rc;

use compose_ui::FillableData;
use compose_ui::platform::{PlatformAutofillCall, PlatformAutofillManager, ViewStructure};
use jni::objects::{Global, JMethodID, JObject};
use jni::signature::{Primitive, ReturnType};
use jni::sys::jint;
use jni::{Env, JValue, jni_sig, jni_str};

use crate::bridge::{check, method, with_main_env};
use crate::serialize::Writer;

/// `autofillCall`'s kinds.
pub mod kind {
    pub const VIEW_ENTERED: i32 = 0;
    pub const VIEW_EXITED: i32 = 1;
    pub const VALUE_CHANGED: i32 = 2;
    /// Its number is 1 for visible, 0 for not.
    pub const VISIBILITY_CHANGED: i32 = 3;
    pub const COMMIT: i32 = 4;
    pub const CANCEL: i32 = 5;
    pub const REQUEST: i32 = 6;
}

/// A value's parts as they cross: its type (`View.AUTOFILL_TYPE_*`), its text (a text's) and its
/// number (a toggle's 0 or 1, a list's index, a date's milliseconds).
pub fn value_parts(value: &FillableData) -> ValueParts<'_> {
    let data_type = value.data_type().data_type();
    if let Some(text) = value.text_value() {
        return (data_type, Some(text.as_str()), 0);
    }
    let number = value
        .boolean_value()
        .map(i64::from)
        .or_else(|| value.list_index_value().map(i64::from))
        .or_else(|| value.date_millis_value())
        .unwrap_or(0);
    (data_type, None, number)
}

/// The value the platform gave as `value_parts`, as Compose's `AndroidFillableData` over an
/// `AutofillValue`: `None` for a type it doesn't know.
pub fn fillable_data(data_type: i32, text: Option<String>, number: i64) -> Option<FillableData> {
    match data_type {
        1 => Some(FillableData::from_platform_text(text.unwrap_or_default())),
        2 => FillableData::create_from_boolean(number != 0),
        3 => FillableData::create_from_list_index(number as i32),
        4 => FillableData::create_from_date_millis(number),
        _ => None,
    }
}

/// A value's type, text and number (`value_parts`).
pub type ValueParts<'a> = (i32, Option<&'a str>, i64);

/// A call's parts as `autofillCall` takes them: its kind, the node's id, bounds, then the value's
/// parts.
pub fn call_parts(call: &PlatformAutofillCall) -> (i32, i32, [i32; 4], ValueParts<'_>) {
    use PlatformAutofillCall as C;
    let none = (0, None, 0);
    let rect =
        |bounds: &compose_ui::IntRect| [bounds.left, bounds.top, bounds.right, bounds.bottom];
    match call {
        C::NotifyViewEntered { semantics_id, bounds } => {
            (kind::VIEW_ENTERED, *semantics_id, rect(bounds), none)
        }
        C::NotifyViewExited { semantics_id } => (kind::VIEW_EXITED, *semantics_id, [0; 4], none),
        C::NotifyValueChanged { semantics_id, value } => {
            (kind::VALUE_CHANGED, *semantics_id, [0; 4], value_parts(value))
        }
        C::NotifyViewVisibilityChanged { semantics_id, is_visible } => {
            (kind::VISIBILITY_CHANGED, *semantics_id, [0; 4], (0, None, i64::from(*is_visible)))
        }
        C::Commit => (kind::COMMIT, 0, [0; 4], none),
        C::Cancel => (kind::CANCEL, 0, [0; 4], none),
        C::RequestAutofill { semantics_id, bounds } => {
            (kind::REQUEST, *semantics_id, rect(bounds), none)
        }
    }
}

/// The view's autofill, installed on its tree.
pub(crate) struct JniAutofill {
    view: Rc<Global<JObject<'static>>>,
    call: JMethodID,
}

impl JniAutofill {
    pub(crate) fn new(
        env: &mut Env,
        view: &JObject,
        view_ref: &Rc<Global<JObject<'static>>>,
    ) -> JniAutofill {
        let call =
            method(env, view, jni_str!("autofillCall"), jni_sig!("(IIIIIIILjava/lang/String;J)V"));
        JniAutofill { view: view_ref.clone(), call }
    }
}

impl PlatformAutofillManager for JniAutofill {
    fn call(&self, call: PlatformAutofillCall) {
        let (kind, id, bounds, (value_type, text, number)) = call_parts(&call);
        with_main_env(|env| {
            let _ = env.with_local_frame(2, |env| -> jni::errors::Result<()> {
                let text = match text {
                    Some(text) => env.new_string(text)?.into(),
                    None => JObject::null(),
                };
                let args = [
                    JValue::Int(kind as jint).as_jni(),
                    JValue::Int(id).as_jni(),
                    JValue::Int(bounds[0]).as_jni(),
                    JValue::Int(bounds[1]).as_jni(),
                    JValue::Int(bounds[2]).as_jni(),
                    JValue::Int(bounds[3]).as_jni(),
                    JValue::Int(value_type).as_jni(),
                    JValue::Object(&text).as_jni(),
                    JValue::Long(number).as_jni(),
                ];
                // SAFETY: the method's signature is `(IIIIIIILjava/lang/String;J)V`.
                let _ = unsafe {
                    env.call_method_unchecked(
                        &**self.view,
                        self.call,
                        ReturnType::Primitive(Primitive::Void),
                        &args,
                    )
                };
                check(env, "ComposeRsView.autofillCall");
                Ok(())
            });
        });
    }
}

/// A node's flags in the structure.
pub mod flag {
    pub const CHECKABLE: u32 = 1;
    pub const FOCUSABLE: u32 = 1 << 1;
    pub const SENSITIVE: u32 = 1 << 2;
    pub const ENABLED: u32 = 1 << 3;
    pub const CHECKED: u32 = 1 << 4;
    pub const CLICKABLE: u32 = 1 << 5;
    pub const FOCUSED: u32 = 1 << 6;
    pub const LONG_CLICKABLE: u32 = 1 << 7;
    pub const SELECTED: u32 = 1 << 8;
    pub const HAS_BOUNDS: u32 = 1 << 9;
    pub const HAS_VALUE: u32 = 1 << 10;
    pub const HAS_CLASS_NAME: u32 = 1 << 11;
    pub const HAS_DESCRIPTION: u32 = 1 << 12;
}

fn write_string(out: &mut Writer, text: &str) {
    out.u32(text.len() as u32);
    out.bytes.extend_from_slice(text.as_bytes());
}

/// The structure, little-endian, each node in pre-order: its virtual id, flags, autofill type,
/// input type, maximum length, visibility, bounds (left, top, right, bottom; with
/// `HAS_BOUNDS`), value (type, text, number; with `HAS_VALUE`), hints (a count, then each),
/// class name and description (with their flags), text, then its children's count and the
/// children. `ComposeRsAutofill.kt` writes it into the platform's `ViewStructure`.
pub fn write_structure(structure: &ViewStructure, out: &mut Writer) {
    out.u32(structure.virtual_id as u32);
    let mut flags = 0;
    let pairs = [
        (structure.is_checkable, flag::CHECKABLE),
        (structure.is_focusable, flag::FOCUSABLE),
        (structure.data_is_sensitive, flag::SENSITIVE),
        (structure.is_enabled, flag::ENABLED),
        (structure.is_checked, flag::CHECKED),
        (structure.is_clickable, flag::CLICKABLE),
        (structure.is_focused, flag::FOCUSED),
        (structure.is_long_clickable, flag::LONG_CLICKABLE),
        (structure.is_selected, flag::SELECTED),
        (structure.bounds.is_some(), flag::HAS_BOUNDS),
        (structure.autofill_value.is_some(), flag::HAS_VALUE),
        (structure.class_name.is_some(), flag::HAS_CLASS_NAME),
        (structure.content_description.is_some(), flag::HAS_DESCRIPTION),
    ];
    for (set, bit) in pairs {
        if set {
            flags |= bit;
        }
    }
    out.u32(flags);
    out.u32(structure.autofill_type as u32);
    out.u32(structure.input_type as u32);
    out.u32(structure.max_text_length as u32);
    out.u32(structure.visibility as u32);
    if let Some(bounds) = &structure.bounds {
        for side in [bounds.left, bounds.top, bounds.right, bounds.bottom] {
            out.u32(side as u32);
        }
    }
    if let Some(value) = &structure.autofill_value {
        let (data_type, text, number) = value_parts(value);
        out.u32(data_type as u32);
        write_string(out, text.unwrap_or(""));
        out.u64(number as u64);
    }
    out.u32(structure.autofill_hints.len() as u32);
    for hint in &structure.autofill_hints {
        write_string(out, hint);
    }
    if let Some(class_name) = &structure.class_name {
        write_string(out, class_name);
    }
    if let Some(description) = &structure.content_description {
        write_string(out, description);
    }
    write_string(out, &structure.text);
    out.u32(structure.children.len() as u32);
    for child in &structure.children {
        write_structure(child, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_cross_and_come_back() {
        let values = [
            FillableData::from_platform_text("name"),
            FillableData::create_from_boolean(true).unwrap(),
            FillableData::create_from_list_index(3).unwrap(),
            FillableData::create_from_date_millis(946_684_800_000).unwrap(),
        ];
        for value in values {
            let (data_type, text, number) = value_parts(&value);
            let back = fillable_data(data_type, text.map(str::to_string), number);
            assert_eq!(back, Some(value));
        }
        assert_eq!(fillable_data(0, None, 0), None);
    }

    #[test]
    fn calls_cross_as_their_parts() {
        let bounds = compose_ui::IntRect { left: 1, top: 2, right: 3, bottom: 4 };
        let entered = PlatformAutofillCall::NotifyViewEntered { semantics_id: 7, bounds };
        assert_eq!(call_parts(&entered), (kind::VIEW_ENTERED, 7, [1, 2, 3, 4], (0, None, 0)));
        let visible =
            PlatformAutofillCall::NotifyViewVisibilityChanged { semantics_id: 8, is_visible: true };
        assert_eq!(call_parts(&visible), (kind::VISIBILITY_CHANGED, 8, [0; 4], (0, None, 1)));
        let value = PlatformAutofillCall::NotifyValueChanged {
            semantics_id: 9,
            value: FillableData::from_platform_text("x"),
        };
        assert_eq!(call_parts(&value), (kind::VALUE_CHANGED, 9, [0; 4], (1, Some("x"), 0)));
        assert_eq!(call_parts(&PlatformAutofillCall::Commit).0, kind::COMMIT);
        assert_eq!(call_parts(&PlatformAutofillCall::Cancel).0, kind::CANCEL);
    }

    /// The structure's bytes, read back as `ComposeRsAutofill.kt` reads them.
    #[test]
    fn the_structure_s_bytes() {
        let child = ViewStructure {
            virtual_id: 5,
            autofill_hints: vec!["username".into()],
            autofill_type: 1,
            autofill_value: FillableData::create_from_text("ab"),
            class_name: Some("android.widget.EditText".into()),
            bounds: Some(compose_ui::IntRect { left: 0, top: 0, right: 10, bottom: 20 }),
            data_is_sensitive: true,
            ..ViewStructure::default()
        };
        let root = ViewStructure { virtual_id: -1, children: vec![child], ..Default::default() };
        let mut out = Writer::default();
        write_structure(&root, &mut out);
        let bytes = out.bytes;
        let mut at = 0;
        let mut u32_at = || {
            let value = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
            at += 4;
            value
        };
        assert_eq!(u32_at() as i32, -1);
        assert_eq!(u32_at(), flag::ENABLED);
        assert_eq!(u32_at(), 0); // the type
        assert_eq!(u32_at(), 0); // the input type
        assert_eq!(u32_at() as i32, -1); // no maximum
        assert_eq!(u32_at(), 0); // visible
        assert_eq!(u32_at(), 0); // no hints
        assert_eq!(u32_at(), 0); // no text
        assert_eq!(u32_at(), 1); // one child
        assert_eq!(u32_at(), 5);
        let flags = u32_at();
        assert_eq!(
            flags,
            flag::SENSITIVE
                | flag::ENABLED
                | flag::HAS_BOUNDS
                | flag::HAS_VALUE
                | flag::HAS_CLASS_NAME
        );
    }
}
