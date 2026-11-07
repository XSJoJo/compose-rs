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
    /// Whether a layer of it was placed with one: its layers are in
    /// `UiTree::placement_layers` then.
    #[inline]
    pub(crate) fn has_placed_layers(&self) -> bool {
        self.has_placement_layers
    }

    /// The first chain entry of layer `layer`: after the layout modifier measuring it.
    pub(crate) fn start_of_layer(&self, layer: usize) -> usize {
        match layer {
            0 => 0,
            _ => self.layout_indices[layer - 1] as usize + 1,
        }
    }
}

impl UiTree {
    /// The layers `node` was placed with, by layer: none for most nodes.
    #[inline]
    pub(crate) fn placed_layers(&self, node: NodeId) -> &[PlacementLayer] {
        if !self.node(node).has_placed_layers() {
            return &[];
        }
        self.placement_layers.get(&node).map_or(&[], Vec::as_slice)
    }

    /// The graphics layer layer `layer` of `node` was placed with, if it was.
    #[inline]
    pub(crate) fn placed_layer(&self, node: NodeId, layer: usize) -> Option<&GraphicsLayerNode> {
        (self.placed_layers(node).iter())
            .find(|placed| placed.layer as usize == layer)
            .map(|placed| &placed.graphics)
    }

    /// The layer of `node` placed with one whose first chain entry is `index`, if it's
    /// `max_layer` or one outside it, with that layer: what a walk out of a place of `max_layer`
    /// meets once past entry `index` (or at it, for a place before it), inside the graphics
    /// layers of the chain before it. Layer 0's is met last, before the node's position.
    #[inline]
    pub(crate) fn placed_layer_starting_at(
        &self,
        node: NodeId,
        index: usize,
        max_layer: usize,
    ) -> Option<(&GraphicsLayerNode, usize)> {
        let value = self.node(node);
        self.placed_layers(node).iter().find_map(|placed| {
            let layer = placed.layer as usize;
            (layer <= max_layer
                && layer < value.layers.len()
                && value.start_of_layer(layer) == index)
                .then_some((&placed.graphics, layer))
        })
    }

    /// The graphics layers a point of layer `layer` of `node` before chain entry `end` goes out
    /// through to the node's outside, innermost first, each with the layer it's in and whether a
    /// placement gave it: the chain's, and those layers were placed with, interleaved (D1909).
    /// For a node with layers placed with one: the walks keep their loops over the chain for the
    /// others.
    #[cold]
    #[inline(never)]
    pub(crate) fn layers_out_with_placed(
        &self,
        node: NodeId,
        layer: usize,
        end: usize,
    ) -> SmallVec<[(&GraphicsLayerNode, usize, bool); 4]> {
        let value = self.node(node);
        let mut out = SmallVec::new();
        if let Some((graphics, at)) = self.placed_layer_starting_at(node, end, layer) {
            out.push((graphics, at, true));
        }
        for index in (0..end).rev() {
            if let Some(graphics) = value.chain[index].node.downcast_ref::<GraphicsLayerNode>() {
                out.push((graphics, value.layer_of(index), false));
            }
            if let Some((graphics, at)) = self.placed_layer_starting_at(node, index, layer) {
                out.push((graphics, at, true));
            }
        }
        out
    }

    /// The graphics layer whose properties are `node`'s group's (`group_graphics`, without its
    /// layer).
    pub(crate) fn group_graphics_node(&self, node: NodeId) -> Option<&GraphicsLayerNode> {
        self.group_graphics(node).map(|(graphics, _)| graphics)
    }

    /// The graphics layer whose properties are `node`'s group's, with the layer it's in: the one
    /// its parent placed it with, else its first drawing modifier if that's one.
    pub(crate) fn group_graphics(&self, node: NodeId) -> Option<(&GraphicsLayerNode, usize)> {
        let value = self.node(node);
        let retained = value.retained.as_ref()?;
        if retained.placed_layer {
            return self.placed_layer(node, 0).map(|graphics| (graphics, 0));
        }
        let index = retained.layer_entry? as usize;
        let graphics = value.chain[index].node.downcast_ref::<GraphicsLayerNode>()?;
        Some((graphics, value.layer_of(index)))
    }

    /// Layer `layer` of `node` is placed with `block`, or without a layer (`None`), as
    /// `updateLayerBlock`: a new block computes the layer's properties again, the same one
    /// changes nothing, none drops a layer it had. Returns what it replaced, to drop outside the
    /// tree (application code's captures).
    #[inline(never)]
    pub(crate) fn update_placement_layer(
        &mut self,
        node: NodeId,
        layer: usize,
        block: Option<LayerBlock>,
    ) -> Option<Replaced> {
        let slot =
            (self.placed_layers(node).iter()).position(|placed| placed.layer as usize == layer);
        match (slot, block) {
            (None, None) => None,
            (Some(slot), Some(block)) => {
                let layers = self.placement_layers.get_mut(&node).expect("its layers");
                let graphics = &mut layers[slot].graphics;
                if graphics.runs(&block) {
                    return None;
                }
                let old = graphics.replace_block(block);
                self.placement_layer_updated(node, layer);
                old.map(Replaced::Block)
            }
            (None, Some(block)) => {
                let value = self.node_mut(node);
                let first = !value.has_placement_layers;
                value.has_placement_layers = true;
                let layers = self.placement_layers.entry(node).or_default();
                let at = (layers.iter())
                    .position(|placed| placed.layer as usize > layer)
                    .unwrap_or(layers.len());
                let graphics = GraphicsLayerNode::placed(block);
                layers.insert(at, PlacementLayer { layer: layer as u16, graphics });
                self.placed_layer_nodes += u32::from(first);
                self.placement_layers_changed(node);
                None
            }
            (Some(slot), None) => {
                let layers = self.placement_layers.get_mut(&node).expect("its layers");
                let old = layers.remove(slot);
                if layers.is_empty() {
                    self.placement_layers.remove(&node);
                    self.node_mut(node).has_placement_layers = false;
                    self.placed_layer_nodes -= 1;
                }
                self.placement_layers_changed(node);
                Some(Replaced::Layer(Box::new(old.graphics)))
            }
        }
    }

    /// A layer placed with one came or went: the node's groups are refreshed, what draws them
    /// draws again, and what's under it hears of the move.
    fn placement_layers_changed(&mut self, node: NodeId) {
        let parent_draws_differently = self.refresh_layers(node);
        let value = self.node(node);
        let parent = if value.overlay { None } else { value.parent };
        // A child already a group draws in its parent the same with or without layer 0: the
        // group's layer properties and content are its own (its key stays).
        if parent_draws_differently && let Some(parent) = parent {
            self.invalidate_children_draw_of(parent);
        }
        self.invalidate_drawing_of_node(node);
        self.note_updated_layer(node);
        // What's in it moved, though its layers' places didn't: those listening hear of it, as
        // Compose's `onLayoutChange` for a layer made or destroyed; a text selection's handles
        // follow it too.
        if !self.positioned.is_empty()
            || !self.coordinate_readers.is_empty()
            || self.rects.is_some()
            || self.text_selection.is_some()
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

    /// `refresh_layers` while no node has a layer placed with one: the code the chain's refresh
    /// had before them (D1909), inlined in it as it was in the chain's refresh.
    #[inline(always)]
    fn refresh_layers_plain(&mut self, node: NodeId) -> bool {
        let value = self.node_mut(node);
        // A repaint boundary keeps what it draws: its layer's properties apart if its first
        // drawing modifier is a graphics layer (which makes it one, as each of Compose's
        // graphics layers is a layer). Each other graphics layer is a group of its own inside
        // the node's drawing (an inner layer, D522).
        let is_layer = |entry: &ChainEntry| entry.kinds.any(NodeKinds::GRAPHICS_LAYER);
        let layer_entry = if value.kinds.any(NodeKinds::GRAPHICS_LAYER) {
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
        let boundary = layer_entry.is_some() || value.kinds.any(NodeKinds::REPAINT_BOUNDARY);
        let inner: SmallVec<[u16; 2]> = if value.kinds.any(NodeKinds::GRAPHICS_LAYER) {
            (0..value.chain.len())
                .filter(|&index| is_layer(&value.chain[index]) && layer_entry != Some(index as u16))
                .map(|index| index as u16)
                .collect()
        } else {
            SmallVec::new()
        };
        if !inner.is_empty() || !value.inner_layers.is_empty() {
            let mut layers = std::mem::take(&mut value.inner_layers);
            // Kept by chain index: the same render node for the same layer.
            layers.retain(|layer| {
                // A placed layer's went with the last placed layer. Unobservable in what's
                // drawn: kept at a graphics layer's entry, it would draw as that layer's (these
                // paths read an inner layer by its entry), and the general refresh would give
                // that layer a group of its own once a node places with a layer again. Dropped,
                // no inner layer has a placement while no node has a placed layer (checked
                // below).
                let keep = layer.placement.is_none() && inner.contains(&layer.entry);
                if !keep {
                    self.inner_layer_owners.remove(&layer.key);
                }
                keep
            });
            for &entry in &inner {
                if !layers.iter().any(|layer| layer.entry == entry) {
                    self.next_inner_layer += 1;
                    let key = INNER_LAYER_KEY | self.next_inner_layer;
                    self.inner_layer_owners.insert(key, node);
                    layers.push(InnerLayer::new(entry, key));
                }
            }
            layers.sort_by_key(|layer| layer.entry);
            self.node_mut(node).inner_layers = layers;
        }
        let value = self.node_mut(node);
        debug_assert!(
            value.inner_layers.iter().all(|layer| layer.placement.is_none()),
            "an inner layer of a placement while no node has a layer placed with one"
        );
        let parent_draws_differently = boundary != value.retained.is_some();
        match &mut value.retained {
            Some(retained) if boundary => {
                retained.layer_entry = layer_entry;
                retained.placed_layer = false;
                retained.dirty = true;
                retained.layer_dirty = true;
            }
            _ if boundary => {
                value.retained = Some(Box::new(RetainedDrawing::new(layer_entry, false)))
            }
            _ => value.retained = None,
        }
        parent_draws_differently
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
    #[inline(always)]
    pub(crate) fn refresh_layers(&mut self, node: NodeId) -> bool {
        if self.placed_layer_nodes == 0 {
            self.refresh_layers_plain(node)
        } else {
            self.refresh_layers_placed(node)
        }
    }

    /// `refresh_layers` while a node has a layer placed with one (D1909).
    #[cold]
    #[inline(never)]
    fn refresh_layers_placed(&mut self, node: NodeId) -> bool {
        // Those of layers the chain no longer has go.
        let layers = self.node(node).layout_indices.len() + 1;
        let gone: Vec<PlacementLayer> = match self.placement_layers.get_mut(&node) {
            None => Vec::new(),
            Some(placed) => {
                let (kept, gone) = std::mem::take(placed)
                    .into_iter()
                    .partition::<Vec<_>, _>(|placed| (placed.layer as usize) < layers);
                *placed = kept;
                gone
            }
        };
        // Its last went.
        if !gone.is_empty() && self.placement_layers.get(&node).is_some_and(Vec::is_empty) {
            self.placement_layers.remove(&node);
            self.node_mut(node).has_placement_layers = false;
            self.placed_layer_nodes -= 1;
        }
        let placed_layer = self.placed_layer(node, 0).is_some();
        // The layers a layout modifier placed with one: inner layers.
        let placed_inner: SmallVec<[u16; 2]> = (self.placed_layers(node).iter())
            .filter(|placed| placed.layer > 0)
            .map(|placed| placed.layer)
            .collect();
        let value = self.node_mut(node);
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
        for placed in placed_inner {
            let start = value.start_of_layer(placed as usize) as u16;
            inner.push((start, Some(placed)));
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
