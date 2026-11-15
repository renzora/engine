//! The link to the editor that started this companion.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc::Sender;
use std::time::Duration;

use renzora_companion_proto::protocol::{self, Message, PROTOCOL_VERSION};

use crate::server::Event;

/// How long to wait for the editor to answer before giving up. The editor
/// started this process a moment ago with the port it is listening on, so a
/// long silence means it is gone.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// `--companion-port <n> --companion-token <t>`, as the editor passes them.
pub(crate) fn args() -> Option<(u16, String)> {
    parse(&std::env::args().collect::<Vec<_>>())
}

fn parse(args: &[String]) -> Option<(u16, String)> {
    let value = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).cloned();
    Some((value("--companion-port")?.parse().ok()?, value("--companion-token")?))
}

/// Connect and say hello, listing the job `kinds` this companion runs; the
/// editor does any other kind itself.
pub(crate) fn connect(port: u16, token: &str, kinds: Vec<String>) -> std::io::Result<TcpStream> {
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT)?;
    let hello = Message::Hello { token: token.to_string(), version: PROTOCOL_VERSION, kinds };
    stream.write_all(protocol::encode(&hello).as_bytes())?;
    Ok(stream)
}

/// Read the editor's lines on a thread of their own, ending with
/// [`Event::EditorClosed`].
pub(crate) fn read_lines(stream: TcpStream, events: Sender<Event>) {
    let spawned = std::thread::Builder::new().name("companion-read".into()).spawn(move || {
        for line in BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            if let Some(message) = protocol::decode(&line) {
                if events.send(Event::Editor(message)).is_err() {
                    return;
                }
            }
        }
        let _ = events.send(Event::EditorClosed);
    });
    if let Err(e) = spawned {
        crate::log::error(format!("could not read from the editor: {e}; exiting"));
        std::process::exit(4);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_port_and_token_are_read_from_the_command_line() {
        let args: Vec<String> =
            ["renzora-companion", "--companion-port", "4100", "--companion-token", "abc", "--profiling-key", "k"]
                .iter()
                .map(|s| s.to_string())
                .collect();
        assert_eq!(parse(&args), Some((4100, "abc".to_string())));
        assert_eq!(parse(&args[..3]), None, "no token");
    }
}
