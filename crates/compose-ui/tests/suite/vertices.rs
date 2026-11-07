//! `Vertices` and the canvas's small enums: Compose's common tests of them.

use compose_ui::canvas::{ClipOp, PointMode, VertexMode, Vertices};
use compose_ui::{Color, Offset};

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/VerticesTest.kt, testVertices_listConstructor_valid
#[test]
fn vertices_keep_what_theyre_made_of() {
    let positions = [Offset::new(0.0, 0.0), Offset::new(10.0, 0.0), Offset::new(10.0, 10.0)];
    let textures = [Offset::new(0.0, 0.0), Offset::new(1.0, 0.0), Offset::new(1.0, 1.0)];
    let colors = [Color::RED, Color::GREEN, Color::BLUE];
    let vertices = Vertices::new(VertexMode::Triangles, &positions, &textures, &colors, &[0, 1, 2]);
    assert_eq!(vertices.positions().len(), 3);
    assert_eq!(vertices.texture_coordinates().len(), 3);
    assert_eq!(vertices.colors().len(), 3);
    assert_eq!(vertices.indices().len(), 3);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/VerticesTest.kt, testVertices_createFromRawArrays_valid
#[test]
fn vertices_are_made_from_flat_arrays() {
    let vertices =
        Vertices::from_raw(VertexMode::Triangles, &[0.0; 6], &[0.0; 6], &[0; 3], &[0, 1, 2]);
    assert_eq!(vertices.positions().len(), 3);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/VerticesTest.kt, testVertices_constructorsProduceSameDrawData
#[test]
fn both_constructors_make_the_same_vertices() {
    let positions = [Offset::new(0.0, 0.0), Offset::new(10.0, 0.0), Offset::new(5.0, 10.0)];
    let textures = [Offset::new(0.0, 0.0), Offset::new(1.0, 0.0), Offset::new(0.5, 1.0)];
    let colors = [Color::RED, Color::GREEN, Color::BLUE];
    let listed = Vertices::new(VertexMode::Triangles, &positions, &textures, &colors, &[0, 1, 2]);
    let flat = |offsets: &[Offset]| -> Vec<f32> {
        offsets.iter().flat_map(|offset| [offset.x, offset.y]).collect()
    };
    let argb: Vec<u32> = colors.iter().map(|color| color.to_argb()).collect();
    let raw = Vertices::from_raw(
        VertexMode::Triangles,
        &flat(&positions),
        &flat(&textures),
        &argb,
        &[0, 1, 2],
    );
    assert_eq!(listed, raw);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/VerticesTest.kt, testVertices_oddPositionsLength_throwsException
#[test]
#[should_panic(expected = "positions length must be even")]
fn vertices_of_an_odd_number_of_values_panic() {
    Vertices::from_raw(VertexMode::Triangles, &[0.0; 3], &[0.0; 3], &[0; 1], &[]);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/VerticesTest.kt, testVertices_mismatchedColors_throwsException
#[test]
#[should_panic(expected = "positions and colors lengths must match.")]
fn vertices_with_too_few_colors_panic() {
    let positions = [Offset::ZERO; 3];
    Vertices::new(VertexMode::Triangles, &positions, &positions, &[Color::RED; 2], &[]);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/VerticesTest.kt, testVertices_outOfBoundsIndex_throwsException
#[test]
#[should_panic(expected = "indices values must be valid indices in the positions list.")]
fn vertices_with_an_index_past_the_positions_panic() {
    let positions = [Offset::ZERO; 3];
    Vertices::new(VertexMode::Triangles, &positions, &positions, &[Color::RED; 3], &[0, 1, 3]);
}

// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/ClipOpTest.kt, testToString
// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/PointModeTest.kt, testToString
// Compose: compose/ui/ui-graphics/src/commonTest/kotlin/androidx/compose/ui/graphics/VertexModeTest.kt, testToString
#[test]
fn the_canvas_enums_print_their_names() {
    assert_eq!(format!("{:?}", ClipOp::Difference), "Difference");
    assert_eq!(format!("{:?}", ClipOp::Intersect), "Intersect");
    assert_eq!(format!("{:?}", PointMode::Points), "Points");
    assert_eq!(format!("{:?}", PointMode::Lines), "Lines");
    assert_eq!(format!("{:?}", PointMode::Polygon), "Polygon");
    assert_eq!(format!("{:?}", VertexMode::TriangleFan), "TriangleFan");
    assert_eq!(format!("{:?}", VertexMode::TriangleStrip), "TriangleStrip");
    assert_eq!(format!("{:?}", VertexMode::Triangles), "Triangles");
}
