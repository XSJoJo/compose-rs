//! Android's floating text toolbar: compose-ui's `TextActionModes` through the view's
//! `ActionMode` of `TYPE_FLOATING`, as Compose's `AndroidTextContextMenuToolbarProvider` starts
//! it (D2033).
//!
//! Rust names each mode by an id; `ComposeRsView.startTextActionMode(id)` starts it with a
//! `Callback2` whose create and prepare ask Rust for the menu (`textActionModeMenu`, then each
//! item's title), whose items' clicks call Rust back (`textActionModeClick`), whose content rect
//! is Rust's (`textActionModeRect`, in the view: compose-rs's root), and whose destruction Rust
//! is told of (`textActionModeDestroyed`). Those callbacks touch only the modes kept here, never
//! the host, so the view may call them while Rust is calling the view.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use compose_ui::{TextActionMenu, TextActionMode, TextActionModeCallback, TextActionModes};
use jni::objects::JMethodID;
use jni::objects::{Global, JClass, JFloatArray, JIntArray, JObject, JString};
use jni::signature::{Primitive, ReturnType};
use jni::sys::{jboolean, jint};
use jni::{Env, EnvUnowned, JValue, jni_sig, jni_str};

use crate::bridge::{check, method, with_env, with_main_env};

/// A mode Rust started: what fills its menu, and the menu it last filled (its items' clicks).
struct Mode {
    callback: Rc<dyn TextActionModeCallback>,
    menu: TextActionMenu,
}

thread_local! {
    static MODES: RefCell<HashMap<i32, Mode>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<i32> = const { Cell::new(1) };
}

/// The view's floating toolbars.
pub(crate) struct JniTextActionModes {
    view: Rc<Global<JObject<'static>>>,
    methods: Rc<Methods>,
}

struct Methods {
    start: JMethodID,
    invalidate: JMethodID,
    invalidate_rect: JMethodID,
    finish: JMethodID,
}

impl JniTextActionModes {
    pub(crate) fn new(
        env: &mut Env,
        view: &JObject,
        view_ref: &Rc<Global<JObject<'static>>>,
    ) -> JniTextActionModes {
        let methods = Methods {
            start: method(env, view, jni_str!("startTextActionMode"), jni_sig!("(I)Z")),
            invalidate: method(env, view, jni_str!("invalidateTextActionMode"), jni_sig!("(I)V")),
            invalidate_rect: method(
                env,
                view,
                jni_str!("invalidateTextActionModeRect"),
                jni_sig!("(I)V"),
            ),
            finish: method(env, view, jni_str!("finishTextActionMode"), jni_sig!("(I)V")),
        };
        JniTextActionModes { view: view_ref.clone(), methods: Rc::new(methods) }
    }
}

/// Calls the view's `(I)V` or `(I)Z` method `id` with the mode `mode`.
fn call(
    view: &Global<JObject<'static>>,
    id: JMethodID,
    mode: i32,
    boolean: bool,
    what: &str,
) -> bool {
    with_main_env(|env| {
        let returns = if boolean {
            ReturnType::Primitive(Primitive::Boolean)
        } else {
            ReturnType::Primitive(Primitive::Void)
        };
        // SAFETY: the method takes an `int` and returns what `returns` says.
        let result = unsafe {
            env.call_method_unchecked(&**view, id, returns, &[JValue::Int(mode).as_jni()])
        };
        check(env, what);
        boolean && result.and_then(|value| value.z()).unwrap_or(false)
    })
}

impl TextActionModes for JniTextActionModes {
    fn start(&self, callback: Rc<dyn TextActionModeCallback>) -> Option<Rc<dyn TextActionMode>> {
        let id = NEXT_ID.with(|next| {
            let id = next.get();
            next.set(id.wrapping_add(1).max(1));
            id
        });
        MODES.with(|modes| {
            modes.borrow_mut().insert(id, Mode { callback, menu: TextActionMenu::new() })
        });
        let started =
            call(&self.view, self.methods.start, id, true, "ComposeRsView.startTextActionMode");
        if !started {
            MODES.with(|modes| modes.borrow_mut().remove(&id));
            return None;
        }
        Some(Rc::new(JniTextActionMode {
            view: self.view.clone(),
            methods: self.methods.clone(),
            id,
        }))
    }
}

struct JniTextActionMode {
    view: Rc<Global<JObject<'static>>>,
    methods: Rc<Methods>,
    id: i32,
}

impl JniTextActionMode {
    fn alive(&self) -> bool {
        MODES.with(|modes| modes.borrow().contains_key(&self.id))
    }
}

impl TextActionMode for JniTextActionMode {
    fn invalidate(&self) {
        if self.alive() {
            call(&self.view, self.methods.invalidate, self.id, false, "invalidateTextActionMode");
        }
    }

    fn invalidate_content_rect(&self) {
        if self.alive() {
            call(
                &self.view,
                self.methods.invalidate_rect,
                self.id,
                false,
                "invalidateTextActionModeRect",
            );
        }
    }

    fn finish(&self) {
        if self.alive() {
            call(&self.view, self.methods.finish, self.id, false, "finishTextActionMode");
        }
    }
}

/// The callback of mode `id`, if it's alive.
fn callback_of(id: i32) -> Option<Rc<dyn TextActionModeCallback>> {
    MODES.with(|modes| modes.borrow().get(&id).map(|mode| mode.callback.clone()))
}

/// Fills mode `id`'s menu: on its creation (`create`), whether it shows; else, whether it
/// changed. Then its items, each as its group, id and order.
fn fill_menu(id: i32, create: bool) -> Vec<i32> {
    let Some(callback) = callback_of(id) else { return vec![0] };
    let mut menu = TextActionMenu::new();
    let result = if create {
        callback.on_create_action_mode(&mut menu)
    } else {
        callback.on_prepare_action_mode(&mut menu)
    };
    let mut out = Vec::with_capacity(1 + menu.len() * 3);
    out.push(result as i32);
    for item in menu.items() {
        out.extend([item.group, item.id, item.order]);
    }
    MODES.with(|modes| {
        if let Some(mode) = modes.borrow_mut().get_mut(&id) {
            mode.menu = menu;
        }
    });
    out
}

/// `ComposeRsNative.textActionModeMenu(id, create)`: mode `id`'s menu, filled: its result, then
/// each item's group, id and order (`fill_menu`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_rs_compose_android_ComposeRsNative_textActionModeMenu<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jint,
    create: jboolean,
) -> JIntArray<'local> {
    let values = fill_menu(id, create);
    with_env(&mut env, |env| {
        let array = env.new_int_array(values.len())?;
        array.set_region(env, 0, &values)?;
        Ok::<_, jni::errors::Error>(array)
    })
    .unwrap_or_default()
}

/// `ComposeRsNative.textActionModeTitle(id, index)`: the title of item `index` of mode `id`'s
/// menu.
#[unsafe(no_mangle)]
pub extern "system" fn Java_rs_compose_android_ComposeRsNative_textActionModeTitle<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jint,
    index: jint,
) -> JString<'local> {
    let title = MODES.with(|modes| {
        let modes = modes.borrow();
        let item = modes.get(&id).and_then(|mode| mode.menu.items().get(index as usize));
        item.map(|item| item.title.clone())
    });
    with_env(&mut env, |env| env.new_string(title.unwrap_or_default())).unwrap_or_default()
}

/// `ComposeRsNative.textActionModeClick(id, index)`: item `index` of mode `id`'s menu was
/// clicked.
#[unsafe(no_mangle)]
pub extern "system" fn Java_rs_compose_android_ComposeRsNative_textActionModeClick(
    _env: EnvUnowned,
    _class: JClass,
    id: jint,
    index: jint,
) {
    let action = MODES.with(|modes| {
        let modes = modes.borrow();
        modes.get(&id)?.menu.items().get(index as usize).map(|item| item.on_click.clone())
    });
    // Outside the borrow: the action may close the mode.
    if let Some(action) = action {
        action();
    }
}

/// `ComposeRsNative.textActionModeRect(id)`: where mode `id` goes, in the view, as left, top,
/// right and bottom.
#[unsafe(no_mangle)]
pub extern "system" fn Java_rs_compose_android_ComposeRsNative_textActionModeRect<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    id: jint,
) -> JFloatArray<'local> {
    let rect = callback_of(id).map(|callback| callback.on_get_content_rect()).unwrap_or_default();
    let values = [rect.left, rect.top, rect.right, rect.bottom];
    with_env(&mut env, |env| {
        let array = env.new_float_array(4)?;
        array.set_region(env, 0, &values)?;
        Ok::<_, jni::errors::Error>(array)
    })
    .unwrap_or_default()
}

/// `ComposeRsNative.textActionModeDestroyed(id)`: mode `id` closed (finished by Rust, or by the
/// platform: a touch outside it, back).
#[unsafe(no_mangle)]
pub extern "system" fn Java_rs_compose_android_ComposeRsNative_textActionModeDestroyed(
    _env: EnvUnowned,
    _class: JClass,
    id: jint,
) {
    let mode = MODES.with(|modes| modes.borrow_mut().remove(&id));
    if let Some(mode) = mode {
        mode.callback.on_destroy_action_mode();
    }
}
