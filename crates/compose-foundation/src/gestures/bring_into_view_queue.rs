//! The bring-into-view requests a scrolling container is showing, as Compose's internal
//! `BringIntoViewRequestPriorityQueue`: sorted so each request's bounds contain the next one's,
//! the largest first. A request that neither contains nor is contained by those already there
//! interrupts them: they're cancelled.
//!
//! Each request's bounds are read with [`Request::refresh`], outside the queue (reading them
//! runs a responder's `calculate_rect_for_parent`, which may be application code), and the queue
//! decides from the bounds last read.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use compose_ui::Rect;

/// How a request ended, if it has.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Outcome {
    Active,
    /// Shown: its requester goes on.
    Resumed,
    /// Interrupted, or withdrawn by its requester.
    Cancelled,
}

/// The requester's side of a request, as Compose's `CancellableContinuation`: resumed or
/// cancelled once, then over. Its requester waits on [`Continuation::wait`].
#[derive(Default)]
pub(crate) struct Continuation {
    outcome: Cell<Option<Outcome>>,
    waker: RefCell<Option<Waker>>,
}

impl Continuation {
    pub(crate) fn outcome(&self) -> Outcome {
        self.outcome.get().unwrap_or(Outcome::Active)
    }

    pub(crate) fn is_active(&self) -> bool {
        self.outcome() == Outcome::Active
    }

    fn end(&self, outcome: Outcome) {
        if !self.is_active() {
            return;
        }
        self.outcome.set(Some(outcome));
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }

    pub(crate) fn resume(&self) {
        self.end(Outcome::Resumed);
    }

    pub(crate) fn cancel(&self) {
        self.end(Outcome::Cancelled);
    }

    /// Waits for it to end; dropped before, it's cancelled (the requester went), as a coroutine's
    /// cancellation cancels its continuation.
    pub(crate) fn wait(self: Rc<Self>) -> Wait {
        Wait(self)
    }
}

pub(crate) struct Wait(Rc<Continuation>);

impl Future for Wait {
    type Output = Outcome;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Outcome> {
        match self.0.outcome() {
            Outcome::Active => {
                *self.0.waker.borrow_mut() = Some(context.waker().clone());
                Poll::Pending
            }
            outcome => Poll::Ready(outcome),
        }
    }
}

impl Drop for Wait {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// How a request's bounds are read: what to show now, `Some(None)` once detached, `None` when it
/// can't be told now (the tree is busy).
pub(crate) type ReadBounds = Rc<dyn Fn() -> Option<Option<Rect>>>;

/// Reads in a row that can't tell (the tree busy) after which a request is taken for one whose
/// requester went: a tree busy at every read would otherwise keep its stale bounds forever. A
/// read at a frame's end finds the tree free, so a few frames' worth.
pub(crate) const MOST_BUSY_READS: u32 = 8;

/// A request: how to read what to show, what it last read, and its requester's side.
#[derive(Clone)]
pub(crate) struct Request {
    read: ReadBounds,
    bounds: Rc<Cell<Option<Rect>>>,
    /// The reads in a row that couldn't tell.
    busy_reads: Rc<Cell<u32>>,
    pub(crate) continuation: Rc<Continuation>,
}

impl Request {
    /// A request reading its bounds with `read`, `bounds` until it's first read.
    pub(crate) fn new(read: ReadBounds, bounds: Option<Rect>) -> Request {
        Request {
            read,
            bounds: Rc::new(Cell::new(bounds)),
            busy_reads: Rc::default(),
            continuation: Rc::default(),
        }
    }

    /// Reads its bounds again; when they can't be told now, they stay what they were, until
    /// [`MOST_BUSY_READS`] reads in a row couldn't: then it's taken for gone. Returns whether
    /// the read told.
    pub(crate) fn refresh(&self) -> bool {
        match (self.read)() {
            Some(bounds) => {
                self.busy_reads.set(0);
                self.bounds.set(bounds);
                true
            }
            None => {
                let busy_reads = self.busy_reads.get() + 1;
                self.busy_reads.set(busy_reads);
                if busy_reads >= MOST_BUSY_READS {
                    self.bounds.set(None);
                }
                false
            }
        }
    }

    /// Its bounds as last read: `None` once detached.
    pub(crate) fn bounds(&self) -> Option<Rect> {
        self.bounds.get()
    }

    /// Whether it's `other` (clones are).
    pub(crate) fn is(&self, other: &Request) -> bool {
        Rc::ptr_eq(&self.continuation, &other.continuation)
    }
}

/// Where two rects overlap, as `Rect.intersect` (inverted when they don't).
fn intersect(a: Rect, b: Rect) -> Rect {
    Rect::new(a.left.max(b.left), a.top.max(b.top), a.right.min(b.right), a.bottom.min(b.bottom))
}

/// The requests, the largest first, each containing the next, as
/// `BringIntoViewRequestPriorityQueue`. One cancelled from outside (its requester went) is
/// forgotten before the queue is next used, as Compose's cancellation handler removes it.
#[derive(Default)]
pub(crate) struct BringIntoViewRequestPriorityQueue {
    requests: Vec<Request>,
}

impl BringIntoViewRequestPriorityQueue {
    /// Forgets the requests whose requester went.
    fn forget_cancelled(&mut self) {
        self.requests.retain(|request| request.continuation.is_active());
    }

    /// The requests, the largest first, to read their bounds outside the queue.
    pub(crate) fn snapshot(&mut self) -> Vec<Request> {
        self.forget_cancelled();
        self.requests.clone()
    }

    #[cfg(test)]
    pub(crate) fn size(&mut self) -> usize {
        self.forget_cancelled();
        self.requests.len()
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&mut self) -> bool {
        self.size() == 0
    }

    /// Adds `request` where it keeps the order, cancelling the requests it neither contains nor
    /// is contained by, as `enqueue`, all by the bounds last read. A request with no bounds is
    /// resumed at once and not added: returns whether it was. Once added, it's resumed or
    /// cancelled before it leaves.
    pub(crate) fn enqueue(&mut self, request: Request) -> bool {
        self.forget_cancelled();
        let Some(bounds) = request.bounds() else {
            request.continuation.resume();
            return false;
        };
        let mut index = self.requests.len();
        while index > 0 {
            index -= 1;
            let Some(other) = self.requests[index].bounds() else { continue };
            let overlap = intersect(bounds, other);
            if overlap == bounds {
                // It contains the new one: after it.
                self.requests.insert(index + 1, request);
                return true;
            }
            if overlap != other {
                // Neither contains the other: the new one interrupts it. (Compose means to
                // cancel it and those after it, but its loop only cancels it when it's the last;
                // those after it here are inside the new one, so they're kept, in order.)
                let interrupted = self.requests.remove(index);
                interrupted.continuation.cancel();
            }
            // Otherwise the new one contains it: further up.
        }
        // It contains every request left: the first.
        self.requests.insert(0, request);
        true
    }

    /// Calls `block` with each request's bounds as last read, the smallest first, as
    /// `forEachFromSmallest`.
    pub(crate) fn for_each_from_smallest(&mut self, mut block: impl FnMut(Option<Rect>) -> bool) {
        self.forget_cancelled();
        for request in self.requests.iter().rev() {
            if !block(request.bounds()) {
                return;
            }
        }
    }

    /// Resumes and removes every request, as `resumeAndRemoveAll`.
    pub(crate) fn resume_and_remove_all(&mut self) {
        for request in std::mem::take(&mut self.requests) {
            request.continuation.resume();
        }
    }

    /// Resumes and removes the smallest request while `block` says so of it, as
    /// `resumeAndRemoveWhile`.
    pub(crate) fn resume_and_remove_while(&mut self, mut block: impl FnMut(&Request) -> bool) {
        self.forget_cancelled();
        while let Some(request) = self.requests.last() {
            if !block(request) {
                return;
            }
            if let Some(request) = self.requests.pop() {
                request.continuation.resume();
            }
        }
    }

    /// Cancels and removes every request, as `cancelAndRemoveAll`.
    pub(crate) fn cancel_and_remove_all(&mut self) {
        for request in std::mem::take(&mut self.requests) {
            request.continuation.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    //! Compose's `BringIntoViewRequestPriorityQueueTest`, rewritten. Its two `toString` tests
    //! check Kotlin's debugging strings, which compose-rs's requests don't have.

    use super::*;

    /// A request whose bounds are `bounds`, read.
    fn at(bounds: Option<Rect>) -> Request {
        let request = Request::new(Rc::new(move || Some(bounds)), None);
        request.refresh();
        request
    }

    fn request(left: f32, top: f32, size: f32) -> Request {
        at(Some(Rect::new(left, top, left + size, top + size)))
    }

    fn large() -> Request {
        request(0.0, 0.0, 10.0)
    }

    fn medium() -> Request {
        request(0.0, 0.0, 5.0)
    }

    fn other_medium() -> Request {
        request(5.0, 5.0, 5.0)
    }

    fn small() -> Request {
        request(0.0, 0.0, 1.0)
    }

    /// The queue's bounds, the largest first.
    fn bounds(queue: &mut BringIntoViewRequestPriorityQueue) -> Vec<Option<Rect>> {
        let mut all = Vec::new();
        queue.for_each_from_smallest(|bounds| {
            all.push(bounds);
            true
        });
        all.reverse();
        all
    }

    fn of(requests: &[&Request]) -> Vec<Option<Rect>> {
        requests.iter().map(|request| request.bounds()).collect()
    }

    fn outcomes(requests: &[&Request]) -> Vec<Outcome> {
        requests.iter().map(|request| request.continuation.outcome()).collect()
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, empty
    #[test]
    fn a_new_queue_is_empty() {
        let mut queue = BringIntoViewRequestPriorityQueue::default();
        assert_eq!((queue.size(), queue.is_empty()), (0, true));
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, enqueueSingleRequest_nullBounds
    #[test]
    fn a_request_without_bounds_is_resumed_and_not_kept() {
        let mut queue = BringIntoViewRequestPriorityQueue::default();
        let request = at(None);
        assert!(!queue.enqueue(request.clone()));
        assert!(queue.is_empty());
        assert_eq!(request.continuation.outcome(), Outcome::Resumed);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, enqueueSingleRequest
    #[test]
    fn a_request_is_kept_active() {
        let mut queue = BringIntoViewRequestPriorityQueue::default();
        let small = small();
        assert!(queue.enqueue(small.clone()));
        assert_eq!((queue.size(), queue.is_empty()), (1, false));
        assert_eq!(bounds(&mut queue), of(&[&small]));
        assert!(small.continuation.is_active());
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, enqueueOverlappingRequests_largeToSmall_isSorted
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, enqueueOverlappingRequests_smallToLarge_isSorted
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, enqueueOverlappingRequests_outOfOrder_isSorted
    #[test]
    fn requests_inside_each_other_are_kept_largest_first_whatever_their_order() {
        for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
            let (large, medium, small) = (large(), medium(), small());
            let given = [&large, &medium, &small];
            let mut queue = BringIntoViewRequestPriorityQueue::default();
            for index in order {
                queue.enqueue(given[index].clone());
            }
            assert_eq!(bounds(&mut queue), of(&given));
            assert_eq!(outcomes(&given), [Outcome::Active; 3]);
        }
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, enqueueNonOverlappingRequest
    #[test]
    fn a_request_beside_others_cancels_them_but_keeps_those_containing_it() {
        let (large, medium, other_medium, small) = (large(), medium(), other_medium(), small());
        let mut queue = BringIntoViewRequestPriorityQueue::default();
        for request in [&small, &medium, &large, &other_medium] {
            queue.enqueue(request.clone());
        }
        assert_eq!(bounds(&mut queue), of(&[&large, &other_medium]));
        assert_eq!(outcomes(&[&large, &other_medium]), [Outcome::Active; 2]);
        assert_eq!(outcomes(&[&medium, &small]), [Outcome::Cancelled; 2]);
    }

    // Compose's loop meant to keep the order this way: a request beside one but containing those
    // inside it keeps them, after it.
    #[test]
    fn a_request_beside_one_keeps_those_inside_it_that_it_contains() {
        let (large, medium, small) = (large(), medium(), small());
        // Contains `small`, beside `medium`, inside `large`.
        let between = at(Some(Rect::new(0.0, 0.0, 6.0, 1.0)));
        let mut queue = BringIntoViewRequestPriorityQueue::default();
        for request in [&large, &medium, &small, &between] {
            queue.enqueue(request.clone());
        }
        assert_eq!(bounds(&mut queue), of(&[&large, &between, &small]));
        assert_eq!(medium.continuation.outcome(), Outcome::Cancelled);
    }

    // compose-rs's own: a read that can't tell (the tree busy, read from inside a read of it)
    // keeps the bounds read before: the request isn't taken for one whose requester went.
    #[test]
    fn a_read_that_cant_tell_keeps_the_bounds_read_before() {
        let busy = Rc::new(Cell::new(false));
        let reads = busy.clone();
        let rect = Rect::new(0.0, 0.0, 1.0, 1.0);
        let request =
            Request::new(Rc::new(move || if reads.get() { None } else { Some(Some(rect)) }), None);
        request.refresh();
        busy.set(true);
        request.refresh();
        assert_eq!(request.bounds(), Some(rect));
        let mut queue = BringIntoViewRequestPriorityQueue::default();
        assert!(queue.enqueue(request.clone()));
        queue.resume_and_remove_while(|request| request.bounds().is_none());
        assert!(request.continuation.is_active());
    }

    // compose-rs's own: a request whose reads can't tell, read after read, is taken for one whose
    // requester went once they reach the cap, rather than keeping stale bounds forever; a read
    // that tells starts the count again.
    #[test]
    fn reads_that_cant_tell_past_the_cap_are_a_requester_gone() {
        let busy = Rc::new(Cell::new(false));
        let reads = busy.clone();
        let rect = Rect::new(0.0, 0.0, 1.0, 1.0);
        let request =
            Request::new(Rc::new(move || if reads.get() { None } else { Some(Some(rect)) }), None);
        request.refresh();
        busy.set(true);
        for _ in 1..MOST_BUSY_READS {
            request.refresh();
        }
        assert_eq!(request.bounds(), Some(rect), "under the cap");
        busy.set(false);
        request.refresh();
        busy.set(true);
        for _ in 1..MOST_BUSY_READS {
            request.refresh();
        }
        assert_eq!(request.bounds(), Some(rect), "counted again after a read that told");
        request.refresh();
        assert_eq!(request.bounds(), None, "at the cap");
        let mut queue = BringIntoViewRequestPriorityQueue::default();
        assert!(!queue.enqueue(request.clone()));
        assert_eq!(request.continuation.outcome(), Outcome::Resumed);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, enqueuedRequest_isRemoved_whenCancelled
    #[test]
    fn a_request_cancelled_by_its_requester_leaves() {
        let mut queue = BringIntoViewRequestPriorityQueue::default();
        let request = at(Some(Rect::new(0.0, 0.0, 0.0, 0.0)));
        queue.enqueue(request.clone());
        // Its requester's wait, dropped.
        drop(request.continuation.clone().wait());
        assert!(queue.is_empty());
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, removeAll_cancelsAllContinuations
    #[test]
    fn cancelling_all_cancels_each() {
        let (large, medium, small) = (large(), medium(), small());
        let mut queue = BringIntoViewRequestPriorityQueue::default();
        for request in [&small, &medium, &large] {
            queue.enqueue(request.clone());
        }
        queue.cancel_and_remove_all();
        assert!(queue.is_empty());
        assert_eq!(outcomes(&[&small, &medium, &large]), [Outcome::Cancelled; 3]);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, resumeAndRemoveAll
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, resumeAndRemoveWhile_whenAlwaysTrue
    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, resumeAndRemoveWhile_whenAlwaysFalse
    #[test]
    fn resuming_while_asked_resumes_from_the_smallest() {
        for resume in [true, false] {
            let (large, medium, small) = (large(), medium(), small());
            let mut queue = BringIntoViewRequestPriorityQueue::default();
            for request in [&small, &medium, &large] {
                queue.enqueue(request.clone());
            }
            queue.resume_and_remove_while(|_| resume);
            let expected = if resume { Outcome::Resumed } else { Outcome::Active };
            assert_eq!(queue.size(), if resume { 0 } else { 3 });
            assert_eq!(outcomes(&[&small, &medium, &large]), [expected; 3]);
        }
        let (large, medium, small) = (large(), medium(), small());
        let mut queue = BringIntoViewRequestPriorityQueue::default();
        for request in [&small, &medium, &large] {
            queue.enqueue(request.clone());
        }
        queue.resume_and_remove_all();
        assert!(queue.is_empty());
        assert_eq!(outcomes(&[&small, &medium, &large]), [Outcome::Resumed; 3]);
    }

    // Compose: compose/foundation/foundation/src/androidHostTest/kotlin/androidx/compose/foundation/gestures/BringIntoViewRequestPriorityQueueTest.kt, resumeAndRemoveWhile_removeSingleItem
    #[test]
    fn resuming_while_asked_stops_at_the_first_refused() {
        let (large, medium, small) = (large(), medium(), small());
        let mut queue = BringIntoViewRequestPriorityQueue::default();
        for request in [&small, &medium, &large] {
            queue.enqueue(request.clone());
        }
        queue.resume_and_remove_while(|request| request.is(&small));
        assert_eq!(bounds(&mut queue), of(&[&large, &medium]));
        assert_eq!(small.continuation.outcome(), Outcome::Resumed);
        assert_eq!(outcomes(&[&large, &medium]), [Outcome::Active; 2]);
    }
}
