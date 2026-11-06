//! `FlexBox`, as Compose's (foundation-layout's `FlexBox.kt`): CSS flexbox's layout of children
//! along a main axis (a row or a column, either way), wrapping onto lines if asked, each child
//! growing, shrinking and aligning by its own `flex` config.

use std::any::{Any, TypeId};
use std::rc::Rc;

use compose_runtime::{ComposeArg, composable, remember, remember_updated_state};
use compose_ui::{
    AlignmentLine, Constraints, Density, Dp, IntrinsicMeasurable, IntrinsicMeasureScope, Layout,
    Measurable, MeasurePolicy, MeasureResult, MeasureScope, Measured, Modifier, ModifierNode,
    ModifierNodeElement, ParentDataModifierNode, Placeable,
};

/// Which way a `FlexBox` lays its children out, as Compose's `FlexDirection`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FlexDirection {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// Whether its children wrap onto new lines, and which way the lines go, as `FlexWrap`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// How a line's children share its free space along the main axis, as `FlexJustifyContent`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FlexJustifyContent {
    #[default]
    Start,
    End,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// How children sit across their line, as `FlexAlignItems`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FlexAlignItems {
    #[default]
    Start,
    End,
    Center,
    Stretch,
    Baseline,
}

/// How a child sits across its line, `Auto` taking its box's `FlexAlignItems`, as `FlexAlignSelf`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FlexAlignSelf {
    #[default]
    Auto,
    Start,
    End,
    Center,
    Stretch,
    Baseline,
}

/// How the lines share the cross axis's free space, as `FlexAlignContent`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum FlexAlignContent {
    #[default]
    Start,
    End,
    Center,
    Stretch,
    SpaceBetween,
    SpaceAround,
}

/// A child's size along the main axis before it grows or shrinks, as `FlexBasis`: its content's
/// (`Auto`), a size, or a fraction of the box's maximum.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum FlexBasis {
    #[default]
    Auto,
    Dp(Dp),
    Percent(f32),
}

impl From<Dp> for FlexBasis {
    fn from(value: Dp) -> FlexBasis {
        FlexBasis::Dp(value)
    }
}

/// A fraction (0 to 1) of the box's main axis, as `basis(Float)`.
impl From<f32> for FlexBasis {
    fn from(value: f32) -> FlexBasis {
        FlexBasis::Percent(value)
    }
}

/// Where a child's line (or the box's) for baseline alignment comes from.
#[derive(Clone, Default)]
enum Baseline {
    /// The first baseline, as Compose's default.
    #[default]
    Default,
    Line(AlignmentLine),
    Block(Rc<dyn Fn(&mut Measured<'_, '_>) -> i32>),
}

impl Baseline {
    /// Its position in `placeable`, the placeable's size along the line's axis where it has none.
    fn position(&self, scope: &mut MeasureScope<'_>, placeable: &Placeable) -> i32 {
        match self {
            Baseline::Block(block) => scope.with_measured(placeable, |measured| block(measured)),
            Baseline::Line(line) => match scope.alignment_line(placeable, *line) {
                AlignmentLine::UNSPECIFIED if line.is_horizontal() => placeable.height,
                AlignmentLine::UNSPECIFIED => placeable.width,
                value => value,
            },
            Baseline::Default => {
                match scope.alignment_line(placeable, compose_ui::FIRST_BASELINE) {
                    AlignmentLine::UNSPECIFIED => placeable.height,
                    value => value,
                }
            }
        }
    }
}

/// A list of configuring blocks, compared as Kotlin compares its lambdas: by identity, a block
/// capturing nothing by its type.
/// A block, and its closure's type if it captures nothing.
type Block<S> = (Rc<dyn Fn(&mut S)>, Option<TypeId>);

struct Blocks<S: 'static>(Rc<[Block<S>]>);

impl<S: 'static> Clone for Blocks<S> {
    fn clone(&self) -> Self {
        Blocks(self.0.clone())
    }
}

impl<S: 'static> Blocks<S> {
    fn new<F: Fn(&mut S) + 'static>(block: F) -> Self {
        let kind = (std::mem::size_of::<F>() == 0).then(TypeId::of::<F>);
        Blocks(Rc::new([(Rc::new(block) as Rc<dyn Fn(&mut S)>, kind)]))
    }

    fn empty() -> Self {
        Blocks(Rc::new([]))
    }

    fn then(&self, other: &Self) -> Self {
        Blocks(self.0.iter().chain(other.0.iter()).cloned().collect())
    }

    fn configure(&self, scope: &mut S) {
        for (block, _) in self.0.iter() {
            block(scope);
        }
    }

    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
            || (self.0.len() == other.0.len()
                && self
                    .0
                    .iter()
                    .zip(other.0.iter())
                    .all(|((a, ka), (b, kb))| Rc::ptr_eq(a, b) || (ka.is_some() && ka == kb)))
    }
}

/// A `FlexBox`'s configuration, as Compose's `FlexBoxConfig`: blocks setting its direction,
/// wrapping, alignments and gaps on a `FlexBoxConfigScope`, run at each measure (so they may read
/// the constraints). The default is empty (a row from the start, no wrap); `then` runs another's
/// after.
#[derive(Clone)]
pub struct FlexBoxConfig(Blocks<FlexBoxConfigScope>);

impl FlexBoxConfig {
    /// The configuration `block` sets, as `FlexBoxConfig { … }`.
    pub fn new(block: impl Fn(&mut FlexBoxConfigScope) + 'static) -> FlexBoxConfig {
        FlexBoxConfig(Blocks::new(block))
    }

    /// This one then `other`, as `then`.
    pub fn then(&self, other: &FlexBoxConfig) -> FlexBoxConfig {
        FlexBoxConfig(self.0.then(&other.0))
    }

    /// These in order, as `FlexBoxConfig(vararg configs)`.
    pub fn of(configs: &[FlexBoxConfig]) -> FlexBoxConfig {
        configs.iter().fold(FlexBoxConfig::default(), |all, config| all.then(config))
    }
}

impl Default for FlexBoxConfig {
    fn default() -> FlexBoxConfig {
        FlexBoxConfig(Blocks::empty())
    }
}

impl PartialEq for FlexBoxConfig {
    fn eq(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
}

impl ComposeArg for FlexBoxConfig {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

impl<F: Fn(&mut FlexBoxConfigScope) + 'static> From<F> for FlexBoxConfig {
    fn from(block: F) -> FlexBoxConfig {
        FlexBoxConfig::new(block)
    }
}

/// A child's configuration in a `FlexBox`, as Compose's `FlexConfig`: blocks on a
/// `FlexConfigScope` setting its grow, shrink, basis, alignment and order.
#[derive(Clone)]
pub struct FlexConfig(Blocks<FlexConfigScope>);

impl FlexConfig {
    /// The configuration `block` sets, as `FlexConfig { … }`.
    pub fn new(block: impl Fn(&mut FlexConfigScope) + 'static) -> FlexConfig {
        FlexConfig(Blocks::new(block))
    }

    /// This one then `other`, as `then`.
    pub fn then(&self, other: &FlexConfig) -> FlexConfig {
        FlexConfig(self.0.then(&other.0))
    }

    /// These in order, as `FlexConfig(vararg configs)`.
    pub fn of(configs: &[FlexConfig]) -> FlexConfig {
        configs.iter().fold(FlexConfig::default(), |all, config| all.then(config))
    }
}

impl Default for FlexConfig {
    fn default() -> FlexConfig {
        FlexConfig(Blocks::empty())
    }
}

impl PartialEq for FlexConfig {
    fn eq(&self, other: &Self) -> bool {
        self.0.same(&other.0)
    }
}

impl<F: Fn(&mut FlexConfigScope) + 'static> From<F> for FlexConfig {
    fn from(block: F) -> FlexConfig {
        FlexConfig::new(block)
    }
}

/// What a `FlexBoxConfig` sets, as Compose's `FlexBoxConfigScope`, with the density and the
/// constraints the box is measured with.
pub struct FlexBoxConfigScope {
    density: Density,
    constraints: Constraints,
    direction: FlexDirection,
    wrap: FlexWrap,
    justify_content: FlexJustifyContent,
    align_items: FlexAlignItems,
    align_content: FlexAlignContent,
    max_items_in_each_line: usize,
    row_gap: Dp,
    column_gap: Dp,
    baseline: Baseline,
}

impl FlexBoxConfigScope {
    fn resolve(config: &FlexBoxConfig, density: Density, constraints: Constraints) -> Self {
        let mut scope = FlexBoxConfigScope {
            density,
            constraints,
            direction: FlexDirection::Row,
            wrap: FlexWrap::NoWrap,
            justify_content: FlexJustifyContent::Start,
            align_items: FlexAlignItems::Start,
            align_content: FlexAlignContent::Start,
            max_items_in_each_line: usize::MAX,
            row_gap: Dp(0.0),
            column_gap: Dp(0.0),
            baseline: Baseline::Default,
        };
        config.0.configure(&mut scope);
        scope
    }

    /// The density, as the scope's `Density`.
    pub fn density(&self) -> Density {
        self.density
    }

    /// The box's constraints, as `constraints`.
    pub fn constraints(&self) -> Constraints {
        self.constraints
    }

    pub fn direction(&mut self, value: FlexDirection) {
        self.direction = value;
    }

    pub fn wrap(&mut self, value: FlexWrap) {
        self.wrap = value;
    }

    pub fn justify_content(&mut self, value: FlexJustifyContent) {
        self.justify_content = value;
    }

    pub fn align_items(&mut self, value: FlexAlignItems) {
        self.align_items = value;
    }

    /// Aligns the children by `line`, as `alignItems(alignmentLine)`.
    pub fn align_items_by(&mut self, line: impl Into<AlignmentLine>) {
        self.align_items = FlexAlignItems::Baseline;
        self.baseline = Baseline::Line(line.into());
    }

    /// Aligns the children where `block` says their line is, as `alignItems(alignmentLineBlock)`.
    pub fn align_items_by_block(&mut self, block: impl Fn(&mut Measured<'_, '_>) -> i32 + 'static) {
        self.align_items = FlexAlignItems::Baseline;
        self.baseline = Baseline::Block(Rc::new(block));
    }

    pub fn align_content(&mut self, value: FlexAlignContent) {
        self.align_content = value;
    }

    /// # Panics
    ///
    /// For 0, as Compose's `requirePrecondition`.
    pub fn max_items_in_each_line(&mut self, value: usize) {
        assert!(value > 0, "maxItemsInEachLine must be positive: {value}");
        self.max_items_in_each_line = value;
    }

    /// Between rows, as `rowGap`.
    pub fn row_gap(&mut self, value: Dp) {
        self.row_gap = value;
    }

    /// Between columns, as `columnGap`.
    pub fn column_gap(&mut self, value: Dp) {
        self.column_gap = value;
    }

    /// Between rows and columns, as `gap(all)`.
    pub fn gap(&mut self, all: Dp) {
        (self.row_gap, self.column_gap) = (all, all);
    }

    /// As `gap(row, column)`.
    pub fn gap_with(&mut self, row: Dp, column: Dp) {
        (self.row_gap, self.column_gap) = (row, column);
    }

    fn is_horizontal(&self) -> bool {
        matches!(self.direction, FlexDirection::Row | FlexDirection::RowReverse)
    }

    fn is_wrap_enabled(&self) -> bool {
        self.wrap != FlexWrap::NoWrap
    }

    fn is_cross_axis_reverse(&self) -> bool {
        self.wrap == FlexWrap::WrapReverse
    }

    fn is_main_axis_reverse(&self) -> bool {
        matches!(self.direction, FlexDirection::RowReverse | FlexDirection::ColumnReverse)
    }

    fn main_axis_gap(&self) -> i32 {
        let gap = if self.is_horizontal() { self.column_gap } else { self.row_gap };
        self.density.round_to_px(gap)
    }

    fn cross_axis_gap(&self) -> i32 {
        let gap = if self.is_horizontal() { self.row_gap } else { self.column_gap };
        self.density.round_to_px(gap)
    }
}

/// What a child's `FlexConfig` sets, as Compose's `FlexConfigScope`, with the box's constraints
/// along each axis.
pub struct FlexConfigScope {
    density: Density,
    main_axis_max: i32,
    main_axis_min: i32,
    cross_axis_max: i32,
    cross_axis_min: i32,
    align_self: FlexAlignSelf,
    baseline: Option<Baseline>,
    order: i32,
    /// `NaN` until set: a fill fraction stands in, as Compose's.
    grow: f32,
    shrink: f32,
    basis: FlexBasis,
}

impl FlexConfigScope {
    pub fn density(&self) -> Density {
        self.density
    }

    /// As `flexBoxMainAxisMaxPx`.
    pub fn flex_box_main_axis_max_px(&self) -> i32 {
        self.main_axis_max
    }

    /// As `flexBoxMainAxisMinPx`.
    pub fn flex_box_main_axis_min_px(&self) -> i32 {
        self.main_axis_min
    }

    /// As `flexBoxCrossAxisMaxPx`.
    pub fn flex_box_cross_axis_max_px(&self) -> i32 {
        self.cross_axis_max
    }

    /// As `flexBoxCrossAxisMinPx`.
    pub fn flex_box_cross_axis_min_px(&self) -> i32 {
        self.cross_axis_min
    }

    pub fn align_self(&mut self, value: FlexAlignSelf) {
        self.align_self = value;
    }

    /// Aligns it by `line`, as `alignSelf(alignmentLine)`.
    pub fn align_self_by(&mut self, line: impl Into<AlignmentLine>) {
        self.align_self = FlexAlignSelf::Baseline;
        self.baseline = Some(Baseline::Line(line.into()));
    }

    /// Aligns it where `block` says its line is, as `alignSelf(alignmentLineBlock)`.
    pub fn align_self_by_block(&mut self, block: impl Fn(&mut Measured<'_, '_>) -> i32 + 'static) {
        self.align_self = FlexAlignSelf::Baseline;
        self.baseline = Some(Baseline::Block(Rc::new(block)));
    }

    /// Where it comes among its siblings, lowest first, as `order`.
    pub fn order(&mut self, value: i32) {
        self.order = value;
    }

    /// # Panics
    ///
    /// If `value` is negative or NaN, as Compose's `requirePrecondition`.
    pub fn grow(&mut self, value: f32) {
        assert!(value >= 0.0, "Flex grow cannot be negative: {value}");
        self.grow = value;
    }

    /// # Panics
    ///
    /// If `value` is negative or NaN.
    pub fn shrink(&mut self, value: f32) {
        assert!(value >= 0.0, "Flex shrink cannot be negative: {value}");
        self.shrink = value;
    }

    /// As `basis(FlexBasis)`, `basis(Dp)` and `basis(Float)`.
    pub fn basis(&mut self, value: impl Into<FlexBasis>) {
        self.basis = value.into();
    }
}

/// The content scope of a `FlexBox`, as Compose's `FlexBoxScope`: `Modifier::flex` takes it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FlexBoxScope(());

impl ComposeArg for FlexBoxScope {
    fn same(&self, _: &Self) -> bool {
        true
    }
}

/// `FlexBoxScope`'s modifier, on `Modifier`.
pub trait FlexBoxModifiers {
    /// How it grows, shrinks, aligns and orders in its `FlexBox`, as `FlexBoxScope.flex(config)`
    /// and `flex { … }`.
    fn flex(self, scope: FlexBoxScope, config: impl Into<FlexConfig>) -> Modifier;
}

impl FlexBoxModifiers for Modifier {
    fn flex(self, _: FlexBoxScope, config: impl Into<FlexConfig>) -> Modifier {
        self.then_custom(FlexElement(config.into()))
    }
}

/// `Modifier::flex`'s element, as `FlexBoxChildElement`: its config the parent data.
struct FlexElement(FlexConfig);

struct FlexNode(Rc<FlexConfig>);

impl ModifierNodeElement for FlexElement {
    type Node = FlexNode;
    fn create(&self) -> FlexNode {
        FlexNode(Rc::new(self.0.clone()))
    }
    fn update(&self, node: &mut FlexNode) {
        node.0 = Rc::new(self.0.clone());
    }
    fn same(&self, other: &Self) -> bool {
        self.0 == other.0
    }
    fn name(&self) -> &'static str {
        "flex"
    }
}

impl ModifierNode for FlexNode {
    fn as_parent_data(&self) -> Option<&dyn ParentDataModifierNode> {
        Some(self)
    }
}

impl ParentDataModifierNode for FlexNode {
    fn modify_parent_data(&self, _: Density, _: Option<Rc<dyn Any>>) -> Option<Rc<dyn Any>> {
        Some(self.0.clone())
    }

    // As `FlexBoxChildDataNode`, a `FillModifierParentData`.
    fn holds_fill_fractions(&self) -> bool {
        true
    }
}

/// Children along a main axis, as Compose's `FlexBox`: `config` (a `FlexBoxConfig`, or a block
/// on its scope) sets the direction, wrapping, alignments and gaps; each child's `Modifier::flex`
/// its grow, shrink, basis, alignment and order (a `fill_max_width` in a row growing it by its
/// fraction where it sets no grow).
#[composable]
pub fn FlexBox(
    #[default(crate::Modifier)] modifier: Modifier,
    #[default(crate::layout::FlexBoxConfig::default())] config: impl Into<FlexBoxConfig>,
    content: impl Fn(FlexBoxScope) + 'static,
) {
    let config = remember_updated_state(config);
    let policy = remember(|| Rc::new(FlexBoxMeasurePolicy { config }) as Rc<dyn MeasurePolicy>);
    Layout(modifier, policy, compose_runtime::callback(move || content(FlexBoxScope(()))));
}

struct FlexBoxMeasurePolicy {
    config: compose_runtime::State<FlexBoxConfig>,
}

/// One child being laid out, as `ResolvedFlexItemInfo`.
struct Item {
    measurable: Measurable,
    order: i32,
    align_self: FlexAlignSelf,
    baseline_source: Option<Baseline>,
    grow: f32,
    shrink: f32,
    min_main: i32,
    flex_base_size: i32,
    hypothetical: i32,
    target: i32,
    frozen: bool,
    placeable: Option<Placeable>,
    baseline: i32,
    cross_size: i32,
    main_position: i32,
    cross_position: i32,
}

impl Item {
    fn has_baseline(&self) -> bool {
        self.align_self == FlexAlignSelf::Baseline
    }

    /// Its baseline in its placeable, its own line's or the box's, as `getBaseline`.
    fn baseline_in(&self, scope: &mut MeasureScope<'_>, config: &FlexBoxConfigScope) -> i32 {
        let placeable = self.placeable.as_ref().expect("measured");
        match &self.baseline_source {
            Some(source) => source.position(scope, placeable),
            None if self.has_baseline()
                || (config.align_items == FlexAlignItems::Baseline
                    && self.align_self == FlexAlignSelf::Auto) =>
            {
                config.baseline.position(scope, placeable)
            }
            None => AlignmentLine::UNSPECIFIED,
        }
    }
}

/// A line of children, `start..end` of the items, as `FlexLine`.
#[derive(Default)]
struct Line {
    start: usize,
    end: usize,
    main_size: i32,
    cross_size: i32,
    cross_start: i32,
    max_above_baseline: i32,
}

/// Constraints along the main and cross axes.
#[derive(Clone, Copy)]
struct Axes {
    main_min: i32,
    main_max: i32,
    cross_min: i32,
    cross_max: i32,
}

impl Axes {
    fn of(constraints: Constraints, horizontal: bool) -> Axes {
        let Constraints { min_width, max_width, min_height, max_height } = constraints;
        if horizontal {
            Axes {
                main_min: min_width,
                main_max: max_width,
                cross_min: min_height,
                cross_max: max_height,
            }
        } else {
            Axes {
                main_min: min_height,
                main_max: max_height,
                cross_min: min_width,
                cross_max: max_width,
            }
        }
    }
}

fn main_intrinsic(
    scope: &mut IntrinsicMeasureScope<'_>,
    m: Measurable,
    horizontal: bool,
    max: bool,
    other: i32,
) -> i32 {
    match (horizontal, max) {
        (true, false) => scope.min_intrinsic_width(m, other),
        (true, true) => scope.max_intrinsic_width(m, other),
        (false, false) => scope.min_intrinsic_height(m, other),
        (false, true) => scope.max_intrinsic_height(m, other),
    }
}

impl MeasurePolicy for FlexBoxMeasurePolicy {
    fn measure(
        &self,
        scope: &mut MeasureScope<'_>,
        measurables: &[Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        if measurables.is_empty() {
            return scope.layout(constraints.min_width, constraints.min_height);
        }
        let config = FlexBoxConfigScope::resolve(&self.config.get(), scope.density(), constraints);
        let horizontal = config.is_horizontal();
        let axes = Axes::of(constraints, horizontal);
        let mut upfront = config.align_items == FlexAlignItems::Stretch
            || config.align_items == FlexAlignItems::Baseline
            || (config.is_wrap_enabled()
                && config.align_content == FlexAlignContent::Stretch
                && axes.cross_max != Constraints::INFINITY);
        let mut items: Vec<Item> =
            measurables.iter().map(|&m| create_item(scope, m, horizontal, axes)).collect();
        if items
            .iter()
            .any(|item| matches!(item.align_self, FlexAlignSelf::Baseline | FlexAlignSelf::Stretch))
        {
            upfront = true;
        }
        if items.iter().any(|item| item.order != 0) {
            items.sort_by_key(|item| item.order);
        }
        let (main_gap, cross_gap) = (config.main_axis_gap(), config.cross_axis_gap());
        let (mut lines, mut total_cross) =
            build_lines(scope, &config, &mut items, axes, main_gap, cross_gap, upfront);
        if !config.is_wrap_enabled()
            && let Some(line) = lines.first_mut()
        {
            line.cross_size = line.cross_size.max(axes.cross_min);
        }
        total_cross = stretch_lines(&config, &mut lines, axes, total_cross, cross_gap);
        total_cross =
            measure_items(scope, &config, &mut lines, &mut items, axes, total_cross, upfront);
        line_cross_positions(
            &config,
            total_cross.max(axes.cross_min),
            &mut lines,
            total_cross,
            cross_gap,
        );
        let last = if config.is_cross_axis_reverse() { lines.first() } else { lines.last() };
        let total_cross =
            last.map_or(0, |line| line.cross_start + line.cross_size).max(axes.cross_min);
        let max_line = lines.iter().map(|line| line.main_size).max().unwrap_or(axes.main_min);
        let main_size = max_line.max(axes.main_min).clamp(axes.main_min, axes.main_max);
        let cross_size = total_cross.clamp(axes.cross_min, axes.cross_max);
        let (width, height) =
            if horizontal { (main_size, cross_size) } else { (cross_size, main_size) };
        let main_reverse = config.is_main_axis_reverse();
        let line_order: Vec<usize> = if config.is_cross_axis_reverse() {
            (0..lines.len()).rev().collect()
        } else {
            (0..lines.len()).collect()
        };
        for index in line_order {
            let line = &lines[index];
            position_on_main_axis(&mut items, &config, main_size, line, main_gap, main_reverse);
            let indices: Vec<usize> = if main_reverse {
                (line.start..line.end).rev().collect()
            } else {
                (line.start..line.end).collect()
            };
            for item in indices {
                let item = &items[item];
                if let Some(placeable) = &item.placeable {
                    let (x, y) = if horizontal {
                        (item.main_position, item.cross_position)
                    } else {
                        (item.cross_position, item.main_position)
                    };
                    scope.place_relative(placeable, x, y);
                }
            }
        }
        scope.layout(width, height)
    }

    fn min_intrinsic_width(
        &self,
        scope: &mut IntrinsicMeasureScope<'_>,
        measurables: &[IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        self.intrinsic(scope, measurables, true, false, height)
    }

    fn min_intrinsic_height(
        &self,
        scope: &mut IntrinsicMeasureScope<'_>,
        measurables: &[IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        self.intrinsic(scope, measurables, false, false, width)
    }

    fn max_intrinsic_width(
        &self,
        scope: &mut IntrinsicMeasureScope<'_>,
        measurables: &[IntrinsicMeasurable],
        height: i32,
    ) -> i32 {
        self.intrinsic(scope, measurables, true, true, height)
    }

    fn max_intrinsic_height(
        &self,
        scope: &mut IntrinsicMeasureScope<'_>,
        measurables: &[IntrinsicMeasurable],
        width: i32,
    ) -> i32 {
        self.intrinsic(scope, measurables, false, true, width)
    }
}

impl FlexBoxMeasurePolicy {
    /// The `width` (else height) intrinsic, `max` or min, at `other` on the other axis, as the four
    /// overrides: along the main axis the line's children and gaps (the widest child if a min
    /// that wraps), across it the lines the children would make.
    fn intrinsic(
        &self,
        scope: &mut IntrinsicMeasureScope<'_>,
        measurables: &[IntrinsicMeasurable],
        width: bool,
        max: bool,
        other: i32,
    ) -> i32 {
        if measurables.is_empty() {
            return 0;
        }
        let constraints = if width {
            Constraints::new(0, Constraints::INFINITY, 0, other)
        } else {
            Constraints::new(0, other, 0, Constraints::INFINITY)
        };
        let config = FlexBoxConfigScope::resolve(&self.config.get(), scope.density(), constraints);
        let horizontal = config.is_horizontal();
        let gap = config.main_axis_gap();
        if width == horizontal {
            // Along the main axis.
            let size = |scope: &mut IntrinsicMeasureScope<'_>, m| {
                main_intrinsic(scope, m, horizontal, max, other)
            };
            if !max && config.is_wrap_enabled() {
                return measurables.iter().map(|&m| size(scope, m)).max().unwrap_or(0);
            }
            let per_line = if config.is_wrap_enabled() && max {
                config.max_items_in_each_line
            } else {
                usize::MAX
            };
            let (mut widest, mut line, mut count) = (0, 0, 0);
            for &m in measurables {
                line += if count == 0 { size(scope, m) } else { gap + size(scope, m) };
                count += 1;
                if count == per_line {
                    widest = widest.max(line);
                    (line, count) = (0, 0);
                }
            }
            return widest.max(line);
        }
        // Across: lines as the measure would break them at `other`.
        let cross_gap = config.cross_axis_gap();
        let (mut line_main, mut line_cross, mut total, mut count) = (0, 0, 0, 0usize);
        for &m in measurables {
            let item_main = main_intrinsic(scope, m, horizontal, max, Constraints::INFINITY);
            let item_cross = main_intrinsic(scope, m, !horizontal, max, item_main);
            let projected = if count == 0 { item_main } else { line_main + gap + item_main };
            if config.is_wrap_enabled()
                && count > 0
                && (count >= config.max_items_in_each_line || projected > other)
            {
                total += line_cross + cross_gap;
                (line_main, line_cross, count) = (item_main, item_cross, 1);
            } else {
                line_main = projected;
                line_cross = line_cross.max(item_cross);
                count += 1;
            }
        }
        total + line_cross
    }
}

/// A child with its config resolved and its sizes before flexing, as `createFlexItem`.
fn create_item(
    scope: &mut MeasureScope<'_>,
    measurable: Measurable,
    horizontal: bool,
    axes: Axes,
) -> Item {
    let mut config = FlexConfigScope {
        density: scope.density(),
        main_axis_max: axes.main_max,
        main_axis_min: axes.main_min,
        cross_axis_max: axes.cross_max,
        cross_axis_min: axes.cross_min,
        align_self: FlexAlignSelf::Auto,
        baseline: None,
        order: 0,
        grow: f32::NAN,
        shrink: 1.0,
        basis: FlexBasis::Auto,
    };
    if let Some(flex) =
        scope.parent_data_any(measurable).and_then(|data| data.downcast::<FlexConfig>().ok())
    {
        flex.0.configure(&mut config);
    }
    if config.grow.is_nan() {
        let (fill_horizontal, fill_vertical) = scope.fill_fractions(measurable);
        let fill = if horizontal { fill_horizontal } else { fill_vertical };
        config.grow = fill.filter(|fill| *fill > 0.0).unwrap_or(0.0);
    }
    let min_main = main_intrinsic(scope, measurable, horizontal, false, axes.cross_max);
    let mut max_content = || main_intrinsic(scope, measurable, horizontal, true, axes.cross_max);
    let flex_base_size = match config.basis {
        FlexBasis::Dp(value) => scope.density().round_to_px(value),
        FlexBasis::Percent(value) if axes.main_max != Constraints::INFINITY && !value.is_nan() => {
            (axes.main_max as f32 * value) as i32
        }
        FlexBasis::Percent(_) | FlexBasis::Auto => max_content(),
    };
    let hypothetical = flex_base_size.max(min_main);
    Item {
        measurable,
        order: config.order,
        align_self: config.align_self,
        baseline_source: config.baseline,
        grow: config.grow,
        shrink: config.shrink,
        min_main,
        flex_base_size,
        hypothetical,
        target: hypothetical,
        frozen: false,
        placeable: None,
        baseline: AlignmentLine::UNSPECIFIED,
        cross_size: 0,
        main_position: 0,
        cross_position: 0,
    }
}

/// The children broken into lines, each flexed (and measured across if `upfront`), and the lines'
/// total cross size, as `buildFlexLines`.
fn build_lines(
    scope: &mut MeasureScope<'_>,
    config: &FlexBoxConfigScope,
    items: &mut [Item],
    axes: Axes,
    main_gap: i32,
    cross_gap: i32,
    upfront: bool,
) -> (Vec<Line>, i32) {
    let mut lines = Vec::new();
    let (mut line_start, mut line_size, mut cross_position, mut total) = (0, 0i64, 0, 0);
    let mut remaining_cross = axes.cross_max;
    let finish =
        |scope: &mut MeasureScope<'_>, items: &mut [Item], start, end, size: i64, remaining| {
            let mut line = Line { start, end, ..Line::default() };
            line.main_size = resolve_flexible_lengths(
                config,
                items,
                start,
                end,
                (size - main_gap as i64) as i32,
                axes.main_max,
            );
            if upfront {
                line_cross_size(scope, config, &mut line, items, remaining);
            }
            line
        };
    for index in 0..items.len() {
        let hypothetical = items[index].hypothetical as i64;
        let breaks = config.is_wrap_enabled()
            && index > line_start
            && (index - line_start >= config.max_items_in_each_line
                || line_size + hypothetical > axes.main_max as i64);
        if breaks {
            let mut line = finish(scope, items, line_start, index, line_size, remaining_cross);
            total += line.cross_size;
            line.cross_start = cross_position;
            cross_position += line.cross_size + cross_gap;
            remaining_cross = (remaining_cross - (line.cross_size + cross_gap)).max(0);
            lines.push(line);
            line_start = index;
            line_size = hypothetical + main_gap as i64;
        } else {
            line_size += hypothetical + main_gap as i64;
        }
    }
    if line_start < items.len() {
        let mut line = finish(scope, items, line_start, items.len(), line_size, remaining_cross);
        total += line.cross_size;
        line.cross_start = cross_position;
        lines.push(line);
    }
    (lines, total)
}

/// Grows or shrinks a line's children to its main size, as `resolveFlexibleLengths`: the line's
/// size.
fn resolve_flexible_lengths(
    config: &FlexBoxConfigScope,
    items: &mut [Item],
    start: usize,
    end: usize,
    hypothetical_line: i32,
    container: i32,
) -> i32 {
    let count = end - start;
    let total_gap = if count > 0 { (count as i32 - 1) * config.main_axis_gap() } else { 0 };
    let line = &mut items[start..end];
    if container == Constraints::INFINITY {
        let mut size = total_gap;
        for item in line.iter_mut() {
            item.target = item.hypothetical;
            size += item.target;
        }
        return size;
    }
    let growing = hypothetical_line < container;
    let (mut unfrozen, mut sum_grow, mut sum_scaled_shrink) = (0, 0.0f32, 0.0f32);
    let (mut sum_base, mut sum_frozen, mut sum_factors) = (0, 0, 0.0f32);
    for item in line.iter_mut() {
        let factor = if growing { item.grow } else { item.shrink };
        if factor == 0.0 || (!growing && item.flex_base_size < item.hypothetical) {
            item.target = item.hypothetical;
            item.frozen = true;
            sum_frozen += item.target;
        } else {
            item.frozen = false;
            unfrozen += 1;
            sum_base += item.flex_base_size;
            sum_factors += factor;
            if growing {
                sum_grow += item.grow;
            } else {
                sum_scaled_shrink += item.shrink * item.flex_base_size as f32;
            }
        }
    }
    let initial_free = (container - total_gap - sum_frozen - sum_base) as f32;
    // A sum of factors below 1 takes that share of the free space, as CSS.
    let free_space = |sum_frozen: i32, sum_base: i32, sum_factors: f32| {
        let free = (container - (total_gap + sum_frozen + sum_base)) as f32;
        let fractional = initial_free * sum_factors;
        if sum_factors < 1.0 && fractional.abs() < free.abs() { fractional } else { free }
    };
    let mut allocated = 0;
    if growing {
        let mut free = free_space(sum_frozen, sum_base, sum_factors);
        let mut grow_left = sum_grow;
        for item in line.iter_mut().filter(|item| !item.frozen) {
            let share = if grow_left > 0.0 { item.grow / grow_left } else { 0.0 };
            let space = (free * share).round() as i32;
            free -= space as f32;
            grow_left -= item.grow;
            item.target = item.flex_base_size + space;
            allocated += item.target;
        }
    } else {
        let mut redistribute = true;
        let mut loops = 0;
        while redistribute && loops < count {
            redistribute = false;
            loops += 1;
            allocated = 0;
            if unfrozen == 0 {
                break;
            }
            let mut free = free_space(sum_frozen, sum_base, sum_factors).abs();
            let mut shrink_left = sum_scaled_shrink;
            for item in line.iter_mut().filter(|item| !item.frozen) {
                let scaled = item.shrink * item.flex_base_size as f32;
                let share = if shrink_left > 0.0 { scaled / shrink_left } else { 0.0 };
                let space = (free * share).round() as i32;
                let target = item.flex_base_size - space;
                if target < item.min_main {
                    // Frozen at its minimum, the rest shared again.
                    item.frozen = true;
                    redistribute = true;
                    free -= (item.flex_base_size - item.min_main) as f32;
                    shrink_left -= scaled;
                    item.target = item.min_main;
                    unfrozen -= 1;
                    sum_frozen += item.min_main;
                    sum_base -= item.flex_base_size;
                    sum_scaled_shrink -= scaled;
                    sum_factors -= item.shrink;
                } else {
                    free -= space as f32;
                    shrink_left -= scaled;
                    item.target = target;
                    allocated += item.target;
                }
            }
        }
    }
    total_gap + sum_frozen + allocated
}

/// The lines grown to fill the box across when they stretch, as `applyAlignContentStretch`.
fn stretch_lines(
    config: &FlexBoxConfigScope,
    lines: &mut [Line],
    axes: Axes,
    total: i32,
    cross_gap: i32,
) -> i32 {
    if !config.is_wrap_enabled()
        || config.align_content != FlexAlignContent::Stretch
        || axes.cross_min == Constraints::INFINITY
        || lines.is_empty()
    {
        return total;
    }
    let spacing = (lines.len() as i32 - 1) * cross_gap;
    if total + spacing >= axes.cross_min {
        return total;
    }
    let per_line = (axes.cross_min - total - spacing).max(0) / lines.len() as i32;
    let mut position = 0;
    for line in lines.iter_mut() {
        line.cross_start = position;
        line.cross_size += per_line;
        position += line.cross_size + cross_gap;
    }
    total + per_line * lines.len() as i32
}

/// Measures the children not measured yet, each line as high as its tallest if not known up
/// front, as `measureFlexItems`: the lines' total cross size.
fn measure_items(
    scope: &mut MeasureScope<'_>,
    config: &FlexBoxConfigScope,
    lines: &mut [Line],
    items: &mut [Item],
    axes: Axes,
    total: i32,
    upfront: bool,
) -> i32 {
    let mut updated = total;
    let mut remaining = axes.cross_max;
    for line in lines.iter_mut() {
        let mut line_cross = if upfront { line.cross_size } else { 0 };
        for item in &mut items[line.start..line.end] {
            if item.placeable.is_some() {
                if !upfront {
                    line_cross = line_cross.max(item.cross_size);
                }
                continue;
            }
            let stretch = item.align_self == FlexAlignSelf::Stretch
                || (item.align_self == FlexAlignSelf::Auto
                    && config.align_items == FlexAlignItems::Stretch);
            let cross = measure_item(
                scope,
                item,
                config,
                if stretch { line.cross_size } else { 0 },
                stretch,
                remaining,
            );
            if !upfront {
                line_cross = line_cross.max(cross);
            }
            if !config.is_wrap_enabled() {
                line_cross = line_cross.max(axes.cross_min);
            }
        }
        if !upfront {
            line.cross_size = line_cross;
            updated += line_cross;
        }
        remaining = (remaining - line.cross_size - config.cross_axis_gap()).max(0);
    }
    if upfront { total } else { updated }
}

/// Where each line starts across, as `calculateLineCrossPositions`.
fn line_cross_positions(
    config: &FlexBoxConfigScope,
    space: i32,
    lines: &mut [Line],
    total: i32,
    cross_gap: i32,
) {
    if !config.is_wrap_enabled() || lines.is_empty() {
        return;
    }
    let count = lines.len() as i32;
    let free = space - total - (count - 1) * cross_gap;
    let between = match config.align_content {
        FlexAlignContent::SpaceAround => free / count,
        FlexAlignContent::SpaceBetween if count > 1 => free / (count - 1),
        _ => 0,
    };
    let reverse = config.is_cross_axis_reverse();
    let mut position = match config.align_content {
        FlexAlignContent::End => {
            if reverse {
                0
            } else {
                free
            }
        }
        FlexAlignContent::Center => free / 2,
        FlexAlignContent::SpaceAround => between / 2,
        _ => {
            if reverse {
                free
            } else {
                0
            }
        }
    };
    let mut place = |line: &mut Line| {
        line.cross_start = position;
        position += line.cross_size + between + cross_gap;
    };
    if reverse {
        lines.iter_mut().rev().for_each(&mut place);
    } else {
        lines.iter_mut().for_each(&mut place);
    }
}

/// Where a line's children go along the main axis and across, as `positionItemsOnMainAxis`.
fn position_on_main_axis(
    items: &mut [Item],
    config: &FlexBoxConfigScope,
    container: i32,
    line: &Line,
    gap: i32,
    reverse: bool,
) {
    let count = (line.end - line.start) as i32;
    if count == 0 {
        return;
    }
    let remaining = container - line.main_size;
    let between = match config.justify_content {
        FlexJustifyContent::SpaceAround => remaining / count,
        FlexJustifyContent::SpaceBetween if count > 1 => remaining / (count - 1),
        FlexJustifyContent::SpaceEvenly => remaining / (count + 1),
        _ => 0,
    };
    let mut position = match config.justify_content {
        FlexJustifyContent::End => {
            if reverse {
                0
            } else {
                remaining
            }
        }
        FlexJustifyContent::Center => remaining / 2,
        FlexJustifyContent::SpaceAround => between / 2,
        FlexJustifyContent::SpaceEvenly => between,
        FlexJustifyContent::SpaceBetween => {
            if count == 1 && reverse {
                remaining
            } else {
                0
            }
        }
        FlexJustifyContent::Start => {
            if reverse {
                remaining
            } else {
                0
            }
        }
    };
    let mut place = |item: &mut Item| {
        item.main_position = position;
        position += item.target + between + gap;
        let alignment = match (item.align_self, config.align_items) {
            (FlexAlignSelf::Auto, FlexAlignItems::Start) => FlexAlignSelf::Start,
            (FlexAlignSelf::Auto, FlexAlignItems::End) => FlexAlignSelf::End,
            (FlexAlignSelf::Auto, FlexAlignItems::Center) => FlexAlignSelf::Center,
            (FlexAlignSelf::Auto, FlexAlignItems::Stretch) => FlexAlignSelf::Stretch,
            (FlexAlignSelf::Auto, FlexAlignItems::Baseline) => FlexAlignSelf::Baseline,
            (own, _) => own,
        };
        let offset = match alignment {
            FlexAlignSelf::End => line.cross_size - item.cross_size,
            FlexAlignSelf::Center => (line.cross_size - item.cross_size) / 2,
            FlexAlignSelf::Baseline if item.baseline != AlignmentLine::UNSPECIFIED => {
                line.max_above_baseline - item.baseline
            }
            _ => 0,
        };
        item.cross_position = line.cross_start + offset;
    };
    let line_items = &mut items[line.start..line.end];
    if reverse {
        line_items.iter_mut().rev().for_each(&mut place);
    } else {
        line_items.iter_mut().for_each(&mut place);
    }
}

/// A line's cross size before measuring its children for good: those aligned by a baseline
/// measured now, the others by their intrinsic size, as `calculateLineCrossAxisSize`.
fn line_cross_size(
    scope: &mut MeasureScope<'_>,
    config: &FlexBoxConfigScope,
    line: &mut Line,
    items: &mut [Item],
    remaining: i32,
) {
    let horizontal = config.is_horizontal();
    let (mut cross, mut above, mut below) = (0, 0, 0);
    for item in &mut items[line.start..line.end] {
        let size = if item.has_baseline()
            || (config.align_items == FlexAlignItems::Baseline
                && item.align_self == FlexAlignSelf::Auto)
        {
            measure_item(scope, item, config, 0, false, remaining);
            let baseline = item.baseline_in(scope, config);
            item.baseline = baseline;
            above = above.max(baseline);
            below = below.max(item.cross_size - baseline);
            above + below
        } else {
            item.cross_size =
                main_intrinsic(scope, item.measurable, !horizontal, true, item.target);
            item.cross_size
        };
        cross = cross.max(size);
    }
    line.max_above_baseline = above;
    line.cross_size = cross;
}

/// Measures a child at its target main size, stretched across to `line_cross` if `stretch`, as
/// `measureItem`: its cross size.
fn measure_item(
    scope: &mut MeasureScope<'_>,
    item: &mut Item,
    config: &FlexBoxConfigScope,
    line_cross: i32,
    stretch: bool,
    remaining: i32,
) -> i32 {
    let horizontal = config.is_horizontal();
    let main = item.target.max(0);
    let constraints = if stretch && line_cross > 0 {
        let cross = line_cross.min(remaining);
        if horizontal { Constraints::fixed(main, cross) } else { Constraints::fixed(cross, main) }
    } else if horizontal {
        Constraints::new(main, main, 0, remaining.max(0))
    } else {
        Constraints::new(0, remaining.max(0), main, main)
    };
    let placeable = scope.measure(item.measurable, constraints);
    item.cross_size = if horizontal { placeable.height } else { placeable.width };
    item.placeable = Some(placeable);
    item.cross_size
}
