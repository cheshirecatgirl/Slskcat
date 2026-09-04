//! The seam between the application and whatever actually speaks Soulseek.
//!
//! Everything above this trait deals only in [`Command`] and [`Event`]. That
//! is what keeps the choice of protocol implementation, the in-process
//! library today and conceivably a sidecar later, from leaking into the UI.

use crate::command::Command;
use crate::event::Event;
use std::sync::mpsc::{SendError, Sender};
use std::time::Duration;

/// How often [`Backend::poll`] is called when no commands are arriving.
///
/// Fast enough that progress bars and streaming search hits feel immediate,
/// slow enough to stay invisible on an idle machine.
pub const TICK: Duration = Duration::from_millis(120);

/// The channel a backend reports through.
///
/// Sends are deliberately infallible from the backend's point of view: once
/// the UI has gone away there is nobody to tell, so a failed send is dropped
/// rather than escalated.
#[derive(Debug, Clone)]
pub struct EventSink {
    tx: Sender<Event>,
}

impl EventSink {
    #[must_use]
    pub const fn new(tx: Sender<Event>) -> Self {
        Self { tx }
    }

    /// Report an event, ignoring a closed receiver.
    pub fn emit(&self, event: Event) {
        let _: Result<(), SendError<Event>> = self.tx.send(event);
    }

    /// Report a non-fatal problem.
    pub fn warn(&self, message: impl Into<String>) {
        self.emit(Event::Warning(message.into()));
    }

    /// Report several events in order.
    pub fn emit_all(&self, events: impl IntoIterator<Item = Event>) {
        for event in events {
            self.emit(event);
        }
    }
}

/// Something that can carry out [`Command`]s and report [`Event`]s.
///
/// Implementations run on the engine's worker thread and may block, but should
/// return from `execute` promptly so the command queue keeps moving; anything
/// long-running belongs on its own thread with results surfaced from `poll`.
pub trait Backend: Send {
    /// Carry out one command.
    fn execute(&mut self, command: Command, out: &EventSink);

    /// Called on every tick and after every command. Used to drain progress
    /// from work started earlier.
    fn poll(&mut self, out: &EventSink) {
        let _ = out;
    }

    /// Release resources. Called once when the engine stops.
    fn shutdown(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn sink_delivers_and_tolerates_a_dropped_receiver() {
        let (tx, rx) = mpsc::channel();
        let sink = EventSink::new(tx);
        sink.warn("careful");
        assert_eq!(rx.recv().unwrap(), Event::Warning("careful".into()));

        drop(rx);
        sink.warn("nobody is listening"); // must not panic
    }

    #[test]
    fn emit_all_sends_each_event_in_order() {
        let (tx, rx) = mpsc::channel();
        let sink = EventSink::new(tx);
        sink.emit_all([Event::Warning("a".into()), Event::Warning("b".into())]);
        assert_eq!(rx.recv().unwrap(), Event::Warning("a".into()));
        assert_eq!(rx.recv().unwrap(), Event::Warning("b".into()));
    }
}
