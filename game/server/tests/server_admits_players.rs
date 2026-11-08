//! `server_admits_players.rs` runs the built `warped-server` with the flags the cloud's startup
//! script passes, and checks that it admits the game's window and the demo's (still running on
//! players' machines) with the game's password, refuses them with a wrong one, and refuses a map it
//! doesn't know.
//!
//! Partitions and the tests that cover each part:
//!
//! | Partitioning | Part | Tests |
//! | --- | --- | --- |
//! | Window | the game's (`warped_core::net`) | `admits_windows_with_the_password`, `refuses_wrong_passwords` |
//! | | the demo's (`legacy-oracle`, macOS only) | `admits_windows_with_the_password`, `refuses_wrong_passwords` |
//! | Game password | right, wrong | `admits_windows_with_the_password`, `refuses_wrong_passwords` |
//! | `--map` | a built-in map, an unknown one | every test |

use std::io::{BufRead, BufReader};
use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use legacy_oracle as demo;
use warped_core as core;

const MAP_NAME: &str = "lunar_surface";
const MAP_FLAG: &str = "lunar-surface";
const GAME_PASSWORD: &[u8] = b"warped2-server-test";
const WRONG_PASSWORD: &[u8] = b"warped2-wrong";
const JOIN_TIMEOUT_S: f64 = 5.0;
const POLL_MS: u64 = 10;
const USAGE_EXIT_CODE: i32 = 2;

/// `Server` is a running `warped-server` process on `address`, stopped when dropped, and the
/// rest of its output, kept open so its prints don't fail.
struct Server {
    process: Child,
    address: SocketAddr,
    _output: BufReader<ChildStdout>,
}

impl Server {
    /// `Start` runs `warped-server` on a free port with `--map lunar-surface` and a password
    /// file holding `GAME_PASSWORD`, and returns once it says it is listening.
    ///
    /// Example: `let server = Server::Start(&"admits");`
    #[allow(non_snake_case)]
    fn Start(label: &str) -> Server {
        let password_path: PathBuf =
            PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("game_password_{label}"));
        std::fs::write(&password_path, GAME_PASSWORD).expect("a password file");
        let port: u16 = UdpSocket::bind("127.0.0.1:0")
            .and_then(|socket| socket.local_addr())
            .expect("a free UDP port")
            .port();
        let mut process: Child = Command::new(env!("CARGO_BIN_EXE_warped-server"))
            .args([
                "--port",
                &port.to_string(),
                "--map",
                MAP_FLAG,
                "--game-password-file",
            ])
            .arg(&password_path)
            .stdout(Stdio::piped())
            .spawn()
            .expect("warped-server starts");
        let mut output: BufReader<ChildStdout> =
            BufReader::new(process.stdout.take().expect("the server's output"));
        let mut first_line: String = String::new();
        output
            .read_line(&mut first_line)
            .expect("the server says it listens");
        assert!(
            first_line.contains("on UDP port"),
            "the server said {first_line:?}"
        );
        Server {
            process,
            address: SocketAddr::from(([127, 0, 0, 1], port)),
            _output: output,
        }
    }
}

impl Drop for Server {
    /// `drop` stops the server process.
    ///
    /// Example: `drop(server);`
    fn drop(&mut self) {
        self.process.kill().expect("the server stops");
        self.process.wait().expect("the server exits");
    }
}

/// `try_join` connects a `$net::Client` to `$server` with `$password`, joins any team and polls
/// until the server welcomes it (`true`) or refuses its password (`false`); it fails after
/// `JOIN_TIMEOUT_S`.
///
/// Example: `assert!(try_join!(core::net, server.address, GAME_PASSWORD));`
macro_rules! try_join {
    ($net:ident :: $net_module:ident, $server:expr, $password:expr) => {{
        use $net::$net_module as net;
        let mut client: net::Client =
            net::Client::Connect($server, MAP_NAME, $password).expect("a client socket");
        client.join(None).expect("a join request");
        let start: Instant = Instant::now();
        loop {
            assert!(
                start.elapsed().as_secs_f64() < JOIN_TIMEOUT_S,
                "no answer from {}",
                $server
            );
            match client.poll() {
                Ok(_) if !client.is_waiting_for_welcome() => break true,
                Ok(_) => std::thread::sleep(Duration::from_millis(POLL_MS)),
                Err(error) if error.is::<net::PasswordRefused>() => break false,
                Err(error) => panic!("the client failed: {error}"),
            }
        }
    }};
}

#[test]
fn admits_windows_with_the_password() {
    let server: Server = Server::Start("admits");
    assert!(try_join!(core::net, server.address, GAME_PASSWORD));
    #[cfg(target_os = "macos")]
    assert!(try_join!(demo::net, server.address, GAME_PASSWORD));
}

#[test]
fn refuses_wrong_passwords() {
    let server: Server = Server::Start("refuses");
    assert!(!try_join!(core::net, server.address, WRONG_PASSWORD));
    #[cfg(target_os = "macos")]
    assert!(!try_join!(demo::net, server.address, WRONG_PASSWORD));
}

#[test]
fn refuses_an_unknown_map() {
    let status = Command::new(env!("CARGO_BIN_EXE_warped-server"))
        .args(["--map", "no-such-map"])
        .stderr(Stdio::null())
        .status()
        .expect("warped-server runs");
    assert_eq!(status.code(), Some(USAGE_EXIT_CODE));
}
