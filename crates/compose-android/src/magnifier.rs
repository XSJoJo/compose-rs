//! Android's magnifier (`android.widget.Magnifier`) behind compose-ui's `PlatformMagnifierFactory`,
//! as Compose's `PlatformMagnifierFactoryApi29Impl`: `ComposeRsView.createMagnifier` makes a
//! `ComposeRsMagnifier` (Kotlin) per magnifier node, which Rust then shows, moves, copies and
//! dismisses (D2074). The module's minimum API is 29: the zoom is set in place, and the builder's
//! size, corner radius, elevation and clipping are there.

use std::cell::OnceCell;
use std::rc::Rc;

use compose_ui::{
    IntSize, Offset, PlatformMagnifier, PlatformMagnifierConfig, PlatformMagnifierFactory,
};
use jni::ids::JMethodID;
use jni::objects::JObject;
use jni::refs::Global;
use jni::signature::{Primitive, ReturnType};
use jni::{Env, JValue, jni_sig, jni_str};

use crate::bridge::{check, method, with_main_env};

/// Makes the view's magnifiers.
pub(crate) struct JniMagnifiers {
    view: Rc<Global<JObject<'static>>>,
    create: JMethodID,
    /// `ComposeRsMagnifier`'s methods, looked up with the first one made.
    methods: OnceCell<Rc<MagnifierMethods>>,
}

struct MagnifierMethods {
    get_width: JMethodID,
    get_height: JMethodID,
    update_content: JMethodID,
    show: JMethodID,
    dismiss: JMethodID,
}

impl JniMagnifiers {
    pub(crate) fn new(
        env: &mut Env,
        view: &JObject,
        view_ref: &Rc<Global<JObject<'static>>>,
    ) -> JniMagnifiers {
        let create = method(
            env,
            view,
            jni_str!("createMagnifier"),
            jni_sig!("(ZIIFFZF)Lrs/compose/android/ComposeRsMagnifier;"),
        );
        JniMagnifiers { view: view_ref.clone(), create, methods: OnceCell::new() }
    }
}

/// The platform's pixels of a size in dp, or -1 (the platform's) where it's unspecified.
fn pixels(config: &PlatformMagnifierConfig) -> (i32, i32) {
    if config.size.is_specified() {
        let density = config.density;
        let width = density.to_px(config.size.width).round() as i32;
        let height = density.to_px(config.size.height).round() as i32;
        (width, height)
    } else {
        (-1, -1)
    }
}

impl PlatformMagnifierFactory for JniMagnifiers {
    fn can_update_zoom(&self) -> bool {
        true
    }

    fn create(&self, config: &PlatformMagnifierConfig) -> Rc<dyn PlatformMagnifier> {
        let (width, height) = pixels(config);
        // NaN where unspecified: `Dp::UNSPECIFIED` is NaN dp.
        let corner_radius = config.density.to_px(config.corner_radius);
        let elevation = config.density.to_px(config.elevation);
        let made = with_main_env(|env| {
            env.with_local_frame(2, |env| -> jni::errors::Result<Rc<dyn PlatformMagnifier>> {
                // SAFETY: `createMagnifier` is
                // `(ZIIFFZF)Lrs/compose/android/ComposeRsMagnifier;`.
                let object = unsafe {
                    env.call_method_unchecked(
                        &**self.view,
                        self.create,
                        ReturnType::Object,
                        &[
                            JValue::Bool(config.use_text_default).as_jni(),
                            JValue::Int(width).as_jni(),
                            JValue::Int(height).as_jni(),
                            JValue::Float(corner_radius).as_jni(),
                            JValue::Float(elevation).as_jni(),
                            JValue::Bool(config.clipping_enabled).as_jni(),
                            JValue::Float(config.initial_zoom).as_jni(),
                        ],
                    )
                }?
                .l()?;
                let methods = self
                    .methods
                    .get_or_init(|| {
                        Rc::new(MagnifierMethods {
                            get_width: method(env, &object, jni_str!("getWidth"), jni_sig!("()I")),
                            get_height: method(
                                env,
                                &object,
                                jni_str!("getHeight"),
                                jni_sig!("()I"),
                            ),
                            update_content: method(
                                env,
                                &object,
                                jni_str!("updateContent"),
                                jni_sig!("()V"),
                            ),
                            show: method(env, &object, jni_str!("show"), jni_sig!("(FFFFF)V")),
                            dismiss: method(env, &object, jni_str!("dismiss"), jni_sig!("()V")),
                        })
                    })
                    .clone();
                let object = env.new_global_ref(&object)?;
                Ok(Rc::new(JniMagnifier { object, methods }) as Rc<dyn PlatformMagnifier>)
            })
        });
        match made {
            Ok(magnifier) => magnifier,
            Err(_) => {
                with_main_env(|env| check(env, "ComposeRsView.createMagnifier"));
                // Nothing shown rather than a panic: a magnifier is never needed.
                Rc::new(NoMagnifier)
            }
        }
    }
}

/// One `ComposeRsMagnifier`. A call that fails is logged and counts as done.
struct JniMagnifier {
    object: Global<JObject<'static>>,
    methods: Rc<MagnifierMethods>,
}

impl JniMagnifier {
    fn call_void(&self, method: JMethodID, args: &[jni::sys::jvalue], name: &str) {
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
            check(env, name);
        });
    }

    fn call_int(&self, method: JMethodID, name: &str) -> i32 {
        with_main_env(|env| {
            // SAFETY: each caller's `method` is `()I`.
            let value = unsafe {
                env.call_method_unchecked(
                    &*self.object,
                    method,
                    ReturnType::Primitive(Primitive::Int),
                    &[],
                )
            };
            check(env, name);
            value.and_then(|value| value.i()).unwrap_or(0)
        })
    }
}

impl PlatformMagnifier for JniMagnifier {
    fn size(&self) -> IntSize {
        IntSize::new(
            self.call_int(self.methods.get_width, "ComposeRsMagnifier.getWidth"),
            self.call_int(self.methods.get_height, "ComposeRsMagnifier.getHeight"),
        )
    }

    fn update_content(&self) {
        self.call_void(self.methods.update_content, &[], "ComposeRsMagnifier.updateContent");
    }

    fn update(&self, source_center: Offset, magnifier_center: Offset, zoom: f32) {
        // Unspecified is NaN, which the Kotlin side reads as the platform's place.
        let args = [
            JValue::Float(source_center.x).as_jni(),
            JValue::Float(source_center.y).as_jni(),
            JValue::Float(magnifier_center.x).as_jni(),
            JValue::Float(magnifier_center.y).as_jni(),
            JValue::Float(zoom).as_jni(),
        ];
        self.call_void(self.methods.show, &args, "ComposeRsMagnifier.show");
    }

    fn dismiss(&self) {
        self.call_void(self.methods.dismiss, &[], "ComposeRsMagnifier.dismiss");
    }
}

/// A magnifier that couldn't be made: shows nothing.
struct NoMagnifier;

impl PlatformMagnifier for NoMagnifier {
    fn size(&self) -> IntSize {
        IntSize::ZERO
    }
    fn update_content(&self) {}
    fn update(&self, _source_center: Offset, _magnifier_center: Offset, _zoom: f32) {}
    fn dismiss(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use compose_ui::{Density, Dp, DpSize};

    fn config(size: DpSize, density: f32) -> PlatformMagnifierConfig {
        PlatformMagnifierConfig {
            use_text_default: false,
            size,
            corner_radius: Dp::UNSPECIFIED,
            elevation: Dp::UNSPECIFIED,
            clipping_enabled: true,
            density: Density::new(density),
            initial_zoom: f32::NAN,
        }
    }

    // After Compose's compose/foundation/foundation/src/androidDeviceTest/kotlin/androidx/compose/foundation/PlatformMagnifierTest.kt, androidPlatformMagnifier_usesRequestedSize:
    // the size asked for goes to the builder in pixels, rounded; an unspecified one is the
    // platform's (-1).
    #[test]
    fn the_size_asked_for_is_given_in_pixels() {
        assert_eq!(pixels(&config(DpSize::new(Dp(10.0), Dp(11.0)), 2.0)), (20, 22));
        assert_eq!(pixels(&config(DpSize::new(Dp(10.2), Dp(11.0)), 2.5)), (26, 28));
        assert_eq!(pixels(&config(DpSize::UNSPECIFIED, 2.0)), (-1, -1));
    }

    // The unspecified values are NaN in pixels, which `ComposeRsMagnifier.create` leaves to the
    // builder's defaults, as Compose's `isNaN` checks.
    #[test]
    fn unspecified_radius_and_elevation_are_nan_pixels() {
        let density = Density::new(2.0);
        assert!(density.to_px(Dp::UNSPECIFIED).is_nan());
        assert_eq!(density.to_px(Dp(3.0)), 6.0);
    }
}
