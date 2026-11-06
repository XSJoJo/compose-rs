//! The building blocks of lazy layouts, as Compose's `androidx.compose.foundation.lazy.layout`:
//! what lists, grids and staggered grids are made of, public so an app can make its own.

use std::cell::Cell;

pub use super::animate_scroll::LazyLayoutScrollScope;
pub use super::cache_window::LazyLayoutCacheWindow;

/// Runs of items, each holding a value, found by an item's index, as Compose's `IntervalList`:
/// what a lazy layout's content declares (`item`, `items(count)`) is one run each.
pub trait IntervalList<T> {
    /// The items in all the runs.
    fn size(&self) -> usize;

    /// The run holding item `index`. Panics if `index` isn't below `size()`, as Compose's throws
    /// an `IndexOutOfBoundsException`.
    fn get(&self, index: usize) -> &Interval<T>;

    /// Calls `block` with each run holding an item from `from_index` to `to_index`, both
    /// included, in order. Panics if either is out of bounds, or `to_index` is before
    /// `from_index`.
    fn for_each_in(&self, from_index: usize, to_index: usize, block: &mut dyn FnMut(&Interval<T>));

    /// Calls `block` with each run, in order: Compose's `forEach` with its default bounds.
    fn for_each(&self, block: &mut dyn FnMut(&Interval<T>)) {
        if let Some(last) = self.size().checked_sub(1) {
            self.for_each_in(0, last, block);
        }
    }
}

/// One run of an [`IntervalList`]: its first item's index, its number of items and its value, as
/// Compose's `IntervalList.Interval`.
#[derive(Clone, Debug, PartialEq)]
pub struct Interval<T> {
    start_index: usize,
    size: usize,
    value: T,
}

impl<T> Interval<T> {
    /// The index of the run's first item.
    pub fn start_index(&self) -> usize {
        self.start_index
    }

    /// The run's number of items, more than none.
    pub fn size(&self) -> usize {
        self.size
    }

    /// The run's value.
    pub fn value(&self) -> &T {
        &self.value
    }

    fn contains(&self, index: usize) -> bool {
        self.start_index <= index && index - self.start_index < self.size
    }
}

/// An [`IntervalList`] runs are added to, as Compose's `MutableIntervalList`: a run is found by
/// a binary search over the runs' first items, the run found last looked at first (a walk over
/// the items stays in one for a while).
#[derive(Debug)]
pub struct MutableIntervalList<T> {
    intervals: Vec<Interval<T>>,
    size: usize,
    /// The run found last, by its place in `intervals`, as Compose's `lastInterval`.
    last_found: Cell<usize>,
}

impl<T> Default for MutableIntervalList<T> {
    fn default() -> Self {
        MutableIntervalList { intervals: Vec::new(), size: 0, last_found: Cell::new(0) }
    }
}

impl<T> MutableIntervalList<T> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a run of `size` items holding `value` after the others. A run of no items isn't
    /// added, as Compose's.
    pub fn add_interval(&mut self, size: usize, value: T) {
        if size == 0 {
            return;
        }
        self.intervals.push(Interval { start_index: self.size, size, value });
        self.size += size;
    }

    /// The runs, in order.
    pub fn intervals(&self) -> &[Interval<T>] {
        &self.intervals
    }

    /// The place in `intervals` of the run holding item `index`, which is in bounds.
    fn position_of(&self, index: usize) -> usize {
        let last = self.last_found.get();
        if self.intervals.get(last).is_some_and(|interval| interval.contains(index)) {
            return last;
        }
        let found = self.intervals.partition_point(|interval| interval.start_index <= index) - 1;
        self.last_found.set(found);
        found
    }

    fn check_index_bounds(&self, index: usize) {
        if index >= self.size {
            panic!("compose-foundation: index {index} out of bounds, size {}", self.size);
        }
    }
}

impl<T> IntervalList<T> for MutableIntervalList<T> {
    fn size(&self) -> usize {
        self.size
    }

    fn get(&self, index: usize) -> &Interval<T> {
        self.check_index_bounds(index);
        &self.intervals[self.position_of(index)]
    }

    fn for_each_in(&self, from_index: usize, to_index: usize, block: &mut dyn FnMut(&Interval<T>)) {
        self.check_index_bounds(from_index);
        self.check_index_bounds(to_index);
        assert!(
            to_index >= from_index,
            "compose-foundation: to_index ({to_index}) should be not smaller than from_index \
             ({from_index})"
        );
        let first =
            self.intervals.partition_point(|interval| interval.start_index <= from_index) - 1;
        for interval in &self.intervals[first..] {
            if interval.start_index > to_index {
                break;
            }
            block(interval);
        }
    }
}

#[cfg(test)]
mod interval_list_tests {
    //! Compose's `MutableIntervalListTest`, rewritten. Kotlin's negative indices are `usize::MAX`
    //! here, what `-1` wraps to: out of bounds all the same.

    use super::{Interval, IntervalList, MutableIntervalList};

    fn panics(f: impl FnOnce()) -> bool {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).is_err()
    }

    fn five_single() -> MutableIntervalList<i32> {
        let mut list = MutableIntervalList::new();
        for value in [10, 20, 30, 40, 50] {
            list.add_interval(1, value);
        }
        list
    }

    fn five_variable() -> MutableIntervalList<i32> {
        let mut list = MutableIntervalList::new();
        for (size, value) in [(3, 10), (1, 20), (7, 30), (1, 40), (11, 50)] {
            list.add_interval(size, value);
        }
        list
    }

    fn found(list: &MutableIntervalList<i32>, index: usize) -> (i32, usize, usize) {
        let interval = list.get(index);
        (*interval.value(), interval.size(), interval.start_index())
    }

    fn starts(list: &MutableIntervalList<i32>, from: usize, to: usize) -> Vec<usize> {
        let mut starts = Vec::new();
        list.for_each_in(from, to, &mut |interval| starts.push(interval.start_index()));
        starts
    }

    // Compose: MutableIntervalListTest.addOneItem_searchInterval
    #[test]
    fn one_run_of_one_item_is_found_at_its_index() {
        let mut list = MutableIntervalList::new();
        list.add_interval(1, 10);
        assert_eq!(found(&list, 0), (10, 1, 0));
    }

    // Compose: MutableIntervalListTest.addOneItem_searchIndexOutOfBounds
    #[test]
    fn an_index_past_the_items_panics() {
        let mut list = MutableIntervalList::new();
        list.add_interval(1, 10);
        assert!(panics(|| {
            list.get(2);
        }));
    }

    // Compose: MutableIntervalListTest.addOneItem_searchIndexOutOfBoundsWithNegativeValue
    #[test]
    fn an_index_wrapped_below_zero_panics() {
        let mut list = MutableIntervalList::new();
        list.add_interval(1, 10);
        assert!(panics(|| {
            list.get(usize::MAX);
        }));
    }

    // Compose: MutableIntervalListTest.addSingleItems_searchFirstInterval
    #[test]
    fn single_items_the_first_is_found() {
        assert_eq!(found(&five_single(), 0), (10, 1, 0));
    }

    // Compose: MutableIntervalListTest.addSingleItems_searchLastInterval
    #[test]
    fn single_items_the_last_is_found() {
        assert_eq!(found(&five_single(), 4), (50, 1, 4));
    }

    // Compose: MutableIntervalListTest.addSingleItems_searchMidInterval
    #[test]
    fn single_items_the_middle_one_is_found() {
        assert_eq!(found(&five_single(), 2), (30, 1, 2));
    }

    // Compose: MutableIntervalListTest.addVariableItems_searchFirstInterval
    #[test]
    fn runs_of_several_items_the_first_is_found_at_its_start() {
        assert_eq!(found(&five_variable(), 0), (10, 3, 0));
    }

    // Compose: MutableIntervalListTest.addVariableItems_searchLastInterval
    #[test]
    fn runs_of_several_items_the_last_is_found_at_its_end() {
        assert_eq!(found(&five_variable(), 22), (50, 11, 12));
    }

    // Compose: MutableIntervalListTest.addVariableItems_searchMidInterval
    #[test]
    fn runs_of_several_items_one_is_found_inside_it() {
        assert_eq!(found(&five_variable(), 6), (30, 7, 4));
    }

    // Compose: MutableIntervalListTest.addVariableItems_searchSecondInterval
    #[test]
    fn runs_of_several_items_a_run_of_one_after_one_of_three() {
        assert_eq!(found(&five_variable(), 3), (20, 1, 3));
    }

    // Compose: MutableIntervalListTest.addVariableItems_searchIndexOutOfBounds
    #[test]
    fn runs_of_several_items_the_index_after_the_last_panics() {
        let list = five_variable();
        assert!(panics(|| {
            list.get(23);
        }));
    }

    // Compose: MutableIntervalListTest.forEach_withoutParamsIterateThroughAll
    #[test]
    fn for_each_visits_every_run_in_order() {
        let list = five_single();
        let mut intervals: Vec<Interval<i32>> = Vec::new();
        list.for_each(&mut |interval| intervals.push(interval.clone()));
        let starts: Vec<usize> = intervals.iter().map(Interval::start_index).collect();
        let sizes: Vec<usize> = intervals.iter().map(Interval::size).collect();
        let values: Vec<i32> = intervals.iter().map(|interval| *interval.value()).collect();
        assert_eq!(starts, [0, 1, 2, 3, 4]);
        assert_eq!(sizes, [1, 1, 1, 1, 1]);
        assert_eq!(values, [10, 20, 30, 40, 50]);
    }

    // Compose: MutableIntervalListTest.forEach_withStartInterval
    #[test]
    fn for_each_from_an_index_visits_the_runs_from_its_own() {
        assert_eq!(starts(&five_single(), 2, 4), [2, 3, 4]);
    }

    // Compose: MutableIntervalListTest.forEach_withStartAndEndInterval
    #[test]
    fn for_each_between_two_indices_visits_the_runs_holding_them_and_between() {
        assert_eq!(starts(&five_single(), 2, 3), [2, 3]);
    }

    // Compose: MutableIntervalListTest.forEach_withTheSameStartAndEndInterval
    #[test]
    fn for_each_from_an_index_to_itself_visits_its_run() {
        assert_eq!(starts(&five_single(), 2, 2), [2]);
    }

    // Compose: MutableIntervalListTest.forEach_fromIndexIsInTheMiddleOfInterval
    #[test]
    fn for_each_from_inside_a_run_visits_that_run_first() {
        let mut list = MutableIntervalList::new();
        list.add_interval(10, 0);
        list.add_interval(1, 1);
        list.add_interval(1, 2);
        assert_eq!(starts(&list, 5, 10), [0, 10]);
    }

    // Compose: MutableIntervalListTest.forEach_startLargerThanEndThrows
    #[test]
    #[should_panic(expected = "should be not smaller than")]
    fn for_each_ending_before_it_starts_panics() {
        five_single().for_each_in(3, 2, &mut |_| {});
    }

    // Compose: MutableIntervalListTest.forEach_outOfBounds
    #[test]
    fn for_each_from_or_to_an_index_out_of_bounds_panics() {
        let list = five_single();
        assert!(panics(|| list.for_each_in(usize::MAX, 4, &mut |_| {})));
        assert!(panics(|| list.for_each_in(0, usize::MAX, &mut |_| {})));
        assert!(panics(|| list.for_each_in(6, 4, &mut |_| {})));
        assert!(panics(|| list.for_each_in(0, 6, &mut |_| {})));
    }

    // Compose: MutableIntervalListTest.forEach_MAX_VALUE_size_andFromIndexIsInTheMiddle
    #[test]
    fn for_each_inside_a_run_of_the_most_items_visits_it() {
        let mut list = MutableIntervalList::new();
        let most = i32::MAX as usize;
        list.add_interval(most, 0);
        assert_eq!(starts(&list, most / 2, most - 1), [0]);
    }

    #[test]
    fn a_run_of_no_items_is_not_added() {
        let mut list = MutableIntervalList::new();
        list.add_interval(0, 1);
        list.add_interval(2, 2);
        assert_eq!(list.size(), 2);
        assert_eq!(list.intervals().len(), 1);
        assert_eq!(found(&list, 1), (2, 2, 0));
    }
}
