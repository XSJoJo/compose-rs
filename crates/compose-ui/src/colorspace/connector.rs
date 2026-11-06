//! Connectors between color spaces, as Compose's `colorspace.Connector`.

use std::borrow::Cow;
use std::fmt;
use std::sync::{Arc, OnceLock};

use super::{
    Adaptation, ColorModel, ColorSpace, D50_XYZ, Illuminant, NAMED_COUNT, RenderIntent,
    chromatic_adaptation, compare_white_points, inverse3x3, mul3x3, mul3x3_diag, mul3x3_float3,
};
use crate::graphics::Color;

/// Converts colors from one space to another, as Compose's `Connector`: a shared handle,
/// compared by identity as Compose's (`connect` gives the same one for the same named spaces
/// and intent).
#[derive(Clone)]
pub struct Connector(Arc<ConnectorData>);

struct ConnectorData {
    source: ColorSpace,
    destination: ColorSpace,
    render_intent: RenderIntent,
    kind: ConnectorKind,
}

enum ConnectorKind {
    /// From a space to itself.
    Identity,
    /// Between two RGB spaces: linear RGB to linear RGB.
    Rgb { transform: [f32; 9] },
    /// Through XYZ (D50): the RGB spaces adapted to D50, and the white point scaling of an
    /// absolute intent.
    General {
        transform_source: ColorSpace,
        transform_destination: ColorSpace,
        transform: Option<[f32; 3]>,
    },
}

impl Connector {
    pub fn source(&self) -> &ColorSpace {
        &self.0.source
    }

    pub fn destination(&self) -> &ColorSpace {
        &self.0.destination
    }

    pub fn render_intent(&self) -> RenderIntent {
        self.0.render_intent
    }

    /// The color `(r, g, b)` of the source in the destination, as `transform(r, g, b)`.
    pub fn transform(&self, r: f32, g: f32, b: f32) -> [f32; 3] {
        let mut values = [r, g, b, 0.0];
        let count = self.0.source.component_count().max(self.0.destination.component_count());
        self.transform_in_place(&mut values[..count.max(3)]);
        [values[0], values[1], values[2]]
    }

    /// Converts `values` (the source's components, with room for the destination's) in place,
    /// as `transform(v)`.
    pub fn transform_in_place(&self, values: &mut [f32]) {
        let data = &*self.0;
        match &data.kind {
            ConnectorKind::Identity => {}
            ConnectorKind::Rgb { transform } => {
                let (source, destination) = rgb_pair(data);
                let [r, g, b] = source.decode_values(values[0], values[1], values[2]);
                let [r, g, b] = mul3x3_float3(transform, r, g, b);
                values[..3].copy_from_slice(&destination.encode_values(r, g, b));
            }
            ConnectorKind::General { transform_source, transform_destination, transform } => {
                transform_source.to_xyz_in_place(values);
                if let Some(transform) = transform {
                    for (value, factor) in values.iter_mut().zip(transform) {
                        *value *= factor;
                    }
                }
                transform_destination.from_xyz_in_place(values);
            }
        }
    }

    /// `color` (of the source) in the destination, as Compose's internal `transformToColor`.
    pub(crate) fn transform_to_color(&self, color: Color) -> Color {
        let data = &*self.0;
        match &data.kind {
            ConnectorKind::Identity => color,
            ConnectorKind::Rgb { transform } => {
                let (source, destination) = rgb_pair(data);
                let [r, g, b] = source.decode_values(color.red(), color.green(), color.blue());
                let [r, g, b] = mul3x3_float3(transform, r, g, b);
                let [r, g, b] = destination.encode_values(r, g, b);
                Color::new_with(r, g, b, color.alpha(), &data.destination)
            }
            ConnectorKind::General { transform_source, transform_destination, transform } => {
                let [mut x, mut y, mut z] =
                    transform_source.to_xyz_values(color.red(), color.green(), color.blue());
                if let Some(transform) = transform {
                    x *= transform[0];
                    y *= transform[1];
                    z *= transform[2];
                }
                transform_destination.xyza_to_color(x, y, z, color.alpha(), &data.destination)
            }
        }
    }
}

fn rgb_pair(data: &ConnectorData) -> (&super::RgbData, &super::RgbData) {
    let source = data.source.rgb_data().expect("compose-ui: an RGB connector's source");
    let destination =
        data.destination.rgb_data().expect("compose-ui: an RGB connector's destination");
    (source, destination)
}

impl PartialEq for Connector {
    fn eq(&self, other: &Connector) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for Connector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Connector")
            .field("source", &self.0.source)
            .field("destination", &self.0.destination)
            .field("render_intent", &self.0.render_intent)
            .finish()
    }
}

/// The connector from `source` to `destination` with `intent`, as Compose's `connect`: made
/// once for two named spaces (kept in a table read without a lock), anew for any other.
pub(crate) fn connect(
    source: &ColorSpace,
    destination: &ColorSpace,
    intent: RenderIntent,
) -> Cow<'static, Connector> {
    const CACHED: usize = NAMED_COUNT * NAMED_COUNT * 4;
    static CACHE: [OnceLock<Connector>; CACHED] = [const { OnceLock::new() }; CACHED];
    let (source_id, destination_id) = (source.id(), destination.id());
    if source_id < 0
        || destination_id < 0
        || source_id as usize >= NAMED_COUNT
        || destination_id as usize >= NAMED_COUNT
    {
        return Cow::Owned(create(source, destination, intent));
    }
    let index = (intent as usize * NAMED_COUNT + destination_id as usize) * NAMED_COUNT
        + source_id as usize;
    Cow::Borrowed(CACHE[index].get_or_init(|| create(source, destination, intent)))
}

/// As Compose's `createConnector`.
fn create(source: &ColorSpace, destination: &ColorSpace, intent: RenderIntent) -> Connector {
    if source.same_instance(destination) {
        return Connector(Arc::new(ConnectorData {
            source: source.clone(),
            destination: source.clone(),
            render_intent: RenderIntent::Relative,
            kind: ConnectorKind::Identity,
        }));
    }
    let kind = match (source.rgb_data(), destination.rgb_data()) {
        (Some(source), Some(destination)) => {
            ConnectorKind::Rgb { transform: rgb_transform(source, destination, intent) }
        }
        _ => {
            let adapted = |space: &ColorSpace| {
                if space.model() == ColorModel::Rgb {
                    space.adapt(Illuminant::D50)
                } else {
                    space.clone()
                }
            };
            ConnectorKind::General {
                transform_source: adapted(source),
                transform_destination: adapted(destination),
                transform: general_transform(source, destination, intent),
            }
        }
    };
    Connector(Arc::new(ConnectorData {
        source: source.clone(),
        destination: destination.clone(),
        render_intent: intent,
        kind,
    }))
}

/// The linear RGB to linear RGB transform, as Compose's `RgbConnector.computeTransform`.
fn rgb_transform(
    source: &super::RgbData,
    destination: &super::RgbData,
    intent: RenderIntent,
) -> [f32; 9] {
    if compare_white_points(source.white_point, destination.white_point) {
        return mul3x3(&destination.inverse_transform, &source.transform);
    }
    let mut transform = source.transform;
    let mut inverse_transform = destination.inverse_transform;
    let mut source_xyz = source.white_point.to_xyz();
    let mut destination_xyz = destination.white_point.to_xyz();
    let bradford = &Adaptation::BRADFORD.transform;
    if !compare_white_points(source.white_point, Illuminant::D50) {
        let adaptation = chromatic_adaptation(bradford, source_xyz, D50_XYZ);
        transform = mul3x3(&adaptation, &source.transform);
        // Compose's `chromaticAdaptation` turns the white point it's given into cone responses
        // in place, which the absolute intent below then divides.
        source_xyz = mul3x3_float3(bradford, source_xyz[0], source_xyz[1], source_xyz[2]);
    }
    if !compare_white_points(destination.white_point, Illuminant::D50) {
        let adaptation = chromatic_adaptation(bradford, destination_xyz, D50_XYZ);
        inverse_transform = inverse3x3(&mul3x3(&adaptation, &destination.transform));
        destination_xyz =
            mul3x3_float3(bradford, destination_xyz[0], destination_xyz[1], destination_xyz[2]);
    }
    if intent == RenderIntent::Absolute {
        transform = mul3x3_diag(
            [
                source_xyz[0] / destination_xyz[0],
                source_xyz[1] / destination_xyz[1],
                source_xyz[2] / destination_xyz[2],
            ],
            &transform,
        );
    }
    mul3x3(&inverse_transform, &transform)
}

/// The white point scaling of an absolute intent between an RGB space and another kind, as
/// Compose's `Connector.computeTransform`.
fn general_transform(
    source: &ColorSpace,
    destination: &ColorSpace,
    intent: RenderIntent,
) -> Option<[f32; 3]> {
    if intent != RenderIntent::Absolute {
        return None;
    }
    let source_rgb = source.model() == ColorModel::Rgb;
    let destination_rgb = destination.model() == ColorModel::Rgb;
    if source_rgb == destination_rgb {
        return None;
    }
    let rgb = if source_rgb { source } else { destination }.rgb_data()?;
    let source_xyz = if source_rgb { rgb.white_point.to_xyz() } else { D50_XYZ };
    let destination_xyz = if destination_rgb { rgb.white_point.to_xyz() } else { D50_XYZ };
    Some([
        source_xyz[0] / destination_xyz[0],
        source_xyz[1] / destination_xyz[1],
        source_xyz[2] / destination_xyz[2],
    ])
}
