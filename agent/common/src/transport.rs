use crate::protocol::{Request, Response};
use std::fmt;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

/// Generous cap for a single newline-delimited-JSON message on either the
/// agentd socket or the pairing TCP protocol (issue #36) — real payloads in
/// both protocols are small, fixed-shape JSON objects; this only exists to
/// stop an unbounded `read_line()` from letting a hostile/misbehaving peer
/// grow a buffer without limit.
pub const MAX_LINE_BYTES: usize = 64 * 1024;

/// Reads one newline-terminated line, refusing to buffer past `max_bytes`.
/// Returns `Ok(None)` on a clean EOF before any byte was read (mirrors what
/// callers previously checked via `read_line(..) == 0`); `Err` for a real
/// I/O error, invalid UTF-8, or a line that exceeds `max_bytes`. The
/// returned string has its trailing `\n` (and a preceding `\r`, if any)
/// stripped.
pub fn read_line_bounded(reader: &mut impl BufRead, max_bytes: usize) -> io::Result<Option<String>> {
    let mut buf = Vec::new();
    let mut limited = reader.by_ref().take(max_bytes as u64 + 1);
    let n = limited.read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Ok(None);
    }
    if buf.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("line exceeds the {max_bytes}-byte limit"),
        ));
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
        if buf.last() == Some(&b'\r') {
            buf.pop();
        }
    }
    String::from_utf8(buf)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

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
    let resp_line = read_line_bounded(&mut reader, MAX_LINE_BYTES)
        .map_err(SendError::Io)?
        .ok_or(SendError::NoResponse)?;
    serde_json::from_str(&resp_line).map_err(SendError::Decode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_normal_line() {
        let mut reader = io::Cursor::new(b"hello world\n".to_vec());
        assert_eq!(
            read_line_bounded(&mut reader, 1024).unwrap(),
            Some("hello world".to_string())
        );
    }

    #[test]
    fn strips_trailing_crlf() {
        let mut reader = io::Cursor::new(b"hello\r\n".to_vec());
        assert_eq!(read_line_bounded(&mut reader, 1024).unwrap(), Some("hello".to_string()));
    }

    #[test]
    fn clean_eof_before_any_byte_is_none() {
        let mut reader = io::Cursor::new(Vec::<u8>::new());
        assert_eq!(read_line_bounded(&mut reader, 1024).unwrap(), None);
    }

    #[test]
    fn oversized_line_is_rejected() {
        let mut data = vec![b'a'; 100];
        data.push(b'\n');
        let mut reader = io::Cursor::new(data);
        let err = read_line_bounded(&mut reader, 10).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn oversized_line_with_no_terminator_is_also_rejected() {
        let data = vec![b'a'; 100]; // never terminated, attacker just keeps sending
        let mut reader = io::Cursor::new(data);
        let err = read_line_bounded(&mut reader, 10).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn line_exactly_at_the_limit_is_accepted() {
        let mut data = vec![b'a'; 10];
        data.push(b'\n');
        let mut reader = io::Cursor::new(data);
        assert!(read_line_bounded(&mut reader, 11).unwrap().is_some());
    }

    #[test]
    fn a_second_line_can_still_be_read_after_the_first() {
        let mut reader = io::Cursor::new(b"first\nsecond\n".to_vec());
        assert_eq!(read_line_bounded(&mut reader, 1024).unwrap(), Some("first".to_string()));
        assert_eq!(read_line_bounded(&mut reader, 1024).unwrap(), Some("second".to_string()));
    }
}
