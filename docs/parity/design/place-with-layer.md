# Placing with a layer

Design note for D1909 and D1910, from Compose's `Placeable.kt` (`placeWithLayer`, `placeRelativeWithLayer`), `NodeCoordinator.kt` (`placeAt`, `placeSelf`, `updateLayerBlock`, `updateLayerParameters`) and `OwnedLayer`, and kept to what was built.

## What Compose does

A placement block can place a placeable with a layer: `placeWithLayer(x, y, zIndex) { … }` with a layer block, or `placeWithLayer(x, y, layer)` with a `GraphicsLayer` object. The layer belongs to the coordinator placed: a child's outermost coordinator (its whole node), or a layout modifier's wrapped coordinator (what follows the modifier in the chain, and the children). It is an `OwnedLayer`, a render node on Android, positioned by the placement: moving the placeable moves the layer (`layer.move(position)`) and draws nothing again. The block runs in `updateLayerParameters`, under an observation of its own, so a state it reads changes the layer's properties only, not the layout. A new block (compared by identity) runs at once; the same block placed again doesn't. Placing without a layer after placing with one destroys the layer (`updateLayerBlock(null)`), and tells the layout callbacks if it transformed. Coordinates map through the layer's matrix, so positions in the root, bounds and the layout-rect callbacks see its scale, rotation and translation.

## How it fits compose-rs

compose-rs has no coordinator objects: a node's layers are its `Layer` entries (layer 0 is placed by its parent, layer k by its k-th layout modifier), and graphics layers are chain entries, drawn as a repaint boundary's group layer when first in the chain, else as inner layers (D522). A placement layer is put where that machinery already looks.

**Where it's kept.** `LayoutNode::placement_layers` holds, per layer placed with one, a `GraphicsLayerNode` built from the block: the same node a `graphics_layer` modifier makes, with its last run, positional properties and outline cache, so every walk that maps through graphics layers treats it as one. `None` (a boxed vector otherwise) for nodes never placed with a layer, and `UiTree::placed_layer_nodes` counts those that are: while it's 0, the walks through a node's layers (`map_all_to_root`, `map_out_of_node`, `map_within_node`, `clip_by_node`, `alpha_around`, `point_through_chain`, `walk_out`, `walk_in`, `clips_between`) run the code they had before, each its general form (`…_placed`) otherwise. The draw pass's chain walk reads whether the node has any in the one look it takes at the node before its loop, and stops its loop where such a layer starts (`placed_group_at`, out of line). Placing tests the node's field in the look it takes already.

**Placing.** `PlacementScope` and `MeasureScope` (the fused path) get `place_with_layer`, `place_at_with_layer`, `place_relative_with_layer` (a `LayerBlock`, from any closure or kept to keep its identity) and `place_with_graphics_layer`, `place_relative_with_graphics_layer` (a `GraphicsLayer`, its properties read by a block). `place_in_run` takes the block: a new one is kept and its layer marked to compute again, the same one changes nothing, none drops a layer placed before. The lookahead pass ignores layers, as Compose's `LookaheadDelegate`.

**Layer 0: the node's group.** A node placed with a layer is a repaint boundary whose group layer is the placement's: `RetainedDrawing::placed_layer`, and its chain's graphics layers all become inner layers (none is lifted). Its parent draws it by reference at its position, as any boundary, so moving it records only the parent's translation, and its block runs under `Observation::Layer`: a state it reads changes the group's properties, not its version. On Android that is one render node per placed child, as Compose's, and nothing new on the wire.

**Layer k > 0: an inner layer.** A layout modifier placing its content with a layer gets an inner layer anchored at the first entry of layer k (`InnerLayer::placement`), outside a graphics layer at that same entry: a group holding what follows the modifier and the children, drawn by reference where the modifier's content starts. It's recorded in the node's coordinates, as every inner layer, so a move of that layer records its content again (a difference: Compose moves its layer).

**Invalidation.** Adding or dropping a layer refreshes the node's boundaries (`refresh_layers`, shared with the chain's refresh), draws again what draws it, and notes the layer for the layout callbacks (`new_layers`, `note_layer_change`), as a graphics layer's update does; dropping one tells those under it (`place_readers_under`). A new block marks the group's layer (or the inner layer) to compute again.

**Coordinates.** A placement layer k transforms every place of layer k and inside it, after the chain's graphics layers there: walking out from a place before entry `b` of layer `L`, it's met after entry `start(k)` for each `k <= L`, and layer 0's last, before the node's position. `LayoutNode::placed_layer_at(index, layer)` answers that in the walks: coordinates (`walk_out`, `walk_in`, the clips), `map_all_to_root` and `map_out_of_node`, the rect manager's `map_within_node`, alignment lines, semantics' hit path and the layers' alphas.

## What's left

- The `zIndex` parameter of `place` and `placeWithLayer` (placement order and the `z_index` modifier only).
- A `GraphicsLayer` placed is a source of properties (transform, alpha, clip to bounds, shadow, compositing): the content isn't recorded into it, so another drawing can't draw it, and its color filter, blend mode and outline aren't applied.
- Hit testing ignores layers' transforms, as it does for graphics layers (`dispatch.rs`).
- `LayoutModifierNode.updateLayerBlock` (no layout modifier node keeps a layer block apart from placing).
