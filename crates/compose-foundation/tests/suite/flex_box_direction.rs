//! Compose's `FlexBoxDirectionTest.kt`, each test in a row and in a column as Compose's runs
//! parameterized: justification, alignment, gaps, wrapping, grow, shrink and basis, alignment of
//! the lines, overflow, fill modifiers and the most items in a line. Density 1: a dp is a pixel.

use std::cell::RefCell;
use std::rc::Rc;

use compose_foundation::layout::*;
use compose_test::*;
use compose_ui::{Dp, IntSize, LayoutCoordinates, Modifier, Rect, UnitExt};

/// The direction a test runs in, and sizes and positions along its axes.
#[derive(Clone, Copy, Debug)]
struct Dir {
    row: bool,
}

const DIRS: [Dir; 2] = [Dir { row: true }, Dir { row: false }];

impl Dir {
    fn direction(self) -> FlexDirection {
        if self.row { FlexDirection::Row } else { FlexDirection::Column }
    }

    fn reverse(self) -> FlexDirection {
        if self.row { FlexDirection::RowReverse } else { FlexDirection::ColumnReverse }
    }

    /// A size `main` along the main axis and `cross` across.
    fn size(self, main: f32, cross: f32) -> Modifier {
        if self.row {
            Modifier.size((main.dp(), cross.dp()))
        } else {
            Modifier.size((cross.dp(), main.dp()))
        }
    }

    fn main_size(self, modifier: Modifier, main: Dp) -> Modifier {
        if self.row { modifier.width(main) } else { modifier.height(main) }
    }

    fn cross_size(self, modifier: Modifier, cross: Dp) -> Modifier {
        if self.row { modifier.height(cross) } else { modifier.width(cross) }
    }

    fn fill_main(self, modifier: Modifier, fraction: f32) -> Modifier {
        if self.row {
            modifier.fill_max_width_with(fraction)
        } else {
            modifier.fill_max_height_with(fraction)
        }
    }

    fn main(self, rect: Rect) -> f32 {
        if self.row { rect.left } else { rect.top }
    }

    fn cross(self, rect: Rect) -> f32 {
        if self.row { rect.top } else { rect.left }
    }

    fn main_len(self, rect: Rect) -> f32 {
        if self.row { rect.width() } else { rect.height() }
    }

    fn cross_len(self, rect: Rect) -> f32 {
        if self.row { rect.height() } else { rect.width() }
    }
}

/// What a laid out FlexBox gives: its bounds and its children's, relative to it.
struct Laid {
    flex: Rect,
    items: Vec<Rect>,
    dir: Dir,
}

impl Laid {
    fn mains(&self) -> Vec<f32> {
        self.items.iter().map(|item| self.dir.main(*item) - self.dir.main(self.flex)).collect()
    }

    fn crosses(&self) -> Vec<f32> {
        self.items.iter().map(|item| self.dir.cross(*item) - self.dir.cross(self.flex)).collect()
    }

    fn main_lens(&self) -> Vec<f32> {
        self.items.iter().map(|item| self.dir.main_len(*item)).collect()
    }

    fn cross_lens(&self) -> Vec<f32> {
        self.items.iter().map(|item| self.dir.cross_len(*item)).collect()
    }

    fn main_len(&self) -> f32 {
        self.dir.main_len(self.flex)
    }

    fn cross_len(&self) -> f32 {
        self.dir.cross_len(self.flex)
    }
}

type Child = Rc<dyn Fn(Dir, FlexBoxScope) -> Modifier>;

fn child(modifier: impl Fn(Dir, FlexBoxScope) -> Modifier + 'static) -> Child {
    Rc::new(modifier)
}

/// `count` children of `size` px.
fn squares(count: usize, size: f32) -> Vec<Child> {
    (0..count).map(|_| child(move |_, _| Modifier.size(size.dp()))).collect()
}

/// A FlexBox of `children` with `flex` as its modifier and `config`, in a box with `outer` as
/// its modifier, at density 1 in a root of 1200 px.
fn lay(
    dir: Dir,
    outer: Modifier,
    flex: Modifier,
    config: impl Into<FlexBoxConfig>,
    children: Vec<Child>,
) -> Laid {
    let count = children.len();
    let recorded: Rc<RefCell<Vec<Option<Rect>>>> = Rc::new(RefCell::new(vec![None; count + 1]));
    let into = |recorded: &Rc<RefCell<Vec<Option<Rect>>>>, index: usize| {
        let recorded = recorded.clone();
        Modifier.on_globally_positioned(move |coordinates: LayoutCoordinates| {
            let (at, size) = (coordinates.position_in_root(), coordinates.size());
            let bounds = Rect::new(at.x, at.y, at.x + size.width as f32, at.y + size.height as f32);
            recorded.borrow_mut()[index] = Some(bounds);
        })
    };
    let config: FlexBoxConfig = config.into();
    let _ui = ComposeTestRule::sized(IntSize::new(1200, 1200), {
        let recorded = recorded.clone();
        move || {
            let (config, children, recorded) = (config.clone(), children.clone(), recorded.clone());
            let flex = flex.clone().then(into(&recorded, count));
            Box!(modifier = outer.clone(), {
                let (config, children, recorded) =
                    (config.clone(), children.clone(), recorded.clone());
                FlexBox!(
                    modifier = flex.clone(),
                    config = config.clone(),
                    |scope: FlexBoxScope| {
                        for (index, child) in children.iter().enumerate() {
                            Box!(modifier = child(dir, scope).then(into(&recorded, index)));
                        }
                    }
                );
            });
        }
    });
    let recorded = recorded.borrow();
    let rect = |index: usize| recorded[index].unwrap_or_else(|| panic!("{index} wasn't placed"));
    Laid { flex: rect(count), items: (0..count).map(rect).collect(), dir }
}

fn box200() -> Modifier {
    Modifier.size(200.dp())
}

/// Three 20 px squares in a 200 px FlexBox filling its box, `justify` along `direction`.
fn justified(
    dir: Dir,
    direction: FlexDirection,
    justify: FlexJustifyContent,
    count: usize,
) -> Vec<f32> {
    lay(
        dir,
        box200(),
        Modifier.fill_max_size(),
        move |c: &mut FlexBoxConfigScope| {
            c.direction(direction);
            c.justify_content(justify);
        },
        squares(count, 20.0),
    )
    .mains()
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, defaults_noConfig_wrapsContent
#[test]
fn without_a_config_the_children_line_up_and_it_wraps_them() {
    for dir in DIRS {
        let laid = lay(
            dir,
            box200(),
            Modifier,
            move |c: &mut FlexBoxConfigScope| c.direction(dir.direction()),
            squares(3, 20.0),
        );
        assert_eq!(laid.mains(), [0.0, 20.0, 40.0]);
        assert_eq!((laid.main_len(), laid.cross_len()), (60.0, 20.0));
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_start
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_end
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_center
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_spaceBetween
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_spaceAround
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_spaceEvenly
#[test]
fn each_justification_shares_the_free_space() {
    use FlexJustifyContent::*;
    for dir in DIRS {
        let d = dir.direction();
        assert_eq!(justified(dir, d, Start, 3), [0.0, 20.0, 40.0]);
        assert_eq!(justified(dir, d, End, 3), [140.0, 160.0, 180.0]);
        assert_eq!(justified(dir, d, Center, 3), [70.0, 90.0, 110.0]);
        assert_eq!(justified(dir, d, SpaceBetween, 3), [0.0, 90.0, 180.0]);
        assert_eq!(justified(dir, d, SpaceAround, 3), [23.0, 89.0, 155.0]);
        assert_eq!(justified(dir, d, SpaceEvenly, 3), [35.0, 90.0, 145.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_start_reverse
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_end_reverse
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_center_reverse
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_spaceBetween_reverse
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_spaceAround_reverse
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, justifyContent_spaceEvenly_reverse
#[test]
fn each_justification_reversed_starts_at_the_end() {
    use FlexJustifyContent::*;
    for dir in DIRS {
        let r = dir.reverse();
        assert_eq!(justified(dir, r, Start, 3), [180.0, 160.0, 140.0]);
        assert_eq!(justified(dir, r, End, 3), [40.0, 20.0, 0.0]);
        assert_eq!(justified(dir, r, Center, 3), [110.0, 90.0, 70.0]);
        assert_eq!(justified(dir, r, SpaceBetween, 3), [180.0, 90.0, 0.0]);
        assert_eq!(justified(dir, r, SpaceAround, 3), [155.0, 89.0, 23.0]);
        assert_eq!(justified(dir, r, SpaceEvenly, 3), [145.0, 90.0, 35.0]);
    }
}

/// Three items 20 along and 20, 40 and 30 across, aligned by `align`, in `flex`.
fn aligned(dir: Dir, align: FlexAlignItems, outer: Modifier, flex: Modifier) -> Vec<f32> {
    let children =
        [20.0, 40.0, 30.0].map(|cross| child(move |dir, _| dir.size(20.0, cross))).to_vec();
    lay(
        dir,
        outer,
        flex,
        move |c: &mut FlexBoxConfigScope| {
            c.direction(dir.direction());
            c.align_items(align);
        },
        children,
    )
    .crosses()
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignItems_start_nonUniformSizes
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignItems_end_nonUniformSizes
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignItems_center_nonUniformSizes
#[test]
fn children_align_across_their_line() {
    for dir in DIRS {
        assert_eq!(aligned(dir, FlexAlignItems::Start, box200(), Modifier), [0.0, 0.0, 0.0]);
        assert_eq!(aligned(dir, FlexAlignItems::End, box200(), Modifier), [20.0, 0.0, 10.0]);
        assert_eq!(aligned(dir, FlexAlignItems::Center, box200(), Modifier), [10.0, 0.0, 5.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignItems_end_fixedCrossAxis
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignItems_center_fixedCrossAxis
#[test]
fn a_single_line_is_the_box_across() {
    for dir in DIRS {
        assert_eq!(aligned(dir, FlexAlignItems::End, Modifier, box200()), [180.0, 160.0, 170.0]);
        assert_eq!(aligned(dir, FlexAlignItems::Center, Modifier, box200()), [90.0, 80.0, 85.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignItems_stretch
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignItems_stretch_fixedCrossAxis
#[test]
fn stretched_children_are_their_line_across() {
    for dir in DIRS {
        let mut children = vec![child(|dir, _| dir.size(20.0, 40.0))];
        children.extend((0..2).map(|_| child(|dir, _| dir.main_size(Modifier, 20.dp()))));
        let config = move |c: &mut FlexBoxConfigScope| {
            c.direction(dir.direction());
            c.align_items(FlexAlignItems::Stretch);
        };
        let laid = lay(dir, box200(), Modifier, config, children);
        assert_eq!(laid.cross_lens()[1..], [40.0, 40.0]);
        let children = (0..2).map(|_| child(|dir, _| dir.main_size(Modifier, 20.dp()))).collect();
        let laid = lay(dir, Modifier, box200(), config, children);
        assert_eq!(laid.cross_lens(), [200.0, 200.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignItems_end_reverse
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignItems_center_reverse
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, combined_reverse_center_alignCenter
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, combined_reverse_spaceBetween_alignEnd
#[test]
fn reversed_children_align_across_as_the_others() {
    for dir in DIRS {
        let run = |justify, align| {
            lay(
                dir,
                box200(),
                Modifier.fill_max_size(),
                move |c: &mut FlexBoxConfigScope| {
                    c.direction(dir.reverse());
                    c.justify_content(justify);
                    c.align_items(align);
                },
                squares(3, 20.0),
            )
        };
        assert_eq!(run(FlexJustifyContent::Start, FlexAlignItems::End).crosses(), [180.0; 3]);
        assert_eq!(run(FlexJustifyContent::Start, FlexAlignItems::Center).crosses(), [90.0; 3]);
        let laid = run(FlexJustifyContent::Center, FlexAlignItems::Center);
        assert_eq!((laid.mains(), laid.crosses()), (vec![110.0, 90.0, 70.0], vec![90.0; 3]));
        let laid = run(FlexJustifyContent::SpaceBetween, FlexAlignItems::End);
        assert_eq!((laid.mains(), laid.crosses()), (vec![180.0, 90.0, 0.0], vec![180.0; 3]));
    }
}

/// Three 20 px squares 10 apart, `justify` along `direction`, in `flex`.
fn gapped(
    dir: Dir,
    direction: FlexDirection,
    justify: FlexJustifyContent,
    flex: Modifier,
) -> Vec<f32> {
    lay(
        dir,
        box200(),
        flex,
        move |c: &mut FlexBoxConfigScope| {
            c.direction(direction);
            c.justify_content(justify);
            c.gap(10.dp());
        },
        squares(3, 20.0),
    )
    .mains()
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, gap_addsSpacingBetweenItems
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, gap_withReverse
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, gap_withSpaceEvenly_reducesDistributedSpace
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, gap_withSpaceAround_reducesDistributedSpace
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, gap_withSpaceBetween_subsumedByLargerSpacing
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, gap_withJustifyEnd_positionsCorrectly
#[test]
fn gaps_space_the_children_and_take_from_the_free_space() {
    use FlexJustifyContent::*;
    for dir in DIRS {
        let (d, fill) = (dir.direction(), Modifier.fill_max_size());
        assert_eq!(gapped(dir, d, Start, fill.clone()), [0.0, 30.0, 60.0]);
        assert_eq!(gapped(dir, dir.reverse(), Start, fill.clone()), [180.0, 150.0, 120.0]);
        assert_eq!(gapped(dir, d, SpaceEvenly, fill.clone()), [30.0, 90.0, 150.0]);
        assert_eq!(gapped(dir, d, SpaceAround, fill.clone()), [20.0, 90.0, 160.0]);
        assert_eq!(gapped(dir, d, SpaceBetween, fill), [0.0, 90.0, 180.0]);
        assert_eq!(gapped(dir, d, End, dir.fill_main(Modifier, 1.0)), [120.0, 150.0, 180.0]);
    }
}

/// `children` in a wrapping FlexBox (gaps of `gap`) in a 100 px box: its size across.
fn wrapped_across(dir: Dir, gap: f32, children: Vec<Child>) -> f32 {
    lay(
        dir,
        Modifier.size(100.dp()),
        Modifier,
        move |c: &mut FlexBoxConfigScope| {
            c.direction(dir.direction());
            c.wrap(FlexWrap::Wrap);
            c.gap(gap.dp());
        },
        children,
    )
    .cross_len()
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, wrap_basic
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, gap_withWrap_appliesCrossAxisGap
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, wrap_excludesTrailingGap
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, wrap_gapPreservedAfterLineBreak
#[test]
fn wrapping_makes_lines_the_cross_gap_apart() {
    for dir in DIRS {
        assert_eq!(wrapped_across(dir, 0.0, squares(6, 20.0)), 40.0);
        assert_eq!(wrapped_across(dir, 10.0, squares(6, 20.0)), 50.0);
        let pair = (0..2).map(|_| child(|dir, _| dir.size(45.0, 45.0))).collect();
        assert_eq!(wrapped_across(dir, 10.0, pair), 45.0);
        let three =
            [100.0, 45.0, 45.0].map(|main| child(move |dir, _| dir.size(main, 20.0))).to_vec();
        assert_eq!(wrapped_across(dir, 15.0, three), 90.0);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, wrapReverse
#[test]
fn reversed_wrapping_puts_the_first_line_last() {
    for dir in DIRS {
        let laid = lay(
            dir,
            Modifier.size(100.dp()),
            Modifier,
            move |c: &mut FlexBoxConfigScope| {
                c.direction(dir.direction());
                c.wrap(FlexWrap::WrapReverse);
            },
            squares(6, 20.0),
        );
        assert_eq!(laid.crosses(), [20.0, 20.0, 20.0, 20.0, 20.0, 0.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, gap_zeroSizeItem_hasGap
#[test]
fn an_empty_child_still_has_its_gap() {
    for dir in DIRS {
        let children = [0.0, 20.0].map(|main| child(move |dir, _| dir.size(main, 20.0))).to_vec();
        let laid = lay(
            dir,
            Modifier.size(100.dp()),
            Modifier,
            move |c: &mut FlexBoxConfigScope| {
                c.direction(dir.direction());
                c.gap(10.dp());
            },
            children,
        );
        assert_eq!(laid.mains(), [0.0, 10.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, flexGrow
#[test]
fn children_grow_by_their_share() {
    for dir in DIRS {
        let children = vec![
            child(|_, _| Modifier.size(20.dp())),
            child(|_, scope| {
                Modifier.size(20.dp()).flex(scope, |c: &mut FlexConfigScope| c.grow(1.0))
            }),
            child(|_, scope| {
                Modifier.size(20.dp()).flex(scope, |c: &mut FlexConfigScope| c.grow(2.0))
            }),
        ];
        let laid = lay(
            dir,
            box200(),
            dir.fill_main(Modifier, 1.0),
            move |c: &mut FlexBoxConfigScope| c.direction(dir.direction()),
            children,
        );
        let sizes = laid.main_lens();
        assert_eq!(sizes[0], 20.0);
        assert!(sizes[1] > 20.0 && sizes[2] > sizes[1], "{sizes:?}");
        // 140 px free, a third and two.
        assert_eq!(sizes, [20.0, 67.0, 113.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, flexShrink
#[test]
fn children_shrink_unless_told_not_to() {
    for dir in DIRS {
        let children = vec![
            child(|dir, scope| {
                dir.size(60.0, 20.0).flex(scope, |c: &mut FlexConfigScope| c.shrink(0.0))
            }),
            child(|dir, scope| {
                let modifier = if dir.row {
                    Modifier.width_in(20.dp(), Dp::UNSPECIFIED)
                } else {
                    Modifier.height_in(20.dp(), Dp::UNSPECIFIED)
                };
                dir.cross_size(modifier, 20.dp())
                    .flex(scope, |c: &mut FlexConfigScope| c.basis(60.dp()))
            }),
        ];
        let laid = lay(
            dir,
            Modifier.size(100.dp()),
            dir.main_size(Modifier, 100.dp()),
            move |c: &mut FlexBoxConfigScope| {
                c.direction(dir.direction());
                c.wrap(FlexWrap::NoWrap);
            },
            children,
        );
        assert_eq!(laid.main_lens(), [60.0, 40.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, flexBasisDp
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, flexBasisPercent
#[test]
fn a_basis_sets_the_size_before_flexing() {
    for dir in DIRS {
        let by_dp = vec![child(|dir, scope| {
            dir.cross_size(
                Modifier.flex(scope, |c: &mut FlexConfigScope| c.basis(50.dp())),
                20.dp(),
            )
        })];
        let laid = lay(
            dir,
            box200(),
            Modifier,
            move |c: &mut FlexBoxConfigScope| c.direction(dir.direction()),
            by_dp,
        );
        assert_eq!(laid.main_lens(), [50.0]);
        let by_percent = vec![child(|dir, scope| {
            dir.cross_size(Modifier.flex(scope, |c: &mut FlexConfigScope| c.basis(0.5)), 20.dp())
        })];
        let laid = lay(
            dir,
            box200(),
            dir.fill_main(Modifier, 1.0),
            move |c: &mut FlexBoxConfigScope| c.direction(dir.direction()),
            by_percent,
        );
        assert_eq!(laid.main_lens(), [100.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignSelf
#[test]
fn a_childs_own_alignment_wins() {
    for dir in DIRS {
        let children = vec![
            child(|dir, _| dir.size(20.0, 40.0)),
            child(|_, scope| {
                Modifier
                    .size(20.dp())
                    .flex(scope, |c: &mut FlexConfigScope| c.align_self(FlexAlignSelf::End))
            }),
            child(|_, scope| {
                Modifier
                    .size(20.dp())
                    .flex(scope, |c: &mut FlexConfigScope| c.align_self(FlexAlignSelf::Center))
            }),
        ];
        let laid = lay(
            dir,
            box200(),
            Modifier,
            move |c: &mut FlexBoxConfigScope| {
                c.direction(dir.direction());
                c.align_items(FlexAlignItems::Start);
            },
            children,
        );
        assert_eq!(laid.crosses(), [0.0, 20.0, 10.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, order
#[test]
fn children_go_by_their_order() {
    for dir in DIRS {
        let children = vec![
            child(|_, scope| {
                Modifier.size(20.dp()).flex(scope, |c: &mut FlexConfigScope| c.order(2))
            }),
            child(|_, _| Modifier.size(20.dp())),
            child(|_, scope| {
                Modifier.size(20.dp()).flex(scope, |c: &mut FlexConfigScope| c.order(1))
            }),
        ];
        let laid = lay(
            dir,
            box200(),
            Modifier,
            move |c: &mut FlexBoxConfigScope| c.direction(dir.direction()),
            children,
        );
        assert_eq!(laid.mains(), [40.0, 0.0, 20.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, empty
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, singleItem
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, zeroSizeChildren
#[test]
fn an_empty_box_is_empty_and_one_child_is_its_size() {
    for dir in DIRS {
        let config = move |c: &mut FlexBoxConfigScope| c.direction(dir.direction());
        let laid = lay(dir, Modifier.size(100.dp()), Modifier, config, vec![]);
        assert_eq!((laid.main_len(), laid.cross_len()), (0.0, 0.0));
        let laid = lay(dir, Modifier.size(100.dp()), Modifier, config, squares(1, 20.0));
        assert_eq!((laid.main_len(), laid.cross_len()), (20.0, 20.0));
        assert_eq!((laid.mains(), laid.crosses()), (vec![0.0], vec![0.0]));
        let laid = lay(dir, Modifier, Modifier, config, squares(3, 0.0));
        assert_eq!((laid.main_len(), laid.cross_len()), (0.0, 0.0));
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, spaceBetween_singleItem
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, spaceBetween_singleItem_reverse
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, spaceEvenly_singleItem
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, spaceEvenly_singleItem_reverse
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, spaceAround_singleItem
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, spaceAround_singleItem_reverse
#[test]
fn a_single_child_spaced_sits_as_css_puts_it() {
    use FlexJustifyContent::*;
    for dir in DIRS {
        let (d, r) = (dir.direction(), dir.reverse());
        assert_eq!(justified(dir, d, SpaceBetween, 1), [0.0]);
        assert_eq!(justified(dir, r, SpaceBetween, 1), [180.0]);
        for direction in [d, r] {
            assert_eq!(justified(dir, direction, SpaceEvenly, 1), [90.0]);
            assert_eq!(justified(dir, direction, SpaceAround, 1), [90.0]);
        }
    }
}

/// Six 50 px squares wrapping in a 200 px FlexBox, its lines aligned by `align`: where the lines
/// are across.
fn lines_at(dir: Dir, align: FlexAlignContent) -> Vec<f32> {
    let laid = lay(
        dir,
        box200(),
        Modifier.fill_max_size(),
        move |c: &mut FlexBoxConfigScope| {
            c.direction(dir.direction());
            c.wrap(FlexWrap::Wrap);
            c.align_content(align);
        },
        squares(6, 50.0),
    );
    let mut lines = laid.crosses();
    lines.sort_by(f32::total_cmp);
    lines.dedup();
    lines
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignContent_start
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignContent_center
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignContent_spaceBetween
#[test]
fn the_lines_share_the_free_space_across() {
    for dir in DIRS {
        assert_eq!(lines_at(dir, FlexAlignContent::Start), [0.0, 50.0]);
        assert_eq!(lines_at(dir, FlexAlignContent::Center), [50.0, 100.0]);
        assert_eq!(lines_at(dir, FlexAlignContent::SpaceBetween), [0.0, 150.0]);
    }
}

/// Children `cross` px across and `main` along in a wrapping FlexBox `box_main` by `box_cross`,
/// lines aligned by `content` and children by `items`: where they are across and how large.
fn one_line(
    dir: Dir,
    (box_main, box_cross): (f32, f32),
    content: FlexAlignContent,
    items: FlexAlignItems,
    children: &[(f32, f32)],
) -> (Vec<f32>, Vec<f32>) {
    let children =
        children.iter().map(|&(main, cross)| child(move |dir, _| dir.size(main, cross))).collect();
    let laid = lay(
        dir,
        dir.size(box_main, box_cross),
        Modifier.fill_max_size(),
        move |c: &mut FlexBoxConfigScope| {
            c.direction(dir.direction());
            c.wrap(FlexWrap::Wrap);
            c.align_content(content);
            c.align_items(items);
        },
        children,
    );
    (laid.crosses(), laid.cross_lens())
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignContent_start_singleLine_withAlignItemsCenter
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignContent_center_singleLine_withAlignItemsCenter
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignContent_end_singleLine_withAlignItemsCenter
#[test]
fn a_single_wrapped_line_is_its_tallest_and_moves_as_a_whole() {
    use FlexAlignContent::*;
    let children = [(50.0, 30.0), (50.0, 10.0)];
    for dir in DIRS {
        let center = FlexAlignItems::Center;
        assert_eq!(one_line(dir, (200.0, 100.0), Start, center, &children).0, [0.0, 10.0]);
        assert_eq!(one_line(dir, (200.0, 100.0), Center, center, &children).0, [35.0, 45.0]);
        assert_eq!(one_line(dir, (200.0, 100.0), End, center, &children).0, [70.0, 80.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignContent_stretch_withAlignItemsCenter
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignContent_stretch_withAlignItemsStretch_stretchesItems
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, alignContent_stretch_singleLine_withAlignItemsCenter
#[test]
fn stretched_lines_share_the_box_across() {
    let (stretch, center) = (FlexAlignContent::Stretch, FlexAlignItems::Center);
    for dir in DIRS {
        let two = [(80.0, 30.0); 2];
        assert_eq!(
            one_line(dir, (100.0, 200.0), stretch, center, &two),
            (vec![35.0, 135.0], vec![30.0, 30.0])
        );
        let stretched = one_line(dir, (100.0, 200.0), stretch, FlexAlignItems::Stretch, &two);
        assert_eq!(stretched, (vec![0.0, 100.0], vec![100.0, 100.0]));
        assert_eq!(
            one_line(dir, (200.0, 200.0), stretch, center, &[(80.0, 30.0)]),
            (vec![85.0], vec![30.0])
        );
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, overflow_mainAxis_itemOverflows
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, overflow_crossAxis_itemsClipped
#[test]
fn unshrinking_children_overflow_and_the_last_lines_are_cut_across() {
    for dir in DIRS {
        let rigid = |count| {
            (0..count)
                .map(|_| {
                    child(|_, scope| {
                        Modifier.size(50.dp()).flex(scope, |c: &mut FlexConfigScope| c.shrink(0.0))
                    })
                })
                .collect::<Vec<_>>()
        };
        let laid = lay(
            dir,
            Modifier,
            Modifier.size(120.dp()),
            move |c: &mut FlexBoxConfigScope| {
                c.direction(dir.direction());
                c.wrap(FlexWrap::NoWrap);
            },
            rigid(3),
        );
        assert_eq!(laid.main_lens(), [50.0; 3]);
        let laid = lay(
            dir,
            Modifier,
            Modifier.size(120.dp()),
            move |c: &mut FlexBoxConfigScope| {
                c.direction(dir.direction());
                c.wrap(FlexWrap::Wrap);
            },
            rigid(6),
        );
        assert_eq!(laid.cross_lens(), [50.0, 50.0, 50.0, 50.0, 20.0, 20.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, manyChildren
#[test]
fn a_hundred_children_wrap_into_ten_lines() {
    for dir in DIRS {
        let laid = lay(
            dir,
            Modifier.size(1000.dp()),
            dir.main_size(Modifier, 100.dp()),
            move |c: &mut FlexBoxConfigScope| {
                c.direction(dir.direction());
                c.wrap(FlexWrap::Wrap);
            },
            squares(100, 10.0),
        );
        assert_eq!(laid.cross_len(), 100.0);
        assert_eq!(laid.items.len(), 100);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, reusableStyle
#[test]
fn one_config_serves_two_boxes() {
    for dir in DIRS {
        let centered = FlexBoxConfig::new(move |c: &mut FlexBoxConfigScope| {
            c.direction(dir.direction());
            c.justify_content(FlexJustifyContent::Center);
        });
        for _ in 0..2 {
            let laid = lay(
                dir,
                Modifier,
                dir.main_size(Modifier, 100.dp()),
                centered.clone(),
                squares(2, 20.0),
            );
            assert_eq!(laid.mains(), [30.0, 50.0]);
        }
        assert!(centered == centered.clone());
        assert!(
            centered
                != FlexBoxConfig::new(
                    move |c: &mut FlexBoxConfigScope| c.direction(dir.direction())
                )
        );
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, nestedFlexBox
#[test]
fn a_flexbox_nests_in_one_of_the_other_direction() {
    for dir in DIRS {
        let inner_size = Rc::new(std::cell::Cell::new(IntSize::ZERO));
        let outer_size = Rc::new(std::cell::Cell::new(IntSize::ZERO));
        let (i, o) = (inner_size.clone(), outer_size.clone());
        let _ui = ComposeTestRule::sized(IntSize::new(1200, 1200), move || {
            let (i, o) = (i.clone(), o.clone());
            let across = if dir.row { FlexDirection::Column } else { FlexDirection::Row };
            FlexBox!(
                modifier = Modifier.on_size_changed(move |size: IntSize| o.set(size)),
                config = move |c: &mut FlexBoxConfigScope| c.direction(across),
                |_: FlexBoxScope| {
                    let i = i.clone();
                    FlexBox!(
                        modifier = Modifier.on_size_changed(move |size: IntSize| i.set(size)),
                        config = move |c: &mut FlexBoxConfigScope| c.direction(dir.direction()),
                        |_: FlexBoxScope| {
                            for _ in 0..3 {
                                Box!(modifier = Modifier.size(20.dp()));
                            }
                        }
                    );
                    Box!(modifier = Modifier.size(100.dp()));
                }
            );
        });
        let main = |size: IntSize| if dir.row { size.width } else { size.height };
        assert_eq!(main(inner_size.get()), 60);
        assert_eq!(main(outer_size.get()), 100);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, complexMultiLine
#[test]
fn spaced_centered_wrapping_lines_place_every_child() {
    for dir in DIRS {
        let children = [30.0, 40.0, 20.0, 50.0, 25.0]
            .map(|main| child(move |dir, _| dir.size(main, 20.0)))
            .to_vec();
        let laid = lay(
            dir,
            Modifier,
            dir.main_size(Modifier, 100.dp()),
            move |c: &mut FlexBoxConfigScope| {
                c.direction(dir.direction());
                c.wrap(FlexWrap::Wrap);
                c.justify_content(FlexJustifyContent::SpaceBetween);
                c.align_items(FlexAlignItems::Center);
                c.gap(10.dp());
            },
            children,
        );
        assert_eq!(laid.items.len(), 5);
        // 30 and 40 make a line (a third would pass 100 with the gaps), 20 and 50 the next, 25
        // the last; each spaced to the ends, the lines 10 apart.
        assert_eq!(laid.mains(), [0.0, 60.0, 0.0, 50.0, 0.0]);
        assert_eq!(laid.crosses(), [0.0, 0.0, 30.0, 30.0, 60.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, fillMaxMainAxis_occupiesRemainingSpace
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, fillMaxMainAxisFraction_occupiesFractionOfRemainingSpace
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, fillMaxMainAxis_withExplicitGrowZero_doesNotGrow
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, fillMaxMainAxisFraction_withExplicitGrow_explicitGrowWins
#[test]
fn filling_the_main_axis_grows_by_the_fraction_unless_a_grow_is_set() {
    for dir in DIRS {
        let run = |second: Child| {
            let first = child(|dir, _| dir.size(50.0, 50.0));
            lay(
                dir,
                box200(),
                Modifier.fill_max_size(),
                move |c: &mut FlexBoxConfigScope| c.direction(dir.direction()),
                vec![first, second],
            )
        };
        let laid = run(child(|dir, _| dir.cross_size(dir.fill_main(Modifier, 1.0), 50.dp())));
        assert_eq!((laid.mains(), laid.main_lens()), (vec![0.0, 50.0], vec![50.0, 150.0]));
        let laid = run(child(|dir, _| dir.cross_size(dir.fill_main(Modifier, 0.5), 50.dp())));
        assert_eq!(laid.main_lens(), [50.0, 75.0]);
        let laid = run(child(|dir, scope| {
            dir.fill_main(Modifier.flex(scope, |c: &mut FlexConfigScope| c.grow(0.0)), 1.0)
                .then(dir.size(50.0, 50.0))
        }));
        assert_eq!(laid.main_lens(), [50.0, 50.0]);
        let laid = run(child(|dir, scope| {
            dir.cross_size(
                dir.fill_main(Modifier.flex(scope, |c: &mut FlexConfigScope| c.grow(1.0)), 0.5),
                50.dp(),
            )
        }));
        assert_eq!(laid.main_lens(), [50.0, 150.0]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, maxItemsInEachLine_wrap_startsNewLineAtCap
// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, maxItemsInEachLine_noWrap_hasNoEffect
#[test]
fn a_line_holds_no_more_than_its_maximum_when_wrapping() {
    for dir in DIRS {
        let laid = lay(
            dir,
            box200(),
            Modifier,
            move |c: &mut FlexBoxConfigScope| {
                c.direction(dir.direction());
                c.wrap(FlexWrap::Wrap);
                c.max_items_in_each_line(2);
            },
            squares(4, 20.0),
        );
        assert_eq!(
            (laid.mains(), laid.crosses()),
            (vec![0.0, 20.0, 0.0, 20.0], vec![0.0, 0.0, 20.0, 20.0])
        );
        let laid = lay(
            dir,
            box200(),
            Modifier,
            move |c: &mut FlexBoxConfigScope| {
                c.direction(dir.direction());
                c.max_items_in_each_line(2);
            },
            squares(4, 20.0),
        );
        assert_eq!(laid.crosses(), [0.0; 4]);
    }
}

// Compose: compose/foundation/foundation-layout/src/androidDeviceTest/kotlin/androidx/compose/foundation/layout/FlexBoxDirectionTest.kt, fillMaxCrossAxis_occupiesContainerCrossAxisSize
#[test]
fn filling_across_takes_the_box_across() {
    for dir in DIRS {
        let filling = child(|dir, _| {
            let modifier = dir.main_size(Modifier, 50.dp());
            if dir.row { modifier.fill_max_height() } else { modifier.fill_max_width() }
        });
        let laid = lay(
            dir,
            box200(),
            Modifier.fill_max_size(),
            move |c: &mut FlexBoxConfigScope| c.direction(dir.direction()),
            vec![filling],
        );
        assert_eq!(laid.cross_lens(), [200.0]);
    }
}
