//! What the tree looks at for the platform's autofill (D1688): only the nodes noted since the last
//! sync (semantics invalidated or forgotten, attached, detached, deactivated, reused; measured
//! where what they had was autofill's), so a frame that changes nothing autofill reads looks at
//! nothing; and nothing at all while the platform's autofill is off.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::PlatformAutofillCall as Call;
use compose_ui::*;

const SIDE: f32 = 50.0;

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

fn visibility(semantics_id: i32, is_visible: bool) -> Call {
    Call::NotifyViewVisibilityChanged { semantics_id, is_visible }
}

fn looked_at(ui: &mut ComposeTestRule) -> usize {
    ui.host().with_tree(|tree| tree.autofill_nodes_looked_at())
}

fn pending(ui: &mut ComposeTestRule) -> bool {
    ui.host().with_tree(|tree| tree.autofill_pending())
}

fn username(tag: &'static str) -> Modifier {
    Modifier.semantics(move |config| {
        config.set_test_tag(tag);
        config.set_content_type(ContentType::USERNAME);
    })
}

// ---- Nothing changed, nothing looked at ----------------------------------------------------------

/// A box measured again at each frame (its parent's width a state), its semantics nothing
/// autofill reads, beside a field.
#[test]
fn a_node_measured_again_without_autofill_semantics_is_not_looked_at() {
    let width = mutable_state_of(SIDE);
    let read = width.handle();
    let (mut ui, am) = set_test_content(move || {
        Column!({
            // The counted box measured again with new constraints at each width.
            Box!(modifier = Modifier.layout_width(read), {
                Box!(
                    modifier = Modifier
                        .semantics(|config| config.set_test_tag("box"))
                        .fill_max_width()
                        .height(SIDE.dp())
                );
            });
            let text = remember(|| mutable_state_of(String::from("kept")));
            BasicTextField!(text.get(), move |value| text.set(value));
        });
    });
    let before = looked_at(&mut ui);
    for step in 1..5 {
        ui.run_on_idle(|_| width.set(SIDE + step as f32));
        ui.wait_for_idle();
    }
    assert_eq!(looked_at(&mut ui), before, "the sync looked at a measured node");
    assert!(!pending(&mut ui));
    am.assert_no_more_calls();
}

trait LayoutWidth {
    /// Its width read from `width` while it's measured, as a layout modifier reading state.
    fn layout_width(self, width: MutableState<f32>) -> Modifier;
}

impl LayoutWidth for Modifier {
    fn layout_width(self, width: MutableState<f32>) -> Modifier {
        self.then(Modifier.layout(move |scope, measurable, constraints| {
            let px = width.get().round() as i32;
            let fixed = Constraints::new(px, px, 0, constraints.max_height);
            let placeable = scope.measure(measurable, fixed);
            scope.layout_with(placeable.width, placeable.height, move |placement| {
                placement.place(&placeable, 0, 0);
            })
        }))
    }
}

// A frame where nothing was noted doesn't sync: an idle frame, a frame drawing only.
#[test]
fn nothing_noted_nothing_to_sync() {
    let alpha = mutable_state_of(1.0f32);
    let read = alpha.handle();
    let (mut ui, am) = set_test_content(move || {
        Box!(modifier = username("username").graphics_layer_alpha(read).size(SIDE.dp()));
    });
    assert!(!pending(&mut ui));
    ui.run_on_idle(|_| alpha.set(0.5));
    ui.wait_for_idle();
    assert!(!pending(&mut ui));
    am.assert_no_more_calls();
}

trait LayerAlpha {
    fn graphics_layer_alpha(self, alpha: MutableState<f32>) -> Modifier;
}

impl LayerAlpha for Modifier {
    fn graphics_layer_alpha(self, alpha: MutableState<f32>) -> Modifier {
        self.graphics_layer(move |layer| layer.alpha = alpha.get())
    }
}

// ---- What's noted is looked at -------------------------------------------------------------------

// A semantics block reading state, invalidated by its write: looked at, its new content type
// told.
#[test]
fn semantics_invalidated_by_a_write_are_looked_at() {
    let has = mutable_state_of(false);
    let read = has.handle();
    let (mut ui, am) = set_test_content(move || {
        Box!(
            modifier = Modifier
                .semantics(move |config| {
                    config.set_test_tag("node");
                    if read.get() {
                        config.set_fillable_data(FillableData::create_from_text("a").unwrap());
                    }
                })
                .size(SIDE.dp())
        );
    });
    ui.run_on_idle(|_| has.set(true));
    ui.wait_for_idle();
    let node = id(&mut ui, "node");
    am.verify(&visibility(node, true));
    am.assert_no_more_calls();
}

// A content type's modifier taken off its node (the chain changed, no entry attached): the last
// one with a content type gone, the platform commits.
#[test]
fn a_content_type_s_modifier_taken_off_commits() {
    let has = mutable_state_of(true);
    let read = has.handle();
    let (mut ui, am) = set_test_content(move || {
        let semantics = if read.get() { username("username") } else { Modifier };
        Box!(modifier = semantics.size(SIDE.dp()));
    });
    ui.run_on_idle(|_| has.set(false));
    ui.wait_for_idle();
    am.verify(&Call::Commit);
    am.assert_no_more_calls();
}

// A field's text changed by typing: its value told, at each key.
#[test]
fn a_field_typed_in_tells_each_value() {
    let (mut ui, am) = set_test_content(|| {
        let text = remember(|| mutable_state_of(String::new()));
        BasicTextField!(
            text.get(),
            move |value| text.set(value),
            modifier = Modifier.test_tag("f")
        );
    });
    ui.on_node_with_tag("f").request_focus();
    ui.wait_for_idle();
    let field = id(&mut ui, "f");
    am.clear();
    for typed in ["a", "ab", "abc"] {
        ui.host().send_text(&typed[typed.len() - 1..]);
        ui.wait_for_idle();
        let value = FillableData::create_from_text(typed).unwrap();
        am.verify(&Call::NotifyValueChanged { semantics_id: field, value });
    }
    am.assert_no_more_calls();
}

// A state-based field changed from its state, outside any event: its value told.
#[test]
fn a_state_field_edited_from_its_state_tells_its_value() {
    let state = TextFieldState::new("");
    let state = **Box::leak(Box::new(state));
    let (mut ui, am) = set_test_content(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag("f"));
    });
    let field = id(&mut ui, "f");
    ui.run_on_idle(|_| state.set_text_and_place_cursor_at_end("typed"));
    ui.wait_for_idle();
    let value = FillableData::create_from_text("typed").unwrap();
    // Twice, as Compose's: its input text changed, and its fillable data.
    am.verify_times(&Call::NotifyValueChanged { semantics_id: field, value }, 2);
}

// Content moved by movable content (detached and attached in one pass, its id kept) isn't shown
// again.
#[test]
fn content_moved_in_one_pass_is_not_shown_again() {
    let left = mutable_state_of(true);
    let read = left.handle();
    let content = movable_content_of(|| {
        Box!(modifier = username("username").size(SIDE.dp()));
    });
    let (mut ui, am) = set_test_content(move || {
        let moved = !read.get();
        let content = content.clone();
        Row!({
            let first = content.clone();
            Box!({
                if !moved {
                    first();
                }
            });
            let then = content.clone();
            Box!({
                if moved {
                    then();
                }
            });
        });
    });
    let before = id(&mut ui, "username");
    ui.run_on_idle(|_| left.set(false));
    ui.wait_for_idle();
    assert_eq!(id(&mut ui, "username"), before);
    am.assert_no_more_calls();
}

// A lazy item scrolled out and back: hidden as deactivated, shown again with a new id when
// reused, through a `BoxWithConstraints` (another composition inside the item).
#[test]
fn a_lazy_item_inside_another_composition_hides_and_shows() {
    let state = Rc::new(Cell::new(None));
    let into = state.clone();
    let (mut ui, am) = set_test_content(move || {
        let list = compose_foundation::lazy::remember_lazy_list_state(0, 0);
        into.set(Some(list));
        LazyColumn!(
            modifier = Modifier.size(SIDE.dp()),
            state = list,
            move |scope: LazyListScope| {
                scope.items(10, move |index| {
                    BoxWithConstraints!(modifier = Modifier.size(SIDE.dp()), move |_scope| {
                        if index == 0 {
                            Box!(modifier = username("username").size(SIDE.dp()));
                        }
                    });
                });
            }
        );
    });
    let first = id(&mut ui, "username");
    let list = state.get().expect("composed");
    ui.run_on_idle(|_| list.request_scroll_to_item(5, 0));
    ui.wait_for_idle();
    am.verify(&visibility(first, false));
    // The only one with a content type gone.
    am.verify(&Call::Commit);
    am.assert_no_more_calls();
    ui.run_on_idle(|_| list.request_scroll_to_item(0, 0));
    ui.wait_for_idle();
    let again = id(&mut ui, "username");
    am.verify(&visibility(again, true));
    am.assert_no_more_calls();
}

// ---- The platform's switch -----------------------------------------------------------------------

// Off, the platform hears nothing and nothing is noted; the manager stays the local's value and
// the platform has autofill still. On again, it hears of what's there.
#[test]
fn off_nothing_is_tracked_and_on_again_what_s_there_is_told() {
    let shown = mutable_state_of(false);
    let read = shown.handle();
    let manager = Rc::new(Cell::new(false));
    let has_manager = manager.clone();
    let (mut ui, am) = set_test_content(move || {
        has_manager.set(LOCAL_AUTOFILL_MANAGER.current().is_some());
        if read.get() {
            Box!(modifier = username("username").size(SIDE.dp()));
        }
    });
    ui.host().with_tree(|tree| tree.set_platform_autofill_enabled(false));
    ui.run_on_idle(|_| shown.set(true));
    ui.wait_for_idle();
    assert!(!pending(&mut ui));
    am.assert_no_more_calls();
    assert!(manager.get());
    assert!(ui.host().with_tree(|tree| tree.is_autofill_available()));
    assert!(!ui.host().with_tree(|tree| tree.is_platform_autofill_enabled()));
    ui.host().with_tree(|tree| tree.set_platform_autofill_enabled(true));
    assert!(pending(&mut ui));
    ui.run_on_idle(|ui| {
        let calls = ui.host().with_tree(|tree| tree.sync_autofill());
        for call in calls {
            compose_ui::platform::PlatformAutofillManager::call(&*am, call);
        }
    });
    let node = id(&mut ui, "username");
    am.verify(&visibility(node, true));
    am.assert_no_more_calls();
}
