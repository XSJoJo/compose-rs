//! The composables a panic unwound through, as Compose's diagnostic stack traces
//! (`tooling.ComposeStackTraceMode`, D1507).
//!
//! When a panic unwinds through a pass, the pass's open groups say where it was: the trace is made
//! then, from them, innermost first, and a pass nested in another adds the outer pass's when the
//! panic reaches it. A Rust panic's payload can't carry it (Compose attaches a suppressed
//! exception): [`take_compose_stack_trace`] gives the last one made on the thread, which belongs
//! to the last panic that unwound a pass, not necessarily to the panic a caller just caught (one
//! outside any pass makes none); [`compose_stack_trace_sequence`], read before, tells them apart. A
//! pass that doesn't panic does nothing for it.
//!
//! Frames name the composables `#[composable]` made, by their function's path, or give their
//! groups' keys ([`ComposeStackTraceMode::GroupKeys`]). Compose's frames carry a file and line from
//! the compiler's source information; compose-rs's groups keep none, and a location per group
//! would cost every pass.

use std::fmt;

use crate::composer::Identity;
use crate::runtime::{CompositionId, Runtime, with_runtime};
use crate::slot_table::SENTINEL;

/// What a diagnostic stack trace says, as Compose's `ComposeStackTraceMode`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ComposeStackTraceMode {
    /// No trace is made.
    None,
    /// Each group on the path, by its key.
    GroupKeys,
    /// Each composable on the path, by name.
    SourceInformation,
    /// As `SourceInformation`: compose-rs always has the names.
    #[default]
    Auto,
}

/// Sets what the traces made on this thread say, as `Composer.setDiagnosticStackTraceMode`.
pub fn set_diagnostic_stack_trace_mode(mode: ComposeStackTraceMode) {
    with_runtime(|runtime| runtime.stack_trace_mode = mode);
}

/// The last trace made on this thread, taken: the composables that the last panic to unwind a pass
/// went through, if it was traced.
///
/// It belongs to the last panic that unwound a pass, which needn't be the one just caught: a panic
/// outside any pass makes no trace and leaves the last one there. A caller reads
/// [`compose_stack_trace_sequence`] before the code it runs and keeps a trace whose
/// [`sequence`](ComposeStackTrace::sequence) is above it. With the mode `None`, a panic through a
/// pass clears it.
pub fn take_compose_stack_trace() -> Option<ComposeStackTrace> {
    with_runtime(|runtime| runtime.stack_trace.take()).map(|pending| pending.trace)
}

/// How many traces were begun on this thread: the last one's
/// [`sequence`](ComposeStackTrace::sequence). A trace made by a panic in code run after reading it
/// has a sequence above it (as has one made by a panic that code caught itself).
pub fn compose_stack_trace_sequence() -> u64 {
    with_runtime(|runtime| runtime.stack_trace_sequence)
}

/// The composables a panic unwound through, innermost first, as Compose's
/// `DiagnosticComposeException` lists them.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ComposeStackTrace {
    pub frames: Vec<ComposeStackFrame>,
    /// Its number on this thread, from 1: what [`compose_stack_trace_sequence`] gave once it was
    /// begun.
    pub sequence: u64,
}

/// A group on the path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ComposeStackFrame {
    /// Its key, as the compiler (here `#[composable]`) made it.
    pub key: u64,
    /// The composable's function, short (`Linear`), unless the trace gives keys only.
    pub name: Option<&'static str>,
    /// Its path (`app::screen::Linear`), likewise.
    pub path: Option<&'static str>,
}

impl ComposeStackTrace {
    /// The names of its frames, innermost first.
    pub fn names(&self) -> Vec<&'static str> {
        self.frames.iter().filter_map(|frame| frame.name).collect()
    }
}

impl fmt::Display for ComposeStackTrace {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(formatter, "Composition stack when thrown:")?;
        for frame in &self.frames {
            match frame.path {
                Some(path) => writeln!(formatter, "\tat {path}")?,
                None => writeln!(formatter, "\tat $$compose.m${}", frame.key)?,
            }
        }
        Ok(())
    }
}

/// The trace being made: the pass a panic will reach next adds its groups.
pub(crate) struct PendingTrace {
    trace: ComposeStackTrace,
    /// The composition whose pass this one was nested in.
    extended_by: Option<CompositionId>,
}

/// `app::screen::Linear` from `app::screen::Linear::{{closure}}`, generics dropped.
fn path_of(full: &'static str) -> &'static str {
    let path = full.split('<').next().unwrap_or(full);
    let mut path = path;
    while let Some(shorter) = path.strip_suffix("::{{closure}}") {
        path = shorter;
    }
    path
}

/// `group` of `table` on the trace, if `mode` lists it. It's live: a pass's open group, or one at
/// or above a place still holding its context, never a freed one.
fn push_frame(
    frames: &mut Vec<ComposeStackFrame>,
    mode: ComposeStackTraceMode,
    table: &crate::slot_table::SlotTable,
    group: u32,
) {
    if group == SENTINEL {
        return;
    }
    let group = &table[group];
    let named = match group.identity {
        Some(Identity::Type(_, name) | Identity::Name(name)) => Some(path_of(name)),
        None => None,
    };
    // A group of a composable; with keys, any group.
    let listed = named.is_some() || (mode == ComposeStackTraceMode::GroupKeys);
    if !listed {
        return;
    }
    let path = if mode == ComposeStackTraceMode::GroupKeys { None } else { named };
    let name = path.map(|path| path.rsplit("::").next().unwrap_or(path));
    frames.push(ComposeStackFrame { key: group.key, name, path });
}

/// A pass of `id`, nested in a pass of `outer` if any, is unwound by a panic: its open groups go
/// on the trace, after those of a pass nested in it.
#[cold]
pub(crate) fn record(runtime: &mut Runtime, id: CompositionId, outer: Option<CompositionId>) {
    let mode = runtime.stack_trace_mode;
    if mode == ComposeStackTraceMode::None {
        // Nothing left from an earlier panic stands for this one.
        runtime.stack_trace = None;
        return;
    }
    let Some(data) = runtime.compositions.get(id) else { return };
    let mut frames = Vec::new();
    for frame in runtime.frames.iter().rev() {
        push_frame(&mut frames, mode, &data.table, frame.group);
    }
    // A pass of its own, not in its parent's: where it is in its parent, from the parent's table,
    // while its place is there. A place removed leaves a freed group, or one another composable
    // has since, with a stale parent link: the walk stops there.
    if outer.is_none() {
        let mut parent = data.parent;
        while let Some((_, data, group)) =
            parent.and_then(|context| runtime.compositions.context_at(context))
        {
            let mut group = group;
            while group != SENTINEL && group != crate::slot_table::NONE {
                push_frame(&mut frames, mode, &data.table, group);
                group = data.table[group].parent;
            }
            parent = data.parent;
        }
    }
    match &mut runtime.stack_trace {
        Some(pending) if pending.extended_by == Some(id) => {
            pending.trace.frames.extend(frames);
            pending.extended_by = outer;
        }
        slot => {
            runtime.stack_trace_sequence += 1;
            let sequence = runtime.stack_trace_sequence;
            *slot = Some(Box::new(PendingTrace {
                trace: ComposeStackTrace { frames, sequence },
                extended_by: outer,
            }))
        }
    }
}
