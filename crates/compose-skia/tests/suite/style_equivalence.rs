//! Compose's `StyleEquivalenceTests` (device): a style draws as the modifiers it stands for,
//! captured with Skia and compared pixel by pixel. Written for Compose's deprecated
//! `styleable(state, style)`, here through the resolver chain and the predefined properties.

#![allow(non_snake_case)]

use std::rc::Rc;

use compose::foundation::BasicText;
use compose::foundation::layout::*;
use compose::foundation::shape::{CutCornerShape, RoundedCornerShape};
use compose::foundation::style::properties::*;
use compose::foundation::style::*;
use compose::runtime::*;
use compose::test::*;
use compose::ui::{
    Alignment, BorderStroke, Brush, Color, CornerSize, CustomShape, Density, Dp, DpSize,
    GraphicsLayerScope, IntSize, LayoutDirection, Modifier, Outline, Path, Rect, Shape, Size,
    TextStyle, UnitExt,
};

/// The style's blocks, as Compose's deprecated `StyleScope` setters.
type Block = Rc<dyn Fn(&mut dyn CommonStyleScope)>;

fn block(f: impl Fn(&mut dyn CommonStyleScope) + 'static) -> Block {
    Rc::new(f)
}

fn border(s: &mut dyn CommonStyleScope, width: Dp, color: Color) {
    s.provide(&BORDER_WIDTH, width);
    s.provide(&BORDER_FILL, Fill::Color(color));
}

fn content_padding(s: &mut dyn CommonStyleScope, all: Dp) {
    for property in [
        &CONTENT_PADDING_START,
        &CONTENT_PADDING_TOP,
        &CONTENT_PADDING_END,
        &CONTENT_PADDING_BOTTOM,
    ] {
        s.provide(property, all);
    }
}

/// `BaseStyleableButton`: a centered row styled by `style` through the resolver chain (the
/// click and its source left out: nothing presses it).
fn BaseStyleableButton(style: Block, content: impl Fn(RowScope) + Clone + 'static) {
    let state = remember(|| MutableStyleState::new(None));
    let resolver = remember_observer(move || {
        StyleResolver::new(CommonStyle::new(move |s| style(s)), state.clone())
    });
    let r = (*resolver).clone();
    Row!(
        modifier = Modifier
            .style_resolver(r.clone())
            .style_external_padding(r.clone())
            .style_graphics_layer(r.clone())
            .style_placement(r.clone())
            .style_appearance(r.clone())
            .style_border_padding(r.clone())
            .style_content_padding(r),
        horizontal_arrangement = Arrangement::CENTER,
        vertical_alignment = Alignment::CENTER_VERTICALLY,
        content = content
    );
}

/// `BaseModifierButton`'s parameters these tests use.
#[derive(Default)]
struct Modifiers {
    external_padding: Option<Dp>,
    border: Option<BorderStroke>,
    background: Option<Brush>,
    content_padding: Option<Dp>,
    size: Option<DpSize>,
    layer: Option<GraphicsLayerScope>,
    shape: Option<Shape>,
}

/// `BaseModifierButton`: the same row made of modifiers.
fn BaseModifierButton(spec: Modifiers, content: impl Fn(RowScope) + Clone + 'static) {
    let shape = spec.shape.clone().unwrap_or(Shape::Rectangle);
    let mut modifier = Modifier;
    if let Some(layer) = spec.layer.clone() {
        modifier = modifier.graphics_layer_with(layer);
    }
    if let Some(padding) = spec.external_padding {
        modifier = modifier.padding(padding);
    }
    if let Some(size) = spec.size {
        modifier = modifier.size(size);
    }
    match &spec.border {
        Some(stroke) => {
            // The border's pixels as a padding, as the style's border padding.
            let padding =
                if stroke.width == Dp::HAIRLINE { Dp(1.0) } else { Dp(stroke.width.0.ceil()) };
            modifier = modifier.border_stroke(stroke.clone(), shape.clone());
            if let Some(brush) = spec.background.clone() {
                modifier = modifier.background_with(brush, shape.clone());
            }
            modifier = modifier.padding(padding);
        }
        None => {
            if let Some(brush) = spec.background.clone() {
                modifier = modifier.background_with(brush, shape.clone());
            }
        }
    }
    if let Some(padding) = spec.content_padding {
        modifier = modifier.padding(padding);
    }
    Row!(
        modifier = modifier,
        horizontal_arrangement = Arrangement::CENTER,
        vertical_alignment = Alignment::CENTER_VERTICALLY,
        content = content
    );
}

const ROOT: &str = "style_equivalence_root";

/// `checkEquivalence`: the style version and the modifier version draw the same pixels.
fn check_equivalence(style_version: impl Fn() + 'static, modifier_version: impl Fn() + 'static) {
    set_capture_renderer(compose_skia::raster_colors);
    let with_style = mutable_state_of(true);
    let shown = with_style.handle();
    let (style_version, modifier_version) = (Rc::new(style_version), Rc::new(modifier_version));
    let mut ui = ComposeTestRule::sized(IntSize::new(200, 200), move || {
        let (style_version, modifier_version) = (style_version.clone(), modifier_version.clone());
        Box!(modifier = Modifier.test_tag(ROOT), move |_| {
            if shown.get() {
                style_version();
            } else {
                modifier_version();
            }
        });
    });
    let style_image = ui.on_node_with_tag(ROOT).capture_to_image();
    with_style.set(false);
    ui.wait_for_idle();
    let modifier_image = ui.on_node_with_tag(ROOT).capture_to_image();
    assert_eq!(style_image.size(), modifier_image.size(), "size mismatch");
    let differing =
        style_image.pixels().iter().zip(modifier_image.pixels()).filter(|(a, b)| a != b).count();
    assert_eq!(differing, 0, "style and modifier versions draw different pixels");
}

fn sized_box(size: Dp) -> impl Fn(RowScope) + Clone + 'static {
    move |_| Box!(modifier = Modifier.size(size))
}

fn blue_box(_: RowScope) {
    Box!(modifier = Modifier.size(10.dp()).background(Color::BLUE));
}

#[test]
fn background() {
    check_equivalence(
        || {
            BaseStyleableButton(
                block(|s| s.provide(&BACKGROUND, Fill::Color(Color::BLUE))),
                sized_box(10.dp()),
            )
        },
        || {
            let spec =
                Modifiers { background: Some(Brush::Solid(Color::BLUE)), ..Modifiers::default() };
            BaseModifierButton(spec, sized_box(10.dp()))
        },
    );
}

#[test]
fn border_() {
    check_equivalence(
        || BaseStyleableButton(block(|s| border(s, 2.dp(), Color::RED)), sized_box(10.dp())),
        || {
            let spec = Modifiers {
                border: Some(BorderStroke::new(2.dp(), Color::RED)),
                ..Modifiers::default()
            };
            BaseModifierButton(spec, sized_box(10.dp()))
        },
    );
}

#[test]
fn content_padding_() {
    check_equivalence(
        || BaseStyleableButton(block(|s| content_padding(s, 5.dp())), blue_box),
        || {
            let spec = Modifiers { content_padding: Some(5.dp()), ..Modifiers::default() };
            BaseModifierButton(spec, blue_box)
        },
    );
}

#[test]
fn border_fill_area() {
    check_equivalence(
        || {
            BaseStyleableButton(
                block(|s| {
                    border(s, 20.dp(), Color::RED);
                    s.provide(&SHAPE, RoundedCornerShape(5.dp()));
                }),
                |_| {},
            )
        },
        || {
            let spec = Modifiers {
                border: Some(BorderStroke::new(20.dp(), Color::RED)),
                shape: Some(RoundedCornerShape(5.dp())),
                ..Modifiers::default()
            };
            BaseModifierButton(spec, |_| {})
        },
    );
}

/// The style and modifier versions of a red 2 dp border around `content` in `shape`, over
/// `background`.
fn check_border(shape: Shape, background: Option<Color>, content: Dp) {
    let style_shape = shape.clone();
    check_equivalence(
        move || {
            let shape = style_shape.clone();
            BaseStyleableButton(
                block(move |s| {
                    border(s, 2.dp(), Color::RED);
                    s.provide(&SHAPE, shape.clone());
                    if let Some(color) = background {
                        s.provide(&BACKGROUND, Fill::Color(color));
                    }
                }),
                sized_box(content),
            )
        },
        move || {
            let spec = Modifiers {
                border: Some(BorderStroke::new(2.dp(), Color::RED)),
                shape: Some(shape.clone()),
                background: background.map(Brush::Solid),
                ..Modifiers::default()
            };
            BaseModifierButton(spec, sized_box(content))
        },
    );
}

#[test]
fn border_generic_shape_background() {
    check_border(CutCornerShape(3.dp()), Some(Color::BLUE), 20.dp());
}

#[test]
fn border_shape_not_simple() {
    let corners =
        [CornerSize::Dp(5.dp()), CornerSize::ZERO, CornerSize::Dp(10.dp()), CornerSize::ZERO];
    check_border(Shape::Rounded(corners), None, 20.dp());
}

/// A shape whose outline `outline` makes from the size.
struct OutlineShape(fn(Size) -> Outline);

impl CustomShape for OutlineShape {
    fn create_outline(&self, size: Size, _: LayoutDirection, _: &Density) -> Outline {
        (self.0)(size)
    }
    fn same(&self, other: &dyn CustomShape) -> bool {
        let other: &dyn std::any::Any = other;
        other
            .downcast_ref::<OutlineShape>()
            .is_some_and(|other| std::ptr::fn_addr_eq(other.0, self.0))
    }
}

#[test]
fn border_custom_outline_rectangle() {
    let shape = Shape::custom(OutlineShape(|size| {
        Outline::Rectangle(Rect::new(5.0, 5.0, size.width - 5.0, size.height - 5.0))
    }));
    check_border(shape, None, 30.dp());
}

#[test]
fn border_custom_outline_rounded() {
    let shape = Shape::custom(OutlineShape(|size| {
        Outline::Rounded(Rect::new(5.0, 5.0, size.width - 5.0, size.height - 5.0), [8.0; 4])
    }));
    check_border(shape, None, 30.dp());
}

#[test]
fn border_custom_outline_rounded_not_simple() {
    let shape = Shape::custom(OutlineShape(|size| {
        let rect = Rect::new(5.0, 5.0, size.width - 5.0, size.height - 5.0);
        Outline::Rounded(rect, [4.0, 0.0, 10.0, 0.0])
    }));
    check_border(shape, None, 30.dp());
}

#[test]
fn border_custom_outline_generic() {
    let shape = Shape::custom(OutlineShape(|size| {
        let path = Path::new();
        path.add_rect(Rect::new(5.0, 5.0, size.width - 5.0, size.height - 5.0));
        Outline::generic(&path)
    }));
    check_border(shape, None, 30.dp());
}

#[test]
fn border_custom_outline_generic_background() {
    let shape = Shape::custom(OutlineShape(|size| {
        let path = Path::new();
        path.move_to(size.width / 2.0, 0.0);
        path.line_to(size.width, size.height);
        path.line_to(0.0, size.height);
        path.close();
        Outline::generic(&path)
    }));
    check_border(shape, Some(Color::BLUE), 30.dp());
}

#[test]
fn external_padding() {
    check_equivalence(
        || {
            BaseStyleableButton(
                block(|s| {
                    for property in [
                        &EXTERNAL_PADDING_START,
                        &EXTERNAL_PADDING_TOP,
                        &EXTERNAL_PADDING_END,
                        &EXTERNAL_PADDING_BOTTOM,
                    ] {
                        s.provide(property, 10.dp());
                    }
                }),
                blue_box,
            )
        },
        || {
            let spec = Modifiers { external_padding: Some(10.dp()), ..Modifiers::default() };
            BaseModifierButton(spec, blue_box)
        },
    );
}

fn red_box(_: RowScope) {
    Box!(modifier = Modifier.size(10.dp()).background(Color::RED));
}

#[test]
fn shape() {
    check_equivalence(
        || {
            BaseStyleableButton(
                block(|s| {
                    s.provide(&BACKGROUND, Fill::Color(Color::BLUE));
                    s.provide(&SHAPE, RoundedCornerShape(5.dp()));
                    content_padding(s, 10.dp());
                }),
                red_box,
            )
        },
        || {
            let spec = Modifiers {
                background: Some(Brush::Solid(Color::BLUE)),
                shape: Some(RoundedCornerShape(5.dp())),
                content_padding: Some(10.dp()),
                ..Modifiers::default()
            };
            BaseModifierButton(spec, red_box)
        },
    );
}

#[test]
fn alpha() {
    check_equivalence(
        || BaseStyleableButton(block(|s| s.provide(&ALPHA, 0.5)), blue_box),
        || {
            let layer = GraphicsLayerScope { alpha: 0.5, ..GraphicsLayerScope::default() };
            BaseModifierButton(Modifiers { layer: Some(layer), ..Modifiers::default() }, blue_box)
        },
    );
}

#[test]
fn size() {
    check_equivalence(
        || {
            BaseStyleableButton(
                block(|s| {
                    s.provide(&WIDTH, Breadth::Distance(50.dp()));
                    s.provide(&HEIGHT, Breadth::Distance(50.dp()));
                    s.provide(&BACKGROUND, Fill::Color(Color::GREEN));
                }),
                blue_box,
            )
        },
        || {
            let spec = Modifiers {
                size: Some(DpSize::new(50.dp(), 50.dp())),
                background: Some(Brush::Solid(Color::GREEN)),
                ..Modifiers::default()
            };
            BaseModifierButton(spec, blue_box)
        },
    );
}

#[test]
fn rounded_corner_shape_brush_background() {
    check_equivalence(
        || {
            BaseStyleableButton(
                block(|s| {
                    s.provide(&SHAPE, RoundedCornerShape(5.dp()));
                    s.provide(&BACKGROUND, Fill::Brush(Brush::Solid(Color::RED)));
                    content_padding(s, 10.dp());
                }),
                blue_box,
            )
        },
        || {
            let spec = Modifiers {
                content_padding: Some(10.dp()),
                shape: Some(RoundedCornerShape(5.dp())),
                background: Some(Brush::Solid(Color::RED)),
                ..Modifiers::default()
            };
            BaseModifierButton(spec, blue_box)
        },
    );
}

#[test]
fn text_style_priority() {
    let text = |_: BoxScope| {
        BasicText!(
            "Expected yellow",
            style = TextStyle { color: Color::YELLOW, ..TextStyle::default() }
        );
    };
    check_equivalence(
        move || {
            let state = remember(|| MutableStyleState::new(None));
            let resolver = remember_observer(move || {
                let style = CommonStyle::new(|s| {
                    content_padding(s, 10.dp());
                    s.provide(&CONTENT_FILL, Fill::Color(Color::RED));
                });
                StyleResolver::new(style, state.clone())
            });
            let r = (*resolver).clone();
            Box!(
                modifier = Modifier
                    .style_resolver(r.clone())
                    .style_appearance(r.clone())
                    .style_content_padding(r),
                content = text
            );
        },
        move || Box!(modifier = Modifier.padding(10.dp()), content = text),
    );
}
