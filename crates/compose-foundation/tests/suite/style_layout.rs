//! Compose's `StyleLayoutTest` (device): a style's paddings, sizes, offsets and border, through
//! the resolver chain (the test's deprecated `styleable`).

#![allow(non_snake_case)]

use compose_foundation::layout::*;
use compose_foundation::style::*;
use compose_foundation::*;
use compose_test::*;
use compose_ui::{Dp, IntSize, UnitExt};

use crate::style_support::{StyleSetters, styled};

const LEAF: &str = "style_layout_test_leaf";
const PARENT: &str = "style_layout_test_parent";
const CHILD: &str = "style_layout_test_child";

type Block = fn(&mut dyn CommonStyleScope);

fn on_leaf_with(style: Block) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(500, 500), move || {
        Box!(modifier = styled(Modifier.test_tag(LEAF), None, style));
    })
}

fn on_parent_with(parent: Block, child: Block) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(500, 500), move || {
        Box!(modifier = styled(Modifier.test_tag(PARENT), None, parent), {
            Box!(modifier = styled(Modifier, None, child));
        });
    })
}

fn on_child_with(parent: Block, child: Block) -> ComposeTestRule {
    ComposeTestRule::sized(IntSize::new(500, 500), move || {
        Box!(modifier = styled(Modifier, None, parent), {
            Box!(modifier = styled(Modifier.test_tag(CHILD), None, child));
        });
    })
}

fn assert_size(ui: &mut ComposeTestRule, tag: &str, width: Dp, height: Dp, tolerance: Dp) {
    ui.on_node_with_tag(tag)
        .assert_width_is_equal_to_with_tolerance(width, tolerance)
        .assert_height_is_equal_to_with_tolerance(height, tolerance);
}

#[test]
fn test_padding() {
    let mut ui = on_parent_with(|s| s.content_padding(10.dp()), |s| s.size(10.dp()));
    assert_size(&mut ui, PARENT, 30.dp(), 30.dp(), 1.dp());
}

#[test]
fn test_padding_values() {
    let mut ui = on_parent_with(
        |s| s.content_padding_each(10.dp(), 20.dp(), 30.dp(), 40.dp()),
        |s| s.size(10.dp()),
    );
    assert_size(&mut ui, PARENT, 50.dp(), 70.dp(), 1.dp());
}

#[test]
fn test_external_padding_values() {
    let mut ui = on_parent_with(
        |s| {
            s.external_padding_each(10.dp(), 20.dp(), 30.dp(), 40.dp());
            s.size(100.dp());
        },
        |s| s.fill_size(),
    );
    assert_size(&mut ui, PARENT, 140.dp(), 160.dp(), 1.dp());
}

#[test]
fn test_width() {
    let mut ui = on_leaf_with(|s| s.width(100.dp()));
    ui.on_node_with_tag(LEAF).assert_width_is_equal_to(100.dp());
}

#[test]
fn test_height() {
    let mut ui = on_leaf_with(|s| s.height(100.dp()));
    ui.on_node_with_tag(LEAF).assert_height_is_equal_to(100.dp());
}

#[test]
fn test_size() {
    let mut ui = on_leaf_with(|s| s.size(100.dp()));
    assert_size(&mut ui, LEAF, 100.dp(), 100.dp(), 0.dp());
}

#[test]
fn test_size_with_width_and_height() {
    let mut ui = on_leaf_with(|s| s.size_of(100.dp(), 50.dp()));
    assert_size(&mut ui, LEAF, 100.dp(), 50.dp(), 0.dp());
}

#[test]
fn test_min_width() {
    let mut ui = on_leaf_with(|s| s.min_width(100.dp()));
    ui.on_node_with_tag(LEAF).assert_width_is_equal_to(100.dp());
}

#[test]
fn test_max_width_limited_by_parent() {
    let mut ui = on_child_with(
        |s| s.size(50.dp()),
        |s| {
            s.width(100.dp());
            s.max_width(80.dp());
        },
    );
    ui.on_node_with_tag(CHILD).assert_width_is_equal_to(50.dp());
}

#[test]
fn test_max_width_limited_by_style() {
    let mut ui = on_leaf_with(|s| {
        s.size(200.dp());
        s.max_width(100.dp());
    });
    ui.on_node_with_tag(LEAF).assert_width_is_equal_to(100.dp());
}

#[test]
fn test_fractional_width_with_style_max_width() {
    let mut ui = on_child_with(
        |s| s.size(200.dp()),
        |s| {
            s.max_width(100.dp());
            s.width_fraction(0.5);
        },
    );
    ui.on_node_with_tag(CHILD).assert_width_is_equal_to(50.dp());
}

#[test]
fn test_min_height() {
    let mut ui = on_leaf_with(|s| s.min_height(100.dp()));
    ui.on_node_with_tag(LEAF).assert_height_is_equal_to(100.dp());
}

#[test]
fn test_max_height_limited_by_parent() {
    let mut ui = on_child_with(
        |s| s.size(50.dp()),
        |s| {
            s.height(100.dp());
            s.max_height(80.dp());
        },
    );
    ui.on_node_with_tag(CHILD).assert_height_is_equal_to(50.dp());
}

#[test]
fn test_max_height_limited_by_style() {
    let mut ui = on_leaf_with(|s| {
        s.size(200.dp());
        s.max_height(100.dp());
    });
    ui.on_node_with_tag(LEAF).assert_height_is_equal_to(100.dp());
}

#[test]
fn test_fractional_height_with_style_max_height() {
    let mut ui = on_child_with(
        |s| s.size(200.dp()),
        |s| {
            s.max_height(100.dp());
            s.height_fraction(0.5);
        },
    );
    ui.on_node_with_tag(CHILD).assert_height_is_equal_to(50.dp());
}

#[test]
fn test_fill_width() {
    let mut ui = on_child_with(|s| s.size(100.dp()), |s| s.width_fraction(1.0));
    ui.on_node_with_tag(CHILD).assert_width_is_equal_to(100.dp());
}

#[test]
fn test_fill_height() {
    let mut ui = on_child_with(|s| s.size(100.dp()), |s| s.height_fraction(1.0));
    ui.on_node_with_tag(CHILD).assert_height_is_equal_to(100.dp());
}

#[test]
fn test_fill_size() {
    let mut ui = on_child_with(|s| s.size(100.dp()), |s| s.fill_size());
    assert_size(&mut ui, CHILD, 100.dp(), 100.dp(), 0.dp());
}

#[test]
fn test_fractional_width() {
    let mut ui = on_child_with(|s| s.size(100.dp()), |s| s.width_fraction(0.5));
    ui.on_node_with_tag(CHILD).assert_width_is_equal_to(50.dp());
}

#[test]
fn test_fractional_height() {
    let mut ui = on_child_with(|s| s.size(100.dp()), |s| s.height_fraction(0.5));
    ui.on_node_with_tag(CHILD).assert_height_is_equal_to(50.dp());
}

#[test]
fn test_padding_individual_sides() {
    let mut ui = on_parent_with(
        |s| s.content_padding_each(5.dp(), 10.dp(), 15.dp(), 20.dp()),
        |s| s.size(50.dp()),
    );
    assert_size(&mut ui, PARENT, 70.dp(), 80.dp(), 1.dp());
}

#[test]
fn test_border_width_as_additional_padding() {
    let mut ui = on_parent_with(
        |s| {
            s.border(10.dp(), Color::RED);
            s.content_padding(5.dp());
        },
        |s| s.size(50.dp()),
    );
    assert_size(&mut ui, PARENT, 80.dp(), 80.dp(), Dp(1.5));
}

#[test]
fn test_padding_horizontal_vertical() {
    let mut ui = on_parent_with(
        |s| s.content_padding_each(10.dp(), 20.dp(), 10.dp(), 20.dp()),
        |s| s.size(50.dp()),
    );
    assert_size(&mut ui, PARENT, 70.dp(), 90.dp(), 1.dp());
}

#[test]
fn test_offset() {
    let mut ui = ComposeTestRule::sized(IntSize::new(500, 500), || {
        Box!(
            modifier = styled(Modifier, None, |s| {
                s.left(10.dp());
                s.top(20.dp());
            }),
            {
                Box!(modifier = styled(Modifier.test_tag("offsetTag"), None, |_| {}));
            }
        );
    });
    ui.on_node_with_tag("offsetTag")
        .assert_left_position_in_root_is_equal_to(10.dp())
        .assert_top_position_in_root_is_equal_to(20.dp());
}

#[test]
fn test_border_size() {
    let mut ui = on_leaf_with(|s| {
        s.size(100.dp());
        s.border(10.dp(), Color::RED);
    });
    assert_size(&mut ui, LEAF, 100.dp(), 100.dp(), 0.dp());
}

#[test]
fn test_padding_all_sides_overload() {
    let mut ui = on_parent_with(
        |s| s.content_padding_each(5.dp(), 10.dp(), 15.dp(), 20.dp()),
        |s| s.size(50.dp()),
    );
    assert_size(&mut ui, PARENT, 70.dp(), 80.dp(), 1.dp());
}

#[test]
fn test_offset_positioning_right_bottom() {
    let mut ui = ComposeTestRule::sized(IntSize::new(500, 500), || {
        Box!(
            modifier = styled(Modifier, None, |s| {
                s.size(100.dp());
                s.background(Color::BLUE);
            }),
            {
                Box!(
                    modifier = styled(Modifier, None, |s| {
                        s.size(20.dp());
                        s.right(10.dp());
                        s.bottom(30.dp());
                        s.background(Color::RED);
                    }),
                    {
                        Box!(
                            modifier = styled(Modifier.test_tag("child"), None, |s| s.fill_size())
                        );
                    }
                );
            }
        );
    });
    ui.on_node_with_tag("child")
        .assert_left_position_in_root_is_equal_to(70.dp())
        .assert_top_position_in_root_is_equal_to(50.dp());
}
