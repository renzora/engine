//! A frame's profile, and how the recorder's events become one.

use std::collections::HashMap;

use renzora::profiling::{Edge, SpanEvent};

/// One span: a system run, or anything else the engine wrapped in a span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    /// The thread it ran on; `renzora::profiling::thread_name` names it.
    pub thread: u32,
    /// Interned; `renzora::profiling::name` gives the text.
    pub name: u32,
    pub start_ns: u64,
    pub end_ns: u64,
    /// How many spans it sits inside on its thread; 0 is outermost.
    pub depth: u16,
}

/// One render pass's GPU time, as Bevy's render diagnostics measured it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GpuPass {
    pub name: u32,
    pub ms: f32,
    /// Nesting in Bevy's diagnostic path; 0 is a top-level pass.
    pub depth: u8,
}

/// A number sampled once a frame: entity count, frame time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Counter {
    pub name: u32,
    pub value: f64,
}

/// Everything recorded between two frame marks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProfileFrame {
    /// Counts up from the first frame recorded in this session.
    pub index: u64,
    pub start_ns: u64,
    pub end_ns: u64,
    /// Spans that FINISHED in this frame. A span still running at the frame
    /// mark (a render system on its own thread) lands in the frame it ends in,
    /// with its true start.
    pub spans: Vec<Span>,
    pub gpu: Vec<GpuPass>,
    pub counters: Vec<Counter>,
}

/// Pairs each thread's enters with their exits, across frames.
#[derive(Default)]
pub struct SpanBuilder {
    /// Per thread, the spans entered and not yet exited, innermost last.
    open: HashMap<u32, Vec<(u32, u64)>>,
}

impl SpanBuilder {
    /// Feed one thread's events in order; returns the spans they closed.
    ///
    /// An exit with no matching enter (recording switched on mid-span) is
    /// dropped. An exit that does not match the innermost span closes the
    /// nearest match and everything inside it, so one lost event cannot leave
    /// a thread's stack wrong for the rest of the session.
    pub fn feed(&mut self, thread: u32, events: &[SpanEvent], out: &mut Vec<Span>) {
        let stack = self.open.entry(thread).or_default();
        for event in events {
            match event.edge {
                Edge::Enter => stack.push((event.name, event.at_ns)),
                Edge::Exit => {
                    let Some(at) = stack.iter().rposition(|(name, _)| *name == event.name) else {
                        continue;
                    };
                    let (name, start_ns) = stack[at];
                    stack.truncate(at);
                    out.push(Span {
                        thread,
                        name,
                        start_ns,
                        end_ns: event.at_ns,
                        depth: at as u16,
                    });
                }
            }
        }
    }

    /// Forget every open span, for when recording stops.
    pub fn reset(&mut self) {
        self.open.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(at_ns: u64, name: u32, edge: Edge) -> SpanEvent {
        SpanEvent { at_ns, name, edge }
    }

    #[test]
    fn nested_spans_pair_up_with_their_depth() {
        let mut builder = SpanBuilder::default();
        let mut out = Vec::new();
        builder.feed(
            0,
            &[ev(0, 1, Edge::Enter), ev(2, 2, Edge::Enter), ev(5, 2, Edge::Exit), ev(9, 1, Edge::Exit)],
            &mut out,
        );
        assert_eq!(
            out,
            vec![
                Span { thread: 0, name: 2, start_ns: 2, end_ns: 5, depth: 1 },
                Span { thread: 0, name: 1, start_ns: 0, end_ns: 9, depth: 0 },
            ]
        );
    }

    /// A render system that starts before the frame mark and ends after it
    /// comes out whole, in the later frame.
    #[test]
    fn a_span_can_cross_a_frame_mark() {
        let mut builder = SpanBuilder::default();
        let mut first = Vec::new();
        builder.feed(3, &[ev(10, 7, Edge::Enter)], &mut first);
        assert!(first.is_empty());
        let mut second = Vec::new();
        builder.feed(3, &[ev(25, 7, Edge::Exit)], &mut second);
        assert_eq!(second, vec![Span { thread: 3, name: 7, start_ns: 10, end_ns: 25, depth: 0 }]);
    }

    #[test]
    fn threads_are_paired_separately() {
        let mut builder = SpanBuilder::default();
        let mut out = Vec::new();
        builder.feed(0, &[ev(0, 1, Edge::Enter)], &mut out);
        builder.feed(1, &[ev(1, 1, Edge::Enter), ev(4, 1, Edge::Exit)], &mut out);
        assert_eq!(out, vec![Span { thread: 1, name: 1, start_ns: 1, end_ns: 4, depth: 0 }]);
    }

    #[test]
    fn a_lost_event_does_not_wreck_the_stack() {
        let mut builder = SpanBuilder::default();
        let mut out = Vec::new();
        // An exit for a span that was entered before recording began.
        builder.feed(0, &[ev(0, 9, Edge::Exit)], &mut out);
        assert!(out.is_empty());
        // An inner span whose exit was lost: closing the outer one clears it.
        builder.feed(0, &[ev(1, 1, Edge::Enter), ev(2, 2, Edge::Enter), ev(8, 1, Edge::Exit)], &mut out);
        assert_eq!(out, vec![Span { thread: 0, name: 1, start_ns: 1, end_ns: 8, depth: 0 }]);
        out.clear();
        builder.feed(0, &[ev(9, 3, Edge::Enter), ev(10, 3, Edge::Exit)], &mut out);
        assert_eq!(out[0].depth, 0, "the lost inner span no longer counts as open");
    }
}
