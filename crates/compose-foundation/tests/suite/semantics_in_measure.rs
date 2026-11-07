//! Semantics a node changes as it's measured (D1689): a state-based field taking in its state's
//! value, or what it shows of it, says its semantics changed, as Compose's decorator observes the
//! state in its semantics; the frame tells accessibility at once, and the platform's autofill is
//! told whether or not a service kept the field's semantics. Also D1689's other fixes: the
//! autofill-only computation keeping the full one's reads, a part moved to another owner, a node
//! noted since the last sync losing the focus, and `use_autofill`'s guard.

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::platform::PlatformAutofillCall as Call;
use compose_ui::*;

const SIDE: f32 = 50.0;

/// A rule with a recording manager from the first frame, its calls cleared once idle. Nothing
/// reads the semantics tree (no finder) unless the test does.
fn set_test_content(content: impl Fn() + 'static) -> (ComposeTestRule, Rc<TestAutofillManager>) {
    let mut ui = ComposeTestRule::empty(default_host());
    let am = ui.autofill();
    ui.set_content(content);
    ui.run_on_idle(|_| am.clear());
    (ui, am)
}

/// A rule without a platform manager, whatever `COMPOSE_TEST_AUTOFILL` says: only accessibility
/// reads the semantics.
fn without_autofill(content: impl Fn() + 'static) -> ComposeTestRule {
    let _autofill = compose_test::use_autofill(false);
    let mut ui = ComposeTestRule::empty(default_host());
    ui.set_content(content);
    ui
}

fn id(ui: &mut ComposeTestRule, tag: &str) -> i32 {
    ui.on_node_with_tag(tag).fetch_semantics_snapshot().semantics_id
}

fn visibility(semantics_id: i32, is_visible: bool) -> Call {
    Call::NotifyViewVisibilityChanged { semantics_id, is_visible }
}

/// Runs the frames asked for by hand, as the platform runs them: whether one said the semantics
/// changed (what Android's accessibility tells a service at once, `tell_at_once`).
fn frames_say_semantics_changed(ui: &mut ComposeTestRule) -> bool {
    let mut changed = false;
    let mut frames = 0;
    while ui.host().needs_frame() && frames < 10 {
        changed |= ui.host().frame().semantics_changed;
        frames += 1;
    }
    changed
}

fn editable_text(ui: &mut ComposeTestRule, tag: &str) -> Option<String> {
    let node = ui.on_node_with_tag(tag).fetch_semantics_snapshot();
    node.config.get(&SemanticsProperties::EDITABLE_TEXT).map(|text| text.to_string())
}

/// A custom node whose semantics are `apply`'s, computed when asked (their reads observed, unlike
/// `Modifier.semantics`', run as it composes), what autofill reads of them `autofill`'s if given.
fn semantics_of(
    apply: impl Fn(&mut SemanticsConfiguration) + 'static,
    autofill: Option<Apply>,
) -> Modifier {
    Modifier.then_custom(SemanticsOf { apply: Rc::new(apply), autofill })
}

type Apply = Rc<dyn Fn(&mut SemanticsConfiguration)>;

#[derive(Clone)]
struct SemanticsOf {
    apply: Apply,
    autofill: Option<Apply>,
}

impl ModifierNodeElement for SemanticsOf {
    type Node = SemanticsOf;
    fn create(&self) -> SemanticsOf {
        self.clone()
    }
    fn update(&self, node: &mut SemanticsOf) {
        *node = self.clone();
    }
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.apply, &other.apply)
    }
}

impl ModifierNode for SemanticsOf {
    fn as_semantics(&mut self) -> Option<&mut dyn SemanticsModifierNode> {
        Some(self)
    }
}

impl SemanticsModifierNode for SemanticsOf {
    fn apply_semantics(&self, config: &mut SemanticsConfiguration) {
        (self.apply)(config);
    }
    fn apply_autofill_semantics(&self, config: &mut SemanticsConfiguration) {
        match &self.autofill {
            Some(autofill) => autofill(config),
            None => (self.apply)(config),
        }
    }
}

fn leaked_state(text: &str) -> TextFieldState {
    **Box::leak(Box::new(TextFieldState::new(text)))
}

// ---- A state-based field's semantics changed as it's measured -----------------------------------

// Its text set from its state, outside any event (a "clear" button, a value restored): the frame
// that measures it says its semantics changed, as Compose's `onSemanticsChange` for the state's
// write. It said only that it measured, which Android's accessibility tells within 100 ms.
#[test]
fn a_state_field_edited_from_its_state_says_its_semantics_changed() {
    let state = leaked_state("one");
    let mut ui = without_autofill(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag("f"));
    });
    assert_eq!(editable_text(&mut ui, "f").as_deref(), Some("one"));
    state.set_text_and_place_cursor_at_end("two");
    assert!(frames_say_semantics_changed(&mut ui));
    assert_eq!(editable_text(&mut ui, "f").as_deref(), Some("two"));
}

// Only its selection set from its state: the same.
#[test]
fn a_state_field_s_selection_set_from_its_state_says_its_semantics_changed() {
    let state = leaked_state("text");
    let mut ui = without_autofill(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag("f"));
    });
    ui.on_node_with_tag("f").assert_exists();
    state.edit(|buffer| buffer.select_all());
    assert!(frames_say_semantics_changed(&mut ui));
    let node = ui.on_node_with_tag("f").fetch_semantics_snapshot();
    assert_eq!(
        node.config.get(&SemanticsProperties::TEXT_SELECTION_RANGE),
        Some(&TextRange::new(0, 4))
    );
}

// What its output transformation shows changed (it reads a state of its own), its text the same:
// the field shows it anew as it's measured, and says its semantics changed (its editable text is
// what's shown, as Compose's `visualText`).
#[test]
fn a_state_field_s_output_shown_anew_says_its_semantics_changed() {
    let suffix = mutable_state_of("!");
    let read = suffix.handle();
    let state = leaked_state("a");
    let mut ui = without_autofill(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag("f"),
            output_transformation =
                Some(OutputTransformation::from_fn(move |buffer| buffer.append(read.get()))),
        );
    });
    assert_eq!(editable_text(&mut ui, "f").as_deref(), Some("a!"));
    suffix.set("?");
    assert!(frames_say_semantics_changed(&mut ui));
    assert_eq!(editable_text(&mut ui, "f").as_deref(), Some("a?"));
}

// A secure field's last typed character hidden again, as it's measured: its semantics tell the
// password's own text (D1143), which didn't change; nothing says they did.
#[test]
fn a_secure_field_hiding_its_last_typed_character_says_nothing_changed() {
    let state = leaked_state("");
    let _autofill = compose_test::use_autofill(false);
    let mut ui = ComposeTestRule::with_manual_clock(move || {
        BasicSecureTextField!(
            state,
            modifier = Modifier.width(300.dp()).test_tag("f"),
            text_obfuscation_mode = TextObfuscationMode::RevealLastTyped,
        );
    });
    ui.wait_for_idle();
    ui.on_node_with_tag("f").perform_text_input("a");
    ui.advance_time_by_frame();
    let shown = |ui: &mut ComposeTestRule| {
        let mut results: Vec<TextLayoutResult> = Vec::new();
        ui.on_node_with_tag("f").perform_semantics_action_with(
            &SemanticsActions::GET_TEXT_LAYOUT_RESULT,
            |get| {
                get(&mut results);
            },
        );
        results.pop().expect("a layout").layout_input.text.text().to_string()
    };
    assert_eq!(shown(&mut ui), "a");
    let version = ui.host().semantics_version();
    ui.advance_time_by(std::time::Duration::from_millis(2000));
    assert_eq!(shown(&mut ui), "\u{2022}");
    assert_eq!(ui.host().semantics_version(), version);
}

// A frame measuring the field with nothing of its state changed (its width) doesn't say so.
#[test]
fn a_state_field_measured_again_unchanged_says_nothing_changed() {
    let width = mutable_state_of(200.0f32);
    let read = width.handle();
    let state = leaked_state("text");
    let mut ui = without_autofill(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag("f").width(read.get().dp()));
    });
    ui.on_node_with_tag("f").assert_exists();
    width.set(150.0);
    assert!(!frames_say_semantics_changed(&mut ui));
}

// With autofill and nothing keeping its semantics: what it shows (its fillable data) told, as the
// review's tests (`review_autofill_2`) tell its text.
#[test]
fn a_state_field_s_output_shown_anew_tells_its_fillable_data() {
    let suffix = mutable_state_of("!");
    let read = suffix.handle();
    let state = leaked_state("a");
    let (mut ui, am) = set_test_content(move || {
        BasicTextFieldOfState!(
            state,
            modifier = Modifier.test_tag("f"),
            output_transformation =
                Some(OutputTransformation::from_fn(move |buffer| buffer.append(read.get()))),
        );
    });
    ui.run_on_idle(|_| suffix.set("?"));
    ui.wait_for_idle();
    // Read after: a finder keeps the semantics.
    let field = id(&mut ui, "f");
    let value = FillableData::create_from_text("a?").unwrap();
    am.verify(&Call::NotifyValueChanged { semantics_id: field, value });
    am.assert_no_more_calls();
}

// ---- A custom node saying its semantics changed as it's measured ---------------------------------

/// A custom node telling, as input text, what it read of `text` at its last measurement, saying
/// its semantics changed when that did (`MeasureScope::invalidate_semantics`, as Compose's
/// `invalidateSemantics` from `measure`): heard whether or not a service kept its semantics. A
/// measurement alone tells nothing (D1689), as Compose's.
#[derive(Clone)]
struct TellsMeasured(MutableState<String>);

struct TellsMeasuredNode {
    text: MutableState<String>,
    seen: String,
}

impl ModifierNodeElement for TellsMeasured {
    type Node = TellsMeasuredNode;
    fn create(&self) -> TellsMeasuredNode {
        TellsMeasuredNode { text: self.0, seen: String::new() }
    }
    fn update(&self, node: &mut TellsMeasuredNode) {
        node.text = self.0;
    }
    fn same(&self, _other: &Self) -> bool {
        true
    }
}

impl ModifierNode for TellsMeasuredNode {
    fn as_layout(&mut self) -> Option<&mut dyn LayoutModifierNode> {
        Some(self)
    }
    fn as_semantics(&mut self) -> Option<&mut dyn SemanticsModifierNode> {
        Some(self)
    }
}

impl LayoutModifierNode for TellsMeasuredNode {
    fn measure(
        &mut self,
        scope: &mut MeasureScope<'_>,
        content: Measurable,
        constraints: Constraints,
    ) -> MeasureResult {
        // Read observed: a write measures it again.
        let text = self.text.get();
        if text != self.seen {
            self.seen = text;
            scope.invalidate_semantics();
        }
        let placeable = scope.measure(content, constraints);
        scope.place(&placeable, 0, 0);
        scope.layout(placeable.width, placeable.height)
    }
}

impl SemanticsModifierNode for TellsMeasuredNode {
    fn apply_semantics(&self, config: &mut SemanticsConfiguration) {
        config.set_test_tag("node");
        config.set_content_data_type(ContentDataType::TEXT);
        config.set_input_text(self.seen.clone());
    }
}

fn tells_measured(kept_by_a_finder: bool) {
    let text = mutable_state_of(String::from("one"));
    let read = text.handle();
    let (mut ui, am) = set_test_content(move || {
        Box!(modifier = Modifier.then_custom(TellsMeasured(read)).size(SIDE.dp()));
    });
    let node = id(&mut ui, "node");
    if !kept_by_a_finder {
        // Measured again, its semantics computed for autofill only, not kept.
        ui.run_on_idle(|_| text.set(String::from("two")));
        ui.wait_for_idle();
    }
    ui.run_on_idle(|_| text.set(String::from("three")));
    ui.wait_for_idle();
    let value = FillableData::create_from_text("three").unwrap();
    am.verify(&Call::NotifyValueChanged { semantics_id: node, value });
}

#[test]
fn a_custom_node_changed_as_it_s_measured_with_its_semantics_kept_is_told() {
    tells_measured(true);
}

#[test]
fn a_custom_node_changed_as_it_s_measured_with_its_semantics_not_kept_is_told() {
    tells_measured(false);
}

// ---- The autofill-only computation keeps the full one's reads ------------------------------------

// A custom node's semantics reading a state autofill doesn't read (a label), kept by a finder,
// then computed for autofill only after a test tag's update (its own `apply_autofill_semantics`
// reading only its content type): a write to the label still says the semantics changed, as
// without autofill (`review_autofill_2`'s scroll, for a custom node).
#[test]
fn with_autofill_a_label_s_write_after_the_sync_says_the_semantics_changed() {
    let _autofill = compose_test::use_autofill(true);
    let label = mutable_state_of(String::from("a"));
    let tag = mutable_state_of("x");
    let (read_label, read_tag) = (label.handle(), tag.handle());
    let mut ui = ComposeTestRule::empty(default_host());
    ui.set_content(move || {
        let username: Apply = Rc::new(|config| config.set_content_type(ContentType::USERNAME));
        let apply = username.clone();
        Box!(
            modifier = Modifier
                .test_tag(read_tag.get())
                .then(semantics_of(
                    move |config| {
                        apply(config);
                        config.set_content_description(read_label.get());
                    },
                    Some(username),
                ))
                .size(SIDE.dp())
        );
    });
    ui.on_node_with_tag("x").assert_content_description_equals(&["a"]);
    tag.set("y");
    frames_say_semantics_changed(&mut ui);
    label.set(String::from("b"));
    assert!(frames_say_semantics_changed(&mut ui));
    ui.on_node_with_tag("y").assert_content_description_equals(&["b"]);
}

// ---- A part moved to another owner ---------------------------------------------------------------

/// A parts' host with a username's content type.
fn host(tag: &'static str) -> Modifier {
    Modifier
        .semantics(move |config| {
            config.set_test_tag(tag);
            config.parts_host();
            config.set_content_type(ContentType::USERNAME);
        })
        .size(SIDE.dp())
}

// A part (a field's text, as its owner's) moved by movable content from one host to another in
// one pass: noted as attached, its owner found again (not the one it had), so the host it left
// hides its text and the one it joined shows it.
#[test]
fn a_part_moved_to_another_host_is_its_new_host_s() {
    let left = mutable_state_of(true);
    let read = left.handle();
    let part = movable_content_of(|| {
        Box!(
            modifier = Modifier
                .semantics(|config| {
                    config.part_of_parent();
                    config.set_content_data_type(ContentDataType::TEXT);
                    config.set_input_text("typed");
                })
                .size(SIDE.dp())
        );
    });
    let (mut ui, am) = set_test_content(move || {
        let moved = !read.get();
        let part = part.clone();
        Row!({
            let first = part.clone();
            Box!(modifier = host("a"), {
                if !moved {
                    first();
                }
            });
            let then = part.clone();
            Box!(modifier = host("b"), {
                if moved {
                    then();
                }
            });
        });
    });
    let (a, b) = (id(&mut ui, "a"), id(&mut ui, "b"));
    ui.run_on_idle(|_| left.set(false));
    ui.wait_for_idle();
    am.verify(&visibility(a, false));
    am.verify(&visibility(b, true));
    am.assert_no_more_calls();
}

// ---- A node noted since the last sync loses the focus --------------------------------------------

// A focused node that didn't fill at the last sync gains `OnFillData` (a write an accessibility
// read applied between frames, D1688's S2), then loses the focus before the next frame: the sync
// looks at it, and it exits, as Compose reads its configuration as it loses the focus.
#[test]
fn a_node_filling_since_the_last_sync_losing_the_focus_exits() {
    let fills = mutable_state_of(false);
    let read = fills.handle();
    let focus_manager = Rc::new(RefCell::new(None));
    let into = focus_manager.clone();
    let (mut ui, am) = set_test_content(move || {
        *into.borrow_mut() = Some(LOCAL_FOCUS_MANAGER.current());
        Box!(
            modifier = semantics_of(
                move |config| {
                    config.set_test_tag("node");
                    if read.get() {
                        config.on_fill_data(None, |_| true);
                    }
                },
                None
            )
            .size(SIDE.dp())
            .focusable()
        );
    });
    ui.on_node_with_tag("node").request_focus();
    let node = id(&mut ui, "node");
    ui.run_on_idle(|_| am.clear());
    // Between frames: the write, read by a service (applied: the node noted), then the focus
    // cleared.
    fills.set(true);
    let _ = ui.host().semantics_tree(false);
    let manager = focus_manager.borrow().clone().expect("composed");
    manager.clear_focus(false);
    ui.wait_for_idle();
    am.verify(&Call::NotifyViewExited { semantics_id: node });
    am.assert_no_more_calls();
}

// ---- `use_autofill`'s guard ----------------------------------------------------------------------

/// Whether a rule made now has a platform manager.
fn rule_has_autofill() -> bool {
    let mut ui = ComposeTestRule::empty(default_host());
    ui.host().with_tree(|tree| tree.is_platform_autofill_enabled())
}

// What `use_autofill` says holds while its guard lives, then what was said before does: a helper
// saying it (a test's `before API 26`) doesn't leak into the tests run after it on its thread.
#[test]
fn use_autofill_holds_while_its_guard_lives() {
    let outer = compose_test::use_autofill(false);
    assert!(!rule_has_autofill());
    {
        let _inner = compose_test::use_autofill(true);
        assert!(rule_has_autofill());
    }
    assert!(!rule_has_autofill());
    drop(outer);
    let from_env = std::env::var("COMPOSE_TEST_AUTOFILL").is_ok_and(|value| value == "1");
    assert_eq!(rule_has_autofill(), from_env);
}

// ---- A selection resolved as it's measured -------------------------------------------------------

// A long press's drag is resolved at the field's measurement (its paragraph laid out): the
// selection it makes changes the semantics there, which the drag's event couldn't say. The
// frame says they changed (`semantics_version` moves with each time a frame's flag is set).
#[test]
fn a_long_press_drag_resolved_as_it_s_measured_says_the_semantics_changed() {
    let state = leaked_state("hello world again");
    let mut ui = without_autofill(move || {
        BasicTextFieldOfState!(state, modifier = Modifier.test_tag("f").width(300.dp()));
    });
    ui.on_node_with_tag("f").perform_touch_input(|touch| {
        let timeout = touch.view_configuration().long_press_timeout_millis;
        touch.down(Offset::new(5.0, 5.0));
        touch.move_pointers_after(timeout + 10);
    });
    ui.wait_for_idle();
    let selection = |ui: &mut ComposeTestRule| {
        let node = ui.on_node_with_tag("f").fetch_semantics_snapshot();
        node.config.get(&SemanticsProperties::TEXT_SELECTION_RANGE).copied()
    };
    let before = selection(&mut ui);
    let version = ui.host().semantics_version();
    ui.on_node_with_tag("f").perform_touch_input(|touch| touch.move_to(Offset::new(150.0, 5.0)));
    ui.wait_for_idle();
    assert_ne!(selection(&mut ui), before, "the drag selected more");
    assert_ne!(ui.host().semantics_version(), version);
}
