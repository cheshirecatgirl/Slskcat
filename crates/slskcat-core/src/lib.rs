//! The engine behind slsk.cat, a Soulseek client.
//!
//! The application talks to this crate in exactly two currencies:
//! [`Command`] going in and [`Event`] coming out. Everything network-facing
//! happens on worker threads owned by [`Engine`], so a UI can drain events
//! once per frame and never block.
//!
//! ```no_run
//! use slskcat_core::{Command, Engine, LiveBackend, model::Config};
//!
//! let engine = Engine::spawn(LiveBackend::new());
//! engine.send(Command::Connect(Box::new(Config::default())));
//!
//! for event in engine.drain() {
//!     println!("{event:?}");
//! }
//! ```
//!
//! The [`Backend`] trait is the seam: [`LiveBackend`] speaks the real
//! protocol, and anything else implementing the trait can stand in for it.

pub mod backend;
pub mod command;
pub mod engine;
pub mod event;
pub mod guard;
pub mod live;
pub mod model;
pub mod proxy;
pub mod recovery;

pub use backend::{Backend, EventSink};
pub use command::{Command, SearchIds};
pub use engine::{Commander, Engine};
pub use event::{Disconnect, Event};
pub use guard::{ShareRisk, assess_share_path};
pub use live::LiveBackend;
pub use recovery::TransferSnapshot;
