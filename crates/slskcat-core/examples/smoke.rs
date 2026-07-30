//! A live check against the real Soulseek network.
//!
//! Nothing else in this repository touches the network — every test is
//! offline — so this is the only thing that can tell you whether the protocol
//! integration actually works. It drives the same [`Engine`] and
//! [`LiveBackend`] the application does, so a pass here means the core works,
//! not merely that the library does.
//!
//! Credentials come from the environment and are never printed, logged, or
//! written anywhere:
//!
//! ```text
//! SLSKCAT_USER=yourname SLSKCAT_PASS=yourpassword \
//!   cargo run -p slskcat-core --example smoke
//! ```
//!
//! Optional:
//! - `SLSKCAT_QUERY` — what to search for (default: `aphex twin`)
//! - `SLSKCAT_DOWNLOAD=1` — also queue the smallest hit and wait for bytes
//! - `SLSKCAT_TIMEOUT` — seconds to allow each step (default: 30)
//!
//! Exits non-zero if any step fails, so it is usable as a gate.

use slskcat_core::model::{Config, Credentials, SearchId, TransferState};
use slskcat_core::{Command, Disconnect, Engine, Event, LiveBackend};

use std::time::{Duration, Instant};

/// One checked step.
struct Step {
    name: &'static str,
    ok: bool,
    detail: String,
    took: Duration,
}

fn main() -> std::process::ExitCode {
    let Some(credentials) = credentials() else {
        eprintln!(
            "Set SLSKCAT_USER and SLSKCAT_PASS. They are read from the environment \
             and never printed or stored."
        );
        return std::process::ExitCode::from(2);
    };

    let query = std::env::var("SLSKCAT_QUERY").unwrap_or_else(|_| "aphex twin".into());
    let budget = Duration::from_secs(
        std::env::var("SLSKCAT_TIMEOUT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30),
    );
    let try_download = std::env::var("SLSKCAT_DOWNLOAD").is_ok_and(|v| v == "1");

    println!(
        "slsk.cat smoke test — user {}, query {query:?}\n",
        credentials.username
    );

    let engine = Engine::spawn(LiveBackend::new());
    let mut steps = Vec::new();

    if !sign_in(&engine, &mut steps, credentials, budget) {
        report(&steps);
        return std::process::ExitCode::FAILURE;
    }

    check_rooms(&engine, &mut steps, budget);
    let best = check_search(&engine, &mut steps, &query, budget);

    if let Some((username, ..)) = best.clone() {
        check_peer(&engine, &mut steps, &username, budget);
    }

    match (try_download, best) {
        (true, Some(target)) => check_download(&engine, &mut steps, target, budget),
        (true, None) => println!("  ·  download       skipped (nothing was found to fetch)"),
        (false, _) => {
            println!("  ·  download       skipped (set SLSKCAT_DOWNLOAD=1 to move real bytes)");
        }
    }

    engine.send(Command::Disconnect);
    report(&steps);

    if steps.iter().all(|s| s.ok) {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

/// Connect and log in. Everything else depends on this, so a failure is fatal.
fn sign_in(
    engine: &Engine,
    steps: &mut Vec<Step>,
    credentials: Credentials,
    budget: Duration,
) -> bool {
    let config = Config {
        credentials,
        ..Config::default()
    }
    .normalized();
    engine.send(Command::Connect(Box::new(config)));

    // Connecting can block inside the library for longer than one step's
    // budget, so this one gets more room; a slow network should not read as a
    // failure.
    step(steps, "sign in", || {
        collect(engine, budget * 2, |event| match event {
            Event::Connected { username } => Some(Ok(format!("as {username}"))),
            Event::LoginFailed { reason } => Some(Err(reason.clone())),
            _ => None,
        })
    })
}

/// Proves the server is answering, not merely accepting the connection.
fn check_rooms(engine: &Engine, steps: &mut Vec<Step>, budget: Duration) {
    engine.send(Command::RequestRoomList);
    step(steps, "room list", || {
        collect(engine, budget, |event| match event {
            Event::RoomList(rooms) if !rooms.is_empty() => {
                Some(Ok(format!("{} rooms", rooms.len())))
            }
            _ => None,
        })
    });
}

/// Exercises the core's streaming search, not the library's blocking one.
///
/// Returns the smallest file seen, so an optional download finishes quickly.
fn check_search(
    engine: &Engine,
    steps: &mut Vec<Step>,
    query: &str,
    budget: Duration,
) -> Option<(String, String, u64)> {
    engine.send(Command::Search {
        id: SearchId(1),
        query: query.to_owned(),
    });

    let mut best: Option<(String, String, u64)> = None;
    let mut peers = 0usize;
    let mut files = 0usize;

    step(steps, "search", || {
        let deadline = Instant::now() + budget;
        while Instant::now() < deadline {
            for event in engine.drain() {
                if let Event::SearchHits { hits, .. } = event {
                    peers += hits.len();
                    for hit in hits {
                        for file in hit.files {
                            files += 1;
                            if best.as_ref().is_none_or(|(_, _, size)| file.size < *size) {
                                best = Some((hit.username.clone(), file.path.clone(), file.size));
                            }
                        }
                    }
                }
            }
            if files >= 20 {
                break; // enough to prove hits stream in
            }
            std::thread::sleep(Duration::from_millis(120));
        }
        if files == 0 {
            Err("no hits — try a broader SLSKCAT_QUERY".into())
        } else {
            Ok(format!("{files} files from {peers} peers"))
        }
    });

    best
}

/// Browse a peer's shares and ask the server about them.
fn check_peer(engine: &Engine, steps: &mut Vec<Step>, username: &str, budget: Duration) {
    engine.send(Command::BrowseUser(username.to_owned()));
    step(steps, "browse a peer", || {
        collect(engine, budget, |event| match event {
            Event::BrowseReady { directories, .. } => {
                Some(Ok(format!("{} folders from {username}", directories.len())))
            }
            _ => None,
        })
    });

    engine.send(Command::RequestUserInfo(username.to_owned()));
    step(steps, "peer details", || {
        collect(engine, budget, |event| match event {
            Event::UserUpdated(user) if user.presence.is_some() || user.shared_files.is_some() => {
                Some(Ok(format!(
                    "{:?}, {} files shared",
                    user.presence,
                    user.shared_files.unwrap_or(0)
                )))
            }
            _ => None,
        })
    });
}

/// Queue a real transfer and wait for bytes to actually arrive.
fn check_download(
    engine: &Engine,
    steps: &mut Vec<Step>,
    (username, path, size): (String, String, u64),
    budget: Duration,
) {
    engine.send(Command::Download {
        username,
        path,
        size,
    });
    step(steps, "download", || {
        collect(engine, budget * 2, |event| match event {
            Event::TransferUpdated { state, .. } => match state {
                TransferState::Active { transferred, .. } if *transferred > 0 => {
                    Some(Ok(format!("{transferred} bytes received")))
                }
                TransferState::Completed => Some(Ok("completed".into())),
                TransferState::Failed { reason } => {
                    Some(Err(reason.clone().unwrap_or_else(|| "failed".into())))
                }
                _ => None,
            },
            _ => None,
        })
    });
}

/// Read credentials, refusing to guess at them.
fn credentials() -> Option<Credentials> {
    let username = std::env::var("SLSKCAT_USER")
        .ok()
        .filter(|v| !v.is_empty())?;
    let password = std::env::var("SLSKCAT_PASS")
        .ok()
        .filter(|v| !v.is_empty())?;
    Some(Credentials { username, password })
}

/// Run one step, printing its outcome as it happens.
fn step(
    steps: &mut Vec<Step>,
    name: &'static str,
    body: impl FnOnce() -> Result<String, String>,
) -> bool {
    let started = Instant::now();
    let outcome = body();
    let took = started.elapsed();
    let ok = outcome.is_ok();
    let detail = outcome.unwrap_or_else(|error| error);
    println!(
        "  {}  {name:<14} {detail}  ({:.1}s)",
        if ok { "ok" } else { "FAIL" },
        took.as_secs_f32()
    );
    steps.push(Step {
        name,
        ok,
        detail,
        took,
    });
    ok
}

/// Drain events until `judge` returns a verdict, or the budget runs out.
///
/// Warnings and disconnects are surfaced as failures rather than ignored: a
/// step that ends because the session dropped has not passed.
fn collect(
    engine: &Engine,
    budget: Duration,
    mut judge: impl FnMut(&Event) -> Option<Result<String, String>>,
) -> Result<String, String> {
    let deadline = Instant::now() + budget;
    let mut warnings = Vec::new();
    while Instant::now() < deadline {
        for event in engine.drain() {
            if let Some(verdict) = judge(&event) {
                return verdict;
            }
            match event {
                Event::Warning(text) => warnings.push(text),
                Event::Disconnected(why) => {
                    return Err(match why {
                        Disconnect::LoggedInElsewhere => "signed out: logged in elsewhere".into(),
                        Disconnect::Lost(detail) => format!("connection lost: {detail}"),
                        Disconnect::Requested => "disconnected".into(),
                    });
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(120));
    }
    Err(if warnings.is_empty() {
        "timed out — check the network, or raise SLSKCAT_TIMEOUT".into()
    } else {
        format!("timed out (warnings: {})", warnings.join("; "))
    })
}

fn report(steps: &[Step]) {
    let failed: Vec<&Step> = steps.iter().filter(|s| !s.ok).collect();
    let total: Duration = steps.iter().map(|s| s.took).sum();
    println!();
    if failed.is_empty() {
        println!(
            "all {} steps passed in {:.1}s",
            steps.len(),
            total.as_secs_f32()
        );
    } else {
        println!("{} of {} steps failed:", failed.len(), steps.len());
        for step in failed {
            println!("  {}: {}", step.name, step.detail);
        }
    }
}
