//! The platform's autofill as the tree tells it, as Compose's `AndroidAutofillManagerTest`: the
//! test rule's tree has a recording platform manager (`rule.autofill()`, D1680), where Compose's
//! tests put a mock in `AndroidAutofillManager`. Each test ends with what Compose's `@After`
//! checks: nothing told that wasn't verified. Compose's tests of the deprecated `onAutofillText`
//! aren't ported (n/a: deprecated API).

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::PlatformAutofillCall as Call;
use compose_ui::*;

const SIDE: f32 = 200.0;

/// Content in a rule, what its first frames told the platform forgotten, as Compose's
/// `setTestContent`.
fn set_test_content(content: impl Fn() + 'static) -> (ComposeTestRule, Rc<TestAutofillManager>) {
    let (mut ui, am) = with_autofill();
    ui.set_content(content);
    ui.run_on_idle(|_| am.clear());
    (ui, am)
}

/// A rule whose tree has the recording platform manager from its first frame.
fn with_autofill() -> (ComposeTestRule, Rc<TestAutofillManager>) {
    let ui = ComposeTestRule::empty(default_host());
    let am = ui.autofill();
    (ui, am)
}

fn id(ui: &mut ComposeTestRule, tag: &str) -> i32 {
    ui.on_node_with_tag(tag).fetch_semantics_snapshot().semantics_id
}

/// A `SIDE`-dp square's bounds at the origin, in pixels.
fn square(ui: &ComposeTestRule) -> IntRect {
    let side = (SIDE * ui.density().density) as i32;
    IntRect { left: 0, top: 0, right: side, bottom: side }
}

fn visibility(semantics_id: i32, is_visible: bool) -> Call {
    Call::NotifyViewVisibilityChanged { semantics_id, is_visible }
}

fn value_changed(semantics_id: i32, value: FillableData) -> Call {
    Call::NotifyValueChanged { semantics_id, value }
}

fn text(value: &str) -> FillableData {
    FillableData::create_from_text(value.to_string()).unwrap()
}

/// Whether it told any value change.
fn told_value_change(am: &TestAutofillManager) -> bool {
    am.calls().iter().any(|call| matches!(call, Call::NotifyValueChanged { .. }))
}

fn square_modifier() -> Modifier {
    Modifier.size(SIDE.dp())
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_initialization
/// The first frame tells the platform of each node with a content type on screen.
#[test]
fn the_first_frame_shows_the_nodes_with_a_content_type() {
    let (mut ui, am) = with_autofill();
    ui.set_content(|| {
        Box!(
            modifier = Modifier
                .semantics(|config| {
                    config.set_test_tag("username");
                    config.set_content_type(ContentType::USERNAME);
                })
                .then(square_modifier())
        );
    });
    ui.wait_for_idle();
    let username = id(&mut ui, "username");
    am.verify(&visibility(username, true));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_doNotCallCommit_nodesAppeared
/// A node with a content type appearing shows, and commits nothing.
#[test]
fn a_node_appearing_shows_without_a_commit() {
    let is_visible = mutable_state_of(false);
    let shown = is_visible.handle();
    let (mut ui, am) = set_test_content(move || {
        if shown.get() {
            Box!(
                modifier = Modifier
                    .semantics(|config| {
                        config.set_test_tag("username");
                        config.set_content_type(ContentType::USERNAME);
                    })
                    .then(square_modifier())
            );
        }
    });
    ui.run_on_idle(|_| is_visible.set(true));
    ui.wait_for_idle();
    am.verify_never(&Call::Commit);
    let username = id(&mut ui, "username");
    am.verify(&visibility(username, true));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_doNotCallCommit_autofillTagsAdded
/// A content type added to a node commits nothing (and tells nothing: the node was there).
#[test]
fn a_content_type_added_commits_nothing() {
    let has_content_type = mutable_state_of(false);
    let has = has_content_type.handle();
    let (mut ui, am) = set_test_content(move || {
        let semantics = if has.get() {
            Modifier.semantics(|config| config.set_content_type(ContentType::USERNAME))
        } else {
            Modifier
        };
        Box!(modifier = semantics.then(square_modifier()));
    });
    ui.run_on_idle(|_| has_content_type.set(true));
    ui.run_on_idle(|_| am.verify_never(&Call::Commit));
    am.assert_no_more_calls();
}

/// Two boxes with a content type, the second shown while `second` is.
fn two_usernames(second: MutableState<bool>) -> impl Fn() + 'static {
    move || {
        Box!(
            modifier = Modifier
                .semantics(|config| config.set_content_type(ContentType::USERNAME))
                .then(square_modifier())
        );
        if second.get() {
            Box!(
                modifier = Modifier
                    .semantics(|config| {
                        config.set_content_type(ContentType::USERNAME);
                        config.set_test_tag("username");
                    })
                    .then(square_modifier())
            );
        }
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_doNotCallCommit_partialRemoval
/// One of two nodes with a content type gone: it's hidden, nothing's committed.
#[test]
fn one_of_two_gone_commits_nothing() {
    let reveal = mutable_state_of(true);
    let (mut ui, am) = set_test_content(two_usernames(reveal.handle()));
    let username = id(&mut ui, "username");
    ui.run_on_idle(|_| reveal.set(false));
    ui.run_on_idle(|_| am.verify_never(&Call::Commit));
    am.verify(&visibility(username, false));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_doNotCallCommit_partialAddition
/// A second node with a content type coming: it shows, nothing's committed.
#[test]
fn a_second_coming_commits_nothing() {
    let reveal = mutable_state_of(false);
    let (mut ui, am) = set_test_content(two_usernames(reveal.handle()));
    ui.run_on_idle(|_| reveal.set(true));
    ui.wait_for_idle();
    am.verify_never(&Call::Commit);
    let username = id(&mut ui, "username");
    am.verify(&visibility(username, true));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_doNotCallCommit_noAutofillableComponents
/// Content without a content type gone: nothing to commit.
#[test]
fn nothing_autofillable_gone_commits_nothing() {
    let reveal = mutable_state_of(true);
    let shown = reveal.handle();
    let (mut ui, am) = set_test_content(move || {
        if shown.get() {
            Box!(modifier = square_modifier());
        }
    });
    ui.run_on_idle(|_| reveal.set(false));
    ui.run_on_idle(|_| am.verify_never(&Call::Commit));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_callCommit_nodesDisappeared
/// The last node with a content type gone: hidden, then the session committed.
#[test]
fn the_last_one_gone_commits() {
    let reveal = mutable_state_of(true);
    let shown = reveal.handle();
    let (mut ui, am) = set_test_content(move || {
        if shown.get() {
            Box!(
                modifier = Modifier
                    .semantics(|config| {
                        config.set_test_tag("username");
                        config.set_content_type(ContentType::USERNAME);
                    })
                    .then(square_modifier())
            );
        }
    });
    let username = id(&mut ui, "username");
    ui.run_on_idle(|_| reveal.set(false));
    ui.wait_for_idle();
    am.verify(&Call::Commit);
    am.verify(&visibility(username, false));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_callCommit_nodesDisappearedAndAppeared
/// The last one gone commits, though another comes after.
#[test]
fn the_last_one_gone_commits_before_another_comes() {
    let first = mutable_state_of(true);
    let second = mutable_state_of(false);
    let (show_first, show_second) = (first.handle(), second.handle());
    let (mut ui, am) = set_test_content(move || {
        for (shown, tag) in [(show_first.get(), "first username"), (show_second.get(), "second")] {
            if shown {
                Box!(
                    modifier = Modifier
                        .semantics(move |config| {
                            config.set_test_tag(tag);
                            config.set_content_type(ContentType::USERNAME);
                        })
                        .then(square_modifier())
                );
            }
        }
    });
    let first_id = id(&mut ui, "first username");
    ui.run_on_idle(|_| first.set(false));
    ui.run_on_idle(|_| second.set(true));
    let second_id = id(&mut ui, "second");
    ui.wait_for_idle();
    am.verify(&visibility(first_id, false));
    am.verify(&Call::Commit);
    am.verify(&visibility(second_id, true));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_doNotCallCommit_nonAutofillRelatedNodesAddedAndDisappear
/// Semantics not for autofill come and go: nothing's committed.
#[test]
fn other_semantics_coming_and_going_commit_nothing() {
    let is_visible = mutable_state_of(true);
    let semantics_exist = mutable_state_of(false);
    let (shown, exists) = (is_visible.handle(), semantics_exist.handle());
    let (mut ui, am) = set_test_content(move || {
        if shown.get() {
            let semantics = if exists.get() {
                Modifier.semantics(|config| config.set_content_description("contentDescription"))
            } else {
                Modifier
            };
            Box!(modifier = semantics.then(square_modifier()));
        }
    });
    ui.run_on_idle(|_| semantics_exist.set(true));
    ui.run_on_idle(|_| is_visible.set(false));
    ui.run_on_idle(|_| am.verify_never(&Call::Commit));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_callCommit_nodesBecomeAutofillRelatedAndDisappear
/// A node given a content type, then gone: committed. (A content type added tells nothing; the
/// node going hides it.)
#[test]
fn a_node_given_a_content_type_then_gone_commits() {
    let is_visible = mutable_state_of(true);
    let has_content_type = mutable_state_of(false);
    let (shown, has) = (is_visible.handle(), has_content_type.handle());
    let (mut ui, am) = set_test_content(move || {
        if shown.get() {
            let semantics = if has.get() {
                Modifier.semantics(|config| {
                    config.set_test_tag("username");
                    config.set_content_type(ContentType::USERNAME);
                })
            } else {
                Modifier
            };
            Box!(modifier = semantics.then(square_modifier()));
        }
    });
    ui.run_on_idle(|_| has_content_type.set(true));
    let username = id(&mut ui, "username");
    ui.run_on_idle(|_| is_visible.set(false));
    ui.wait_for_idle();
    am.verify(&Call::Commit);
    am.verify(&visibility(username, false));
    am.assert_no_more_calls();
}

/// A box filling, with `fillable` as its data, if any, and `extra` set too.
fn filling_box(
    fillable: impl Fn() -> Option<FillableData> + 'static,
    extra: impl Fn(&mut SemanticsConfiguration) + 'static,
) {
    Box!(
        modifier = Modifier
            .semantics(move |config| {
                config.set_test_tag("username");
                config.on_fill_data(None, |_| true);
                extra(config);
                if let Some(data) = fillable() {
                    config.set_fillable_data(data);
                }
            })
            .then(square_modifier())
    );
}

/// The fillable data changes from `from` to `to` (`None`: removed); returns what was told.
fn fillable_data_change(
    from: Option<FillableData>,
    to: Option<FillableData>,
) -> (ComposeTestRule, Rc<TestAutofillManager>, i32) {
    let changed = mutable_state_of(false);
    let read = changed.handle();
    let (mut ui, am) = set_test_content(move || {
        let (from, to) = (from.clone(), to.clone());
        filling_box(move || if read.get() { to.clone() } else { from.clone() }, |_| {});
    });
    ui.run_on_idle(|_| changed.set(true));
    ui.wait_for_idle();
    let username = id(&mut ui, "username");
    (ui, am, username)
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyFillableDataChanged
/// New fillable data: its value is told.
#[test]
fn new_fillable_data_is_told() {
    let (_ui, am, username) = fillable_data_change(Some(text("5678")), Some(text("1234")));
    am.verify(&value_changed(username, text("1234")));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyFillableDataChanged_fromEmpty
#[test]
fn fillable_data_from_empty_is_told() {
    let (_ui, am, username) = fillable_data_change(Some(text("")), Some(text("1234")));
    am.verify(&value_changed(username, text("1234")));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyFillableDataChanged_toEmpty
#[test]
fn fillable_data_to_empty_is_told() {
    let (_ui, am, username) = fillable_data_change(Some(text("1234")), Some(text("")));
    am.verify(&value_changed(username, text("")));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyFillableDataChanged_removed
/// Fillable data removed: the node hides, no value told.
#[test]
fn fillable_data_removed_hides() {
    let (_ui, am, username) = fillable_data_change(Some(text("1234")), None);
    assert!(!told_value_change(&am));
    am.verify(&visibility(username, false));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyFillableDataChanged_addedEmpty
/// Fillable data added, empty: the node shows, no value told.
#[test]
fn empty_fillable_data_added_shows() {
    let (_ui, am, username) = fillable_data_change(None, Some(text("")));
    assert!(!told_value_change(&am));
    am.verify(&visibility(username, true));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyFillableDataChanged_removedEmpty
#[test]
fn empty_fillable_data_removed_hides() {
    let (_ui, am, username) = fillable_data_change(Some(text("")), None);
    assert!(!told_value_change(&am));
    am.verify(&visibility(username, false));
    am.assert_no_more_calls();
}

/// A text node with a content type whose input text goes from `from` to `to` (`None`: none).
fn input_text_change(
    from: Option<&'static str>,
    to: Option<&'static str>,
) -> (ComposeTestRule, Rc<TestAutofillManager>, i32) {
    let changed = mutable_state_of(false);
    let read = changed.handle();
    let (mut ui, am) = set_test_content(move || {
        let input = if read.get() { to } else { from };
        Box!(
            modifier = Modifier
                .semantics(move |config| {
                    config.set_test_tag("username");
                    config.set_content_type(ContentType::USERNAME);
                    config.set_content_data_type(ContentDataType::TEXT);
                    if let Some(input) = input {
                        config.set_input_text(input);
                    }
                })
                .then(square_modifier())
        );
    });
    ui.run_on_idle(|_| changed.set(true));
    ui.wait_for_idle();
    let username = id(&mut ui, "username");
    (ui, am, username)
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyValueChanged
/// The input text changed: its value is told.
#[test]
fn a_changed_input_text_is_told() {
    let (_ui, am, username) = input_text_change(Some(""), Some("1234"));
    am.verify(&value_changed(username, text("1234")));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyValueChanged_toEmpty
#[test]
fn an_input_text_emptied_is_told() {
    let (_ui, am, username) = input_text_change(Some("1234"), Some(""));
    am.verify(&value_changed(username, text("")));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyValueChanged_inputTextAdded
/// An input text added: the node shows, no value told.
#[test]
fn an_input_text_added_shows() {
    let (_ui, am, username) = input_text_change(None, Some("1234"));
    assert!(!told_value_change(&am));
    am.verify(&visibility(username, true));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyValueChanged_inputTextRemoved
#[test]
fn an_input_text_removed_hides() {
    let (_ui, am, username) = input_text_change(Some("1234"), None);
    assert!(!told_value_change(&am));
    am.verify(&visibility(username, false));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyValueChanged_addedEmptyInputText
#[test]
fn an_empty_input_text_added_shows() {
    let (_ui, am, username) = input_text_change(None, Some(""));
    assert!(!told_value_change(&am));
    am.verify(&visibility(username, true));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyValueChanged_removedEmptyInputText
#[test]
fn an_empty_input_text_removed_hides() {
    let (_ui, am, username) = input_text_change(Some(""), None);
    assert!(!told_value_change(&am));
    am.verify(&visibility(username, false));
    am.assert_no_more_calls();
}

/// A focusable square with `semantics`, tagged "username".
fn focusable_box(semantics: impl Fn(&mut SemanticsConfiguration) + Clone + 'static) {
    let semantics = semantics.clone();
    Box!(
        modifier = Modifier
            .semantics(move |config| {
                config.set_test_tag("username");
                semantics(config);
            })
            .then(square_modifier())
            .focusable()
    );
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyViewEntered_previousFocusFalse
// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyViewEntered_previousFocusNull
/// A node that fills taking the focus enters, with its bounds. (Compose's two tests differ by
/// the order of the modifiers' arguments only.)
#[test]
fn a_filling_node_focused_enters() {
    let (mut ui, am) = set_test_content(|| {
        focusable_box(|config| config.on_fill_data(None, |_| true));
    });
    ui.on_node_with_tag("username").request_focus();
    ui.wait_for_idle();
    let username = id(&mut ui, "username");
    am.verify(&Call::NotifyViewEntered { semantics_id: username, bounds: square(&ui) });
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notAutofillable_notifyViewEntered_previousFocusFalse
/// A node with a content type that doesn't fill, focused: nothing's told.
#[test]
fn a_node_not_filling_focused_tells_nothing() {
    let (mut ui, am) = set_test_content(|| {
        focusable_box(|config| {
            config.set_content_type(ContentType::USERNAME);
            config.set_content_data_type(ContentDataType::TEXT);
        });
    });
    ui.on_node_with_tag("username").request_focus();
    ui.run_on_idle(|_| am.assert_no_more_calls());
}

/// The focus manager of the content, kept as it composes.
fn focus_manager_of(into: &Rc<RefCell<Option<FocusManager>>>) {
    *into.borrow_mut() = Some(LOCAL_FOCUS_MANAGER.current());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyViewExited_previousFocusTrue
/// A focused node that fills losing the focus exits. Compose's test checks it enters again in
/// keyboard mode: Android's view gives the focus back to its first focusable when cleared there,
/// which a host's tree doesn't (the test rule's isn't a view).
#[test]
fn a_filling_node_losing_the_focus_exits() {
    let focus_manager = Rc::new(RefCell::new(None));
    let into = focus_manager.clone();
    let (mut ui, am) = set_test_content(move || {
        focus_manager_of(&into);
        focusable_box(|config| config.on_fill_data(None, |_| true));
    });
    ui.on_node_with_tag("username").request_focus();
    let username = id(&mut ui, "username");
    ui.run_on_idle(|_| am.clear());
    let manager = focus_manager.borrow().clone().expect("composed");
    ui.run_on_idle(|_| manager.clear_focus(false));
    ui.wait_for_idle();
    am.verify(&Call::NotifyViewExited { semantics_id: username });
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyViewExited_previouslyFocusedItemNotAutofillable
/// A node that doesn't fill, focused and unfocused: nothing's told.
#[test]
fn a_node_not_filling_unfocused_tells_nothing() {
    let focus_manager = Rc::new(RefCell::new(None));
    let into = focus_manager.clone();
    let (mut ui, am) = set_test_content(move || {
        focus_manager_of(&into);
        focusable_box(|_| {});
    });
    ui.on_node_with_tag("username").request_focus();
    let manager = focus_manager.borrow().clone().expect("composed");
    ui.run_on_idle(|_| manager.clear_focus(false));
    ui.run_on_idle(|_| am.assert_no_more_calls());
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyVisibilityChanged_lazyScroll
/// A lazy row scrolled: the item scrolled out hides, the one scrolled in shows (a new id).
#[test]
fn a_lazy_scroll_hides_and_shows_items() {
    let state = Rc::new(Cell::new(None));
    let into = state.clone();
    let (mut ui, am) = set_test_content(move || {
        let list = compose_foundation::lazy::remember_lazy_list_state(0, 0);
        into.set(Some(list));
        let side = 10.0 / LOCAL_DENSITY.current().density;
        LazyRow!(modifier = Modifier.size(side.dp()), state = list, move |scope: LazyListScope| {
            scope.items(2, move |_| {
                Box!(
                    modifier = Modifier
                        .size(side.dp())
                        .semantics(|config| {
                            config.set_test_tag("username");
                            config.set_content_type(ContentType::USERNAME);
                        })
                        .focusable()
                );
            });
        });
    });
    let before = id(&mut ui, "username");
    let list = state.get().expect("composed");
    ui.run_on_idle(|_| list.request_scroll_to_item(1, 0));
    let after = id(&mut ui, "username");
    assert_ne!(before, after);
    ui.wait_for_idle();
    am.verify(&visibility(before, false));
    am.verify(&visibility(after, true));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyCommit
/// `LOCAL_AUTOFILL_MANAGER`'s `commit` commits.
#[test]
fn the_local_manager_commits() {
    let (mut ui, am) = set_test_content(|| {
        let manager = LOCAL_AUTOFILL_MANAGER.current();
        Box!(
            modifier = Modifier
                .clickable(move || {
                    if let Some(manager) = &manager {
                        manager.commit();
                    }
                })
                .then(square_modifier())
                .test_tag("forward_button_tag")
        );
    });
    ui.on_node_with_tag("forward_button_tag").perform_click();
    ui.run_on_idle(|_| am.verify(&Call::Commit));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_notifyCancel
/// Its `cancel` cancels.
#[test]
fn the_local_manager_cancels() {
    let (mut ui, am) = set_test_content(|| {
        let manager = LOCAL_AUTOFILL_MANAGER.current();
        Box!(
            modifier = Modifier
                .clickable(move || {
                    if let Some(manager) = &manager {
                        manager.cancel();
                    }
                })
                .then(square_modifier())
                .test_tag("back_button_tag")
        );
    });
    ui.on_node_with_tag("back_button_tag").perform_click();
    ui.run_on_idle(|_| am.verify(&Call::Cancel));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_lazyColumnScroll_callsCommit
/// A lazy column scrolled past its only node with a content type: hidden, committed.
#[test]
fn a_lazy_scroll_past_the_last_one_commits() {
    let state = Rc::new(Cell::new(None));
    let into = state.clone();
    let (mut ui, am) = set_test_content(move || {
        let list = compose_foundation::lazy::remember_lazy_list_state(0, 0);
        into.set(Some(list));
        let side = 10.0 / LOCAL_DENSITY.current().density;
        LazyColumn!(
            modifier = Modifier.fill_max_width().height(50.dp()),
            state = list,
            move |scope: LazyListScope| {
                scope.item(move || {
                    Box!(
                        modifier = Modifier
                            .semantics(|config| {
                                config.set_test_tag("username");
                                config.set_content_type(ContentType::USERNAME);
                            })
                            .size(side.dp())
                    );
                });
                scope.items(100, move |_| Box!(modifier = Modifier.size(side.dp())));
            }
        );
    });
    let username = id(&mut ui, "username");
    let list = state.get().expect("composed");
    let scope = CoroutineScope::new();
    scope.launch(async move {
        let _ = list.scroll_to_item(10, 0).await;
    });
    ui.wait_for_idle();
    am.verify(&Call::Commit);
    am.verify(&visibility(username, false));
    am.assert_no_more_calls();
}

/// A scrolling column of 50 boxes after a row with a box with a content type, while `shown`.
fn scrolling_column(state: Rc<Cell<Option<ScrollState>>>, shown: Option<MutableState<bool>>) {
    let scroll = remember_scroll_state(0);
    state.set(Some(scroll));
    let side = 10.0 / LOCAL_DENSITY.current().density;
    Column!(modifier = Modifier.fill_max_width().height(50.dp()).vertical_scroll(scroll), |_| {
        if shown.is_none_or(|shown| shown.get()) {
            Row!({
                Box!(
                    modifier = Modifier
                        .semantics(|config| {
                            config.set_test_tag("username");
                            config.set_content_type(ContentType::USERNAME);
                        })
                        .size(side.dp())
                );
            });
        }
        for _ in 0..50 {
            Box!(modifier = Modifier.size(side.dp()));
        }
    });
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_columnScroll_doesNotCallCommit
/// A column scrolled past its node with a content type: still there, nothing committed.
#[test]
fn a_column_scrolled_commits_nothing() {
    let state = Rc::new(Cell::new(None));
    let into = state.clone();
    let (mut ui, am) = set_test_content(move || scrolling_column(into.clone(), None));
    let scroll = state.get().expect("composed");
    let scope = CoroutineScope::new();
    scope.launch(async move {
        let _ = scroll.scroll_to(scroll.max_value() / 2).await;
    });
    ui.run_on_idle(|_| am.verify_never(&Call::Commit));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_column_nodesDisappearingCallsCommit
/// The column's node with a content type removed: hidden, committed.
#[test]
fn a_column_s_node_removed_commits() {
    let state = Rc::new(Cell::new(None));
    let visible = mutable_state_of(true);
    let (into, shown) = (state.clone(), visible.handle());
    let (mut ui, am) = set_test_content(move || scrolling_column(into.clone(), Some(shown)));
    let username = id(&mut ui, "username");
    ui.run_on_idle(|_| visible.set(false));
    ui.wait_for_idle();
    am.verify(&visibility(username, false));
    am.verify(&Call::Commit);
    am.assert_no_more_calls();
}

/// A custom semantics node with a content type and `apply`'s, as Compose's
/// `TestSemanticsModifier`, whose handle asks for autofill.
#[derive(Clone)]
struct TestSemantics {
    apply: Rc<dyn Fn(&mut SemanticsConfiguration)>,
    handle: Rc<RefCell<Option<NodeHandle>>>,
}

struct TestSemanticsNode(TestSemantics);

impl ModifierNodeElement for TestSemantics {
    type Node = TestSemanticsNode;
    fn create(&self) -> TestSemanticsNode {
        TestSemanticsNode(self.clone())
    }
    fn update(&self, node: &mut TestSemanticsNode) {
        node.0 = self.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.apply, &other.apply)
    }
}

impl ModifierNode for TestSemanticsNode {
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        *self.0.handle.borrow_mut() = Some(context.handle());
    }
    fn as_semantics(&mut self) -> Option<&mut dyn SemanticsModifierNode> {
        Some(self)
    }
}

impl SemanticsModifierNode for TestSemanticsNode {
    fn apply_semantics(&self, config: &mut SemanticsConfiguration) {
        config.set_content_type(ContentType::USERNAME);
        (self.0.apply)(config);
    }
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_requestAutofill
/// A node asking for autofill: the platform is asked, with its id and bounds.
#[test]
fn a_node_requests_autofill() {
    let semantics = TestSemantics {
        apply: Rc::new(|config| config.set_test_tag("TestTag")),
        handle: Rc::new(RefCell::new(None)),
    };
    let given = semantics.clone();
    let (mut ui, am) =
        set_test_content(move || Box!(modifier = square_modifier().then_custom(given.clone())));
    let handle = (*semantics.handle.borrow()).expect("attached");
    ui.run_on_idle(|_| handle.request_autofill());
    ui.wait_for_idle();
    let tag = id(&mut ui, "TestTag");
    am.verify(&Call::RequestAutofill { semantics_id: tag, bounds: square(&ui) });
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_performAutofill_callsOnFillData
/// The platform's values fill the nodes of their ids through `OnFillData`.
#[test]
fn filling_calls_on_fill_data() {
    let filled = Rc::new(RefCell::new(None));
    let into = filled.clone();
    let (mut ui, am) = set_test_content(move || {
        let into = into.clone();
        Box!(
            modifier = Modifier.semantics(move |config| {
                config.set_test_tag("autofill_node");
                let into = into.clone();
                config.on_fill_data(None, move |data| {
                    *into.borrow_mut() = Some(data);
                    true
                });
            })
        );
    });
    let node = id(&mut ui, "autofill_node");
    ui.run_on_idle(|ui| ui.host().perform_autofill(vec![(node, text("autofill text"))]));
    ui.run_on_idle(|_| assert_eq!(*filled.borrow(), Some(text("autofill text"))));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_doesNotNotify_contentDataType_none
/// A node of data type `NONE`: its changes tell nothing.
#[test]
fn a_node_of_data_type_none_tells_nothing() {
    let changed = mutable_state_of(false);
    let read = changed.handle();
    let (mut ui, am) = set_test_content(move || {
        filling_box(
            move || Some(text(if read.get() { "1234" } else { "" })),
            |config| config.set_content_data_type(ContentDataType::NONE),
        );
    });
    ui.run_on_idle(|_| changed.set(true));
    ui.wait_for_idle();
    am.assert_no_more_calls();
}

/// A filling box whose data type goes from `from` to `to`, its data from "" to "1234".
fn data_type_change(
    from: ContentDataType,
    to: ContentDataType,
) -> (ComposeTestRule, Rc<TestAutofillManager>, i32) {
    let changed = mutable_state_of(false);
    let (read, read_type) = (changed.handle(), changed.handle());
    let (mut ui, am) = set_test_content(move || {
        let data_type = if read_type.get() { to } else { from };
        filling_box(
            move || Some(text(if read.get() { "1234" } else { "" })),
            move |config| config.set_content_data_type(data_type),
        );
    });
    ui.run_on_idle(|_| changed.set(true));
    ui.wait_for_idle();
    let username = id(&mut ui, "username");
    (ui, am, username)
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_viewEntered_contentDataType_fromNone
/// From `NONE` to text: the node shows, and its new value is told.
#[test]
fn a_data_type_from_none_shows_the_node() {
    let (_ui, am, username) = data_type_change(ContentDataType::NONE, ContentDataType::TEXT);
    am.verify(&visibility(username, true));
    am.verify(&value_changed(username, text("1234")));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_viewEntered_contentDataType_toNone
/// From text to `NONE`: the node hides, and nothing else is told.
#[test]
fn a_data_type_to_none_hides_the_node() {
    let (_ui, am, username) = data_type_change(ContentDataType::TEXT, ContentDataType::NONE);
    am.verify(&visibility(username, false));
    am.assert_no_more_calls();
}

// Compose: compose/ui/ui/src/androidDeviceTest/kotlin/androidx/compose/ui/autofill/AndroidAutofillManagerTest.kt, autofillManager_focus_contendDataTypeNone
/// A node of data type `NONE` that fills, focused: nothing's told.
#[test]
fn a_node_of_data_type_none_focused_tells_nothing() {
    let (mut ui, am) = set_test_content(|| {
        focusable_box(|config| {
            config.on_fill_data(None, |_| true);
            config.set_content_data_type(ContentDataType::NONE);
        });
    });
    ui.on_node_with_tag("username").request_focus();
    ui.wait_for_idle();
    am.assert_no_more_calls();
}

// ---- compose-rs's own --------------------------------------------------------------------------

/// Without a platform manager (desktop, a rule that installed none), the local is `None`.
#[test]
fn without_a_platform_manager_the_local_is_none() {
    let seen = Rc::new(Cell::new(true));
    let into = seen.clone();
    let _ui = compose_test(move || into.set(LOCAL_AUTOFILL_MANAGER.current().is_some()));
    assert!(!seen.get());
}

/// A lazy row's items with a content type and an input text, scrolled: the item scrolled out (its
/// node deactivated, then reused) hides once, as Compose's deactivation (no semantics change told
/// for it); the one scrolled in shows twice (its input text appearing, then `onPostLayoutNodeReused`).
#[test]
fn a_lazy_scroll_of_text_items_hides_the_old_once() {
    let state = Rc::new(Cell::new(None));
    let into = state.clone();
    let (mut ui, am) = set_test_content(move || {
        let list = compose_foundation::lazy::remember_lazy_list_state(0, 0);
        into.set(Some(list));
        let side = 10.0 / LOCAL_DENSITY.current().density;
        LazyRow!(modifier = Modifier.size(side.dp()), state = list, move |scope: LazyListScope| {
            scope.items(2, move |index| {
                Box!(
                    modifier = Modifier.size(side.dp()).semantics(move |config| {
                        config.set_test_tag("username");
                        config.set_content_type(ContentType::USERNAME);
                        config.set_input_text(format!("item {index}"));
                    })
                );
            });
        });
    });
    let before = id(&mut ui, "username");
    let list = state.get().expect("composed");
    ui.run_on_idle(|_| list.request_scroll_to_item(1, 0));
    let after = id(&mut ui, "username");
    ui.wait_for_idle();
    am.verify(&visibility(before, false));
    am.verify_times(&visibility(after, true), 2);
    am.assert_no_more_calls();
}

/// A field's value goes to the field's node (its parts' host), not a node with semantics between
/// it and its inner field (a decoration's).
#[test]
fn a_field_s_value_goes_to_the_field_not_its_decoration() {
    let (mut ui, am) = set_test_content(|| {
        let state = remember_text_field_state("", None);
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag("field").semantics(|config| {
                config.set_content_type(ContentType::USERNAME);
            }),
            decorator = |inner: compose_runtime::Callback| {
                Box!(
                    modifier = Modifier
                        .semantics(|config| config.set_content_description("decoration"))
                        .test_tag("decoration"),
                    { inner() }
                );
            },
        );
    });
    ui.on_node_with_tag("field").request_focus();
    ui.wait_for_idle();
    am.clear();
    ui.on_node_with_tag("field").perform_text_input("a");
    ui.wait_for_idle();
    let field = id(&mut ui, "field");
    am.verify_times(&value_changed(field, text("a")), 2);
    am.assert_no_more_calls();
}

/// A toggle's new state is told twice, as Compose's: its toggle state and its fillable data.
#[test]
fn a_toggle_s_new_state_is_told() {
    let checked = mutable_state_of(false);
    let handle = checked.handle();
    let (mut ui, am) = set_test_content(move || {
        Box!(
            modifier = Modifier
                .toggleable(handle.get(), move |it| handle.set(it))
                .then(square_modifier())
                .test_tag("toggle")
        );
    });
    ui.run_on_idle(|_| checked.set(true));
    ui.wait_for_idle();
    let toggle = id(&mut ui, "toggle");
    let on = FillableData::create_from_boolean(true).unwrap();
    am.verify_times(&value_changed(toggle, on), 2);
    am.assert_no_more_calls();
}
