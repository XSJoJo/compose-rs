//! `Grid`, as Compose's (foundation-layout's `Grid.kt`): children in rows and columns of tracks
//! sized fixed, by percentage, by flexible weight (`Fr`), by their content or between a minimum
//! and a weight, placed where they say (an index, a range, a named area) or flowing into the next
//! free cells. Not lazy: every child is composed and measured.

use std::any::{Any, TypeId};
use std::collections::HashSet;
use std::fmt::Debug;
use std::rc::Rc;

use compose_runtime::{ComposeArg, composable, remember, remember_updated_state};
use compose_ui::{
    Alignment, Constraints, Density, Dp, IntSize, Layout, LayoutDirection, Measurable,
    MeasurePolicy, MeasureResult, MeasureScope, Modifier, ModifierNode, ModifierNodeElement,
    ParentDataModifierNode, Placeable,
};

/// A flexible track's share of the space left, as Compose's `Fr`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Fr(pub f32);

/// `.fr()` on numbers, as Compose's `Int.fr`, `Float.fr` and `Double.fr`.
pub trait FrExt {
    fn fr(self) -> Fr;
}

impl FrExt for i32 {
    fn fr(self) -> Fr {
        Fr(self as f32)
    }
}

impl FrExt for f32 {
    fn fr(self) -> Fr {
        Fr(self)
    }
}

impl FrExt for f64 {
    fn fr(self) -> Fr {
        Fr(self as f32)
    }
}

/// Which way children without a place flow, as `GridFlow`: along rows (filling a row, then the
/// next) or along columns.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum GridFlow {
    #[default]
    Row,
    Column,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Track {
    Fixed(Dp),
    Percentage(f32),
    Flex(f32),
    MinContent,
    MaxContent,
    Auto,
    MinMax(Dp, f32),
}

/// How large a row or a column is, as Compose's `GridTrackSize`: built with `fixed`,
/// `percentage`, `flex` or `min_max`, which check their values, or one of the content sizes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct GridTrackSize(Track);

impl GridTrackSize {
    /// As large as its widest (or tallest) content can be, as `MaxContent`.
    pub const MAX_CONTENT: GridTrackSize = GridTrackSize(Track::MaxContent);
    /// As small as its content can be, as `MinContent`.
    pub const MIN_CONTENT: GridTrackSize = GridTrackSize(Track::MinContent);
    /// Its content's minimum, grown toward its maximum with the space left, as `Auto` (an implicit
    /// track's size).
    pub const AUTO: GridTrackSize = GridTrackSize(Track::Auto);

    /// # Panics
    ///
    /// For a size unspecified or negative, as Compose's `require`.
    pub fn fixed(size: Dp) -> GridTrackSize {
        assert!(
            size.is_specified() && size.0 >= 0.0,
            "Fixed size must be non-negative and specified (was {size:?})"
        );
        GridTrackSize(Track::Fixed(size))
    }

    /// A fraction of the grid's size (0.25 a quarter), as `Percentage`.
    ///
    /// # Panics
    ///
    /// For a negative value.
    pub fn percentage(value: f32) -> GridTrackSize {
        assert!(value >= 0.0, "Percentage cannot be negative");
        GridTrackSize(Track::Percentage(value))
    }

    /// A share of what's left once the other tracks have their sizes, no smaller than its content's
    /// minimum, as `Flex`.
    ///
    /// # Panics
    ///
    /// For a weight negative or not finite.
    pub fn flex(weight: Fr) -> GridTrackSize {
        assert!(
            weight.0 >= 0.0 && weight.0.is_finite(),
            "Flex weight must be non-negative and finite"
        );
        GridTrackSize(Track::Flex(weight.0))
    }

    /// At least `min`, then a share by `max` of what's left, as `MinMax`.
    ///
    /// # Panics
    ///
    /// For a negative minimum, or a weight negative or not finite.
    pub fn min_max(min: Dp, max: Fr) -> GridTrackSize {
        assert!(min.0 >= 0.0, "MinMax minimum size cannot be negative");
        assert!(
            max.0 >= 0.0 && max.0.is_finite(),
            "MinMax max weight must be non-negative and finite"
        );
        GridTrackSize(Track::MinMax(min, max.0))
    }

    fn weight(self) -> f32 {
        match self.0 {
            Track::Flex(weight) | Track::MinMax(_, weight) => weight,
            _ => 0.0,
        }
    }

    fn grows_with_span(self) -> bool {
        !matches!(self.0, Track::Fixed(_) | Track::Percentage(_))
    }
}

impl From<Dp> for GridTrackSize {
    fn from(size: Dp) -> GridTrackSize {
        GridTrackSize::fixed(size)
    }
}

impl From<Fr> for GridTrackSize {
    fn from(weight: Fr) -> GridTrackSize {
        GridTrackSize::flex(weight)
    }
}

/// A fraction of the grid's size, as `column(percentage: Float)`.
impl From<f32> for GridTrackSize {
    fn from(percentage: f32) -> GridTrackSize {
        GridTrackSize::percentage(percentage)
    }
}

/// An area's name, compared by value: what `area` and `grid_item_area` take.
trait AreaKey: Debug {
    fn as_any(&self) -> &dyn Any;
    fn same(&self, other: &dyn AreaKey) -> bool;
}

impl<T: PartialEq + Debug + 'static> AreaKey for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn same(&self, other: &dyn AreaKey) -> bool {
        other.as_any().downcast_ref::<T>().is_some_and(|other| other == self)
    }
}

/// Where a child goes, as the `gridItem` modifiers say it.
#[derive(Clone, Debug)]
struct Place {
    area: Option<Rc<dyn AreaKey>>,
    row: i32,
    column: i32,
    row_span: i32,
    column_span: i32,
    alignment: Alignment,
}

impl PartialEq for Place {
    fn eq(&self, other: &Self) -> bool {
        let areas = match (&self.area, &other.area) {
            (None, None) => true,
            (Some(a), Some(b)) => (**a).same(&**b),
            _ => false,
        };
        areas
            && (self.row, self.column, self.row_span, self.column_span)
                == (other.row, other.column, other.row_span, other.column_span)
            && self.alignment == other.alignment
    }
}

/// What a `Grid`'s config sets, as Compose's `GridConfigurationScope`: its tracks, gaps, named
/// areas and flow, given the density and the grid's constraints.
pub struct GridConfigurationScope {
    density: Density,
    constraints: Constraints,
    flow: GridFlow,
    columns: Vec<GridTrackSize>,
    rows: Vec<GridTrackSize>,
    areas: Vec<(Rc<dyn AreaKey>, [i32; 4])>,
    column_gap: Dp,
    row_gap: Dp,
}

impl GridConfigurationScope {
    pub fn density(&self) -> Density {
        self.density
    }

    /// The grid's constraints, as `constraints`.
    pub fn constraints(&self) -> Constraints {
        self.constraints
    }

    /// As setting `flow`.
    pub fn flow(&mut self, flow: GridFlow) {
        self.flow = flow;
    }

    /// Adds a column, as the `column` overloads: a `Dp` fixed, an `Fr` flexible, an `f32` a
    /// percentage, or any `GridTrackSize`.
    pub fn column(&mut self, size: impl Into<GridTrackSize>) {
        self.columns.push(size.into());
    }

    /// Adds a row, as the `row` overloads.
    pub fn row(&mut self, size: impl Into<GridTrackSize>) {
        self.rows.push(size.into());
    }

    /// Adds columns, as `columns(vararg specs)`.
    pub fn columns(&mut self, sizes: &[GridTrackSize]) {
        self.columns.extend_from_slice(sizes);
    }

    /// Adds rows, as `rows(vararg specs)`.
    pub fn rows(&mut self, sizes: &[GridTrackSize]) {
        self.rows.extend_from_slice(sizes);
    }

    /// As `minmax(min, max)`.
    pub fn minmax(&self, min: Dp, max: Fr) -> GridTrackSize {
        GridTrackSize::min_max(min, max)
    }

    /// Names the cells from `row` and `column` (1-based, negative from the end, unspecified to
    /// flow) over the spans, as `area(areaId, row, column, rowSpan, columnSpan)`.
    ///
    /// # Panics
    ///
    /// Without a row or a column, or for indices or spans out of range, as Compose's `require`.
    pub fn area(
        &mut self,
        area_id: impl PartialEq + Debug + 'static,
        row: i32,
        column: i32,
        row_span: i32,
        column_span: i32,
    ) {
        let unspecified = GridScope::GRID_INDEX_UNSPECIFIED;
        let max = GridScope::MAX_GRID_INDEX;
        assert!(
            row != unspecified || column != unspecified,
            "An area must specify at least a row or a column."
        );
        assert!((-max..=max).contains(&row), "row must be between -{max} and {max}");
        assert!((-max..=max).contains(&column), "column must be between -{max} and {max}");
        assert!((1..=max).contains(&row_span), "rowSpan must be between 1 and {max}");
        assert!((1..=max).contains(&column_span), "columnSpan must be between 1 and {max}");
        let key: Rc<dyn AreaKey> = Rc::new(area_id);
        self.areas.retain(|(existing, _)| !(**existing).same(&*key));
        self.areas.push((key, [row, column, row_span, column_span]));
    }

    /// Names the cells of `rows` and `columns` (1-based, inclusive), as `area(areaId, rows,
    /// columns)`.
    pub fn area_in(
        &mut self,
        area_id: impl PartialEq + Debug + 'static,
        rows: std::ops::RangeInclusive<i32>,
        columns: std::ops::RangeInclusive<i32>,
    ) {
        assert!(!rows.is_empty(), "Row range ({rows:?}) cannot be empty");
        assert!(!columns.is_empty(), "Column range ({columns:?}) cannot be empty");
        let (row_span, column_span) =
            (rows.end() - rows.start() + 1, columns.end() - columns.start() + 1);
        self.area(area_id, *rows.start(), *columns.start(), row_span, column_span);
    }

    /// Between rows and columns, as `gap(all)`.
    ///
    /// # Panics
    ///
    /// For a negative gap, as all the gaps.
    pub fn gap(&mut self, all: Dp) {
        assert!(all.0 >= 0.0, "Gap must be non-negative");
        (self.row_gap, self.column_gap) = (all, all);
    }

    /// As `gap(row, column)`.
    pub fn gap_with(&mut self, row: Dp, column: Dp) {
        assert!(row.0 >= 0.0, "Row gap must be non-negative");
        assert!(column.0 >= 0.0, "Column gap must be non-negative");
        (self.row_gap, self.column_gap) = (row, column);
    }

    pub fn column_gap(&mut self, gap: Dp) {
        assert!(gap.0 >= 0.0, "Column gap must be non-negative");
        self.column_gap = gap;
    }

    pub fn row_gap(&mut self, gap: Dp) {
        assert!(gap.0 >= 0.0, "Row gap must be non-negative");
        self.row_gap = gap;
    }
}

/// A `Grid`'s configuration: Compose's `config` lambda as a value, compared as Kotlin's lambdas
/// are, by identity, a block capturing nothing by its closure's type.
#[derive(Clone)]
pub struct GridConfig(Rc<dyn Fn(&mut GridConfigurationScope)>, Option<TypeId>);

impl GridConfig {
    pub fn new<F: Fn(&mut GridConfigurationScope) + 'static>(block: F) -> GridConfig {
        let kind = (std::mem::size_of::<F>() == 0).then(TypeId::of::<F>);
        GridConfig(Rc::new(block), kind)
    }
}

impl PartialEq for GridConfig {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || (self.1.is_some() && self.1 == other.1)
    }
}

impl ComposeArg for GridConfig {
    fn same(&self, other: &Self) -> bool {
        self == other
    }
}

/// The content scope of a `Grid`, as Compose's `GridScope`: the `grid_item` modifiers take it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct GridScope(());

impl GridScope {
    /// The largest index, as `MaxGridIndex`.
    pub const MAX_GRID_INDEX: i32 = 1000;
    /// No index: the child flows, as `GridIndexUnspecified`.
    pub const GRID_INDEX_UNSPECIFIED: i32 = 0;
}

impl ComposeArg for GridScope {
    fn same(&self, _: &Self) -> bool {
        true
    }
}

/// `GridScope`'s modifiers, on `Modifier`.
pub trait GridModifiers {
    /// In the cell at `row` and `column` (1-based, negative from the explicit end, 0 to flow), as
    /// `gridItem(row, column)`.
    fn grid_item(self, scope: GridScope, row: i32, column: i32) -> Modifier;
    /// As `gridItem(row, column, rowSpan, columnSpan, alignment)`.
    ///
    /// # Panics
    ///
    /// For an index out of range or a span below 1, as Compose's `require`.
    fn grid_item_with(
        self,
        scope: GridScope,
        row: i32,
        column: i32,
        row_span: i32,
        column_span: i32,
        alignment: Alignment,
    ) -> Modifier;
    /// Over `rows` and `columns` (1-based, inclusive), as `gridItem(rows, columns, alignment)`.
    fn grid_item_in(
        self,
        scope: GridScope,
        rows: std::ops::RangeInclusive<i32>,
        columns: std::ops::RangeInclusive<i32>,
        alignment: Alignment,
    ) -> Modifier;
    /// In the area the config names `area_id` (flowing if it names none), as `gridItem(areaId,
    /// alignment)`.
    fn grid_item_area(
        self,
        scope: GridScope,
        area_id: impl PartialEq + Debug + 'static,
        alignment: Alignment,
    ) -> Modifier;
}

impl GridModifiers for Modifier {
    fn grid_item(self, scope: GridScope, row: i32, column: i32) -> Modifier {
        self.grid_item_with(scope, row, column, 1, 1, Alignment::TOP_START)
    }

    fn grid_item_with(
        self,
        _: GridScope,
        row: i32,
        column: i32,
        row_span: i32,
        column_span: i32,
        alignment: Alignment,
    ) -> Modifier {
        let max = GridScope::MAX_GRID_INDEX;
        assert!((-max..=max).contains(&row), "row must be between -{max} and {max}");
        assert!((-max..=max).contains(&column), "column must be between -{max} and {max}");
        assert!(row_span > 0, "rowSpan must be > 0");
        assert!(column_span > 0, "columnSpan must be > 0");
        self.then_custom(GridItemElement(Place {
            area: None,
            row,
            column,
            row_span,
            column_span,
            alignment,
        }))
    }

    fn grid_item_in(
        self,
        scope: GridScope,
        rows: std::ops::RangeInclusive<i32>,
        columns: std::ops::RangeInclusive<i32>,
        alignment: Alignment,
    ) -> Modifier {
        assert!(!rows.is_empty(), "Row range ({rows:?}) cannot be empty");
        assert!(!columns.is_empty(), "Column range ({columns:?}) cannot be empty");
        let (row_span, column_span) =
            (rows.end() - rows.start() + 1, columns.end() - columns.start() + 1);
        self.grid_item_with(
            scope,
            *rows.start(),
            *columns.start(),
            row_span,
            column_span,
            alignment,
        )
    }

    fn grid_item_area(
        self,
        _: GridScope,
        area_id: impl PartialEq + Debug + 'static,
        alignment: Alignment,
    ) -> Modifier {
        let unspecified = GridScope::GRID_INDEX_UNSPECIFIED;
        let area: Rc<dyn AreaKey> = Rc::new(area_id);
        let place = Place {
            area: Some(area),
            row: unspecified,
            column: unspecified,
            row_span: 1,
            column_span: 1,
            alignment,
        };
        self.then_custom(GridItemElement(place))
    }
}

/// The `grid_item` modifiers' element, as `GridItemElement`: the place is the parent data.
struct GridItemElement(Place);

struct GridItemNode(Rc<Place>);

impl ModifierNodeElement for GridItemElement {
    type Node = GridItemNode;
    fn create(&self) -> GridItemNode {
        GridItemNode(Rc::new(self.0.clone()))
    }
    fn update(&self, node: &mut GridItemNode) {
        node.0 = Rc::new(self.0.clone());
    }
    fn same(&self, other: &Self) -> bool {
        self.0 == other.0
    }
    fn name(&self) -> &'static str {
        "gridItem"
    }
}

impl ModifierNode for GridItemNode {
    fn as_parent_data(&self) -> Option<&dyn ParentDataModifierNode> {
        Some(self)
    }
}

impl ParentDataModifierNode for GridItemNode {
    fn modify_parent_data(&self, _: Density, _: Option<Rc<dyn Any>>) -> Option<Rc<dyn Any>> {
        Some(self.0.clone())
    }
}

/// Children in rows and columns, as Compose's `Grid`: `config` sets the tracks, gaps, areas and
/// flow (run at each measure, so it may read the constraints); each child's `grid_item` its place
/// and alignment in its cell, else it flows into the next free one. Tracks past the config's are
/// implicit, `Auto`.
#[composable]
pub fn Grid(
    config: GridConfig,
    #[default(crate::Modifier)] modifier: Modifier,
    content: impl Fn(GridScope) + 'static,
) {
    let config = remember_updated_state(config);
    let policy = remember(|| Rc::new(GridMeasurePolicy { config }) as Rc<dyn MeasurePolicy>);
    Layout(modifier, policy, compose_runtime::callback(move || content(GridScope(()))));
}

struct GridMeasurePolicy {
    config: compose_runtime::State<GridConfig>,
}

/// A child with its cell, as `GridItem`.
struct Item {
    measurable: Measurable,
    row: usize,
    column: usize,
    row_span: usize,
    column_span: usize,
    alignment: Alignment,
    placeable: Option<Placeable>,
    offset: (i32, i32),
    spanned_width: i32,
}

/// `index` (1-based, negative from the end of `count` explicit tracks) as 0-based, `None` if
/// unspecified or before the start, as `resolveToZeroBasedIndex`.
fn zero_based(index: i32, count: usize) -> Option<usize> {
    match index {
        0 => None,
        index if index > 0 => Some(index as usize - 1),
        index => usize::try_from(count as i32 + index).ok(),
    }
}

/// Each child's cell, by its place or the flow, and the grid's size in tracks (columns, rows), as
/// `resolveGridItemIndices`: the children in row then column order.
fn resolve_cells(
    scope: &MeasureScope<'_>,
    measurables: &[Measurable],
    config: &GridConfigurationScope,
) -> (Vec<Item>, usize, usize) {
    let max = GridScope::MAX_GRID_INDEX as usize;
    let (explicit_columns, explicit_rows) = (config.columns.len(), config.rows.len());
    let (mut max_row, mut max_column) = (explicit_rows, explicit_columns);
    let mut occupied: HashSet<(usize, usize)> = HashSet::new();
    let free = |occupied: &HashSet<(usize, usize)>,
                row: usize,
                column: usize,
                rows: usize,
                columns: usize| {
        row + rows <= max
            && column + columns <= max
            && (row..row + rows)
                .all(|r| (column..column + columns).all(|c| !occupied.contains(&(r, c))))
    };
    let (mut cursor_row, mut cursor_column) = (0usize, 0usize);
    let row_flow = config.flow == GridFlow::Row;
    let mut items = Vec::with_capacity(measurables.len());
    for &measurable in measurables {
        let place =
            scope.parent_data_any(measurable).and_then(|data| data.downcast::<Place>().ok());
        let (mut row_span, mut column_span) = (1usize, 1usize);
        let (mut requested_row, mut requested_column) = (None, None);
        let mut alignment = Alignment::TOP_START;
        if let Some(place) = place {
            alignment = place.alignment;
            let [row, column, rows, columns] = match &place.area {
                Some(area) => config
                    .areas
                    .iter()
                    .find(|(key, _)| (**key).same(&**area))
                    .map_or([0, 0, 1, 1], |(_, bounds)| *bounds),
                None => [place.row, place.column, place.row_span, place.column_span],
            };
            (row_span, column_span) = (rows as usize, columns as usize);
            if row == 0 && explicit_rows > 0 && !row_flow {
                row_span = row_span.min(explicit_rows);
            }
            if column == 0 && explicit_columns > 0 && row_flow {
                column_span = column_span.min(explicit_columns);
            }
            requested_row = zero_based(row, explicit_rows);
            requested_column = zero_based(column, explicit_columns);
        }
        let (mut final_row, mut final_column) = (None, None);
        match (requested_row, requested_column) {
            (Some(row), Some(column)) => (final_row, final_column) = (Some(row), Some(column)),
            (Some(row), None) => {
                final_row = Some(row);
                let start = if row_flow && row == cursor_row { cursor_column } else { 0 };
                final_column =
                    (start..max).find(|&c| free(&occupied, row, c, row_span, column_span));
            }
            (None, Some(column)) => {
                final_column = Some(column);
                let start = if !row_flow && column == cursor_column { cursor_row } else { 0 };
                final_row =
                    (start..max).find(|&r| free(&occupied, r, column, row_span, column_span));
            }
            (None, None) => {
                let (mut row, mut column) = (cursor_row, cursor_column);
                while row < max && column < max {
                    if row_flow {
                        if explicit_columns > 0
                            && column + column_span > explicit_columns
                            && column > 0
                        {
                            (row, column) = (row + 1, 0);
                            continue;
                        }
                    } else if explicit_rows > 0 && row + row_span > explicit_rows && row > 0 {
                        (row, column) = (0, column + 1);
                        continue;
                    }
                    if free(&occupied, row, column, row_span, column_span) {
                        (final_row, final_column) = (Some(row), Some(column));
                        break;
                    }
                    if row_flow {
                        column += 1;
                        if column > max {
                            (row, column) = (row + 1, 0);
                        }
                    } else {
                        row += 1;
                        if row > max {
                            (row, column) = (0, column + 1);
                        }
                    }
                }
            }
        }
        let (row, column) = (final_row.unwrap_or(0), final_column.unwrap_or(0));
        for r in row..row + row_span {
            for c in column..column + column_span {
                occupied.insert((r, c));
            }
        }
        items.push(Item {
            measurable,
            row,
            column,
            row_span,
            column_span,
            alignment,
            placeable: None,
            offset: (0, 0),
            spanned_width: 0,
        });
        max_row = max_row.max(row + row_span);
        max_column = max_column.max(column + column_span);
        if requested_row.is_none() && requested_column.is_none() {
            (cursor_row, cursor_column) =
                if row_flow { (row, column + column_span) } else { (row + row_span, column) };
        }
    }
    // Stable, as Compose's insertion sort.
    items.sort_by_key(|item| (item.row, item.column));
    (items, max_column, max_row)
}

/// One axis's track sizes: the columns' (`rows` false) or the rows' at `column_widths`.
#[allow(clippy::too_many_arguments)]
fn track_sizes(
    scope: &mut MeasureScope<'_>,
    specs: &[GridTrackSize],
    count: usize,
    available: i32,
    items: &[Item],
    rows: bool,
    gap: i32,
    column_widths: &[i32],
    column_gap: i32,
    fallback_width: i32,
) -> (Vec<i32>, i32) {
    let mut sizes = vec![0; count];
    if count == 0 {
        return (sizes, 0);
    }
    let spec = |index: usize| specs.get(index).copied().unwrap_or(GridTrackSize::AUTO);
    let gaps = (gap * (count as i32 - 1)).max(0);
    let space = if available == Constraints::INFINITY {
        Constraints::INFINITY
    } else {
        (available - gaps).max(0)
    };
    let density = scope.density();
    let mut total_flex = 0.0f32;
    let mut autos = Vec::new();
    let mut auto_max = vec![0; count];
    // The largest min and max intrinsic sizes of a track's single-span children.
    let content = |scope: &mut MeasureScope<'_>, index: usize, min: bool, max: bool| {
        let (mut largest_min, mut largest_max) = (0, 0);
        for item in items {
            let (at, span) =
                if rows { (item.row, item.row_span) } else { (item.column, item.column_span) };
            if at != index || span != 1 {
                continue;
            }
            if rows {
                let width = spanned(item, column_widths, fallback_width, column_gap);
                if min {
                    largest_min =
                        largest_min.max(scope.min_intrinsic_height(item.measurable, width));
                }
                if max {
                    largest_max =
                        largest_max.max(scope.max_intrinsic_height(item.measurable, width));
                }
            } else {
                if min {
                    largest_min = largest_min
                        .max(scope.min_intrinsic_width(item.measurable, Constraints::INFINITY));
                }
                if max {
                    largest_max = largest_max
                        .max(scope.max_intrinsic_width(item.measurable, Constraints::INFINITY));
                }
            }
        }
        (largest_min, largest_max)
    };
    for index in 0..count {
        sizes[index] = match spec(index).0 {
            Track::Fixed(size) => density.round_to_px(size),
            Track::Percentage(value) if space != Constraints::INFINITY => {
                (value * space as f32).round() as i32
            }
            Track::Flex(weight) => {
                total_flex += weight;
                content(scope, index, true, false).0
            }
            Track::MinContent => content(scope, index, true, false).0,
            Track::Auto if space != Constraints::INFINITY => {
                let (min, max) = content(scope, index, true, true);
                autos.push(index);
                auto_max[index] = max;
                min
            }
            Track::MinMax(min, weight) => {
                total_flex += weight;
                density.round_to_px(min)
            }
            Track::Percentage(_) | Track::MaxContent | Track::Auto => {
                content(scope, index, false, true).1
            }
        };
    }
    distribute_spans(scope, specs, &mut sizes, items, rows, column_widths, gap, column_gap);
    if space != Constraints::INFINITY && !autos.is_empty() {
        expand_autos(&autos, &mut sizes, &auto_max, space);
    }
    // The flexible tracks share what's left, by their accumulated weights.
    let used: i32 = sizes.iter().sum();
    let remaining = if space == Constraints::INFINITY { 0 } else { (space - used).max(0) };
    let mut added = 0;
    if total_flex > 0.0 && remaining > 0 {
        let (mut distributed, mut accumulated) = (0, 0.0f32);
        for (index, size) in sizes.iter_mut().enumerate() {
            let weight = spec(index).weight();
            if weight > 0.0 {
                accumulated += weight;
                let target = (accumulated / total_flex * remaining as f32).round() as i32;
                let share = (target - distributed).max(0);
                *size += share;
                distributed += share;
                added = distributed;
            }
        }
    }
    (sizes, used + added)
}

/// The width a child spans, its columns and the gaps between, or `fallback` past them, as
/// `getSpannedWidth`.
fn spanned(item: &Item, widths: &[i32], fallback: i32, gap: i32) -> i32 {
    if item.column >= widths.len() {
        return fallback;
    }
    let end = (item.column + item.column_span).min(widths.len());
    widths[item.column..end].iter().sum::<i32>()
        + (end - item.column).saturating_sub(1) as i32 * gap
}

/// Grows the tracks a spanning child crosses (those not fixed nor a percentage) to fit it, as
/// `distributeSpanningSpace`.
#[allow(clippy::too_many_arguments)]
fn distribute_spans(
    scope: &mut MeasureScope<'_>,
    specs: &[GridTrackSize],
    sizes: &mut [i32],
    items: &[Item],
    rows: bool,
    column_widths: &[i32],
    gap: i32,
    column_gap: i32,
) {
    let grows =
        |index: usize| specs.get(index).copied().unwrap_or(GridTrackSize::AUTO).grows_with_span();
    for item in items {
        let (start, span) =
            if rows { (item.row, item.row_span) } else { (item.column, item.column_span) };
        if span <= 1 {
            continue;
        }
        let end = (start + span).min(sizes.len());
        let growing = (start..end).filter(|&index| grows(index)).count() as i32;
        let current = sizes[start..end].iter().sum::<i32>() + (span as i32 - 1).max(0) * gap;
        let required = if rows {
            let end = (item.column + item.column_span).min(column_widths.len());
            let columns: i32 =
                column_widths.get(item.column..end).map_or(0, |widths| widths.iter().sum());
            let width = columns + (item.column_span as i32 - 1).max(0) * column_gap;
            scope.max_intrinsic_height(item.measurable, width)
        } else {
            scope.max_intrinsic_width(item.measurable, Constraints::INFINITY)
        };
        let deficit = required - current;
        if deficit > 0 && growing > 0 {
            let (share, mut remainder) = (deficit / growing, deficit % growing);
            for index in (start..end).filter(|&index| grows(index)) {
                sizes[index] += share + i32::from(remainder > 0);
                remainder = (remainder - 1).max(0);
            }
        }
    }
}

/// Grows the `Auto` tracks toward their content's maximum with the space left, in proportion
/// when there isn't enough, as `expandAutoTracks`.
fn expand_autos(autos: &[usize], sizes: &mut [i32], maxima: &[i32], space: i32) {
    let remaining = space - sizes.iter().sum::<i32>();
    if remaining <= 0 {
        return;
    }
    let potentials: Vec<i32> =
        autos.iter().map(|&index| (maxima[index] - sizes[index]).max(0)).collect();
    let total: i32 = potentials.iter().sum();
    if total == 0 {
        return;
    }
    for (&index, &potential) in autos.iter().zip(&potentials) {
        sizes[index] += if remaining >= total {
            potential
        } else {
            (potential as f32 / total as f32 * remaining as f32).round() as i32
        };
    }
}

impl MeasurePolicy for GridMeasurePolicy {
    fn measure(
        &self,
        scope: &mut MeasureScope<'_>,
        measurables: &[Measurable],
        constraints: Constraints,
    ) -> MeasureResult {
        let mut config = GridConfigurationScope {
            density: scope.density(),
            constraints,
            flow: GridFlow::Row,
            columns: Vec::new(),
            rows: Vec::new(),
            areas: Vec::new(),
            column_gap: Dp(0.0),
            row_gap: Dp(0.0),
        };
        (self.config.get().0)(&mut config);
        let (mut items, column_count, row_count) = resolve_cells(scope, measurables, &config);
        let density = scope.density();
        let (column_gap, row_gap) =
            (density.round_to_px(config.column_gap), density.round_to_px(config.row_gap));
        let (widths, width) = track_sizes(
            scope,
            &config.columns,
            column_count,
            constraints.max_width,
            &items,
            false,
            column_gap,
            &[],
            0,
            0,
        );
        let (heights, height) = track_sizes(
            scope,
            &config.rows,
            row_count,
            constraints.max_height,
            &items,
            true,
            row_gap,
            &widths,
            column_gap,
            constraints.max_width,
        );
        let total_width = width + (column_count as i32 - 1).max(0) * column_gap;
        let total_height = height + (row_count as i32 - 1).max(0) * row_gap;
        let direction = scope.layout_direction();
        for item in &mut items {
            if item.row >= row_count || item.column >= column_count {
                continue;
            }
            let span = |sizes: &[i32], start: usize, span: usize, gap: i32| {
                let end = (start + span).min(sizes.len());
                sizes[start..end].iter().sum::<i32>() + (end - start).saturating_sub(1) as i32 * gap
            };
            let cell_width = span(&widths, item.column, item.column_span, column_gap);
            let cell_height = span(&heights, item.row, item.row_span, row_gap);
            let placeable =
                scope.measure(item.measurable, Constraints::new(0, cell_width, 0, cell_height));
            let at = item.alignment.align(
                IntSize::new(placeable.width, placeable.height),
                IntSize::new(cell_width, cell_height),
                direction,
            );
            item.offset = (at.x, at.y);
            item.spanned_width = cell_width;
            item.placeable = Some(placeable);
        }
        let layout_width = constraints.constrain_width(total_width);
        let layout_height = constraints.constrain_height(total_height);
        let offsets = |sizes: &[i32], gap: i32| {
            let mut at = 0;
            sizes
                .iter()
                .map(|size| {
                    let start = at;
                    at += size + gap;
                    start
                })
                .collect::<Vec<i32>>()
        };
        let (column_offsets, row_offsets) =
            (offsets(&widths, column_gap), offsets(&heights, row_gap));
        for item in &items {
            if let Some(placeable) = &item.placeable {
                let column = if direction == LayoutDirection::Rtl {
                    layout_width - column_offsets[item.column] - item.spanned_width
                } else {
                    column_offsets[item.column]
                };
                scope.place(
                    placeable,
                    column + item.offset.0,
                    row_offsets[item.row] + item.offset.1,
                );
            }
        }
        scope.layout(layout_width, layout_height)
    }
}
