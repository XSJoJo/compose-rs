//! Placing with a layer, as `placeWithLayer` (docs/parity/design/place-with-layer.md, D1909): a
//! placement gives the layer it places a graphics layer whose block is its own. Layer 0 (a node
//! placed by its parent) makes the node a repaint boundary whose group layer it is; a layout
//! modifier's content (layer k) is an inner layer of the node, from the layer's first entry.

use smallvec::SmallVec;

use crate::graphics_layer::{GraphicsLayerNode, LayerBlock};
use crate::node_chain::{ChainEntry, NodeKinds};
use crate::render::{INNER_LAYER_KEY, InnerLayer, RetainedDrawing};
use crate::tree::{LayoutNode, NodeId, UiTree};

/// What a placement's layer let go of: application code's captures, for its caller to drop
/// outside the tree.
#[allow(dead_code)]
pub(crate) enum Replaced {
    Layer(Box<GraphicsLayerNode>),
    Block(LayerBlock),
}

/// A layer of a node placed with a layer: the graphics layer its placement gave it.
pub(crate) struct PlacementLayer {
    /// The node's layer: 0 for the node, k for what its k-th layout modifier placed.
    pub(crate) layer: u16,
    pub(crate) graphics: GraphicsLayerNode,
}

impl LayoutNode {
    /// The first chain entry of layer `layer`: after the layout modifier measuring it.
    pub(crate) fn start_of_layer(&self, layer: usize) -> usize {
        match layer {
            0 => 0,
            _ => self.layout_indices[layer - 1] as usize + 1,
        }
    }

    /// The graphics layer layer `layer` was placed with, if it was.
    #[inline]
    pub(crate) fn placed_layer(&self, layer: usize) -> Option<&GraphicsLayerNode> {
        if self.placement_layers.is_empty() {
            return None;
        }
        (self.placement_layers.iter())
            .find(|placed| placed.layer as usize == layer)
            .map(|placed| &placed.graphics)
    }

    /// The layer placed with one whose first chain entry is `index`, if it's `max_layer` or one
    /// outside it, with that layer: what a walk out of a place of `max_layer` meets once past
    /// entry `index` (or at it, for a place before it), inside the graphics layers of the chain
    /// before it. Layer 0's is met last, before the node's position.
    #[inline]
    pub(crate) fn placed_layer_at(
        &self,
        index: usize,
        max_layer: usize,
    ) -> Option<(&GraphicsLayerNode, usize)> {
        if self.placement_layers.is_empty() {
            return None;
        }
        self.placement_layers.iter().find_map(|placed| {
            let layer = placed.layer as usize;
            (layer <= max_layer && layer < self.layers.len() && self.start_of_layer(layer) == index)
                .then_some((&placed.graphics, layer))
        })
    }

    /// The graphics layer whose properties are its group's, with the layer it's in: the one its
    /// parent placed it with, else its first drawing modifier if that's one.
    pub(crate) fn group_graphics(&self) -> Option<(&GraphicsLayerNode, usize)> {
        let retained = self.retained.as_ref()?;
        if retained.placed_layer {
            return self.placed_layer(0).map(|graphics| (graphics, 0));
        }
        let index = retained.layer_entry? as usize;
        let graphics = self.chain[index].node.downcast_ref::<GraphicsLayerNode>()?;
        Some((graphics, self.layer_of(index)))
    }
}

impl UiTree {
    /// Layer `layer` of `node` is placed with `block`, or without a layer (`None`), as
    /// `updateLayerBlock`: a new block computes the layer's properties again, the same one
    /// changes nothing, none drops a layer it had. Returns what it replaced, to drop outside the
    /// tree (application code's captures).
    pub(crate) fn update_placement_layer(
        &mut self,
        node: NodeId,
        layer: usize,
        block: Option<LayerBlock>,
    ) -> Option<Replaced> {
        let value = self.node_mut(node);
        let slot = value.placement_layers.iter().position(|placed| placed.layer as usize == layer);
        match (slot, block) {
            (None, None) => None,
            (Some(slot), Some(block)) => {
                let graphics = &mut value.placement_layers[slot].graphics;
                if graphics.runs(&block) {
                    return None;
                }
                let old = graphics.replace_block(block);
                self.placement_layer_updated(node, layer);
                old.map(Replaced::Block)
            }
            (None, Some(block)) => {
                let at = (value.placement_layers.iter())
                    .position(|placed| placed.layer as usize > layer)
                    .unwrap_or(value.placement_layers.len());
                let graphics = GraphicsLayerNode::placed(block);
                value.placement_layers.insert(at, PlacementLayer { layer: layer as u16, graphics });
                self.placement_layers_changed(node, layer);
                None
            }
            (Some(slot), None) => {
                let old = value.placement_layers.remove(slot);
                self.placement_layers_changed(node, layer);
                Some(Replaced::Layer(Box::new(old.graphics)))
            }
        }
    }

    /// A layer placed with one came or went: the node's groups are refreshed, what draws them
    /// draws again, and what's under it hears of the move.
    fn placement_layers_changed(&mut self, node: NodeId, layer: usize) {
        let parent_draws_differently = self.refresh_layers(node);
        let value = self.node(node);
        let parent = if value.overlay { None } else { value.parent };
        if (parent_draws_differently || layer == 0)
            && let Some(parent) = parent
        {
            self.invalidate_children_draw_of(parent);
        }
        self.invalidate_drawing_of_node(node);
        self.note_updated_layer(node);
        // What's in it moved, though its layers' places didn't: those listening hear of it, as
        // Compose's `onLayoutChange` for a layer made or destroyed.
        if !self.positioned.is_empty()
            || !self.coordinate_readers.is_empty()
            || self.rects.is_some()
        {
            let value = self.node_mut(node);
            if !value.layer_moved {
                value.layer_moved = true;
                self.layers_moved.push(node);
            }
        }
    }

    /// The block of the layer `layer` of `node` was placed with changed: its properties are
    /// computed again.
    fn placement_layer_updated(&mut self, node: NodeId, layer: usize) {
        let value = self.node_mut(node);
        if layer == 0 {
            if let Some(retained) = value.retained.as_mut() {
                retained.layer_dirty = true;
            }
        } else if let Some(inner) =
            value.inner_layers.iter_mut().find(|inner| inner.placement == Some(layer as u16))
        {
            inner.dirty = true;
        }
        // Being placed (its parent's measurement cleared `placed` before placing it).
        self.needs_draw = true;
        self.note_updated_layer(node);
    }

    /// Works out `node`'s repaint boundary and inner layers from its chain and the layers it was
    /// placed with. Returns whether its parent draws it otherwise (it became a boundary, or no
    /// longer is).
    ///
    /// A repaint boundary keeps what it draws: its layer's properties apart if its first drawing
    /// modifier is a graphics layer (which makes it one, as each of Compose's graphics layers is
    /// a layer), or if its parent placed it with a layer (whose properties its group's are then,
    /// D1909). Each other graphics layer is a group of its own inside the node's drawing (an
    /// inner layer, D522), and so is each layer a layout modifier placed with a layer.
    pub(crate) fn refresh_layers(&mut self, node: NodeId) -> bool {
        let value = self.node_mut(node);
        // Those of layers the chain no longer has go.
        let layers = value.layout_indices.len() + 1;
        let gone: SmallVec<[PlacementLayer; 1]> = if value.placement_layers.is_empty() {
            SmallVec::new()
        } else {
            let (kept, gone) = std::mem::take(&mut value.placement_layers)
                .into_iter()
                .partition::<Vec<_>, _>(|placed| (placed.layer as usize) < layers);
            value.placement_layers = kept;
            gone.into_iter().collect()
        };
        let placed_layer = value.placed_layer(0).is_some();
        let is_layer = |entry: &ChainEntry| entry.kinds.any(NodeKinds::GRAPHICS_LAYER);
        let layer_entry = if value.kinds.any(NodeKinds::GRAPHICS_LAYER) && !placed_layer {
            value
                .chain
                .iter()
                .position(is_layer)
                .filter(|&index| {
                    value.chain[..index].iter().all(|entry| !entry.kinds.any(NodeKinds::DRAW))
                })
                .map(|index| index as u16)
        } else {
            None
        };
        let boundary =
            layer_entry.is_some() || placed_layer || value.kinds.any(NodeKinds::REPAINT_BOUNDARY);
        // Each inner layer: its chain index, or its layer's first one and its layer.
        let mut inner: SmallVec<[(u16, Option<u16>); 2]> = if value
            .kinds
            .any(NodeKinds::GRAPHICS_LAYER)
        {
            (0..value.chain.len())
                .filter(|&index| is_layer(&value.chain[index]) && layer_entry != Some(index as u16))
                .map(|index| (index as u16, None))
                .collect()
        } else {
            SmallVec::new()
        };
        for placed in &value.placement_layers {
            if placed.layer > 0 {
                let start = value.start_of_layer(placed.layer as usize) as u16;
                inner.push((start, Some(placed.layer)));
            }
        }
        if !inner.is_empty() || !value.inner_layers.is_empty() {
            let mut layers = std::mem::take(&mut value.inner_layers);
            // Kept by chain index (or placed layer): the same render node for the same layer.
            layers.retain_mut(|layer| {
                let found = inner.iter().find(|(entry, placement)| match layer.placement {
                    None => placement.is_none() && *entry == layer.entry,
                    Some(placed) => *placement == Some(placed),
                });
                match found {
                    Some(&(entry, _)) => {
                        if layer.entry != entry {
                            layer.entry = entry;
                            layer.dirty = true;
                        }
                        true
                    }
                    None => {
                        self.inner_layer_owners.remove(&layer.key);
                        false
                    }
                }
            });
            for &(entry, placement) in &inner {
                let kept = layers.iter().any(|layer| match placement {
                    None => layer.is_entry(entry as usize),
                    Some(placed) => layer.placement == Some(placed),
                });
                if !kept {
                    self.next_inner_layer += 1;
                    let key = INNER_LAYER_KEY | self.next_inner_layer;
                    self.inner_layer_owners.insert(key, node);
                    let mut layer = InnerLayer::new(entry, key);
                    layer.placement = placement;
                    layers.push(layer);
                }
            }
            layers.sort_by_key(InnerLayer::order);
            self.node_mut(node).inner_layers = layers;
        }
        let value = self.node_mut(node);
        let parent_draws_differently = boundary != value.retained.is_some();
        match &mut value.retained {
            Some(retained) if boundary => {
                retained.layer_entry = layer_entry;
                retained.placed_layer = placed_layer;
                retained.dirty = true;
                retained.layer_dirty = true;
                retained.layer_ahead = None;
            }
            _ if boundary => {
                value.retained = Some(Box::new(RetainedDrawing::new(layer_entry, placed_layer)))
            }
            _ => value.retained = None,
        }
        // Application code's captures, the tree's own data: dropped with it busy, as a chain's
        // nodes are when they go.
        drop(gone);
        parent_draws_differently
    }
}
