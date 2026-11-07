//! A resolver's animations, as Compose's `StyleAnimations.kt`: each animated property that
//! changed gets an entry, an `Animatable` run from 0 to 1 whose value lerps the value shown when
//! it started to the new one (D1585).

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use compose_animation::{Animatable, AnimationSpec};
use compose_runtime::{CoroutineScope, Job};

use super::property::{ErasedProperty, Value};
use super::resolver::{StyleProperties, StyleResolver, WeakStyleResolver};

/// The spring a style animates with by default, as `DefaultSpringSpec` (`spring()`).
pub(crate) fn default_spring_spec() -> AnimationSpec {
    compose_animation::spring().into()
}

/// The spec a write animates with: none given yet (Compose's `UnspecifiedAnimationSpec`), or
/// one.
#[derive(Clone, Default)]
pub(crate) enum SpecSlot {
    #[default]
    Unspecified,
    Spec(AnimationSpec),
}

/// Where an entry is, as `EntryState`.
#[derive(Clone, Copy, PartialEq, Debug)]
enum EntryState {
    Untouched,
    Unchanged,
    Changed,
    Inserted,
    Interrupted,
    Removing,
}

/// A property's animation, as `StyleAnimations.Entry`.
pub(crate) struct Entry {
    property: ErasedProperty,
    spec: RefCell<AnimationSpec>,
    start: RefCell<Value>,
    end: RefCell<Value>,
    animation: Animatable<f32>,
    state: Cell<EntryState>,
    job: Cell<Option<Job>>,
    /// From its task's launch until the task ends or is cancelled.
    pending: Cell<bool>,
}

impl Entry {
    /// The value shown now, as `animatedValueOrElse`: the animation's value read (observed), the
    /// start while it's interrupted (about to restart from 0).
    pub(crate) fn value(&self) -> Value {
        let fraction = self.animation.value();
        let fraction = if self.state.get() == EntryState::Interrupted { 0.0 } else { fraction };
        self.property.vtable.lerp(&self.start.borrow(), &self.end.borrow(), fraction)
    }

    /// Runs the animation from 0 to 1 again, with the velocity it had, as `animate`.
    fn animate(self: &Rc<Self>, scope: Option<CoroutineScope>, animations: Weak<AnimationsInner>) {
        if let Some(job) = self.job.take() {
            job.cancel();
        }
        let Some(scope) = scope.filter(CoroutineScope::is_active) else {
            // No scope (a test collecting without one): the end at once.
            self.animation.snap_to(1.0);
            self.state.set(EntryState::Unchanged);
            if let Some(animations) = animations.upgrade() {
                animations.cleanup();
            }
            return;
        };
        self.pending.set(true);
        let entry = self.clone();
        let job = scope.launch(async move {
            let _cleanup = Cleanup(entry.clone(), animations);
            if entry.state.get() == EntryState::Interrupted {
                entry.state.set(EntryState::Unchanged);
            }
            let velocity = entry.animation.velocity();
            entry.animation.snap_to(0.0);
            let spec = entry.spec.borrow().clone();
            entry.animation.animate_to_with_velocity(1.0, spec, velocity).await;
        });
        self.job.set(Some(job));
    }

    fn close(&self) {
        if let Some(job) = self.job.take() {
            job.cancel();
        }
    }
}

/// Removes the entries that ended once an animation's task ends, as its `finally`.
struct Cleanup(Rc<Entry>, Weak<AnimationsInner>);

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.0.pending.set(false);
        if let Some(animations) = self.1.upgrade() {
            animations.cleanup();
        }
    }
}

#[derive(Default)]
pub(crate) struct AnimationsInner {
    entries: RefCell<Vec<(u64, Rc<Entry>)>>,
    /// The resolver whose readers are told an entry ended.
    resolver: RefCell<Option<WeakStyleResolver>>,
}

impl AnimationsInner {
    /// Drops the entries neither interrupted nor running, as `cleanupAnimations`, and tells
    /// their readers.
    fn cleanup(&self) {
        let mut ended = Vec::new();
        self.entries.borrow_mut().retain(|(key, entry)| {
            let keep = entry.state.get() == EntryState::Interrupted || entry.pending.get();
            if !keep {
                ended.push(*key);
            }
            keep
        });
        if ended.is_empty() {
            return;
        }
        let resolver = self.resolver.borrow().as_ref().and_then(WeakStyleResolver::upgrade);
        if let Some(resolver) = resolver {
            resolver.publish_animations();
        }
    }
}

/// A resolver's animations, as Compose's `StyleAnimations`.
#[derive(Default)]
pub(crate) struct StyleAnimations(Rc<AnimationsInner>);

impl StyleAnimations {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.entries.borrow().is_empty()
    }

    /// Whether `key` animates, as `animating`.
    pub(crate) fn animating(&self, key: u64) -> bool {
        self.0.entries.borrow().iter().any(|(set, _)| *set == key)
    }

    /// `key`'s entry, if it animates.
    pub(crate) fn entry(&self, key: u64) -> Option<Rc<Entry>> {
        self.0.entries.borrow().iter().find(|(set, _)| *set == key).map(|(_, entry)| entry.clone())
    }

    /// Records the animations of a collection, as `recordAnimations`: each property animated
    /// now or before gets an entry if it changed (or its spec did), animating from the value
    /// shown now to the new one; an entry no longer animated is left to end.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_animations(
        &self,
        animating: &[u64],
        changes: &[u64],
        to_specs: &[(u64, AnimationSpec)],
        from_specs: &[(u64, AnimationSpec)],
        previous_from_specs: &[(u64, AnimationSpec)],
        start_values: &StyleProperties,
        end_values: &StyleProperties,
        resolver: &StyleResolver,
        scope: Option<CoroutineScope>,
    ) {
        self.bind(resolver);
        let find = |specs: &[(u64, AnimationSpec)], key: u64| {
            specs.iter().find(|(set, _)| *set == key).map(|(_, spec)| spec.clone())
        };
        for entry in self.0.entries.borrow().iter() {
            if matches!(
                entry.1.state.get(),
                EntryState::Inserted | EntryState::Unchanged | EntryState::Changed
            ) {
                entry.1.state.set(EntryState::Untouched);
            }
        }
        for &key in animating {
            let spec = find(to_specs, key)
                .or_else(|| find(previous_from_specs, key))
                .or_else(|| find(from_specs, key))
                .unwrap_or_else(default_spring_spec);
            let Some(property) = start_values
                .iter()
                .chain(end_values.iter())
                .find(|(property, _)| property.key == key)
                .map(|(property, _)| *property)
            else {
                continue;
            };
            let value_in = |values: &StyleProperties| {
                values.get(key).cloned().unwrap_or_else(|| property.vtable.default_value())
            };
            let changed = changes.contains(&key);
            let existing = self.entry(key);
            match existing {
                Some(entry) => {
                    if changed || *entry.spec.borrow() != spec {
                        let shown =
                            compose_runtime::snapshots::Snapshot::without_read_observation(|| {
                                entry.value()
                            });
                        *entry.spec.borrow_mut() = spec;
                        entry.state.set(EntryState::Changed);
                        *entry.start.borrow_mut() = shown;
                        *entry.end.borrow_mut() = value_in(end_values);
                    } else {
                        entry.state.set(EntryState::Unchanged);
                    }
                }
                None => {
                    if changed {
                        let entry = Rc::new(Entry {
                            property,
                            spec: RefCell::new(spec),
                            start: RefCell::new(value_in(start_values)),
                            end: RefCell::new(value_in(end_values)),
                            animation: Animatable::new(0.0),
                            state: Cell::new(EntryState::Inserted),
                            job: Cell::new(None),
                            pending: Cell::new(false),
                        });
                        self.0.entries.borrow_mut().push((key, entry));
                    }
                }
            }
        }
        let entries: Vec<Rc<Entry>> =
            self.0.entries.borrow().iter().map(|(_, entry)| entry.clone()).collect();
        for entry in entries {
            match entry.state.get() {
                EntryState::Inserted => entry.animate(scope, Rc::downgrade(&self.0)),
                EntryState::Untouched => entry.state.set(EntryState::Removing),
                EntryState::Changed => {
                    entry.state.set(EntryState::Interrupted);
                    entry.animate(scope, Rc::downgrade(&self.0));
                }
                _ => {}
            }
        }
    }

    /// Tells `resolver`'s readers when an entry ends.
    fn bind(&self, resolver: &StyleResolver) {
        let mut bound = self.0.resolver.borrow_mut();
        if bound.is_none() {
            *bound = Some(resolver.downgrade());
        }
    }

    /// Stops every animation, as `close`.
    pub(crate) fn close(&self) {
        let entries = std::mem::take(&mut *self.0.entries.borrow_mut());
        for (_, entry) in entries {
            entry.close();
        }
    }
}
