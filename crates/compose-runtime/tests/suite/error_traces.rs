//! Compose's `ErrorTraceTests`, adapted: a panic unwinding a pass leaves a trace of the
//! composables it went through, innermost first (D1507).
//!
//! Compose checks frames with names, files and lines from the compiler's source information, its
//! lambdas among them, and in its keys mode a number of frames. Here a trace names the
//! composables `#[composable]` made, those of this test file (and the mock's) kept, as Compose
//! keeps its test file's frames; closures aren't groups, and frames have no line. In keys mode the
//! trace has a key for each open group.

#![allow(non_snake_case)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use compose_runtime::stack_trace::*;
use compose_runtime::*;

use crate::mock::*;

/// The panic the tests throw, as `TestComposeException`.
struct TestPanic;

fn throw_test_panic() -> ! {
    std::panic::panic_any(TestPanic)
}

/// Runs `scenario` in each mode, as the Kotlin file's `exceptionTest`: its panic must be the test's,
/// and its trace name `names` (this suite's composables, innermost first), with `keys` frames in
/// keys mode.
fn exception_test(names: &[&str], keys: usize, scenario: impl Fn()) {
    let trace = |mode| {
        set_diagnostic_stack_trace_mode(mode);
        let _ = take_compose_stack_trace();
        let panicked = catch_unwind(AssertUnwindSafe(&scenario)).expect_err("it panics");
        assert!(panicked.is::<TestPanic>(), "the test's own panic");
        set_diagnostic_stack_trace_mode(ComposeStackTraceMode::Auto);
        take_compose_stack_trace().expect("a trace")
    };
    let source = trace(ComposeStackTraceMode::SourceInformation);
    let ours: Vec<&str> = source
        .frames
        .iter()
        .filter(|frame| frame.path.is_some_and(|path| path.starts_with("suite::")))
        .filter_map(|frame| frame.name)
        .collect();
    assert_eq!(ours, names, "{source}");
    let keyed = trace(ComposeStackTraceMode::GroupKeys);
    assert!(keyed.frames.iter().all(|frame| frame.name.is_none() && frame.path.is_none()));
    assert_eq!(keyed.frames.len(), keys, "{keyed}");
    // Auto is source information here.
    assert_eq!(trace(ComposeStackTraceMode::Auto), source);
}

/// Composes `content` in a test, recomposing after setting `state` when it's given.
fn compose_then(content: impl Fn(MutableState<bool>) + 'static) -> impl Fn() {
    let content = Rc::new(content);
    move || {
        let mut test = composition_test();
        let state = mutable_state_of(false);
        let s = state.handle();
        let content = content.clone();
        test.compose(move || content(s));
        state.set(true);
        test.advance();
    }
}

/// The Kotlin file's composables (`ErrorTraceComposables.kt`).
#[composable(non_restartable)]
fn InlineWrapper(content: impl FnOnce()) {
    content();
}

#[composable]
fn Wrapper(content: impl Fn() + 'static) {
    content();
}

#[composable(non_restartable)]
fn InlineLinear(content: impl FnOnce()) {
    ReusableComposeNodeWithContent::<ViewApplier>(view("linear"), |_| {}, content);
}

/// A composition of its own composing `content` where it's called: Compose's sets its content
/// in an effect, which runs after the pass.
#[composable]
fn Subcompose(content: Rc<dyn Fn()>) {
    let context = remember_composition_context();
    let child = remember(move || Rc::new(Composition::with_parent(ViewApplier::new(), context)));
    child.set_content(move || content());
}

/// `ComposableWithDefaults`: Kotlin's default argument is a `remember`, in the composable's own
/// group, before its body.
#[composable]
fn ComposableWithDefaults(block: Rc<dyn Fn(&str)>) {
    let value = remember(String::new);
    block(&value);
}

#[composable]
fn MovableWrapper(content: Rc<dyn Fn()>) {
    let movable = remember(move || movable_content_of(move || content()));
    movable();
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, setContent
#[test]
fn set_content() {
    exception_test(&[], 1, || composition_test().compose(|| throw_test_panic()));
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, recompose
#[test]
fn recompose() {
    exception_test(
        &[],
        1,
        compose_then(|state| {
            if state.get() {
                throw_test_panic();
            }
        }),
    );
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, setContentLinear
#[test]
fn set_content_linear() {
    exception_test(&["Linear"], 4, || {
        composition_test().compose(|| Linear!(|| throw_test_panic()))
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, recomposeLinear
#[test]
fn recompose_linear() {
    exception_test(
        &["Linear"],
        4,
        compose_then(|state| {
            Linear!(move || {
                if state.get() {
                    throw_test_panic();
                }
            })
        }),
    );
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, setContentInlineLinear
#[test]
fn set_content_inline_linear() {
    exception_test(&["InlineLinear"], 3, || {
        composition_test().compose(|| InlineLinear(|| throw_test_panic()))
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, recomposeInlineLinear
#[test]
fn recompose_inline_linear() {
    exception_test(
        &["InlineLinear"],
        3,
        compose_then(|state| {
            InlineLinear(|| {
                if state.get() {
                    throw_test_panic();
                }
            })
        }),
    );
}

/// Five texts in a loop in `wrap`, the fifth panicking once `state` (if given) is set.
fn texts_in_a_loop(wrap: fn(Rc<dyn Fn()>), state: Option<MutableState<bool>>) {
    wrap(Rc::new(move || {
        for index in 0..5 {
            Text("test");
            if index > 3 && state.is_none_or(|state| state.get()) {
                throw_test_panic();
            }
        }
    }));
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, setContentAfterTextInLoopInlineWrapper
#[test]
fn set_content_after_text_in_a_loop_inline_wrapper() {
    exception_test(&["InlineWrapper"], 2, || {
        composition_test()
            .compose(|| texts_in_a_loop(|content| InlineWrapper(move || content()), None))
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, recomposeAfterTextInLoopInlineWrapper
#[test]
fn recompose_after_text_in_a_loop_inline_wrapper() {
    exception_test(
        &["InlineWrapper"],
        2,
        compose_then(|state| {
            texts_in_a_loop(|content| InlineWrapper(move || content()), Some(state))
        }),
    );
}

/// The Kotlin test's loop in `Linear`.
fn linear_wrap(content: Rc<dyn Fn()>) {
    Linear!(move || content());
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, setContentAfterTextInLoop
#[test]
fn set_content_after_text_in_a_loop() {
    exception_test(&["Linear"], 4, || {
        composition_test().compose(|| texts_in_a_loop(linear_wrap, None))
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, recomposeAfterTextInLoop
#[test]
fn recompose_after_text_in_a_loop() {
    exception_test(&["Linear"], 4, compose_then(|state| texts_in_a_loop(linear_wrap, Some(state))));
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, setContentSubcomposition
#[test]
fn set_content_subcomposition() {
    exception_test(&["Subcompose"], 4, || {
        composition_test().compose(|| Subcompose(Rc::new(|| throw_test_panic())))
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, setContentNestedSubcomposition
#[test]
fn set_content_nested_subcomposition() {
    exception_test(&["Subcompose", "Subcompose"], 7, || {
        composition_test()
            .compose(|| Subcompose(Rc::new(|| Subcompose(Rc::new(|| throw_test_panic())))))
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, recomposeSubcomposition
#[test]
fn recompose_subcomposition() {
    exception_test(
        &["Subcompose"],
        5,
        compose_then(|state| {
            Subcompose(Rc::new(move || {
                if state.get() {
                    throw_test_panic();
                }
            }))
        }),
    );
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, setContentDefaults
#[test]
fn set_content_defaults() {
    exception_test(&["ComposableWithDefaults"], 2, || {
        composition_test().compose(|| ComposableWithDefaults(Rc::new(|_| throw_test_panic())))
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, recomposeDefaults
#[test]
fn recompose_defaults() {
    exception_test(
        &["ComposableWithDefaults"],
        2,
        compose_then(|state| {
            ComposableWithDefaults(Rc::new(move |_| {
                if state.get() {
                    throw_test_panic();
                }
            }))
        }),
    );
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, setContentRemember
#[test]
fn set_content_remember() {
    // Compose's frame is `remember`'s own group; here a slot is no group: the trace is empty.
    exception_test(&[], 1, || {
        composition_test().compose(|| {
            remember(|| -> i32 { throw_test_panic() });
        })
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, setContentNodeUpdate
#[test]
fn set_content_node_update() {
    exception_test(&["LinearWithCallbacks", "Wrapper"], 4, || {
        composition_test().compose(|| {
            Wrapper!(|| {
                LinearWithCallbacks(
                    nothing(),
                    nothing(),
                    nothing(),
                    Rc::new(|| throw_test_panic()),
                    || {},
                )
            })
        })
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, recomposeUpdate
#[test]
fn recompose_update() {
    exception_test(
        &["LinearWithCallbacks", "Wrapper"],
        4,
        compose_then(|state| {
            Wrapper!(move || {
                if state.get() {
                    let on_set: Action = Rc::new(|| throw_test_panic());
                    LinearWithCallbacks(nothing(), nothing(), nothing(), on_set, || {});
                }
            })
        }),
    );
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, emptySourceInformation
#[test]
fn keyed_groups_without_a_composable() {
    exception_test(&["InlineWrapper"], 3, || {
        let list = [1, 2, 3];
        composition_test().compose(move || {
            InlineWrapper(|| {
                for item in list {
                    key(item, || {
                        if item == 3 {
                            throw_test_panic();
                        }
                    });
                }
            })
        })
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, setContentMovableContent
#[test]
fn set_content_movable_content() {
    exception_test(&["MovableWrapper"], 4, || {
        composition_test().compose(|| MovableWrapper(Rc::new(|| throw_test_panic())))
    });
}

// Compose: compose/runtime/runtime/src/nonEmulatorJvmTest/kotlin/androidx/compose/runtime/tooling/ErrorTraceTests.kt, recomposeMovableContent
#[test]
fn recompose_movable_content() {
    exception_test(
        &["MovableWrapper"],
        4,
        compose_then(|state| {
            MovableWrapper(Rc::new(move || {
                if state.get() {
                    throw_test_panic();
                }
            }))
        }),
    );
}

/// The mode is the thread's: `None` makes no trace.
#[test]
fn no_trace_in_none_mode() {
    set_diagnostic_stack_trace_mode(ComposeStackTraceMode::None);
    let _ = take_compose_stack_trace();
    let panicked = catch_unwind(AssertUnwindSafe(|| {
        composition_test().compose(|| Linear!(|| throw_test_panic()))
    }));
    set_diagnostic_stack_trace_mode(ComposeStackTraceMode::Auto);
    assert!(panicked.is_err());
    assert!(take_compose_stack_trace().is_none());
}
