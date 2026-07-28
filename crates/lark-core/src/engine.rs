//! Owns the worker thread that runs a [`Backend`].
//!
//! The UI holds an [`Engine`], pushes [`Command`]s in and pulls [`Event`]s
//! out. Neither direction ever blocks the caller on network work.

use crate::backend::{Backend, EventSink, TICK};
use crate::command::Command;
use crate::event::Event;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};

/// A running backend on its own thread.
///
/// Dropping the engine asks the worker to stop and waits for it, so a dropped
/// engine never leaves a thread writing to a file behind the application's
/// back.
#[derive(Debug)]
pub struct Engine {
    commands: Sender<Message>,
    events: Receiver<Event>,
    worker: Option<JoinHandle<()>>,
}

/// What the worker thread accepts. Shutdown travels the same channel as
/// commands so that it cannot overtake work already queued.
enum Message {
    Command(Command),
    Stop,
}

impl Engine {
    /// Start `backend` on a worker thread.
    ///
    /// # Panics
    /// If the operating system refuses to create the thread.
    #[must_use]
    pub fn spawn<B: Backend + 'static>(backend: B) -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("lark-core".into())
            .spawn(move || run(backend, &command_rx, &EventSink::new(event_tx)))
            .expect("spawning the core worker thread");

        Self { commands: command_tx, events: event_rx, worker: Some(worker) }
    }

    /// Queue a command. Returns `false` once the worker has stopped, which
    /// most callers can ignore: a stopped engine means the app is shutting
    /// down, and there is nothing useful left to do about it.
    #[allow(clippy::must_use_candidate)]
    pub fn send(&self, command: Command) -> bool {
        self.commands.send(Message::Command(command)).is_ok()
    }

    /// Take the next event if one is ready, without blocking.
    #[must_use]
    pub fn try_next(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// Drain every event currently available.
    ///
    /// This is the shape a UI frame wants: apply everything that has happened
    /// since the last frame, then redraw once.
    #[must_use]
    pub fn drain(&self) -> Vec<Event> {
        let mut drained = Vec::new();
        loop {
            match self.events.try_recv() {
                Ok(event) => drained.push(event),
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => return drained,
            }
        }
    }

    /// Borrow the event channel, for a consumer that wants to block on it.
    #[must_use]
    pub const fn events(&self) -> &Receiver<Event> {
        &self.events
    }

    /// Stop the worker and wait for it to finish.
    ///
    /// Idempotent, and also run by [`Drop`], so calling it is only necessary
    /// when the caller wants to observe shutdown completing.
    pub fn shutdown(&mut self) {
        let Some(worker) = self.worker.take() else { return };
        // An error means the worker already stopped, which is the desired end
        // state either way, so the join still runs.
        let _ = self.commands.send(Message::Stop);
        let _ = worker.join();
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// The worker loop: apply commands as they arrive, and poll between them so
/// that progress started earlier keeps flowing even while the UI is quiet.
fn run<B: Backend>(mut backend: B, commands: &Receiver<Message>, out: &EventSink) {
    loop {
        match commands.recv_timeout(TICK) {
            Ok(Message::Command(command)) => backend.execute(command, out),
            Ok(Message::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        backend.poll(out);
    }
    backend.shutdown();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// Records what it was asked to do and echoes each command back as a
    /// warning, so tests can observe ordering.
    #[derive(Default)]
    struct Spy {
        seen: Arc<Mutex<Vec<Command>>>,
        polls: Arc<AtomicUsize>,
        stopped: Arc<AtomicUsize>,
    }

    impl Backend for Spy {
        fn execute(&mut self, command: Command, out: &EventSink) {
            out.warn(format!("{command:?}"));
            self.seen.lock().unwrap().push(command);
        }
        fn poll(&mut self, _out: &EventSink) {
            self.polls.fetch_add(1, Ordering::Relaxed);
        }
        fn shutdown(&mut self) {
            self.stopped.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Spin until `condition` holds, so tests never depend on a fixed sleep.
    fn wait_for(mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            thread::sleep(Duration::from_millis(5));
        }
        false
    }

    #[test]
    fn commands_reach_the_backend_in_order() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let engine = Engine::spawn(Spy { seen: Arc::clone(&seen), ..Spy::default() });

        engine.send(Command::RequestRoomList);
        engine.send(Command::JoinRoom("nicotine".into()));
        engine.send(Command::LeaveRoom("nicotine".into()));

        assert!(wait_for(|| seen.lock().unwrap().len() == 3), "commands were not all delivered");
        assert_eq!(
            *seen.lock().unwrap(),
            vec![
                Command::RequestRoomList,
                Command::JoinRoom("nicotine".into()),
                Command::LeaveRoom("nicotine".into()),
            ]
        );
    }

    #[test]
    fn events_come_back_to_the_caller() {
        let engine = Engine::spawn(Spy::default());
        engine.send(Command::Disconnect);

        let event = engine.events().recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(event, Event::Warning("Disconnect".into()));
    }

    #[test]
    fn drain_returns_everything_pending_and_then_nothing() {
        let engine = Engine::spawn(Spy::default());
        engine.send(Command::Disconnect);
        engine.send(Command::RequestRoomList);

        // The two commands are handled independently, so the events may not
        // both be queued yet; accumulate across drains until both arrive.
        let mut collected = Vec::new();
        assert!(
            wait_for(|| {
                collected.extend(engine.drain());
                collected.len() == 2
            }),
            "expected both events, saw {collected:?}"
        );
        assert_eq!(
            collected,
            vec![
                Event::Warning("Disconnect".into()),
                Event::Warning("RequestRoomList".into()),
            ]
        );
        assert!(engine.drain().is_empty(), "a second drain should find nothing new");
    }

    #[test]
    fn the_backend_is_polled_while_idle() {
        let polls = Arc::new(AtomicUsize::new(0));
        let _engine = Engine::spawn(Spy { polls: Arc::clone(&polls), ..Spy::default() });

        assert!(
            wait_for(|| polls.load(Ordering::Relaxed) >= 3),
            "an idle engine should still tick"
        );
    }

    #[test]
    fn shutdown_stops_the_backend_and_is_idempotent() {
        let stopped = Arc::new(AtomicUsize::new(0));
        let mut engine = Engine::spawn(Spy { stopped: Arc::clone(&stopped), ..Spy::default() });

        engine.shutdown();
        assert_eq!(stopped.load(Ordering::Relaxed), 1, "shutdown should reach the backend");

        engine.shutdown(); // must not panic or double-stop
        assert_eq!(stopped.load(Ordering::Relaxed), 1);
        assert!(!engine.send(Command::Disconnect), "a stopped engine accepts no commands");
    }

    #[test]
    fn queued_commands_run_before_shutdown_takes_effect() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let mut engine = Engine::spawn(Spy { seen: Arc::clone(&seen), ..Spy::default() });

        engine.send(Command::Connect(Box::default()));
        engine.shutdown();

        assert_eq!(seen.lock().unwrap().len(), 1, "shutdown must not overtake queued work");
    }
}
