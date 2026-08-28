use crate::protocol::{Request, Response};
use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

#[derive(Debug)]
pub enum SendError {
    Connect(std::io::Error),
    Io(std::io::Error),
    Decode(serde_json::Error),
    /// The connection closed without sending a response line (e.g. agentd
    /// crashed mid-request).
    NoResponse,
}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SendError::Connect(e) => write!(f, "could not connect to agentd: {e}"),
            SendError::Io(e) => write!(f, "I/O error talking to agentd: {e}"),
            SendError::Decode(e) => write!(f, "malformed response from agentd: {e}"),
            SendError::NoResponse => write!(f, "agentd closed the connection without responding"),
        }
    }
}

impl std::error::Error for SendError {}

/// Sends one request and waits for one response. Connects fresh every call —
/// requests are infrequent (CLI invocations, app start/stop) so a persistent
/// connection isn't worth the complexity.
pub fn send(socket: &Path, req: &Request, timeout: Duration) -> Result<Response, SendError> {
    let stream = UnixStream::connect(socket).map_err(SendError::Connect)?;
    stream.set_read_timeout(Some(timeout)).map_err(SendError::Io)?;
    stream.set_write_timeout(Some(timeout)).map_err(SendError::Io)?;

    let mut writer = stream.try_clone().map_err(SendError::Io)?;
    let line = serde_json::to_string(req).map_err(SendError::Decode)?;
    writeln!(writer, "{line}").map_err(SendError::Io)?;
    writer.flush().map_err(SendError::Io)?;

    let mut reader = BufReader::new(stream);
    let mut resp_line = String::new();
    let n = reader.read_line(&mut resp_line).map_err(SendError::Io)?;
    if n == 0 {
        return Err(SendError::NoResponse);
    }
    serde_json::from_str(resp_line.trim_end()).map_err(SendError::Decode)
}
