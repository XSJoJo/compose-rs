//! Android's platform services (`LOCAL_URI_HANDLER`, `LOCAL_HAPTIC_FEEDBACK`; the clipboard is
//! the bridge's `JniClipboard`), through the view: `ComposeRsView.openUri` starts an
//! `ACTION_VIEW` intent, `performHapticFeedbackType` calls `View.performHapticFeedback`. Both run
//! in the middle of a call into Rust (a click's callback), so the view posts them to the main
//! looper, as it does host events (D226). Links are checked first, in Rust
//! (`PlatformUriHandler`: allowed schemes only, D283). The edge effects of overscroll are here too:
//! `android.widget.EdgeEffect`s, in Kotlin's `ComposeRsEdgeEffect`, called only from a scroller's
//! first pull at an edge on (D1472).

use std::cell::OnceCell;
use std::rc::Rc;

use compose_ui::{
    Color, EdgeEffect, EdgeEffectFactory, EdgeEffectSide, HapticFeedback, HapticFeedbackType,
    PlatformUriHandler, platform::UiHost,
};
use jni::ids::JMethodID;
use jni::objects::JObject;
use jni::refs::Global;
use jni::signature::{Primitive, ReturnType};
use jni::{Env, JValue, jni_sig, jni_str};

use crate::bridge::{check, method, with_main_env};

/// Sets `ui`'s tree's services to the view's.
pub(crate) fn install(
    env: &mut Env,
    view: &JObject,
    view_ref: &Rc<Global<JObject<'static>>>,
    ui: &UiHost,
) {
    let open_uri = method(env, view, jni_str!("openUri"), jni_sig!("(Ljava/lang/String;)V"));
    let perform = method(env, view, jni_str!("performHapticFeedbackType"), jni_sig!("(I)V"));
    let opener = JniUriOpener { view: view_ref.clone(), method: open_uri };
    let uri_handler = PlatformUriHandler::new(move |uri| opener.open(uri));
    let haptic_feedback = Rc::new(JniHapticFeedback { view: view_ref.clone(), method: perform });
    let edge_effects = Rc::new(JniEdgeEffects {
        view: view_ref.clone(),
        create: method(
            env,
            view,
            jni_str!("createEdgeEffect"),
            jni_sig!("(III)Lrs/compose/android/ComposeRsEdgeEffect;"),
        ),
        release: method(env, view, jni_str!("releaseEdgeEffects"), jni_sig!("(I)V")),
        methods: OnceCell::new(),
    });
    ui.with_tree(|tree| {
        tree.set_platform_uri_handler(uri_handler);
        tree.set_haptic_feedback(haptic_feedback);
        tree.set_edge_effect_factory(edge_effects);
    });
}

/// Makes `ComposeRsEdgeEffect`s through the view.
struct JniEdgeEffects {
    view: Rc<Global<JObject<'static>>>,
    create: JMethodID,
    release: JMethodID,
    methods: OnceCell<Rc<EdgeEffectMethods>>,
}

/// `ComposeRsEdgeEffect`'s methods, looked up with the first one made.
struct EdgeEffectMethods {
    on_pull_distance: JMethodID,
    on_release: JMethodID,
    on_release_with_opposite_delta: JMethodID,
    on_absorb: JMethodID,
    get_distance: JMethodID,
    is_finished: JMethodID,
    finish: JMethodID,
    set_size: JMethodID,
}

impl EdgeEffectFactory for JniEdgeEffects {
    fn create_edge_effect(
        &self,
        owner: u32,
        side: EdgeEffectSide,
        glow_color: Color,
    ) -> Rc<dyn EdgeEffect> {
        with_main_env(|env| {
            env.with_local_frame(2, |env| -> jni::errors::Result<Rc<dyn EdgeEffect>> {
                // SAFETY: `createEdgeEffect` is `(III)Lrs/compose/android/ComposeRsEdgeEffect;`.
                let object = unsafe {
                    env.call_method_unchecked(
                        &**self.view,
                        self.create,
                        ReturnType::Object,
                        &[
                            JValue::Int(owner as i32).as_jni(),
                            JValue::Int(side as i32).as_jni(),
                            JValue::Int(glow_color.argb() as i32).as_jni(),
                        ],
                    )
                }?
                .l()?;
                let methods = self
                    .methods
                    .get_or_init(|| {
                        Rc::new(EdgeEffectMethods {
                            on_pull_distance: method(
                                env,
                                &object,
                                jni_str!("onPullDistance"),
                                jni_sig!("(FF)F"),
                            ),
                            on_release: method(
                                env,
                                &object,
                                jni_str!("onRelease"),
                                jni_sig!("()V"),
                            ),
                            on_release_with_opposite_delta: method(
                                env,
                                &object,
                                jni_str!("onReleaseWithOppositeDelta"),
                                jni_sig!("(F)V"),
                            ),
                            on_absorb: method(env, &object, jni_str!("onAbsorb"), jni_sig!("(I)V")),
                            get_distance: method(
                                env,
                                &object,
                                jni_str!("getDistance"),
                                jni_sig!("()F"),
                            ),
                            is_finished: method(
                                env,
                                &object,
                                jni_str!("isFinished"),
                                jni_sig!("()Z"),
                            ),
                            finish: method(env, &object, jni_str!("finish"), jni_sig!("()V")),
                            set_size: method(env, &object, jni_str!("setSize"), jni_sig!("(II)V")),
                        })
                    })
                    .clone();
                let object = env.new_global_ref(&object)?;
                Ok(Rc::new(JniEdgeEffect { object, methods }))
            })
            .expect("compose-android: an edge effect")
        })
    }

    fn release_edge_effects(&self, owner: u32) {
        with_main_env(|env| {
            // SAFETY: `releaseEdgeEffects` is `(I)V`.
            let _ = unsafe {
                env.call_method_unchecked(
                    &**self.view,
                    self.release,
                    ReturnType::Primitive(Primitive::Void),
                    &[JValue::Int(owner as i32).as_jni()],
                )
            };
            check(env, "ComposeRsView.releaseEdgeEffects");
        });
    }
}

/// One `ComposeRsEdgeEffect`. Calls that fail (an exception, logged) count as an effect at rest.
struct JniEdgeEffect {
    object: Global<JObject<'static>>,
    methods: Rc<EdgeEffectMethods>,
}

impl JniEdgeEffect {
    fn call_void(&self, method: JMethodID, args: &[jni::sys::jvalue]) {
        with_main_env(|env| {
            // SAFETY: each caller's `method` returns void and takes `args`.
            let _ = unsafe {
                env.call_method_unchecked(
                    &*self.object,
                    method,
                    ReturnType::Primitive(Primitive::Void),
                    args,
                )
            };
            check(env, "ComposeRsEdgeEffect");
        });
    }
}

impl EdgeEffect for JniEdgeEffect {
    fn on_pull_distance(&self, delta: f32, displacement: f32) -> f32 {
        with_main_env(|env| {
            // SAFETY: `onPullDistance` is `(FF)F`.
            let taken = unsafe {
                env.call_method_unchecked(
                    &*self.object,
                    self.methods.on_pull_distance,
                    ReturnType::Primitive(Primitive::Float),
                    &[JValue::Float(delta).as_jni(), JValue::Float(displacement).as_jni()],
                )
            };
            check(env, "ComposeRsEdgeEffect.onPullDistance");
            taken.and_then(|value| value.f()).unwrap_or(0.0)
        })
    }

    fn on_release(&self) {
        self.call_void(self.methods.on_release, &[]);
    }

    fn on_release_with_opposite_delta(&self, delta: f32) {
        self.call_void(
            self.methods.on_release_with_opposite_delta,
            &[JValue::Float(delta).as_jni()],
        );
    }

    fn on_absorb(&self, velocity: i32) {
        self.call_void(self.methods.on_absorb, &[JValue::Int(velocity).as_jni()]);
    }

    fn distance(&self) -> f32 {
        with_main_env(|env| {
            // SAFETY: `getDistance` is `()F`.
            let distance = unsafe {
                env.call_method_unchecked(
                    &*self.object,
                    self.methods.get_distance,
                    ReturnType::Primitive(Primitive::Float),
                    &[],
                )
            };
            check(env, "ComposeRsEdgeEffect.getDistance");
            distance.and_then(|value| value.f()).unwrap_or(0.0)
        })
    }

    fn is_finished(&self) -> bool {
        with_main_env(|env| {
            // SAFETY: `isFinished` is `()Z`.
            let finished = unsafe {
                env.call_method_unchecked(
                    &*self.object,
                    self.methods.is_finished,
                    ReturnType::Primitive(Primitive::Boolean),
                    &[],
                )
            };
            check(env, "ComposeRsEdgeEffect.isFinished");
            finished.and_then(|value| value.z()).unwrap_or(true)
        })
    }

    fn finish(&self) {
        self.call_void(self.methods.finish, &[]);
    }

    fn set_size(&self, width: i32, height: i32) {
        self.call_void(
            self.methods.set_size,
            &[JValue::Int(width).as_jni(), JValue::Int(height).as_jni()],
        );
    }
}

/// Hands a checked link to `ComposeRsView.openUri`.
struct JniUriOpener {
    view: Rc<Global<JObject<'static>>>,
    method: JMethodID,
}

impl JniUriOpener {
    fn open(&self, uri: &str) {
        with_main_env(|env| {
            let _ = env.with_local_frame(2, |env| -> jni::errors::Result<()> {
                let uri = env.new_string(uri)?;
                // SAFETY: `openUri` is `(Ljava/lang/String;)V`.
                unsafe {
                    env.call_method_unchecked(
                        &**self.view,
                        self.method,
                        ReturnType::Primitive(Primitive::Void),
                        &[JValue::Object(&uri).as_jni()],
                    )?;
                }
                Ok(())
            });
            check(env, "ComposeRsView.openUri");
        });
    }
}

struct JniHapticFeedback {
    view: Rc<Global<JObject<'static>>>,
    method: JMethodID,
}

impl HapticFeedback for JniHapticFeedback {
    fn perform_haptic_feedback(&self, feedback_type: HapticFeedbackType) {
        with_main_env(|env| {
            // SAFETY: `performHapticFeedbackType` is `(I)V`.
            let _ = unsafe {
                env.call_method_unchecked(
                    &**self.view,
                    self.method,
                    ReturnType::Primitive(Primitive::Void),
                    &[JValue::Int(haptic_code(feedback_type)).as_jni()],
                )
            };
            check(env, "ComposeRsView.performHapticFeedbackType");
        });
    }
}

/// The code `ComposeRsView.performHapticFeedbackType` maps to a `HapticFeedbackConstants` value:
/// the type's index in Compose's order.
fn haptic_code(feedback_type: HapticFeedbackType) -> i32 {
    feedback_type as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ComposeRsView.performHapticFeedbackType`'s `when` depends on these.
    #[test]
    fn haptic_codes_match_the_views() {
        use HapticFeedbackType::*;
        let codes: Vec<i32> = [
            LongPress,
            TextHandleMove,
            Confirm,
            ContextClick,
            GestureEnd,
            GestureThresholdActivate,
            KeyboardTap,
            Reject,
            SegmentFrequentTick,
            SegmentTick,
            ToggleOff,
            ToggleOn,
            VirtualKey,
        ]
        .into_iter()
        .map(haptic_code)
        .collect();
        assert_eq!(codes, (0..13).collect::<Vec<_>>());
    }
}
