//! Pointer icons, as Compose's `PointerIconTest` and `StylusHoverIconTest` (D1676): which node's
//! icon shows as the pointer moves over parents, children and siblings, with and without
//! `override_descendants`, and as nodes and icons change under it. The two Compose files are
//! the same scenarios, one with `pointerHoverIcon` and the mouse, the other with
//! `stylusHoverIcon` and a hovering stylus; each test here runs its scenario with each device.
//! Compose's borders, which draw only, are left out.

use std::cell::Cell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_foundation::*;
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Offset, PointerEvent, PointerIcon, PointerId, PointerType, UnitExt};

const PARENT: &str = "myParentIcon";
const CHILD: &str = "myChildIcon";
const GRANDCHILD: &str = "myGrandchildIcon";
const PARENT_ICON: PointerIcon = PointerIcon::CROSSHAIR;
const CHILD_ICON: PointerIcon = PointerIcon::TEXT;
const GRANDCHILD_ICON: PointerIcon = PointerIcon::HAND;

/// Whose icon: the mouse's (`pointer_hover_icon`, `PointerIconTest`) or a hovering stylus's
/// (`stylus_hover_icon`, `StylusHoverIconTest`).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Device {
    Mouse,
    Stylus,
}

const DEVICES: [Device; 2] = [Device::Mouse, Device::Stylus];

impl Device {
    /// `modifier` with this device's hover icon modifier.
    fn icon(self, modifier: Modifier, icon: PointerIcon, override_descendants: bool) -> Modifier {
        match self {
            Device::Mouse => modifier.pointer_hover_icon_with(icon, override_descendants),
            Device::Stylus => modifier.stylus_hover_icon_with(icon, override_descendants),
        }
    }

    /// What shows over nothing: the mouse's arrow (Compose's `desiredDefaultIcon`); a stylus has
    /// none (`null`).
    fn none(self) -> Option<PointerIcon> {
        match self {
            Device::Mouse => Some(PointerIcon::DEFAULT),
            Device::Stylus => None,
        }
    }
}

/// A place in a node, as the injection scopes' `center`, `bottomRight`, … (`right` and `bottom`
/// are the last pixel in).
#[derive(Clone, Copy)]
enum At {
    Center,
    CenterRight,
    BottomCenter,
    BottomRight,
    BottomLeft,
    Point(f32, f32),
}

impl At {
    /// In a node of `width` × `height` pixels.
    fn in_size(self, width: f32, height: f32) -> Offset {
        let last = |length: f32| if length == 0.0 { 0.0 } else { length - 1.0 };
        match self {
            At::Center => Offset::new(width / 2.0, height / 2.0),
            At::CenterRight => Offset::new(last(width), height / 2.0),
            At::BottomCenter => Offset::new(width / 2.0, last(height)),
            At::BottomRight => Offset::new(last(width), last(height)),
            At::BottomLeft => Offset::new(0.0, last(height)),
            At::Point(x, y) => Offset::new(x, y),
        }
    }

    fn in_scope(self, scope: &InjectionScope<'_>) -> Offset {
        match self {
            At::Center => scope.center(),
            At::CenterRight => scope.center_right(),
            At::BottomCenter => scope.bottom_center(),
            At::BottomRight => scope.bottom_right(),
            At::BottomLeft => scope.bottom_left(),
            At::Point(x, y) => Offset::new(x, y),
        }
    }
}

/// A rule and the device its scenario runs with, as each Compose test's `rule` and
/// `iconService`.
struct Icons {
    ui: ComposeTestRule,
    device: Device,
}

impl Icons {
    fn new(device: Device, content: impl Fn(Device) + 'static) -> Icons {
        Icons { ui: compose_test(move || content(device)), device }
    }

    /// The icon the host shows now, as `iconService.getIcon()` (`getStylusHoverIcon()`), after
    /// what was changed is composed and laid out (`runOnIdle`).
    fn shown(&mut self) -> Option<PointerIcon> {
        self.ui.wait_for_idle();
        match self.device {
            Device::Mouse => Some(self.ui.host().pointer_icon()),
            Device::Stylus => self.ui.host().stylus_hover_icon(),
        }
    }

    /// A hovering stylus's event at `at` in `tag`, as the stylus scope of Compose's tests
    /// (`HandwritingTestStylusInjectScope`) sends a hover move.
    fn stylus_hover(&mut self, tag: &str, at: At) {
        let bounds = self.ui.on_node_with_tag(tag).fetch_semantics_node().bounds_in_root();
        let width = (bounds.right - bounds.left).round();
        let height = (bounds.bottom - bounds.top).round();
        let position = at.in_size(width, height) + Offset::new(bounds.left, bounds.top);
        let event =
            PointerEvent::moved(position, false).with_pointer(PointerId(1), PointerType::Stylus);
        self.ui.host().send_pointer(event);
        self.ui.wait_for_idle();
    }

    #[track_caller]
    fn assert_shown(&mut self, expected: Option<PointerIcon>) {
        assert_eq!(self.shown(), expected, "{:?}", self.device);
    }

    /// The pointer comes into the window over `tag` at `at`, as `enter` (`hoverEnter`).
    fn enter(&mut self, tag: &str, at: At) {
        match self.device {
            Device::Mouse => {
                self.ui.on_node_with_tag(tag).perform_mouse_input(|mouse| {
                    let position = at.in_scope(mouse);
                    mouse.enter(position);
                });
            }
            Device::Stylus => self.stylus_hover(tag, at),
        }
    }

    /// The pointer moves to `at` in `tag`, as `moveTo` (`hoverMoveTo`).
    fn move_to(&mut self, tag: &str, at: At) {
        match self.device {
            Device::Mouse => {
                self.ui.on_node_with_tag(tag).perform_mouse_input(|mouse| {
                    let position = at.in_scope(mouse);
                    mouse.move_to(position);
                });
            }
            Device::Stylus => self.stylus_hover(tag, at),
        }
    }

    /// The pointer leaves the window where it is, as `exit()` (`hoverExit()`).
    fn exit(&mut self, tag: &str) {
        match self.device {
            Device::Mouse => {
                self.ui.on_node_with_tag(tag).perform_mouse_input(|mouse| {
                    let position = mouse.current_position();
                    mouse.exit(position);
                });
            }
            Device::Stylus => {
                self.ui.host().send_stylus_left();
                self.ui.wait_for_idle();
            }
        }
    }

    /// Compose's `verifyIconOnHover`: the icon over `tag`'s bottom right corner, then out.
    #[track_caller]
    fn verify_on_hover(&mut self, tag: &str, expected: Option<PointerIcon>) {
        self.enter(tag, At::BottomRight);
        self.assert_shown(expected);
        self.exit(tag);
    }

    /// Compose's `verifyOverlappingSiblings`: child A at the top left, child B (the grandchild's
    /// tag and icon) over its bottom right, in a parent with no icon.
    #[track_caller]
    fn verify_overlapping_siblings(&mut self) {
        let none = self.device.none();
        self.assert_shown(none.clone());
        self.enter(GRANDCHILD, At::BottomRight);
        self.assert_shown(Some(GRANDCHILD_ICON));
        self.move_to(PARENT, At::BottomRight);
        self.assert_shown(none.clone());
        // Where B is over A: B's.
        self.move_to(GRANDCHILD, At::BottomLeft);
        self.assert_shown(Some(GRANDCHILD_ICON));
        self.move_to(CHILD, At::BottomLeft);
        self.assert_shown(Some(CHILD_ICON));
        self.move_to(PARENT, At::BottomLeft);
        self.assert_shown(none);
        self.exit(PARENT);
    }
}

/// `modifier` with `device`'s icon if `icon` is some (with whether it overrides descendants).
fn maybe_icon(
    device: Device,
    modifier: Modifier,
    icon: PointerIcon,
    overrides: Option<bool>,
) -> Modifier {
    match overrides {
        Some(override_descendants) => device.icon(modifier, icon, override_descendants),
        None => modifier,
    }
}

// ---- A parent and a child -------------------------------------------------------------------------

/// A 200 dp parent and its child over all of it.
fn full_overlap(parent_overrides: bool, child_overrides: bool) -> impl Fn(Device) + 'static {
    move |device| {
        let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
        Box!(modifier = device.icon(parent, PARENT_ICON, parent_overrides), {
            let child = Modifier.required_size(200.dp()).test_tag(CHILD);
            Box!(modifier = device.icon(child, CHILD_ICON, child_overrides));
        });
    }
}

// Compose: PointerIconTest.parentChildFullOverlap_noOverrideDescendants_checkNumberOfCompositions,
// StylusHoverIconTest.parentChildFullOverlap_noOverrideDescendants_checkNumberOfCompositions
#[test]
fn parent_child_full_overlap_no_override_descendants_check_number_of_compositions() {
    for device in DEVICES {
        let compositions = Rc::new(Cell::new(0));
        let counted = compositions.clone();
        let mut icons = Icons::new(device, move |device| {
            let counted = counted.clone();
            let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
            Box!(modifier = device.icon(parent, PARENT_ICON, false), {
                counted.set(counted.get() + 1);
                let counted = counted.clone();
                let child = Modifier.required_size(200.dp()).test_tag(CHILD);
                Box!(modifier = device.icon(child, CHILD_ICON, false), {
                    counted.set(counted.get() + 1);
                });
            });
        });
        icons.assert_shown(device.none());
        assert_eq!(compositions.get(), 2, "{device:?}");
    }
}

// Compose: PointerIconTest.parentChildFullOverlap_noOverrideDescendants,
// StylusHoverIconTest.parentChildFullOverlap_noOverrideDescendants
#[test]
fn parent_child_full_overlap_no_override_descendants() {
    for device in DEVICES {
        let mut icons = Icons::new(device, full_overlap(false, false));
        icons.assert_shown(device.none());
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, Some(CHILD_ICON));
    }
}

// Compose: PointerIconTest.parentChildFullOverlap_parentOverridesDescendants,
// StylusHoverIconTest.parentChildFullOverlap_parentOverridesDescendants
#[test]
fn parent_child_full_overlap_parent_overrides_descendants() {
    for device in DEVICES {
        let mut icons = Icons::new(device, full_overlap(true, false));
        icons.assert_shown(device.none());
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
        icons.verify_on_hover(CHILD, Some(PARENT_ICON));
    }
}

// Compose: PointerIconTest.parentChildFullOverlap_childOverridesDescendants,
// StylusHoverIconTest.parentChildFullOverlap_childOverridesDescendants
#[test]
fn parent_child_full_overlap_child_overrides_descendants() {
    for device in DEVICES {
        let mut icons = Icons::new(device, full_overlap(false, true));
        icons.assert_shown(device.none());
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, Some(CHILD_ICON));
    }
}

// Compose: PointerIconTest.parentChildFullOverlap_bothOverrideDescendants,
// StylusHoverIconTest.parentChildFullOverlap_bothOverrideDescendants
#[test]
fn parent_child_full_overlap_both_override_descendants() {
    for device in DEVICES {
        let mut icons = Icons::new(device, full_overlap(true, true));
        icons.assert_shown(device.none());
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
        icons.verify_on_hover(CHILD, Some(PARENT_ICON));
    }
}

/// A 200 dp parent and a 100 dp child padded 20 dp in it.
fn partial_overlap(parent_overrides: bool, child_overrides: bool) -> impl Fn(Device) + 'static {
    move |device| {
        let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
        Box!(modifier = device.icon(parent, PARENT_ICON, parent_overrides), {
            let child = Modifier.padding(20.dp()).required_size(100.dp()).test_tag(CHILD);
            Box!(modifier = device.icon(child, CHILD_ICON, child_overrides));
        });
    }
}

// Compose: PointerIconTest.parentChildPartialOverlap_noOverrideDescendants,
// StylusHoverIconTest.parentChildPartialOverlap_noOverrideDescendants
#[test]
fn parent_child_partial_overlap_no_override_descendants() {
    for device in DEVICES {
        let mut icons = Icons::new(device, partial_overlap(false, false));
        icons.assert_shown(device.none());
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
    }
}

// Compose: PointerIconTest.parentChildPartialOverlap_parentOverridesDescendants,
// StylusHoverIconTest.parentChildPartialOverlap_parentOverridesDescendants
#[test]
fn parent_child_partial_overlap_parent_overrides_descendants() {
    for device in DEVICES {
        let mut icons = Icons::new(device, partial_overlap(true, false));
        icons.assert_shown(device.none());
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
        icons.verify_on_hover(CHILD, Some(PARENT_ICON));
    }
}

// Compose: PointerIconTest.parentChildPartialOverlap_childOverridesDescendants,
// StylusHoverIconTest.parentChildPartialOverlap_childOverridesDescendants
#[test]
fn parent_child_partial_overlap_child_overrides_descendants() {
    for device in DEVICES {
        let mut icons = Icons::new(device, partial_overlap(false, true));
        icons.assert_shown(device.none());
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
    }
}

// Compose: PointerIconTest.parentChildPartialOverlap_bothOverrideDescendants,
// StylusHoverIconTest.parentChildPartialOverlap_bothOverrideDescendants
#[test]
fn parent_child_partial_overlap_both_override_descendants() {
    for device in DEVICES {
        let mut icons = Icons::new(device, partial_overlap(true, true));
        icons.assert_shown(device.none());
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
        icons.verify_on_hover(CHILD, Some(PARENT_ICON));
    }
}

/// A 200 dp parent whose icon (overriding its descendants) comes with `visible`, and a 150 dp
/// child padded 20 dp in it.
fn parent_icon_added(visible: MutableState<bool>) -> impl Fn(Device) + 'static {
    move |device| {
        let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
        let parent = maybe_icon(device, parent, PARENT_ICON, visible.get().then_some(true));
        Box!(modifier = parent, {
            let child = Modifier.padding(20.dp()).required_size(150.dp()).test_tag(CHILD);
            Box!(modifier = device.icon(child, CHILD_ICON, false));
        });
    }
}

// Compose: PointerIconTest.parentChildPartialOverlap_parentModifierDynamicallyAdded,
// StylusHoverIconTest.parentChildPartialOverlap_parentModifierDynamicallyAdded
#[test]
fn parent_child_partial_overlap_parent_modifier_dynamically_added() {
    for device in DEVICES {
        let visible = mutable_state_of(false);
        let mut icons = Icons::new(device, parent_icon_added(visible.handle()));
        icons.assert_shown(device.none());
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, device.none());
        visible.set(true);
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
        icons.verify_on_hover(CHILD, Some(PARENT_ICON));
    }
}

// Compose: PointerIconTest.parentChildPartialOverlap_parentModifierDynamicallyAddedWithMoveEvents,
// StylusHoverIconTest.parentChildPartialOverlap_parentModifierDynamicallyAddedWithMoveEvents
// (both `@Ignore`d in Compose, b/299482894: a node added under a resting pointer isn't entered
// there; compose-rs hit-tests the resting pointer again after the change, D1159).
#[test]
fn parent_child_partial_overlap_parent_modifier_dynamically_added_with_move_events() {
    for device in DEVICES {
        let visible = mutable_state_of(false);
        let mut icons = Icons::new(device, parent_icon_added(visible.handle()));
        icons.assert_shown(device.none());
        icons.enter(CHILD, At::BottomRight);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(device.none());
        icons.move_to(CHILD, At::Center);
        visible.set(true);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::BottomRight);
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.exit(PARENT);
    }
}

/// A 200 dp parent whose icon overrides its descendants with `overrides`, and a 150 dp child
/// padded 20 dp in it.
fn parent_override_changing(overrides: MutableState<bool>) -> impl Fn(Device) + 'static {
    move |device| {
        let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
        Box!(modifier = device.icon(parent, PARENT_ICON, overrides.get()), {
            let child = Modifier.padding(20.dp()).required_size(150.dp()).test_tag(CHILD);
            Box!(modifier = device.icon(child, CHILD_ICON, false));
        });
    }
}

// Compose: PointerIconTest.parentChildPartialOverlap_parentModifierDynamicallyChangedToOverrideWithMoveEvents,
// StylusHoverIconTest.parentChildPartialOverlap_parentModifierDynamicallyChangedToOverrideWithMoveEvents
#[test]
fn parent_child_partial_overlap_parent_modifier_dynamically_changed_to_override_with_move_events() {
    for device in DEVICES {
        let overrides = mutable_state_of(false);
        let mut icons = Icons::new(device, parent_override_changing(overrides.handle()));
        icons.assert_shown(device.none());
        icons.enter(CHILD, At::BottomRight);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::Center);
        icons.assert_shown(Some(CHILD_ICON));
        // The parent overrides its descendants now, the pointer resting on the child.
        overrides.set(true);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        // And no more.
        overrides.set(false);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::Center);
        icons.assert_shown(Some(CHILD_ICON));
        icons.exit(PARENT);
    }
}

// Compose: PointerIconTest.parentChildPartialOverlap_parentOverrideDescendantsDynamicallyUpdated,
// StylusHoverIconTest.parentChildPartialOverlap_parentOverrideDescendantsDynamicallyUpdated
#[test]
fn parent_child_partial_overlap_parent_override_descendants_dynamically_updated() {
    for device in DEVICES {
        let overrides = mutable_state_of(false);
        let mut icons = Icons::new(device, parent_override_changing(overrides.handle()));
        icons.assert_shown(device.none());
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
        overrides.set(true);
        icons.verify_on_hover(CHILD, Some(PARENT_ICON));
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
    }
}

// Compose: PointerIconTest.parentChildPartialOverlapAndExtendsBeyondParent_dynamicOverrideDescendants,
// StylusHoverIconTest.parentChildPartialOverlapAndExtendsBeyondParent_dynamicOverrideDescendants
#[test]
fn parent_child_partial_overlap_and_extends_beyond_parent_dynamic_override_descendants() {
    for device in DEVICES {
        let overrides = mutable_state_of(false);
        let overriding = overrides.handle();
        let mut icons = Icons::new(device, move |device| {
            Box!(modifier = Modifier.required_size(300.dp()), {
                let parent = Modifier.size(150.dp()).test_tag(PARENT);
                Box!(modifier = device.icon(parent, PARENT_ICON, overriding.get()), {
                    // Past the parent's right edge.
                    let child = Modifier
                        .padding(20.dp())
                        .offset(100.dp(), 0.dp())
                        .width(300.dp())
                        .height(100.dp())
                        .test_tag(CHILD);
                    Box!(modifier = device.icon(child, CHILD_ICON, false));
                });
            });
        });
        icons.assert_shown(device.none());
        // The child outside the parent, then inside.
        icons.enter(CHILD, At::BottomRight);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(CHILD, At::BottomLeft);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::BottomLeft);
        icons.assert_shown(Some(CHILD_ICON));
        overrides.set(true);
        icons.assert_shown(Some(PARENT_ICON));
        // Outside the parent the child's shows: the parent isn't hovered there.
        icons.move_to(CHILD, At::BottomRight);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::BottomLeft);
        icons.assert_shown(Some(PARENT_ICON));
        overrides.set(false);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::BottomLeft);
        icons.assert_shown(Some(CHILD_ICON));
        icons.exit(PARENT);
    }
}

// ---- Siblings -------------------------------------------------------------------------------------

/// A 200 dp parent without an icon over a column of child A (50 dp padded 20 dp) and child B
/// (the grandchild's tag and icon, 50 dp padded 40 dp).
fn non_overlapping(a_overrides: bool, b_overrides: bool) -> impl Fn(Device) + 'static {
    move |device| {
        Box!(modifier = Modifier.required_size(200.dp()).test_tag(PARENT), {
            Column!({
                let a = Modifier.padding(20.dp()).required_size(50.dp()).test_tag(CHILD);
                Box!(modifier = device.icon(a, CHILD_ICON, a_overrides));
                let b = Modifier.padding(40.dp()).required_size(50.dp()).test_tag(GRANDCHILD);
                Box!(modifier = device.icon(b, GRANDCHILD_ICON, b_overrides));
            });
        });
    }
}

/// What every non-overlapping case checks: each child's own icon, none over the parent.
fn verify_non_overlapping(device: Device, a_overrides: bool, b_overrides: bool) {
    let mut icons = Icons::new(device, non_overlapping(a_overrides, b_overrides));
    icons.assert_shown(device.none());
    icons.verify_on_hover(CHILD, Some(CHILD_ICON));
    icons.verify_on_hover(GRANDCHILD, Some(GRANDCHILD_ICON));
    icons.verify_on_hover(PARENT, device.none());
}

// Compose: PointerIconTest.NonOverlappingSiblings_noOverrideDescendants,
// StylusHoverIconTest.NonOverlappingSiblings_noOverrideDescendants
#[test]
fn non_overlapping_siblings_no_override_descendants() {
    for device in DEVICES {
        verify_non_overlapping(device, false, false);
    }
}

// Compose: PointerIconTest.NonOverlappingSiblings_firstChildOverridesDescendants,
// StylusHoverIconTest.NonOverlappingSiblings_firstChildOverridesDescendants
#[test]
fn non_overlapping_siblings_first_child_overrides_descendants() {
    for device in DEVICES {
        verify_non_overlapping(device, true, false);
    }
}

// Compose: PointerIconTest.NonOverlappingSiblings_secondChildOverridesDescendants,
// StylusHoverIconTest.NonOverlappingSiblings_secondChildOverridesDescendants
#[test]
fn non_overlapping_siblings_second_child_overrides_descendants() {
    for device in DEVICES {
        verify_non_overlapping(device, false, true);
    }
}

// Compose: PointerIconTest.NonOverlappingSiblings_bothOverrideDescendants,
// StylusHoverIconTest.NonOverlappingSiblings_bothOverrideDescendants
#[test]
fn non_overlapping_siblings_both_override_descendants() {
    for device in DEVICES {
        verify_non_overlapping(device, true, true);
    }
}

/// A 200 dp parent (its icon if `parent` says) over child A (120 × 60 dp padded 20 dp) and
/// child B over A's bottom right (the grandchild's tag and icon, 120 × 20 dp padded 100 dp
/// horizontally and 40 dp vertically).
fn overlapping(
    parent: Option<bool>,
    a_overrides: bool,
    b_overrides: bool,
) -> impl Fn(Device) + 'static {
    move |device| {
        let modifier = Modifier.required_size(200.dp()).test_tag(PARENT);
        Box!(modifier = maybe_icon(device, modifier, PARENT_ICON, parent), {
            let a = Modifier.padding(20.dp()).required_size((120.dp(), 60.dp())).test_tag(CHILD);
            Box!(modifier = device.icon(a, CHILD_ICON, a_overrides));
            let b = Modifier
                .padding((100.dp(), 40.dp()))
                .required_size((120.dp(), 20.dp()))
                .test_tag(GRANDCHILD);
            Box!(modifier = device.icon(b, GRANDCHILD_ICON, b_overrides));
        });
    }
}

// Compose: PointerIconTest.OverlappingSiblings_noOverrideDescendants,
// StylusHoverIconTest.OverlappingSiblings_noOverrideDescendants
#[test]
fn overlapping_siblings_no_override_descendants() {
    for device in DEVICES {
        Icons::new(device, overlapping(None, false, false)).verify_overlapping_siblings();
    }
}

// Compose: PointerIconTest.OverlappingSiblings_childAOverridesDescendants,
// StylusHoverIconTest.OverlappingSiblings_childAOverridesDescendants
#[test]
fn overlapping_siblings_child_a_overrides_descendants() {
    for device in DEVICES {
        Icons::new(device, overlapping(None, true, false)).verify_overlapping_siblings();
    }
}

// Compose: PointerIconTest.OverlappingSiblings_childBOverridesDescendants,
// StylusHoverIconTest.OverlappingSiblings_childBOverridesDescendants
#[test]
fn overlapping_siblings_child_b_overrides_descendants() {
    for device in DEVICES {
        Icons::new(device, overlapping(None, false, true)).verify_overlapping_siblings();
    }
}

// Compose: PointerIconTest.OverlappingSiblings_bothOverrideDescendants,
// StylusHoverIconTest.OverlappingSiblings_bothOverrideDescendants
#[test]
fn overlapping_siblings_both_override_descendants() {
    for device in DEVICES {
        Icons::new(device, overlapping(None, true, true)).verify_overlapping_siblings();
    }
}

// Compose: PointerIconTest.OverlappingSiblings_parentOverridesDescendants,
// StylusHoverIconTest.OverlappingSiblings_parentOverridesDescendants
#[test]
fn overlapping_siblings_parent_overrides_descendants() {
    for device in DEVICES {
        let mut icons = Icons::new(device, overlapping(Some(true), true, true));
        icons.assert_shown(device.none());
        icons.enter(GRANDCHILD, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::BottomLeft);
        icons.assert_shown(Some(PARENT_ICON));
        icons.exit(PARENT);
    }
}

// ---- Three levels ---------------------------------------------------------------------------------

/// A 200 dp parent, a 150 dp child padded 20 dp in it and a 100 dp grandchild padded 40 dp in
/// that, each with its icon if its option is some (whether it overrides its descendants).
fn nested(
    parent: Option<bool>,
    child: Option<bool>,
    grandchild: Option<bool>,
) -> impl Fn(Device) + 'static {
    move |device| {
        let modifier = Modifier.required_size(200.dp()).test_tag(PARENT);
        Box!(modifier = maybe_icon(device, modifier, PARENT_ICON, parent), {
            let modifier = Modifier.padding(20.dp()).required_size(150.dp()).test_tag(CHILD);
            Box!(modifier = maybe_icon(device, modifier, CHILD_ICON, child), {
                let modifier =
                    Modifier.padding(40.dp()).required_size(100.dp()).test_tag(GRANDCHILD);
                Box!(modifier = maybe_icon(device, modifier, GRANDCHILD_ICON, grandchild));
            });
        });
    }
}

// Compose: PointerIconTest.multiLayeredNesting_grandchildCustomIconNoOverride,
// StylusHoverIconTest.multiLayeredNesting_grandchildCustomIconNoOverride
#[test]
fn multi_layered_nesting_grandchild_custom_icon_no_override() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(None, None, Some(false)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(GRANDCHILD, Some(GRANDCHILD_ICON));
        icons.verify_on_hover(CHILD, device.none());
        icons.verify_on_hover(PARENT, device.none());
    }
}

// Compose: PointerIconTest.multiLayeredNesting_grandchildCustomIconHasOverride,
// StylusHoverIconTest.multiLayeredNesting_grandchildCustomIconHasOverride
#[test]
fn multi_layered_nesting_grandchild_custom_icon_has_override() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(None, None, Some(true)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(GRANDCHILD, Some(GRANDCHILD_ICON));
        icons.verify_on_hover(CHILD, device.none());
        icons.verify_on_hover(PARENT, device.none());
    }
}

// Compose: PointerIconTest.multiLayeredNesting_childAndGrandchildCustomIconsNoOverrides,
// StylusHoverIconTest.multiLayeredNesting_childAndGrandchildCustomIconsNoOverrides
#[test]
fn multi_layered_nesting_child_and_grandchild_custom_icons_no_overrides() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(None, Some(false), Some(false)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(GRANDCHILD, Some(GRANDCHILD_ICON));
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, device.none());
    }
}

// Compose: PointerIconTest.multiLayeredNesting_childCustomIconGrandchildHasOverride,
// StylusHoverIconTest.multiLayeredNesting_childCustomIconGrandchildHasOverride
#[test]
fn multi_layered_nesting_child_custom_icon_grandchild_has_override() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(None, Some(false), Some(true)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(GRANDCHILD, Some(GRANDCHILD_ICON));
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, device.none());
    }
}

// Compose: PointerIconTest.multiLayeredNesting_grandchildCustomIconChildHasOverride,
// StylusHoverIconTest.multiLayeredNesting_grandchildCustomIconChildHasOverride
#[test]
fn multi_layered_nesting_grandchild_custom_icon_child_has_override() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(None, Some(true), Some(false)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(GRANDCHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, device.none());
    }
}

// Compose: PointerIconTest.multiLayeredNesting_childAndGrandchildOverrideDescendants,
// StylusHoverIconTest.multiLayeredNesting_childAndGrandchildOverrideDescendants
#[test]
fn multi_layered_nesting_child_and_grandchild_override_descendants() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(None, Some(true), Some(true)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(GRANDCHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, device.none());
    }
}

// Compose: PointerIconTest.multiLayeredNesting_parentAndGrandchildCustomIconNoOverrides,
// StylusHoverIconTest.multiLayeredNesting_parentAndGrandchildCustomIconNoOverrides
#[test]
fn multi_layered_nesting_parent_and_grandchild_custom_icon_no_overrides() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(Some(false), None, Some(false)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(GRANDCHILD, Some(GRANDCHILD_ICON));
        icons.verify_on_hover(CHILD, Some(PARENT_ICON));
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
    }
}

// Compose: PointerIconTest.multiLayeredNesting_parentCustomIconGrandchildOverrides,
// StylusHoverIconTest.multiLayeredNesting_parentCustomIconGrandchildOverrides
#[test]
fn multi_layered_nesting_parent_custom_icon_grandchild_overrides() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(Some(false), None, Some(true)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(GRANDCHILD, Some(GRANDCHILD_ICON));
        icons.verify_on_hover(CHILD, Some(PARENT_ICON));
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
    }
}

// Compose: PointerIconTest.multiLayeredNesting_allCustomIconsNoOverrides,
// StylusHoverIconTest.multiLayeredNesting_allCustomIconsNoOverrides
#[test]
fn multi_layered_nesting_all_custom_icons_no_overrides() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(Some(false), Some(false), Some(false)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(GRANDCHILD, Some(GRANDCHILD_ICON));
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
    }
}

// Compose: PointerIconTest.multiLayeredNesting_allCustomIconsGrandchildOverrides,
// StylusHoverIconTest.multiLayeredNesting_allCustomIconsGrandchildOverrides
#[test]
fn multi_layered_nesting_all_custom_icons_grandchild_overrides() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(Some(false), Some(false), Some(true)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(GRANDCHILD, Some(GRANDCHILD_ICON));
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
    }
}

// Compose: PointerIconTest.multiLayeredNesting_allCustomIconsChildOverrides,
// StylusHoverIconTest.multiLayeredNesting_allCustomIconsChildOverrides
#[test]
fn multi_layered_nesting_all_custom_icons_child_overrides() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(Some(false), Some(true), Some(false)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(GRANDCHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
    }
}

// Compose: PointerIconTest.multiLayeredNesting_allCustomIconsChildAndGrandchildOverrides,
// StylusHoverIconTest.multiLayeredNesting_allCustomIconsChildAndGrandchildOverrides
#[test]
fn multi_layered_nesting_all_custom_icons_child_and_grandchild_overrides() {
    for device in DEVICES {
        let mut icons = Icons::new(device, nested(Some(false), Some(true), Some(true)));
        icons.assert_shown(device.none());
        icons.verify_on_hover(CHILD, Some(CHILD_ICON));
        icons.verify_on_hover(GRANDCHILD, Some(CHILD_ICON));
        icons.verify_on_hover(PARENT, Some(PARENT_ICON));
    }
}

/// What the cases with an overriding parent check: its icon everywhere.
fn verify_parent_overrides(device: Device, child: Option<bool>, grandchild: Option<bool>) {
    let mut icons = Icons::new(device, nested(Some(true), child, grandchild));
    icons.assert_shown(device.none());
    icons.verify_on_hover(PARENT, Some(PARENT_ICON));
    icons.verify_on_hover(CHILD, Some(PARENT_ICON));
    icons.verify_on_hover(GRANDCHILD, Some(PARENT_ICON));
}

// Compose: PointerIconTest.multiLayeredNesting_parentGrandChildCustomIconsParentOverrides,
// StylusHoverIconTest.multiLayeredNesting_parentGrandChildCustomIconsParentOverrides
#[test]
fn multi_layered_nesting_parent_grandchild_custom_icons_parent_overrides() {
    for device in DEVICES {
        verify_parent_overrides(device, None, Some(false));
    }
}

// Compose: PointerIconTest.multiLayeredNesting_parentGrandChildCustomIconsBothOverride,
// StylusHoverIconTest.multiLayeredNesting_parentGrandChildCustomIconsBothOverride (the child has
// an icon too, as there)
#[test]
fn multi_layered_nesting_parent_grandchild_custom_icons_both_override() {
    for device in DEVICES {
        verify_parent_overrides(device, Some(false), Some(true));
    }
}

// Compose: PointerIconTest.multiLayeredNesting_allCustomIconsParentOverrides,
// StylusHoverIconTest.multiLayeredNesting_allCustomIconsParentOverrides
#[test]
fn multi_layered_nesting_all_custom_icons_parent_overrides() {
    for device in DEVICES {
        verify_parent_overrides(device, Some(false), Some(false));
    }
}

// Compose: PointerIconTest.multiLayeredNesting_allCustomIconsParentAndGrandchildOverride,
// StylusHoverIconTest.multiLayeredNesting_allCustomIconsParentAndGrandchildOverride
#[test]
fn multi_layered_nesting_all_custom_icons_parent_and_grandchild_override() {
    for device in DEVICES {
        verify_parent_overrides(device, Some(false), Some(true));
    }
}

// Compose: PointerIconTest.multiLayeredNesting_allCustomIconsParentAndChildOverride,
// StylusHoverIconTest.multiLayeredNesting_allCustomIconsParentAndChildOverride
#[test]
fn multi_layered_nesting_all_custom_icons_parent_and_child_override() {
    for device in DEVICES {
        verify_parent_overrides(device, Some(true), Some(false));
    }
}

// Compose: PointerIconTest.multiLayeredNesting_allIconsOverride,
// StylusHoverIconTest.multiLayeredNesting_allIconsOverride
#[test]
fn multi_layered_nesting_all_icons_override() {
    for device in DEVICES {
        verify_parent_overrides(device, Some(true), Some(true));
    }
}

// ---- Changes under the pointer --------------------------------------------------------------------

// Compose: PointerIconTest.dynamicallyUpdatedIcon, StylusHoverIconTest.dynamicallyUpdatedIcon
#[test]
fn dynamically_updated_icon() {
    for device in DEVICES {
        let icon = mutable_state_of(CHILD_ICON);
        let child_icon = icon.handle();
        let mut icons = Icons::new(device, move |device| {
            let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
            Box!(modifier = device.icon(parent, PARENT_ICON, false), {
                let child = Modifier.padding(20.dp()).required_size(100.dp()).test_tag(CHILD);
                Box!(modifier = device.icon(child, child_icon.get(), false));
            });
        });
        icons.assert_shown(device.none());
        icons.enter(CHILD, At::BottomRight);
        icons.assert_shown(Some(CHILD_ICON));
        // The child's icon changes under the resting pointer.
        icon.set(GRANDCHILD_ICON);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        icons.move_to(CHILD, At::CenterRight);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        icons.exit(CHILD);
    }
}

/// A 200 dp parent (its icon overriding its descendants with `parent_overrides`) centering a
/// 100 dp child padded 20 dp, there with `visible`.
fn child_appearing(
    parent_overrides: bool,
    visible: MutableState<bool>,
) -> impl Fn(Device) + 'static {
    move |device| {
        let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
        let parent = device.icon(parent, PARENT_ICON, parent_overrides);
        Box!(modifier = parent, content_alignment = Alignment::CENTER, {
            if visible.get() {
                let child = Modifier.padding(20.dp()).required_size(100.dp()).test_tag(CHILD);
                Box!(modifier = device.icon(child, CHILD_ICON, false));
            }
        });
    }
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveChild_noOverrideDescendants,
// StylusHoverIconTest.dynamicallyAddAndRemoveChild_noOverrideDescendants
#[test]
fn dynamically_add_and_remove_child_no_override_descendants() {
    for device in DEVICES {
        let visible = mutable_state_of(false);
        let mut icons = Icons::new(device, child_appearing(false, visible.handle()));
        icons.assert_shown(device.none());
        icons.enter(PARENT, At::Center);
        icons.assert_shown(Some(PARENT_ICON));
        // The child comes under the resting pointer.
        visible.set(true);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(CHILD, At::CenterRight);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomCenter);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::Center);
        // And goes.
        visible.set(false);
        icons.assert_shown(Some(PARENT_ICON));
        icons.exit(PARENT);
    }
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveChild_parentOverridesDescendants,
// StylusHoverIconTest.dynamicallyAddAndRemoveChild_parentOverridesDescendants
#[test]
fn dynamically_add_and_remove_child_parent_overrides_descendants() {
    for device in DEVICES {
        let visible = mutable_state_of(false);
        let mut icons = Icons::new(device, child_appearing(true, visible.handle()));
        icons.assert_shown(device.none());
        icons.enter(PARENT, At::Center);
        icons.assert_shown(Some(PARENT_ICON));
        visible.set(true);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::CenterRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(PARENT, At::BottomCenter);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::Center);
        visible.set(false);
        icons.assert_shown(Some(PARENT_ICON));
        icons.exit(PARENT);
    }
}

/// A 200 dp parent centering a 150 dp child (padded 20 dp) centering a 100 dp grandchild
/// (padded 40 dp), the two there with `visible`, each with its icon (overriding its
/// descendants as said).
fn descendants_appearing(
    child_overrides: bool,
    grandchild_overrides: bool,
    visible: MutableState<bool>,
) -> impl Fn(Device) + 'static {
    move |device| {
        let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
        let parent = device.icon(parent, PARENT_ICON, false);
        Box!(modifier = parent, content_alignment = Alignment::CENTER, {
            if visible.get() {
                let child = Modifier.padding(20.dp()).required_size(150.dp()).test_tag(CHILD);
                let child = device.icon(child, CHILD_ICON, child_overrides);
                Box!(modifier = child, content_alignment = Alignment::CENTER, {
                    let grandchild =
                        Modifier.padding(40.dp()).required_size(100.dp()).test_tag(GRANDCHILD);
                    Box!(modifier = device.icon(grandchild, GRANDCHILD_ICON, grandchild_overrides));
                });
            }
        });
    }
}

/// What the child-and-grandchild cases check: `over_grandchild` where both are, the child's
/// past the grandchild, the parent's past both.
fn verify_descendants_appearing(device: Device, child_overrides: bool, grandchild_overrides: bool) {
    let over_grandchild = if child_overrides { CHILD_ICON } else { GRANDCHILD_ICON };
    let visible = mutable_state_of(false);
    let mut icons = Icons::new(
        device,
        descendants_appearing(child_overrides, grandchild_overrides, visible.handle()),
    );
    icons.assert_shown(device.none());
    icons.enter(PARENT, At::Center);
    icons.assert_shown(Some(PARENT_ICON));
    visible.set(true);
    icons.assert_shown(Some(over_grandchild.clone()));
    icons.move_to(GRANDCHILD, At::CenterRight);
    icons.assert_shown(Some(over_grandchild));
    icons.move_to(CHILD, At::BottomRight);
    icons.assert_shown(Some(CHILD_ICON));
    icons.move_to(PARENT, At::BottomCenter);
    icons.assert_shown(Some(PARENT_ICON));
    icons.move_to(GRANDCHILD, At::Center);
    visible.set(false);
    icons.assert_shown(Some(PARENT_ICON));
    icons.exit(PARENT);
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveChildAndGrandchild_noOverrideDescendants,
// StylusHoverIconTest.dynamicallyAddAndRemoveChildAndGrandchild_noOverrideDescendants
#[test]
fn dynamically_add_and_remove_child_and_grandchild_no_override_descendants() {
    for device in DEVICES {
        verify_descendants_appearing(device, false, false);
    }
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveChildAndGrandchild_grandchildOverridesDescendants,
// StylusHoverIconTest.dynamicallyAddAndRemoveChildAndGrandchild_grandchildOverridesDescendants
#[test]
fn dynamically_add_and_remove_child_and_grandchild_grandchild_overrides_descendants() {
    for device in DEVICES {
        verify_descendants_appearing(device, false, true);
    }
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveChildAndGrandchild_childOverridesDescendants,
// StylusHoverIconTest.dynamicallyAddAndRemoveChildAndGrandchild_childOverridesDescendants
#[test]
fn dynamically_add_and_remove_child_and_grandchild_child_overrides_descendants() {
    for device in DEVICES {
        verify_descendants_appearing(device, true, false);
    }
}

/// A 150 dp child, moved (movable content) into a 200 dp parent centering it with `visible`.
fn parent_appearing(
    parent_overrides: bool,
    visible: MutableState<bool>,
) -> impl Fn(Device) + 'static {
    move |device| {
        let child = remember(move || {
            movable_content_of(move || {
                let child = Modifier.required_size(150.dp()).test_tag(CHILD);
                Box!(modifier = device.icon(child, CHILD_ICON, false));
            })
        });
        if visible.get() {
            let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
            let parent = device.icon(parent, PARENT_ICON, parent_overrides);
            let child = child.clone();
            Box!(modifier = parent, content_alignment = Alignment::CENTER, {
                child();
            });
        } else {
            child();
        }
    }
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveParent_noOverrideDescendants,
// StylusHoverIconTest.dynamicallyAddAndRemoveParent_noOverrideDescendants
#[test]
fn dynamically_add_and_remove_parent_no_override_descendants() {
    for device in DEVICES {
        let visible = mutable_state_of(false);
        let mut icons = Icons::new(device, parent_appearing(false, visible.handle()));
        icons.assert_shown(device.none());
        icons.enter(CHILD, At::Center);
        icons.assert_shown(Some(CHILD_ICON));
        // The parent comes around the child, under the resting pointer.
        visible.set(true);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(CHILD, At::CenterRight);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomCenter);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::Center);
        visible.set(false);
        icons.assert_shown(Some(CHILD_ICON));
        icons.exit(CHILD);
    }
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveParent_parentOverridesDescendants,
// StylusHoverIconTest.dynamicallyAddAndRemoveParent_parentOverridesDescendants
#[test]
fn dynamically_add_and_remove_parent_parent_overrides_descendants() {
    for device in DEVICES {
        let visible = mutable_state_of(false);
        let mut icons = Icons::new(device, parent_appearing(true, visible.handle()));
        icons.assert_shown(device.none());
        icons.enter(CHILD, At::Center);
        icons.assert_shown(Some(CHILD_ICON));
        visible.set(true);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::CenterRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(PARENT, At::BottomCenter);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::Center);
        visible.set(false);
        icons.assert_shown(Some(CHILD_ICON));
        icons.exit(CHILD);
    }
}

/// A 200 dp parent centering a 100 dp grandchild, moved (movable content) into a 150 dp child
/// between them with `visible`.
fn nested_child_appearing(
    child_overrides: bool,
    visible: MutableState<bool>,
) -> impl Fn(Device) + 'static {
    move |device| {
        let grandchild = remember(move || {
            movable_content_of(move || {
                let grandchild = Modifier.required_size(100.dp()).test_tag(GRANDCHILD);
                Box!(modifier = device.icon(grandchild, GRANDCHILD_ICON, false));
            })
        });
        let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
        let parent = device.icon(parent, PARENT_ICON, false);
        Box!(modifier = parent, content_alignment = Alignment::CENTER, {
            let grandchild = grandchild.clone();
            if visible.get() {
                let child = Modifier.required_size(150.dp()).test_tag(CHILD);
                let child = device.icon(child, CHILD_ICON, child_overrides);
                Box!(modifier = child, content_alignment = Alignment::CENTER, {
                    grandchild();
                });
            } else {
                grandchild();
            }
        });
    }
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveNestedChild_noOverrideDescendants,
// StylusHoverIconTest.dynamicallyAddAndRemoveNestedChild_noOverrideDescendants
#[test]
fn dynamically_add_and_remove_nested_child_no_override_descendants() {
    for device in DEVICES {
        let visible = mutable_state_of(false);
        let mut icons = Icons::new(device, nested_child_appearing(false, visible.handle()));
        icons.assert_shown(device.none());
        icons.enter(GRANDCHILD, At::Center);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        visible.set(true);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        icons.move_to(GRANDCHILD, At::CenterRight);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        icons.move_to(CHILD, At::BottomRight);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(GRANDCHILD, At::Center);
        visible.set(false);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        icons.exit(PARENT);
    }
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveNestedChild_ChildOverridesDescendants,
// StylusHoverIconTest.dynamicallyAddAndRemoveNestedChild_ChildOverridesDescendants
#[test]
fn dynamically_add_and_remove_nested_child_child_overrides_descendants() {
    for device in DEVICES {
        let visible = mutable_state_of(false);
        let mut icons = Icons::new(device, nested_child_appearing(true, visible.handle()));
        icons.assert_shown(device.none());
        icons.enter(GRANDCHILD, At::Center);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        visible.set(true);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(GRANDCHILD, At::CenterRight);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(CHILD, At::BottomRight);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomCenter);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(GRANDCHILD, At::Center);
        visible.set(false);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        icons.exit(PARENT);
    }
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveNestedChild_notHoveredOverChild,
// StylusHoverIconTest.dynamicallyAddAndRemoveNestedChild_notHoveredOverChild
#[test]
fn dynamically_add_and_remove_nested_child_not_hovered_over_child() {
    const GRANDPARENT: &str = "myGrandparentIcon";
    for device in DEVICES {
        let visible = mutable_state_of(false);
        let showing = visible.handle();
        let mut icons = Icons::new(device, move |device| {
            let grandchild = remember(move || {
                movable_content_of(move || {
                    let grandchild = Modifier.required_size(100.dp()).test_tag(GRANDCHILD);
                    Box!(modifier = device.icon(grandchild, GRANDCHILD_ICON, false));
                })
            });
            let grandparent = Modifier.required_size(200.dp()).test_tag(GRANDPARENT);
            let grandparent = device.icon(grandparent, PARENT_ICON, false);
            Box!(modifier = grandparent, content_alignment = Alignment::CENTER, {
                let grandchild = grandchild.clone();
                let parent = Modifier.required_size(175.dp()).test_tag(PARENT);
                let parent = device.icon(parent, PARENT_ICON, false);
                Box!(modifier = parent, content_alignment = Alignment::CENTER, {
                    let grandchild = grandchild.clone();
                    if showing.get() {
                        let child = Modifier.required_size(150.dp()).test_tag(CHILD);
                        let child = device.icon(child, CHILD_ICON, false);
                        Box!(modifier = child, content_alignment = Alignment::CENTER, {
                            grandchild();
                        });
                    } else {
                        grandchild();
                    }
                });
            });
        });
        icons.assert_shown(device.none());
        icons.enter(GRANDCHILD, At::Center);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        // Where no descendant is: the grandparent's corner.
        icons.move_to(GRANDPARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        visible.set(true);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(GRANDPARENT, At::CenterRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(GRANDPARENT, At::BottomRight);
        visible.set(false);
        icons.assert_shown(Some(PARENT_ICON));
        icons.exit(PARENT);
    }
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveGrandchild_notHoveredOverGrandchild,
// StylusHoverIconTest.dynamicallyAddAndRemoveGrandchild_notHoveredOverGrandchild
#[test]
fn dynamically_add_and_remove_grandchild_not_hovered_over_grandchild() {
    for device in DEVICES {
        let visible = mutable_state_of(false);
        let showing = visible.handle();
        let mut icons = Icons::new(device, move |device| {
            let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
            let parent = device.icon(parent, PARENT_ICON, false);
            Box!(modifier = parent, content_alignment = Alignment::CENTER, {
                let child = Modifier.required_size(150.dp()).test_tag(CHILD);
                let child = device.icon(child, CHILD_ICON, false);
                Box!(modifier = child, content_alignment = Alignment::CENTER, {
                    if showing.get() {
                        let grandchild = Modifier.required_size(100.dp()).test_tag(GRANDCHILD);
                        Box!(modifier = device.icon(grandchild, GRANDCHILD_ICON, false));
                    }
                });
            });
        });
        icons.assert_shown(device.none());
        icons.enter(CHILD, At::Center);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        visible.set(true);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(PARENT, At::CenterRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::BottomRight);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::BottomRight);
        visible.set(false);
        icons.assert_shown(Some(PARENT_ICON));
        icons.exit(PARENT);
    }
}

/// A 200 dp parent over a column of child A (50 dp) and child B (the grandchild's tag and icon,
/// 50 dp offset 100 dp down), there with `visible`.
///
/// Compose's `dynamicallyAddAndRemoveSibling_hoveredOverAppearingSibling` (`@Ignore`d there,
/// b/271277248) isn't ported: its 150 dp parent puts child B below it, never under the pointer
/// it expects B's icon at.
fn sibling_appearing(visible: MutableState<bool>) -> impl Fn(Device) + 'static {
    move |device| {
        let parent = Modifier.required_size(200.dp()).test_tag(PARENT);
        Box!(modifier = device.icon(parent, PARENT_ICON, false), {
            Column!({
                let a = Modifier.required_size(50.dp()).test_tag(CHILD);
                Box!(modifier = device.icon(a, CHILD_ICON, false));
                if visible.get() {
                    let b = Modifier
                        .required_size(50.dp())
                        .offset(0.dp(), 100.dp())
                        .test_tag(GRANDCHILD);
                    Box!(modifier = device.icon(b, GRANDCHILD_ICON, false));
                }
            });
        });
    }
}

// Compose: PointerIconTest.dynamicallyAddAndRemoveSibling_notHoveredOverAppearingSibling,
// StylusHoverIconTest.dynamicallyAddAndRemoveSibling_notHoveredOverAppearingSibling (both
// `@Ignore`d in Compose, b/271277248)
#[test]
fn dynamically_add_and_remove_sibling_not_hovered_over_appearing_sibling() {
    for device in DEVICES {
        let visible = mutable_state_of(false);
        let mut icons = Icons::new(device, sibling_appearing(visible.handle()));
        icons.assert_shown(device.none());
        icons.enter(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
        icons.move_to(CHILD, At::Center);
        icons.assert_shown(Some(CHILD_ICON));
        visible.set(true);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(GRANDCHILD, At::BottomRight);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        icons.move_to(CHILD, At::Center);
        icons.assert_shown(Some(CHILD_ICON));
        visible.set(false);
        icons.assert_shown(Some(CHILD_ICON));
        icons.exit(CHILD);
    }
}

// Compose: PointerIconTest.childNotFullyContainedInParent_noOverrideDescendants,
// StylusHoverIconTest.childNotFullyContainedInParent_noOverrideDescendants
#[test]
fn child_not_fully_contained_in_parent_no_override_descendants() {
    const WRAPPER: &str = "myDefaultWrapper";
    for device in DEVICES {
        let mut icons = Icons::new(device, move |device| {
            Box!(modifier = Modifier.fill_max_size().test_tag(WRAPPER), {
                let parent = Modifier.required_size((200.dp(), 150.dp())).test_tag(PARENT);
                Box!(modifier = device.icon(parent, PARENT_ICON, false), {
                    let child = Modifier.required_size((150.dp(), 125.dp())).test_tag(CHILD);
                    Box!(modifier = device.icon(child, CHILD_ICON, false), {
                        // Past the child's and the parent's right edges.
                        let grandchild = Modifier
                            .required_size((300.dp(), 100.dp()))
                            .offset(100.dp(), 0.dp())
                            .test_tag(GRANDCHILD);
                        Box!(modifier = device.icon(grandchild, GRANDCHILD_ICON, false));
                    });
                });
            });
        });
        icons.assert_shown(device.none());
        icons.enter(WRAPPER, At::Center);
        icons.assert_shown(device.none());
        icons.move_to(GRANDCHILD, At::BottomRight);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        icons.move_to(CHILD, At::CenterRight);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        icons.move_to(CHILD, At::BottomRight);
        icons.assert_shown(Some(CHILD_ICON));
        icons.move_to(PARENT, At::CenterRight);
        icons.assert_shown(Some(GRANDCHILD_ICON));
        icons.move_to(PARENT, At::BottomRight);
        icons.assert_shown(Some(PARENT_ICON));
    }
}

/// A box filling the window (its icon `wrapper_icon`) with a 10 dp text-icon child at its top
/// left, there with `show`.
fn child_removed(
    wrapper_icon: Option<PointerIcon>,
    show: MutableState<bool>,
) -> impl Fn(Device) + 'static {
    move |device| {
        let wrapper = Modifier.fill_max_size();
        let wrapper = match wrapper_icon.clone() {
            Some(icon) => device.icon(wrapper, icon, false),
            None => wrapper,
        };
        Box!(modifier = wrapper.test_tag("myDefaultWrapper"), {
            if show.get() {
                let child = device.icon(Modifier, PointerIcon::TEXT, false);
                Box!(modifier = child.size((10.dp(), 10.dp())));
            }
        });
    }
}

// Compose: PointerIconTest.resetPointerIconWhenChildRemoved_parentDoesSetIcon_iconIsHand,
// StylusHoverIconTest.resetPointerIconWhenChildRemoved_parentDoesSetIcon_iconIsHand
#[test]
fn reset_pointer_icon_when_child_removed_parent_does_set_icon_icon_is_hand() {
    for device in DEVICES {
        let show = mutable_state_of(true);
        let mut icons = Icons::new(device, child_removed(Some(PointerIcon::HAND), show.handle()));
        icons.assert_shown(device.none());
        icons.move_to("myDefaultWrapper", At::Point(5.0, 5.0));
        icons.assert_shown(Some(PointerIcon::TEXT));
        show.set(false);
        icons.move_to("myDefaultWrapper", At::Point(6.0, 6.0));
        icons.assert_shown(Some(PointerIcon::HAND));
    }
}

// Compose: PointerIconTest.resetPointerIconWhenChildRemoved_parentDoesNotSetIcon_iconIsDefault,
// StylusHoverIconTest.resetPointerIconWhenChildRemoved_parentDoesNotSetIcon_iconIsDefault
#[test]
fn reset_pointer_icon_when_child_removed_parent_does_not_set_icon_icon_is_default() {
    for device in DEVICES {
        let show = mutable_state_of(true);
        let mut icons = Icons::new(device, child_removed(None, show.handle()));
        icons.assert_shown(device.none());
        icons.move_to("myDefaultWrapper", At::Point(5.0, 5.0));
        icons.assert_shown(Some(PointerIcon::TEXT));
        show.set(false);
        icons.move_to("myDefaultWrapper", At::Point(6.0, 6.0));
        icons.assert_shown(device.none());
    }
}

// Compose: no test; `PointerHoverIconModifierNode` and `StylusHoverIconModifierNode`'s
// `isRelevantPointerType`: each hears its own pointers, so a mouse's icon never shows for a
// stylus nor the other way; and a stylus touching down stops hovering, as Android's hover exit
// before `ACTION_DOWN`.
#[test]
fn a_mouse_s_and_a_stylus_s_icons_are_their_own() {
    let mut icons = Icons::new(Device::Mouse, |_| {
        let outer =
            Modifier.required_size(200.dp()).test_tag(PARENT).pointer_hover_icon(PARENT_ICON);
        Box!(modifier = outer, {
            let inner = Modifier.padding(20.dp()).required_size(100.dp()).test_tag(CHILD);
            Box!(modifier = inner.stylus_hover_icon(CHILD_ICON));
        });
    });
    icons.enter(CHILD, At::Center);
    assert_eq!(icons.ui.host().pointer_icon(), PARENT_ICON, "the mouse's, over the stylus's");
    assert_eq!(icons.ui.host().stylus_hover_icon(), None);
    icons.device = Device::Stylus;
    icons.move_to(CHILD, At::Center);
    assert_eq!(icons.ui.host().stylus_hover_icon(), Some(CHILD_ICON));
    assert_eq!(icons.ui.host().pointer_icon(), PARENT_ICON, "the mouse's stays");
    icons.move_to(PARENT, At::BottomRight);
    assert_eq!(icons.ui.host().stylus_hover_icon(), None, "the mouse's isn't the stylus's");
    // Touching down: no hover, no icon, until it hovers again.
    icons.move_to(CHILD, At::Center);
    let position = icons.ui.on_node_with_tag(CHILD).fetch_semantics_node().bounds_in_root();
    let at = Offset::new(position.left + 1.0, position.top + 1.0);
    let stylus = |event: PointerEvent| event.with_pointer(PointerId(1), PointerType::Stylus);
    icons.ui.host().send_pointer(stylus(PointerEvent::press(at)));
    assert_eq!(icons.ui.host().stylus_hover_icon(), None, "pressed");
    icons.ui.host().send_pointer(stylus(PointerEvent::release(at)));
    assert_eq!(icons.ui.host().stylus_hover_icon(), None, "released");
    icons.move_to(CHILD, At::Center);
    assert_eq!(icons.ui.host().stylus_hover_icon(), Some(CHILD_ICON), "hovering again");
    icons.exit(CHILD);
    assert_eq!(icons.ui.host().stylus_hover_icon(), None, "out of range");
}
