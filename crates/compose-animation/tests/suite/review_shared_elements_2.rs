//! Review of `shared-elements-2`: what `testNoAdditionalPlacementWhenNoMatch`'s ignore (D1336)
//! hides, with a control that is in a lookahead scope too.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::rc::Rc;

use compose_animation::*;
use compose_foundation::layout::*;
use compose_foundation::{LazyColumn, LazyListScope, LazyListState, ScrollableStateExt};
use compose_runtime::*;
use compose_test::*;
use compose_ui::{Color, IntSize, LayoutCoordinates, UnitExt};

/// A lazy column of 20 items of 10 × 10 in a `SharedTransitionLayout` under an
/// `AnimatedVisibility`, each counting its placements into `counts`; `shared` makes each a shared
/// element with no match.
fn list(state: LazyListState, counts: Rc<RefCell<Vec<u32>>>, shared: bool) {
    AnimatedVisibility!(true, move |visibility: AnimatedVisibilityScope| {
        let counts = counts.clone();
        SharedTransitionLayout!(move |scope: SharedTransitionScope| {
            let (counts, scope, visibility) = (counts.clone(), scope.clone(), visibility.clone());
            LazyColumn!(
                modifier = Modifier.size(100.dp()),
                state = state,
                move |list: LazyListScope| {
                    for id in 0..20usize {
                        let (counts, scope, visibility) =
                            (counts.clone(), scope.clone(), visibility.clone());
                        list.item_keyed(id, move || {
                            let state = scope.remember_shared_content_state(id.to_string());
                            let counts = counts.clone();
                            let shared = if shared {
                                scope.shared_element(&state, &visibility)
                            } else {
                                Modifier
                            };
                            Box!(
                                modifier = shared
                                    .on_placed(move |_: LayoutCoordinates| {
                                        counts.borrow_mut()[id] += 1;
                                    })
                                    .size(10.dp())
                                    .background(Color::BLACK)
                            );
                        });
                    }
                }
            );
        });
    });
}

/// `testNoAdditionalPlacementWhenNoMatch` with its control list in a `SharedTransitionLayout` too:
/// the shared elements with no match place their items as often as plain items in the same
/// lookahead scope. Compose-ui's missing `on_placed` for a moved lazy item affects both lists
/// alike, so a difference here is the shared elements'.
#[test]
fn shared_elements_with_no_match_place_as_plain_items_in_a_lookahead_scope() {
    let state = LazyListState::new(0, 0);
    let control = LazyListState::new(0, 0);
    let placements = Rc::new(RefCell::new(vec![0u32; 20]));
    let control_placements = Rc::new(RefCell::new(vec![0u32; 20]));
    let (s, c, p, cp) = (*state, *control, placements.clone(), control_placements.clone());
    let mut ui = ComposeTestRule::sized(IntSize::new(1000, 1000), move || {
        let (p, cp) = (p.clone(), cp.clone());
        Row!(move |_| {
            list(s, p.clone(), true);
            list(c, cp.clone(), false);
        });
    });
    ui.wait_for_idle();
    for _ in 0..20 {
        let scope = CoroutineScope::new();
        let (s, c) = (*state, *control);
        scope.launch(async move {
            let _ = s.scroll_by(5.0).await;
            let _ = c.scroll_by(5.0).await;
        });
        ui.wait_for_idle();
    }
    assert_eq!(*control_placements.borrow(), *placements.borrow());
}
