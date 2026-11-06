//! `ButtonGroup`, as Compose's Material 3: a row of buttons where the one pressed grows and its
//! neighbours give way, the items that don't fit in an overflow menu, with `ButtonGroupScope`,
//! `ButtonGroupMenuState` and `ButtonGroupDefaults` (the connected groups' shapes too).

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use compose_animation::{Animatable, AnimationSpec};
use compose_foundation::layout::{
    Arrangement, Box, HorizontalArrangement, PaddingValues, RowScope, Spacer,
};
use compose_runtime::{
    ComposeArg, CoroutineScope, Job, ObjectHandle, Owned, OwnedState, composable, mutable_state_of,
};
use compose_ui::{
    AlignScope, CornerSize, Density, Dp, Interaction, InteractionSource, IntrinsicMeasureScope,
    Layout, Measurable, MeasurePolicy, MeasureResult, MeasureScope, Modifier, ModifierNode,
    ModifierNodeElement, NodeContext, ParentAlignment, ParentDataModifierNode, PressInteraction,
    Shape, SharedString, TextOverflow, VerticalAlignment, WeightScope,
};

use crate::button::{Button, ButtonDefaults};
use crate::components::Text;
use crate::dialog::{DropdownMenu, DropdownMenuItem};
use crate::icon::{FilledIconButton, Icon, IconButtonColors, icons};
use crate::theme::MaterialTheme;
use crate::toggle_button::{ToggleButton, ToggleButtonShapes};
use crate::tooltip::{
    PlainTooltip, TooltipAnchorPosition, TooltipBox, TooltipDefaults, remember_tooltip_state,
};

/// Whether a group's overflow menu shows, as Compose's `ButtonGroupMenuState`. A `Copy` handle
/// (D240): [`new`](Self::new) returns its owner, which `ButtonGroup` remembers.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ButtonGroupMenuState {
    object: ObjectHandle<OwnedState<bool>>,
}

impl ComposeArg for ButtonGroupMenuState {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl ButtonGroupMenuState {
    /// A state showing the menu or not, as `ButtonGroupMenuState(initialIsShowing)`.
    pub fn new(initial_is_showing: bool) -> Owned<ButtonGroupMenuState> {
        Owned::new(mutable_state_of(initial_is_showing), |object| ButtonGroupMenuState { object })
    }

    /// Whether the menu shows, as `isShowing` (a state read).
    pub fn is_showing(&self) -> bool {
        self.object.get().is_some_and(|showing| showing.get())
    }

    /// Hides the menu, as `dismiss()`.
    pub fn dismiss(&self) {
        if let Some(showing) = self.object.get() {
            showing.set(false);
        }
    }

    /// Shows the menu, as `show()`.
    pub fn show(&self) {
        if let Some(showing) = self.object.get() {
            showing.set(true);
        }
    }
}

/// An item of a group: its button, and what stands for it in the overflow menu.
#[derive(Clone)]
struct Item {
    button: Rc<dyn Fn()>,
    menu: Rc<dyn Fn(ButtonGroupMenuState)>,
}

/// What a group's content adds its items with, and the modifiers its own buttons take, as
/// Compose's `ButtonGroupScope`: `Modifier::weight(&scope, …)` and `align(&scope, …)` (the
/// scope by reference, which is `Copy`), and `ButtonGroupModifiers::animate_width`.
#[derive(Clone)]
pub struct ButtonGroupScope {
    items: Rc<RefCell<Vec<Item>>>,
    spec: AnimationSpec,
}

impl WeightScope for &ButtonGroupScope {}

impl AlignScope for &ButtonGroupScope {
    type Alignment = VerticalAlignment;
    fn parent_alignment(alignment: VerticalAlignment) -> ParentAlignment {
        ParentAlignment::Vertical(alignment)
    }
}

impl ButtonGroupScope {
    /// A button labelled `label`, as `clickableItem(onClick, label)`.
    pub fn clickable_item(&self, on_click: impl Fn() + 'static, label: impl Into<SharedString>) {
        self.clickable_item_with(on_click, label, None::<fn()>, None, true);
    }

    /// A button with an icon, its share of the space (`None`, Compose's `NaN`, for its own width)
    /// and whether it's enabled, as `clickableItem(onClick, label, icon, weight, enabled)`.
    ///
    /// # Panics
    ///
    /// If `weight` isn't above zero, as Compose's `require`.
    pub fn clickable_item_with(
        &self,
        on_click: impl Fn() + 'static,
        label: impl Into<SharedString>,
        icon: Option<impl Fn() + 'static>,
        weight: Option<f32>,
        enabled: bool,
    ) {
        check_weight(weight);
        let (label, spec) = (label.into(), self.spec.clone());
        let icon: Option<Rc<dyn Fn()>> = icon.map(|icon| Rc::new(icon) as Rc<dyn Fn()>);
        let on_click: Rc<dyn Fn()> = Rc::new(on_click);
        let button = {
            let (label, icon, on_click) = (label.clone(), icon.clone(), on_click.clone());
            move || {
                let source = compose_runtime::remember(compose_ui::MutableInteractionSource::new);
                let padding = item_padding(icon.is_some());
                let on_click = on_click.clone();
                let content = item_content(icon.clone(), label.clone());
                Button!(
                    move || on_click(),
                    modifier = item_modifier(&source, &spec, padding, weight),
                    enabled = enabled,
                    content_padding = padding,
                    interaction_source = Some(source),
                    content = move |scope: RowScope| content(scope)
                );
            }
        };
        let menu = move |state: ButtonGroupMenuState| {
            let on_click = on_click.clone();
            menu_item(&label, icon.clone(), enabled, move || {
                on_click();
                state.dismiss();
            });
        };
        self.items.borrow_mut().push(Item { button: Rc::new(button), menu: Rc::new(menu) });
    }

    /// A toggle button labelled `label`, as `toggleableItem(checked, label, onCheckedChange)`.
    pub fn toggleable_item(
        &self,
        checked: bool,
        label: impl Into<SharedString>,
        on_checked_change: impl Fn(bool) + 'static,
    ) {
        self.toggleable_item_with(checked, label, on_checked_change, None::<fn()>, None, true);
    }

    /// A toggle button with an icon, its share of the space and whether it's enabled, as
    /// `toggleableItem(checked, label, onCheckedChange, icon, weight, enabled)`.
    ///
    /// # Panics
    ///
    /// If `weight` isn't above zero, as Compose's `require`.
    pub fn toggleable_item_with(
        &self,
        checked: bool,
        label: impl Into<SharedString>,
        on_checked_change: impl Fn(bool) + 'static,
        icon: Option<impl Fn() + 'static>,
        weight: Option<f32>,
        enabled: bool,
    ) {
        check_weight(weight);
        let (label, spec) = (label.into(), self.spec.clone());
        let icon: Option<Rc<dyn Fn()>> = icon.map(|icon| Rc::new(icon) as Rc<dyn Fn()>);
        let on_checked_change: Rc<dyn Fn(bool)> = Rc::new(on_checked_change);
        let button = {
            let (label, icon, on_change) = (label.clone(), icon.clone(), on_checked_change.clone());
            move || {
                let source = compose_runtime::remember(compose_ui::MutableInteractionSource::new);
                let padding = item_padding(icon.is_some());
                let on_change = on_change.clone();
                let content = item_content(icon.clone(), label.clone());
                ToggleButton!(
                    checked,
                    move |now| on_change(now),
                    modifier = item_modifier(&source, &spec, padding, weight),
                    enabled = enabled,
                    content_padding = padding,
                    interaction_source = Some(source),
                    content = move |scope: RowScope| content(scope)
                );
            }
        };
        let menu = move |state: ButtonGroupMenuState| {
            let on_change = on_checked_change.clone();
            menu_item(&label, icon.clone(), enabled, move || {
                on_change(!checked);
                state.dismiss();
            });
        };
        self.items.borrow_mut().push(Item { button: Rc::new(button), menu: Rc::new(menu) });
    }

    /// An item of the app's own: `button_group_content` in the group, `menu_content` in the
    /// overflow menu, as `customItem(buttonGroupContent, menuContent)`.
    pub fn custom_item(
        &self,
        button_group_content: impl Fn() + 'static,
        menu_content: impl Fn(ButtonGroupMenuState) + 'static,
    ) {
        let item = Item { button: Rc::new(button_group_content), menu: Rc::new(menu_content) };
        self.items.borrow_mut().push(item);
    }
}

fn check_weight(weight: Option<f32>) {
    if let Some(weight) = weight {
        assert!(weight > 0.0, "invalid weight {weight}; must be greater than zero or None");
    }
}

/// A clickable or toggleable item's padding: the button's, with an icon or without.
fn item_padding(has_icon: bool) -> PaddingValues {
    if has_icon {
        ButtonDefaults::BUTTON_WITH_ICON_CONTENT_PADDING
    } else {
        ButtonDefaults::CONTENT_PADDING
    }
}

/// A clickable or toggleable item's modifier: growing when pressed, giving no more than its end
/// padding when a neighbour is, and its weight.
fn item_modifier(
    source: &compose_ui::MutableInteractionSource,
    spec: &AnimationSpec,
    padding: PaddingValues,
    weight: Option<f32>,
) -> Modifier {
    let modifier = Modifier.then_custom(EnlargeOnPress {
        source: source.as_source().clone(),
        spec: spec.clone(),
        compression_limit: Some(padding.end),
    });
    match weight {
        Some(weight) => modifier.then_custom(GroupWeight(weight.min(f32::MAX))),
        None => modifier,
    }
}

/// A clickable or toggleable item's content: its icon and a space, then its label on one line.
fn item_content(
    icon: Option<Rc<dyn Fn()>>,
    label: SharedString,
) -> compose_runtime::Callback<dyn Fn(RowScope)> {
    compose_runtime::callback(move |_: RowScope| {
        if let Some(icon) = &icon {
            icon();
            Spacer(Modifier.size(ButtonDefaults::ICON_SPACING));
        }
        Text!(label.clone(), max_lines = 1, soft_wrap = false, overflow = TextOverflow::Visible);
    })
}

fn menu_item(
    label: &SharedString,
    icon: Option<Rc<dyn Fn()>>,
    enabled: bool,
    on_click: impl Fn() + 'static,
) {
    let (label, on_click) = (label.clone(), Rc::new(on_click));
    DropdownMenuItem!(
        move || Text!(label.clone()),
        move || on_click(),
        leading_icon = icon.map(|icon| move || icon()),
        enabled = enabled
    );
}

/// The weight of a clickable or toggleable item, as `ButtonGroupElement`: the built-in weight's
/// parent data, which the group reads.
#[derive(Clone, Copy, PartialEq)]
struct GroupWeight(f32);

impl ModifierNodeElement for GroupWeight {
    type Node = GroupWeight;
    fn create(&self) -> GroupWeight {
        *self
    }
    fn update(&self, node: &mut GroupWeight) {
        *node = *self;
    }
    fn same(&self, other: &Self) -> bool {
        self == other
    }
    fn name(&self) -> &'static str {
        "weight"
    }
}

impl ModifierNode for GroupWeight {
    fn as_parent_data(&self) -> Option<&dyn ParentDataModifierNode> {
        Some(self)
    }
}

impl ParentDataModifierNode for GroupWeight {
    fn modify_parent_data(&self, _: Density, data: Option<Rc<dyn Any>>) -> Option<Rc<dyn Any>> {
        let previous = data.as_ref().and_then(|data| data.downcast_ref::<GroupData>());
        Some(Rc::new(GroupData {
            weight: Some(self.0),
            pressed: previous.and_then(|data| data.pressed.clone()),
            compression_limit: previous.map_or(Dp(0.0), |data| data.compression_limit),
        }))
    }
}

/// What a group's child tells it, as `ButtonGroupParentData`: its weight (a clickable or
/// toggleable item's; a custom one's is the built-in `weight`'s), how far it's pressed, and how
/// much it gives a pressed neighbour.
struct GroupData {
    weight: Option<f32>,
    pressed: Option<Animatable<f32>>,
    compression_limit: Dp,
}

/// `Modifier::animate_width`'s element, as `EnlargeOnPressElement`.
struct EnlargeOnPress {
    source: InteractionSource,
    spec: AnimationSpec,
    compression_limit: Option<Dp>,
}

struct EnlargeOnPressNode {
    source: InteractionSource,
    spec: Rc<RefCell<AnimationSpec>>,
    compression_limit: Option<Dp>,
    pressed: Animatable<f32>,
    listener: Option<Rc<compose_ui::Listener>>,
    scope: Option<CoroutineScope>,
    /// What the listener launched (the growth, the wait and shrink): cancelled with it, as the
    /// children of Compose's `collectionJob`.
    jobs: Rc<RefCell<Vec<Job>>>,
}

impl ModifierNodeElement for EnlargeOnPress {
    type Node = EnlargeOnPressNode;
    fn create(&self) -> EnlargeOnPressNode {
        EnlargeOnPressNode {
            source: self.source.clone(),
            spec: Rc::new(RefCell::new(self.spec.clone())),
            compression_limit: self.compression_limit,
            pressed: Animatable::new(0.0),
            listener: None,
            scope: None,
            jobs: Rc::default(),
        }
    }
    fn update(&self, node: &mut EnlargeOnPressNode) {
        if node.source != self.source {
            node.source = self.source.clone();
            node.listen();
        }
        *node.spec.borrow_mut() = self.spec.clone();
        node.compression_limit = self.compression_limit;
    }
    fn same(&self, other: &Self) -> bool {
        self.source == other.source
            && self.spec == other.spec
            && self.compression_limit == other.compression_limit
    }
    fn name(&self) -> &'static str {
        "EnlargeOnPressElement"
    }
}

impl EnlargeOnPressNode {
    /// Follows the source's presses, as `launchCollectionJob`: pressed, it grows; released, once
    /// it's three quarters grown (or a second has gone), it shrinks back.
    fn listen(&mut self) {
        for job in self.jobs.borrow_mut().drain(..) {
            job.cancel();
        }
        let Some(scope) = self.scope else {
            return;
        };
        let (pressed, spec, jobs) = (self.pressed.clone(), self.spec.clone(), self.jobs.clone());
        let presses = RefCell::new(Vec::new());
        let waiting: Cell<Option<Job>> = Cell::new(None);
        self.listener = Some(self.source.listen(move |interaction| {
            let was_pressed = !presses.borrow().is_empty();
            match interaction {
                Interaction::Press(PressInteraction::Press(press)) => {
                    presses.borrow_mut().push(*press)
                }
                Interaction::Press(
                    PressInteraction::Release(press) | PressInteraction::Cancel(press),
                ) => presses.borrow_mut().retain(|it| it != press),
                _ => {}
            }
            let is_pressed = !presses.borrow().is_empty();
            if is_pressed == was_pressed {
                return;
            }
            if let Some(job) = waiting.take() {
                job.cancel();
            }
            let (pressed, spec) = (pressed.clone(), spec.borrow().clone());
            let mut jobs = jobs.borrow_mut();
            jobs.retain(Job::is_active);
            if is_pressed {
                jobs.push(scope.launch(async move {
                    pressed.animate_to(1.0, spec).await;
                }));
            } else {
                let job = scope.launch(async move {
                    let start = compose_runtime::with_frame_millis(|time| time).await;
                    while pressed.value() <= 0.75 {
                        let now = compose_runtime::with_frame_millis(|time| time).await;
                        if now.saturating_sub(start) > MAX_WAIT_TIME_MILLIS {
                            break;
                        }
                    }
                    pressed.animate_to(0.0, spec).await;
                });
                jobs.push(job);
                waiting.set(Some(job));
            }
        }));
    }
}

/// How long a release waits for the growth to reach three quarters, as `MAX_WAIT_TIME_MILLIS`.
const MAX_WAIT_TIME_MILLIS: u64 = 1_000;

impl ModifierNode for EnlargeOnPressNode {
    fn as_parent_data(&self) -> Option<&dyn ParentDataModifierNode> {
        Some(self)
    }
    fn on_attach(&mut self, context: &mut NodeContext<'_>) {
        self.scope = Some(context.coroutine_scope());
        self.listen();
    }
    fn on_detach(&mut self, _: &mut NodeContext<'_>) {
        self.listener = None;
        self.scope = None;
    }
}

impl ParentDataModifierNode for EnlargeOnPressNode {
    fn modify_parent_data(&self, _: Density, data: Option<Rc<dyn Any>>) -> Option<Rc<dyn Any>> {
        let previous = data.as_ref().and_then(|data| data.downcast_ref::<GroupData>());
        // Without a limit of its own, the button's end padding: Compose's is relative, the same
        // either way.
        let limit = self.compression_limit.unwrap_or(ButtonDefaults::CONTENT_PADDING.end);
        Some(Rc::new(GroupData {
            weight: previous.and_then(|data| data.weight),
            pressed: Some(self.pressed.clone()),
            compression_limit: limit,
        }))
    }
}

/// `Modifier::animate_width`, in a trait as it's of this crate.
pub trait ButtonGroupModifiers {
    /// Grows the button as `interaction_source` is pressed, its neighbours in `scope`'s group
    /// giving way (each no more than the button's end padding by default), as
    /// `ButtonGroupScope.animateWidth(interactionSource)`.
    fn animate_width(
        self,
        scope: &ButtonGroupScope,
        interaction_source: &InteractionSource,
    ) -> Modifier;
    /// As `animate_width`, giving a pressed neighbour no more than `compression_limit`, as
    /// `animateWidth(interactionSource, compressionLimit)`.
    fn animate_width_with(
        self,
        scope: &ButtonGroupScope,
        interaction_source: &InteractionSource,
        compression_limit: Dp,
    ) -> Modifier;
}

impl ButtonGroupModifiers for Modifier {
    fn animate_width(
        self,
        scope: &ButtonGroupScope,
        interaction_source: &InteractionSource,
    ) -> Modifier {
        self.then_custom(EnlargeOnPress {
            source: interaction_source.clone(),
            spec: scope.spec.clone(),
            compression_limit: None,
        })
    }

    fn animate_width_with(
        self,
        scope: &ButtonGroupScope,
        interaction_source: &InteractionSource,
        compression_limit: Dp,
    ) -> Modifier {
        self.then_custom(EnlargeOnPress {
            source: interaction_source.clone(),
            spec: scope.spec.clone(),
            compression_limit: Some(compression_limit),
        })
    }
}

/// As Compose's `ButtonGroupDefaults`.
pub struct ButtonGroupDefaults {}

impl ButtonGroupDefaults {
    /// How much of its width a pressed button grows by, as `ExpandedRatio`.
    pub const EXPANDED_RATIO: f32 = 0.15;
    /// Between a connected group's buttons, as `ConnectedSpaceBetween`.
    pub const CONNECTED_SPACE_BETWEEN: Dp = Dp(2.0);

    /// 12 dp between the buttons, as `HorizontalArrangement`.
    pub fn horizontal_arrangement() -> HorizontalArrangement {
        Arrangement::spaced_by(Dp(12.0)).into()
    }

    /// A connected group's first button: round at the start, the small corners at the end, as
    /// `connectedLeadingButtonShape`.
    pub fn connected_leading_button_shape() -> Shape {
        start_round(Dp(8.0))
    }

    /// Pressed, the extra small corners at its end, as `connectedLeadingButtonPressShape`.
    pub fn connected_leading_button_press_shape() -> Shape {
        start_round(Dp(4.0))
    }

    /// A connected group's last button, as `connectedTrailingButtonShape`.
    pub fn connected_trailing_button_shape() -> Shape {
        end_round(Dp(8.0))
    }

    /// Pressed, as `connectedTrailingButtonPressShape`.
    pub fn connected_trailing_button_press_shape() -> Shape {
        end_round(Dp(4.0))
    }

    /// A connected group's checked button, a pill, as `connectedButtonCheckedShape`.
    pub fn connected_button_checked_shape() -> Shape {
        compose_foundation::CIRCLE_SHAPE
    }

    /// A connected group's middle button pressed, as `connectedMiddleButtonPressShape`.
    pub fn connected_middle_button_press_shape() -> Shape {
        Shape::Rounded([CornerSize::Dp(Dp(4.0)); 4])
    }

    /// As `connectedLeadingButtonShapes()`; change one with struct update.
    pub fn connected_leading_button_shapes() -> ToggleButtonShapes {
        ToggleButtonShapes {
            shape: Self::connected_leading_button_shape(),
            pressed_shape: Self::connected_leading_button_press_shape(),
            checked_shape: Self::connected_button_checked_shape(),
        }
    }

    /// The theme's small corners, as `connectedMiddleButtonShapes()`.
    pub fn connected_middle_button_shapes() -> ToggleButtonShapes {
        ToggleButtonShapes {
            shape: crate::ShapeDefaults::SMALL,
            pressed_shape: Self::connected_middle_button_press_shape(),
            checked_shape: Self::connected_button_checked_shape(),
        }
    }

    /// As `connectedTrailingButtonShapes()`.
    pub fn connected_trailing_button_shapes() -> ToggleButtonShapes {
        ToggleButtonShapes {
            shape: Self::connected_trailing_button_shape(),
            pressed_shape: Self::connected_trailing_button_press_shape(),
            checked_shape: Self::connected_button_checked_shape(),
        }
    }
}

/// Round at the start, `inner` corners at the end.
fn start_round(inner: Dp) -> Shape {
    let full = CornerSize::Percent(50.0);
    Shape::Rounded([full, CornerSize::Dp(inner), CornerSize::Dp(inner), full])
}

/// `inner` corners at the start, round at the end.
fn end_round(inner: Dp) -> Shape {
    let full = CornerSize::Percent(50.0);
    Shape::Rounded([CornerSize::Dp(inner), full, full, CornerSize::Dp(inner)])
}

/// The overflow indicator by default, as `ButtonGroupDefaults.OverflowIndicator`: a filled icon
/// button of three dots, "More options" in a tooltip above it, showing and hiding the menu.
#[composable]
pub fn ButtonGroupOverflowIndicator(
    menu_state: ButtonGroupMenuState,
    #[default(crate::Modifier)] modifier: Modifier,
    #[default(true)] enabled: bool,
    #[default(crate::IconButtonDefaults::filled_shape())] shape: Shape,
    #[default(crate::IconButtonDefaults::filled_icon_button_colors())] colors: IconButtonColors,
    #[default(None)] interaction_source: Option<compose_ui::MutableInteractionSource>,
) {
    const MORE_OPTIONS: &str = "More options";
    let provider =
        TooltipDefaults::remember_tooltip_position_provider(TooltipAnchorPosition::Above);
    TooltipBox!(
        provider,
        |scope| PlainTooltip!(scope = Some(scope), { Text!(MORE_OPTIONS) }),
        remember_tooltip_state!(),
        {
            FilledIconButton!(
                move || if menu_state.is_showing() {
                    menu_state.dismiss()
                } else {
                    menu_state.show()
                },
                modifier = modifier.clone(),
                enabled = enabled,
                shape = shape.clone(),
                colors = colors,
                interaction_source = interaction_source.clone(),
                { Icon!(icons::filled::more_vert(), Some(MORE_OPTIONS)) }
            );
        }
    );
}

/// A row of buttons, as Compose's `ButtonGroup`: the one pressed grows by `expanded_ratio` of its
/// width (its `animate_width`), its neighbours giving way; what doesn't fit goes into a menu that
/// `overflow_indicator` (given the menu's state) opens, standing after the items that fit.
/// `content` adds the items (`ButtonGroupScope`), each time the group composes.
#[composable]
pub fn ButtonGroup(
    overflow_indicator: impl Fn(ButtonGroupMenuState) + 'static,
    #[default(crate::Modifier)] modifier: Modifier,
    #[default(crate::ButtonGroupDefaults::EXPANDED_RATIO)] expanded_ratio: f32,
    #[default(crate::ButtonGroupDefaults::horizontal_arrangement())] horizontal_arrangement: impl Into<
        HorizontalArrangement,
    >,
    #[default(crate::Alignment::TOP)] vertical_alignment: VerticalAlignment,
    content: impl Fn(ButtonGroupScope) + 'static,
) {
    let spec = MaterialTheme::motion_scheme().fast_spatial_spec();
    let scope = ButtonGroupScope { items: Rc::new(RefCell::new(Vec::new())), spec };
    content(scope.clone());
    let items: Rc<Vec<Item>> = Rc::new(scope.items.take());
    let menu_state = compose_runtime::remember(|| ButtonGroupMenuState::new(false));
    let overflow = compose_runtime::remember(|| Rc::new(OverflowState::default()));
    let horizontal_arrangement: HorizontalArrangement = horizontal_arrangement;
    let policy = compose_runtime::remember_keyed(
        (horizontal_arrangement.clone(), vertical_alignment, expanded_ratio.to_bits()),
        || {
            Rc::new(ButtonGroupMeasurePolicy {
                overflow: overflow.clone(),
                horizontal_arrangement: horizontal_arrangement.clone(),
                vertical_alignment,
                expanded_ratio,
            }) as Rc<dyn MeasurePolicy>
        },
    );
    Layout(
        modifier,
        policy,
        compose_runtime::callback(move || {
            for item in items.iter() {
                (item.button)();
            }
            let (items, overflow, overflow_indicator) =
                (items.clone(), overflow.clone(), overflow_indicator.clone());
            // The indicator and its menu, the layout's last child.
            Box!({
                overflow_indicator(menu_state);
                let (items, overflow) = (items.clone(), overflow.clone());
                DropdownMenu!(menu_state.is_showing(), move || menu_state.dismiss(), {
                    for item in &items[overflow.visible.get().min(items.len())..] {
                        (item.menu)(menu_state);
                    }
                });
            });
        }),
    );
}

/// How many items show, as `ButtonGroupOverflowState`: the layout writes it, the menu reads it.
struct OverflowState {
    visible: compose_runtime::MutableState<usize>,
    _owner: OwnedState<usize>,
}

impl Default for OverflowState {
    fn default() -> OverflowState {
        let owner = mutable_state_of(0);
        OverflowState { visible: owner.handle(), _owner: owner }
    }
}

/// `ButtonGroup`'s measure policy, as `ButtonGroupMeasurePolicy`: its children are the nodes the
/// items emit (each node an item to it, as Compose's content measurables), then the overflow
/// indicator's box.
struct ButtonGroupMeasurePolicy {
    overflow: Rc<OverflowState>,
    horizontal_arrangement: HorizontalArrangement,
    vertical_alignment: VerticalAlignment,
    expanded_ratio: f32,
}

fn group_data(scope: &IntrinsicMeasureScope<'_>, child: Measurable) -> Option<Rc<dyn Any>> {
    scope.parent_data_any(child)
}

impl MeasurePolicy for ButtonGroupMeasurePolicy {
    fn measure(
        &self,
        scope: &mut MeasureScope<'_>,
        measurables: &[Measurable],
        constraints: compose_ui::Constraints,
    ) -> MeasureResult {
        let (items, overflow) = measurables.split_at(measurables.len().saturating_sub(1));
        let density = scope.density();
        let spacing = density.round_to_px(self.horizontal_arrangement.spacing());
        let size = items.len();
        // Each item's weight, how far it's pressed and what it gives.
        let data: Vec<(f32, f32, f32)> = items
            .iter()
            .map(|&item| {
                let custom = group_data(scope, item);
                let custom = custom.as_ref().and_then(|data| data.downcast_ref::<GroupData>());
                let weight = custom
                    .and_then(|data| data.weight)
                    .or_else(|| scope.parent_data(item).weight.map(|(weight, _)| weight))
                    .unwrap_or(0.0);
                let pressed =
                    custom.and_then(|data| data.pressed.as_ref()).map_or(0.0, |it| it.value());
                let limit = custom.map_or(0.0, |data| density.to_px(data.compression_limit));
                (weight, pressed, limit)
            })
            .collect();
        let (main_min, main_max) = (constraints.min_width, constraints.max_width);
        let mut total_weight = 0.0f32;
        let mut fixed_space = 0i64;
        let mut weighted_count = 0i64;
        let mut child_constraints: Vec<Option<compose_ui::Constraints>> = vec![None; size];
        for (index, &item) in items.iter().enumerate() {
            let weight = data[index].0;
            if weight > 0.0 {
                total_weight += weight;
                weighted_count += 1;
            } else {
                let remaining = main_max as i64 - fixed_space;
                let desired = scope.max_intrinsic_width(item, constraints.max_height);
                child_constraints[index] = Some(compose_ui::Constraints {
                    min_width: 0,
                    max_width: desired.max(0),
                    ..constraints
                });
                let space_after = (spacing as i64).min((remaining - desired as i64).max(0));
                fixed_space += desired as i64 + space_after;
            }
        }
        // Without weights, the space after the last fixed item is never read: the widths are.
        if weighted_count > 0 {
            let target =
                if main_max != compose_ui::Constraints::INFINITY { main_max } else { main_min };
            let spacing_total = spacing as i64 * (weighted_count - 1);
            let remaining = (target as i64 - fixed_space - spacing_total).max(0);
            let unit = remaining as f32 / total_weight;
            let mut remainder = remaining;
            for entry in &data {
                remainder -= (unit * entry.0).round() as i64;
            }
            for index in 0..size {
                if child_constraints[index].is_none() {
                    let unit_remainder = remainder.signum();
                    remainder -= unit_remainder;
                    let width = ((unit * data[index].0).round() as i64 + unit_remainder).max(0);
                    let width = width.min(i32::MAX as i64) as i32;
                    child_constraints[index] = Some(compose_ui::Constraints {
                        min_width: width,
                        max_width: width,
                        ..constraints
                    });
                }
            }
        }
        let mut widths: Vec<i32> =
            child_constraints.iter().map(|it| it.unwrap_or(constraints).max_width).collect();
        let desired = widths.iter().map(|&it| it as i64).sum::<i64>()
            + spacing as i64 * (size as i64 - 1).max(0);
        let mut remaining = main_max as i64;
        let mut main_space = 0i64;
        let mut shown_space = 0i64;
        let mut last = 0usize;
        let overflow_placeables = if desired <= main_max as i64 {
            last = size;
            main_space = desired;
            Vec::new()
        } else {
            let overflow_width = overflow
                .iter()
                .map(|&it| scope.max_intrinsic_width(it, constraints.max_height))
                .max()
                .unwrap_or(0) as i64;
            remaining -= overflow_width;
            main_space += overflow_width;
            while last < size && widths[last] as i64 <= remaining {
                main_space += widths[last] as i64;
                shown_space += widths[last] as i64;
                remaining -= widths[last] as i64 + spacing as i64;
                last += 1;
            }
            main_space += spacing as i64 * last as i64;
            shown_space += spacing as i64 * last as i64;
            let max_width = (remaining + overflow_width).clamp(0, i32::MAX as i64) as i32;
            overflow
                .iter()
                .map(|&it| {
                    scope.measure(
                        it,
                        compose_ui::Constraints { min_width: 0, max_width, ..constraints },
                    )
                })
                .collect()
        };
        // Equal values don't notify: the menu composes again only when the count changes.
        self.overflow.visible.set(last);
        if size > 1 {
            let ratio = self.expanded_ratio;
            for index in 0..last {
                let pressed = data[index].1;
                if pressed == 0.0 {
                    continue;
                }
                let width = widths[index] as f32;
                let growth = if index >= 1 && index + 1 < last {
                    let target = (pressed
                        * (ratio * width / 2.0).min(data[index - 1].2).min(data[index + 1].2))
                    .round() as i32;
                    let (left, right) =
                        (target.min(widths[index - 1]), target.min(widths[index + 1]));
                    widths[index - 1] -= left;
                    widths[index + 1] -= right;
                    left + right
                } else if index == 0 {
                    let target = (pressed * (ratio * width).min(data[1].2)).round() as i32;
                    let right = target.min(widths[1]);
                    widths[1] -= right;
                    right
                } else {
                    let target = (pressed * (ratio * width).min(data[index - 1].2)).round() as i32;
                    let left = target.min(widths[index - 1]);
                    widths[index - 1] -= left;
                    left
                };
                widths[index] += growth;
            }
        }
        let placeables: Vec<_> = (0..last)
            .map(|index| {
                let base = child_constraints[index].unwrap_or(constraints);
                let width = widths[index];
                scope.measure(
                    items[index],
                    compose_ui::Constraints { min_width: width, max_width: width, ..base },
                )
            })
            .collect();
        let layout_width = (main_space.clamp(0, i32::MAX as i64) as i32).max(main_min);
        let mut positions = vec![0; last];
        let direction = scope.layout_direction();
        self.horizontal_arrangement.arrange(
            &density,
            layout_width,
            &widths[..last],
            direction,
            &mut positions,
        );
        let height = placeables.iter().map(|it| it.height).max().unwrap_or(constraints.min_height);
        let alignments: Vec<VerticalAlignment> = items[..last]
            .iter()
            .map(|&item| match scope.parent_data(item).alignment {
                Some(ParentAlignment::Vertical(alignment)) => alignment,
                _ => self.vertical_alignment,
            })
            .collect();
        for (index, placeable) in placeables.iter().enumerate() {
            let y = alignments[index].align(placeable.height, height);
            scope.place(placeable, positions[index], y);
        }
        for placeable in &overflow_placeables {
            let y = self.vertical_alignment.align(placeable.height, height);
            scope.place_relative(placeable, shown_space as i32, y);
        }
        scope.layout(layout_width, height)
    }

    fn same(&self, other: &dyn Any) -> bool {
        other.downcast_ref::<Self>().is_some_and(|other| {
            Rc::ptr_eq(&self.overflow, &other.overflow)
                && self.horizontal_arrangement == other.horizontal_arrangement
                && self.vertical_alignment == other.vertical_alignment
                && self.expanded_ratio.to_bits() == other.expanded_ratio.to_bits()
        })
    }
}
