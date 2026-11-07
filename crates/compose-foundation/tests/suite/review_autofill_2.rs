//! Review of `autofill-2` (D1688): the dirty marks the platform's autofill hears of, the semantics
//! observation apart, and `semantics_tree` applying pending writes first. Failing ones are
//! `#[ignore = "review: ..."]`.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::PlatformAutofillCall as Call;
use compose_ui::*;
use compose_ui::{Dialog, Popup};

const SIDE: f32 = 50.0;

/// A rule with a recording manager from the first frame, its calls cleared once idle. Nothing
/// reads the semantics tree (no finder): as on a phone without an accessibility service, only
/// the autofill sync computes semantics.
fn set_test_content(content: impl Fn() + 'static) -> (ComposeTestRule, Rc<TestAutofillManager>) {
    let mut ui = ComposeTestRule::empty(default_host());
    let am = ui.autofill();
    ui.set_content(content);
    ui.run_on_idle(|_| am.clear());
    (ui, am)
}

fn id(ui: &mut ComposeTestRule, tag: &str) -> i32 {
    ui.on_node_with_tag(tag).fetch_semantics_snapshot().semantics_id
}

fn values(am: &TestAutofillManager) -> Vec<String> {
    am.calls()
        .into_iter()
        .filter_map(|call| match call {
            Call::NotifyValueChanged { value, .. } => value.text_value().map(|t| t.to_string()),
            _ => None,
        })
        .collect()
}

// ---- 1. A state-based field's text adopted while measuring ---------------------------------------

// The state-based field takes in its state's text while it's measured (`adopt_state_to_measure`),
// without `invalidate_semantics`: D1257's measure hook forgets its kept semantics, and D1688 notes
// the node for autofill only if what was dropped was autofill's. The autofill sync computes the
// field's semantics without keeping them (`AutofillSemantics::Computed`), so with no
// accessibility service (nothing keeps them), the next measurement drops nothing and the field
// isn't noted: a programmatic edit (a "clear" button, a value restored) is never told.
// `autofill_tracking::a_state_field_edited_from_its_state_tells_its_value` passes only because
// its `id()` (a finder) keeps the semantics before the edit.
#[test]
fn a_state_field_edited_from_its_state_without_a_semantics_read_tells_its_value() {
    let state = TextFieldState::new("");
    let state = **Box::leak(Box::new(state));
    let (mut ui, am) = set_test_content(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag("f"));
    });
    ui.run_on_idle(|_| state.set_text_and_place_cursor_at_end("typed"));
    ui.wait_for_idle();
    // Read after the edit only.
    let field = id(&mut ui, "f");
    let value = FillableData::create_from_text("typed").unwrap();
    assert_eq!(values(&am), vec![String::from("typed"); 2], "calls: {:?}", am.calls());
    am.verify_times(&Call::NotifyValueChanged { semantics_id: field, value }, 2);
}

// The same, a second edit after a first one the sync heard of (its semantics computed, not kept,
// at that sync).
#[test]
fn a_state_field_edited_twice_from_its_state_tells_each_value() {
    let state = TextFieldState::new("");
    let state = **Box::leak(Box::new(state));
    let (mut ui, am) = set_test_content(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag("f"));
    });
    // Kept by a finder, so the first edit is noted by the measure hook.
    let field = id(&mut ui, "f");
    ui.run_on_idle(|_| state.set_text_and_place_cursor_at_end("one"));
    ui.wait_for_idle();
    // The sync computed them for autofill only: nothing kept now.
    ui.run_on_idle(|_| state.set_text_and_place_cursor_at_end("two"));
    ui.wait_for_idle();
    let two = FillableData::create_from_text("two").unwrap();
    am.verify_times(&Call::NotifyValueChanged { semantics_id: field, value: two }, 2);
}

// Control: typed through events (`publish_state_in` invalidates the semantics), told.
#[test]
fn a_state_field_typed_in_without_a_semantics_read_tells_its_value() {
    let state = TextFieldState::new("");
    let state = **Box::leak(Box::new(state));
    let focus = FocusRequester::new();
    let (mut ui, am) = set_test_content(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.focus_requester(focus).test_tag("f"));
    });
    ui.run_on_idle(|_| focus.request_focus());
    ui.wait_for_idle();
    am.clear();
    ui.host().send_text("a");
    ui.wait_for_idle();
    assert_eq!(values(&am), vec![String::from("a"); 2], "calls: {:?}", am.calls());
}

// ---- 2. Semantics written in a frame after its last drain ----------------------------------------

// The semantics observer asks for no frame while one runs (`framing`, D1688): a write sent in a
// frame after its last drain (`draw_pass`'s) would wait, `pending`, for an unrelated frame. These
// try the windows after drawing (an effect's first step, a mutable snapshot applied, a send); all
// pass: a frame follows, and the platform is told.

/// A box with fillable data while `has` is true.
fn maybe_username(has: MutableState<bool>) -> Modifier {
    Modifier.semantics(move |config| {
        config.set_test_tag("node");
        if has.get() {
            // Fillable data: shown (`notifyViewVisibilityChanged`), as Compose's.
            config.set_fillable_data(FillableData::create_from_text("a").unwrap());
        }
    })
}

// A `LaunchedEffect`'s step writes state only a semantics block reads.
#[test]
fn a_semantics_read_written_by_an_effect_after_drawing_is_told() {
    let key = mutable_state_of(0);
    let has = mutable_state_of(false);
    let (read_key, read_has) = (key.handle(), has.handle());
    let write_has = has.handle();
    let (mut ui, am) = set_test_content(move || {
        let key = read_key.get();
        LaunchedEffect(key, async move {
            if key > 0 {
                write_has.set(true);
            }
        });
        Box!(modifier = maybe_username(read_has).size(SIDE.dp()));
    });
    ui.run_on_idle(|_| key.set(1));
    ui.wait_for_idle();
    assert_eq!(am.calls().len(), 1, "calls: {:?}", am.calls());
}

// The same write applied from a mutable snapshot in that step.
#[test]
fn a_semantics_read_written_from_a_mutable_snapshot_after_drawing_is_told() {
    let key = mutable_state_of(0);
    let has = mutable_state_of(false);
    let (read_key, read_has) = (key.handle(), has.handle());
    let write_has = has.handle();
    let (mut ui, am) = set_test_content(move || {
        let key = read_key.get();
        LaunchedEffect(key, async move {
            if key > 0 {
                compose_runtime::snapshots::Snapshot::with_mutable_snapshot(|| write_has.set(true));
            }
        });
        Box!(modifier = maybe_username(read_has).size(SIDE.dp()));
    });
    ui.run_on_idle(|_| key.set(1));
    ui.wait_for_idle();
    assert_eq!(am.calls().len(), 1, "calls: {:?}", am.calls());
}

// The same write sent explicitly in that step (`Snapshot.sendApplyNotifications()`, public API).
#[test]
fn a_semantics_read_written_and_sent_after_drawing_is_told() {
    let key = mutable_state_of(0);
    let has = mutable_state_of(false);
    let (read_key, read_has) = (key.handle(), has.handle());
    let write_has = has.handle();
    let (mut ui, am) = set_test_content(move || {
        let key = read_key.get();
        LaunchedEffect(key, async move {
            if key > 0 {
                write_has.set(true);
                compose_runtime::snapshots::Snapshot::send_apply_notifications();
            }
        });
        Box!(modifier = maybe_username(read_has).size(SIDE.dp()));
    });
    ui.run_on_idle(|_| key.set(1));
    ui.wait_for_idle();
    assert_eq!(am.calls().len(), 1, "calls: {:?}", am.calls());
    assert!(!ui.host().needs_frame());
}

// Without autofill, the frame says the semantics changed (what Android's accessibility hears at
// once, `tell_at_once`), for the same write sent after drawing.
#[test]
fn a_semantics_write_sent_after_drawing_says_the_semantics_changed() {
    let _autofill = compose_test::use_autofill(false);
    let key = mutable_state_of(0);
    let label = mutable_state_of(String::from("a"));
    let (read_key, read_label) = (key.handle(), label.handle());
    let write_label = label.handle();
    let mut ui = ComposeTestRule::empty(default_host());
    ui.set_content(move || {
        let key = read_key.get();
        LaunchedEffect(key, async move {
            if key > 0 {
                write_label.set(String::from("b"));
                compose_runtime::snapshots::Snapshot::send_apply_notifications();
            }
        });
        Box!(
            modifier = Modifier
                .semantics(move |config| {
                    config.set_test_tag("node");
                    config.set_content_description(read_label.get());
                })
                .size(SIDE.dp())
        );
    });
    ui.on_node_with_tag("node").assert_content_description_equals(&["a"]);
    let changed = Rc::new(Cell::new(false));
    key.set(1);
    // Frames by hand, as the platform runs them while one's asked for.
    let mut frames = 0;
    while ui.host().needs_frame() && frames < 10 {
        let stats = ui.host().frame();
        changed.set(changed.get() || stats.semantics_changed);
        frames += 1;
    }
    assert!(changed.get(), "no frame said the semantics changed ({frames} frames)");
}

// ---- 3. A parts' host noted itself ------------------------------------------------------------

// A field's content type (on its decoration, the parts' host) taken off while its text is kept: the
// host is noted, so its own semantics are computed again, not taken from the last sync's
// (`Live::hosts`); the last node with a content type gone, the platform commits. (A hand mutant
// always using the kept host snapshot survived the suite.)
#[test]
fn a_field_s_content_type_taken_off_its_host_commits() {
    let has = mutable_state_of(true);
    let read = has.handle();
    let (mut ui, am) = set_test_content(move || {
        let text = remember(|| mutable_state_of(String::from("kept")));
        let modifier =
            if read.get() { Modifier.content_type(ContentType::USERNAME) } else { Modifier };
        BasicTextField!(text.get(), move |value| text.set(value), modifier = modifier);
    });
    ui.run_on_idle(|_| has.set(false));
    ui.wait_for_idle();
    am.verify(&Call::Commit);
    am.assert_no_more_calls();
}

// ---- 4. The autofill sync's computation replaces the semantics' observation ----------------------

/// A scroll whose node's semantics a finder kept (observing its value), then forgotten by a test
/// tag's update: the frame's autofill sync computes what autofill reads of them (nothing, for a
/// scroll and a tag), observed under the same key, which replaces the full semantics' reads. Its
/// value then written: returns whether a frame said the semantics changed.
fn scroll_after_autofill_semantics(autofill: bool) -> bool {
    use compose_foundation::gestures::ScrollableState;
    let _autofill = compose_test::use_autofill(autofill);
    let tag = mutable_state_of("a");
    let read = tag.handle();
    let kept = Rc::new(Cell::new(None));
    let into = kept.clone();
    let mut ui = ComposeTestRule::empty(default_host());
    ui.set_content(move || {
        let scroll = remember_scroll_state(0);
        into.set(Some(scroll));
        Column!(
            modifier = Modifier.test_tag(read.get()).size(SIDE.dp()).vertical_scroll(scroll),
            {
                Box!(modifier = Modifier.size((SIDE * 4.0).dp()));
            }
        );
    });
    ui.on_node_with_tag("a").assert_exists();
    tag.set("b");
    let mut frames = 0;
    while ui.host().needs_frame() && frames < 10 {
        ui.host().frame();
        frames += 1;
    }
    let scroll = kept.get().expect("composed");
    scroll.dispatch_raw_delta(10.0);
    let mut changed = false;
    let mut frames = 0;
    while ui.host().needs_frame() && frames < 10 {
        changed |= ui.host().frame().semantics_changed;
        frames += 1;
    }
    assert_eq!(scroll.value(), 10);
    changed
}

// Control: without autofill, the scroll's frame says its semantics changed (Android's
// accessibility tells a service at once, `tell_at_once`).
#[test]
fn without_autofill_a_scroll_says_its_semantics_changed() {
    assert!(scroll_after_autofill_semantics(false));
}

// With autofill, the sync's computation of what autofill reads (nothing, for a scroll and a tag)
// replaced the full semantics' reads under the node's key: the scroll's write invalidates nothing,
// and no frame says the semantics changed (Android's accessibility tells a service within 100 ms,
// from the measurement, instead of at once). The frame that measured it says it may have.
#[test]
fn with_autofill_a_scroll_says_its_semantics_changed() {
    assert!(scroll_after_autofill_semantics(true));
}

// ---- 5. Content in other windows ------------------------------------------------------------------

/// A box with a username's content type and fillable data, tagged.
fn filled_username(tag: &'static str) -> Modifier {
    Modifier.semantics(move |config| {
        config.set_test_tag(tag);
        config.set_content_type(ContentType::USERNAME);
        config.set_fillable_data(FillableData::create_from_text("a").unwrap());
    })
}

// A popup's and a dialog's content come and go: shown, then hidden, as the same content on the
// screen (twice each, as Compose's `onPostAttach` for the content type and `onSemanticsChanged`
// for the fillable data), then the platform commits.
#[test]
fn a_popup_s_and_a_dialog_s_autofill_content_comes_and_goes() {
    let shown = mutable_state_of(false);
    let read = shown.handle();
    let (mut ui, am) = set_test_content(move || {
        Box!(modifier = Modifier.size(SIDE.dp()));
        if read.get() {
            Box!(modifier = filled_username("screen").size(SIDE.dp()));
            Popup!(on_dismiss_request = || {}, {
                Box!(modifier = filled_username("popup").size(SIDE.dp()));
            });
            Dialog!(on_dismiss_request = || {}, {
                Box!(modifier = filled_username("dialog").size(SIDE.dp()));
            });
        }
    });
    ui.run_on_idle(|_| shown.set(true));
    ui.wait_for_idle();
    let ids = [id(&mut ui, "screen"), id(&mut ui, "popup"), id(&mut ui, "dialog")];
    for is_visible in [true, false] {
        if !is_visible {
            ui.run_on_idle(|_| shown.set(false));
            ui.wait_for_idle();
        }
        let counts = ids.map(|semantics_id| {
            am.count(&Call::NotifyViewVisibilityChanged { semantics_id, is_visible })
        });
        assert!(counts[0] > 0, "calls: {:?}", am.calls());
        assert_eq!(counts, [counts[0]; 3], "calls: {:?}", am.calls());
    }
    am.verify(&Call::Commit);
}

// A state-based field in a popup edited from its state, its semantics kept by a finder first: its
// value told.
#[test]
fn a_state_field_in_a_popup_edited_from_its_state_tells_its_value() {
    let state = TextFieldState::new("");
    let state = **Box::leak(Box::new(state));
    let (mut ui, am) = set_test_content(move || {
        Popup!(on_dismiss_request = || {}, {
            BasicTextFieldOfState!(state, modifier = Modifier.test_tag("f"));
        });
    });
    let field = id(&mut ui, "f");
    ui.run_on_idle(|_| state.set_text_and_place_cursor_at_end("typed"));
    ui.wait_for_idle();
    let value = FillableData::create_from_text("typed").unwrap();
    am.verify_times(&Call::NotifyValueChanged { semantics_id: field, value }, 2);
}
