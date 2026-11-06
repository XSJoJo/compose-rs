//! Path data and image vectors, as Compose's `PathParser`, `PathNode`, `PathBuilder` and
//! `ImageVector.Builder`.

use compose_ui::*;

fn nodes(data: &str) -> Vec<PathNode> {
    PathParser::new().parse_path_string(data).expect("path data").to_nodes()
}

/// The points of a path's lines.
fn line_ends(path: &Path) -> Vec<Offset> {
    path.data()
        .commands()
        .iter()
        .filter_map(|command| match command {
            PathCommand::LineTo(end) => Some(*end),
            _ => None,
        })
        .collect()
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/vector/PathParserTest.kt, negativeExponent
#[test]
fn a_number_may_have_a_negative_exponent() {
    assert_eq!(nodes("H1e-5"), [PathNode::HorizontalTo { x: 1e-5 }]);
    let path = PathParser::new().parse_path_string("H1e-5").unwrap().to_path();
    assert_eq!(line_ends(&path), [Offset::new(1e-5, 0.0)]);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/vector/PathParserTest.kt, dotDot
#[test]
fn a_second_dot_starts_the_next_number() {
    assert_eq!(nodes("m0 0l2..5")[1], PathNode::RelativeLineTo { dx: 2.0, dy: 0.5 });
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/vector/PathParserTest.kt, relativeMoveToBecomesRelativeLineTo, moveToBecomesLineTo
#[test]
fn a_moves_further_pairs_are_lines() {
    assert_eq!(nodes("m0 0 2 5")[1..], [PathNode::RelativeLineTo { dx: 2.0, dy: 5.0 }]);
    assert_eq!(
        nodes("M0 0 2 5 6 7")[1..],
        [PathNode::LineTo { x: 2.0, y: 5.0 }, PathNode::LineTo { x: 6.0, y: 7.0 }]
    );
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/vector/PathParserTest.kt, relativeQuadToTest
#[test]
fn a_relative_quad_moves_the_current_point() {
    let path = PathParser::new().parse_path_string("q5, 5 7, 10H5").unwrap().to_path();
    assert_eq!(line_ends(&path)[0].y, 10.0);
    let path = PathParser::new().parse_path_string("q5, 5, 8, 8 V5").unwrap().to_path();
    assert_eq!(line_ends(&path)[0].x, 8.0);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/vector/PathParserTest.kt, separators
#[test]
fn spaces_and_commas_separate_numbers_in_any_mix() {
    for data in [
        "M5.25,3.9H3.76a0.73 0.73 ,0,0,0-0.54 0.22",
        "M5.25,3.9H3.76a0.73 0.73   ,0,0,0-0.54 0.22",
        "M5.25,3.9H3.76a0.73   0.73  ,  0,  0,  0-0.54 0.22",
        "M5.25,3.9H3.76a0.73 0.73 0 0 0-0.54 0.22",
        "M5.25,3.9H3.76a0.73 0.73, 0, 0, 0 -0.54 0.22",
        "M5.25,3.9H3.76a0.73 0.73,0,0,0-0.54 0.22",
        "M5.25,3.9H3.76a0.73 0.73 ,  0 , 0  ,   0 -0.54 0.22",
        "M5.25,3.9H3.76a0.73 0.73 ,,0,0,,0-0.54 0.22",
        "M5.25,3.9H3.76,a0.73 0.73 ,  0 , 0  ,   0 -0.54 0.22",
    ] {
        assert_eq!(
            nodes(data),
            [
                PathNode::MoveTo { x: 5.25, y: 3.9 },
                PathNode::HorizontalTo { x: 3.76 },
                PathNode::RelativeArcTo {
                    horizontal_ellipse_radius: 0.73,
                    vertical_ellipse_radius: 0.73,
                    theta: 0.0,
                    is_more_than_half: false,
                    is_positive_arc: false,
                    arc_start_dx: -0.54,
                    arc_start_dy: 0.22,
                },
            ],
            "{data}"
        );
    }
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/vector/PathParserTest.kt, ellipticalArcFlagsTest
#[test]
fn an_arcs_flags_may_run_into_its_end_point() {
    let arc = |dx: f32| PathNode::RelativeArcTo {
        horizontal_ellipse_radius: 75.0,
        vertical_ellipse_radius: 75.0,
        theta: 0.0,
        is_more_than_half: true,
        is_positive_arc: false,
        arc_start_dx: dx,
        arc_start_dy: 0.0,
    };
    for data in [
        "M 25 100 a 75 75 0 10150 0 a 75 75 0 10-150 0",
        "M 25 100 a 75 75 0 1 0150 0 a 75 75 0 1 0-150 0",
        "M 25 100 a 75 75 0 10 150 0 a 75 75 0 10 -150 0",
        "M 25 100 a 75 75 0 1 0 150 0 a 75 75 0 1 0 -150 0",
        "M 25 100 a 75 75 0 10150 0 a 75 75 0 1 0-150 0",
        "M 25 100 a 75 75 0 1 0150 0 a 75 75 0 10-150 0",
    ] {
        assert_eq!(nodes(data), [PathNode::MoveTo { x: 25.0, y: 100.0 }, arc(150.0), arc(-150.0)]);
    }
}

/// An unknown command is an error, as Compose throws; parsed data draws what `Path::from_svg`
/// draws from it (Material's icons were drawn by the one, and are by the other).
#[test]
fn parsed_nodes_draw_what_svg_path_data_draws() {
    assert!(PathParser::new().parse_path_string("M1,2 X3").is_err());
    for data in [
        "M19,13h-6v6h-2v-6H5v-2h6V5h2v6h6v2z",
        "M9,16.17L4.83,12l-1.42,1.41L9,19 21,7l-1.41-1.41z",
        "M2,12a10,10 0 1,0 20,0a10,10 0 1,0 -20,0z",
        "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm0 18c-4.41 0-8-3.59-8-8",
        "M3,3Q6,0 9,3T15,3t6,0M1 1z m2 2l1 1",
    ] {
        let parsed = PathParser::new().parse_path_string(data).unwrap().to_path();
        let svg = Path::from_svg(data).unwrap();
        assert_eq!(parsed.data().commands(), svg.data().commands(), "{data}");
    }
}

/// `path_data` builds the nodes its calls name, as `PathData { … }`.
#[test]
fn a_path_builder_adds_a_node_per_call() {
    let built = path_data(|path| {
        path.move_to(1.0, 2.0).line_to_relative(3.0, 4.0).horizontal_line_to(5.0);
        path.arc_to(1.0, 2.0, 3.0, true, false, 4.0, 5.0).close();
    });
    assert_eq!(built.len(), 5);
    assert_eq!(built[1], PathNode::RelativeLineTo { dx: 3.0, dy: 4.0 });
    assert_eq!(built[4], PathNode::Close);
}

fn builder() -> ImageVectorBuilder {
    ImageVector::builder(Dp(10.0), Dp(10.0), 10.0, 10.0)
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/graphics/vector/ImageVectorBuilderTest.kt, topLevelPathAndGroupDsl
#[test]
fn the_group_and_path_shorthands_build_the_same_vector() {
    let group =
        || VectorGroup { name: "Vector".into(), pivot_x: 0.2, pivot_y: 0.4, ..Default::default() };
    let added = builder()
        .add_group(group())
        .add_path(VectorPath {
            path_data: vec![PathNode::LineTo { x: 10.0, y: 10.0 }, PathNode::Close],
            ..Default::default()
        })
        .add_path(VectorPath {
            path_data: vec![
                PathNode::HorizontalTo { x: 20.0 },
                PathNode::RelativeReflectiveCurveTo { dx1: 40.0, dy1: 40.0, dx2: 10.0, dy2: 10.0 },
                PathNode::Close,
            ],
            ..Default::default()
        })
        .clear_group()
        .build();
    let shorthand = builder()
        .group(group(), |vector| {
            vector.path(VectorPath::default(), |path| {
                path.line_to(10.0, 10.0).close();
            });
            vector.path(VectorPath::default(), |path| {
                path.horizontal_line_to(20.0)
                    .reflective_curve_to_relative(40.0, 40.0, 10.0, 10.0)
                    .close();
            });
        })
        .build();
    assert_eq!(shorthand, added);
}

// Compose: compose/ui/ui/src/androidHostTest/kotlin/androidx/compose/ui/graphics/vector/ImageVectorBuilderTest.kt, testAddGroup
#[test]
fn groups_nest_until_cleared_and_build_leaves_them_all() {
    let group = |name: &str| VectorGroup { name: name.into(), ..Default::default() };
    let path = |name: &str| VectorPath { name: name.into(), ..Default::default() };
    let vector = builder()
        .add_group(group("group1"))
        .add_path(path("path1"))
        .add_group(group("group2"))
        .add_path(path("path2"))
        .clear_group()
        .add_group(group("group3"))
        .add_path(path("path3"))
        .add_path(path("path4"))
        .clear_group()
        .clear_group()
        .add_group(group("group4"))
        .add_path(path("path5"))
        .build();
    let names = |group: &VectorGroup| -> Vec<String> {
        group
            .children
            .iter()
            .map(|child| match child {
                VectorNode::Group(group) => group.name.clone(),
                VectorNode::Path(path) => path.name.clone(),
            })
            .collect()
    };
    let root = vector.root();
    assert_eq!(names(root), ["group1", "group4"]);
    let VectorNode::Group(group1) = &root.children[0] else { panic!("a group") };
    assert_eq!(names(group1), ["path1", "group2", "group3"]);
    let VectorNode::Group(group3) = &group1.children[2] else { panic!("a group") };
    assert_eq!(names(group3), ["path3", "path4"]);
    let VectorNode::Group(group4) = &root.children[1] else { panic!("a group") };
    assert_eq!(names(group4), ["path5"]);
}

/// A builder is single use, as Compose's; Compose's defaults are a vector's and its nodes'.
#[test]
fn a_builder_builds_once_with_composes_defaults() {
    let mut once = builder();
    let vector = once.build();
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| once.build())).is_err());
    assert_eq!(
        (vector.tint_color(), vector.tint_blend_mode()),
        (Color::UNSPECIFIED, BlendMode::SrcIn)
    );
    assert!(!vector.auto_mirror());
    let path = VectorPath::default();
    assert_eq!((path.fill_alpha, path.stroke_alpha, path.stroke_line_miter), (1.0, 1.0, 4.0));
    assert_eq!((path.trim_path_start, path.trim_path_end), (0.0, 1.0));
    let group = VectorGroup::default();
    assert_eq!((group.scale_x, group.scale_y, group.rotation), (1.0, 1.0, 0.0));
}
