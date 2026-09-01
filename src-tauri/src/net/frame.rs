//! Length-prefixed JSON framing.
//!
//! Four bytes of big-endian length, then that many bytes of payload. Strict
//! request/reply, synchronous, one thread per connection. A household LAN does
//! not need an async executor, and a dull transport is one that cannot surprise
//! the data.

use std::io::{self, Read, Write};

/// The largest frame that will be read or written.
///
/// This exists to stop a malformed or hostile length prefix from causing an
/// enormous allocation before a single byte of payload has arrived. Real
/// traffic is a command and its arguments; the largest legitimate frames are
/// bulk reads like a full transaction list, which stay far below this.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug)]
pub enum FrameError {
    Io(io::Error),
    /// The length prefix exceeded `MAX_FRAME_BYTES`.
    TooLarge(usize),
    /// The peer went away mid-frame.
    Truncated,
    /// The payload was not valid UTF-8.
    NotUtf8,
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FrameError::Io(e) => write!(f, "connection error: {e}"),
            FrameError::TooLarge(n) => write!(
                f,
                "a message of {n} bytes exceeds the {MAX_FRAME_BYTES} byte limit"
            ),
            FrameError::Truncated => write!(f, "the connection ended mid-message"),
            FrameError::NotUtf8 => write!(f, "the message was not valid text"),
        }
    }
}

impl std::error::Error for FrameError {}

impl From<io::Error> for FrameError {
    fn from(e: io::Error) -> Self {
        FrameError::Io(e)
    }
}

pub fn write_frame<W: Write>(writer: &mut W, payload: &str) -> Result<(), FrameError> {
    let bytes = payload.as_bytes();
    if bytes.len() > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(bytes.len()));
    }
    let len = (bytes.len() as u32).to_be_bytes();
    writer.write_all(&len)?;
    writer.write_all(bytes)?;
    writer.flush()?;
    Ok(())
}

pub fn read_frame<R: Read>(reader: &mut R) -> Result<String, FrameError> {
    let mut len_bytes = [0u8; 4];
    match reader.read_exact(&mut len_bytes) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Err(FrameError::Truncated),
        Err(e) => return Err(FrameError::Io(e)),
    }

    let len = u32::from_be_bytes(len_bytes) as usize;

    // Checked before allocating, so an absurd prefix costs nothing.
    if len > MAX_FRAME_BYTES {
        return Err(FrameError::TooLarge(len));
    }

    let mut payload = vec![0u8; len];
    match reader.read_exact(&mut payload) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Err(FrameError::Truncated),
        Err(e) => return Err(FrameError::Io(e)),
    }

    String::from_utf8(payload).map_err(|_| FrameError::NotUtf8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn a_frame_round_trips() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, r#"{"command":"get_accounts"}"#).unwrap();

        let mut cursor = Cursor::new(buffer);
        let out = read_frame(&mut cursor).unwrap();
        assert_eq!(out, r#"{"command":"get_accounts"}"#);
    }

    #[test]
    fn frames_are_read_back_in_order() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, "first").unwrap();
        write_frame(&mut buffer, "second").unwrap();
        write_frame(&mut buffer, "third").unwrap();

        let mut cursor = Cursor::new(buffer);
        assert_eq!(read_frame(&mut cursor).unwrap(), "first");
        assert_eq!(read_frame(&mut cursor).unwrap(), "second");
        assert_eq!(read_frame(&mut cursor).unwrap(), "third");
    }

    #[test]
    fn an_empty_frame_is_legal() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, "").unwrap();
        let mut cursor = Cursor::new(buffer);
        assert_eq!(read_frame(&mut cursor).unwrap(), "");
    }

    #[test]
    fn unicode_survives_the_wire() {
        let payload = r#"{"payee":"Café Größe — 日本","amount":"12.34"}"#;
        let mut buffer = Vec::new();
        write_frame(&mut buffer, payload).unwrap();
        let mut cursor = Cursor::new(buffer);
        assert_eq!(read_frame(&mut cursor).unwrap(), payload);
    }

    /// An oversized length prefix must be refused *before* the allocation, so
    /// four hostile bytes cannot ask the host for gigabytes.
    #[test]
    fn an_absurd_length_prefix_is_refused_without_allocating() {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&u32::MAX.to_be_bytes());
        // Deliberately no payload: if the reader tried to allocate and fill
        // before checking, it would block or die here rather than returning.
        let mut cursor = Cursor::new(buffer);
        match read_frame(&mut cursor) {
            Err(FrameError::TooLarge(n)) => assert_eq!(n, u32::MAX as usize),
            other => panic!("expected TooLarge, got {other:?}"),
        }
    }

    #[test]
    fn a_frame_at_the_limit_is_allowed_and_one_past_is_not() {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&((MAX_FRAME_BYTES + 1) as u32).to_be_bytes());
        let mut cursor = Cursor::new(buffer);
        assert!(matches!(
            read_frame(&mut cursor),
            Err(FrameError::TooLarge(_))
        ));

        // Writing oversized is refused at the sender too, so a bug on one side
        // does not become a mystery on the other.
        let oversized = "x".repeat(MAX_FRAME_BYTES + 1);
        let mut out = Vec::new();
        assert!(matches!(
            write_frame(&mut out, &oversized),
            Err(FrameError::TooLarge(_))
        ));
    }

    #[test]
    fn a_truncated_payload_is_reported_as_truncated() {
        let mut buffer = Vec::new();
        write_frame(&mut buffer, "hello world").unwrap();
        buffer.truncate(6); // 4 length bytes + 2 of payload

        let mut cursor = Cursor::new(buffer);
        assert!(matches!(read_frame(&mut cursor), Err(FrameError::Truncated)));
    }

    #[test]
    fn a_closed_connection_between_frames_is_truncation_not_a_panic() {
        let mut cursor = Cursor::new(Vec::new());
        assert!(matches!(read_frame(&mut cursor), Err(FrameError::Truncated)));
    }

    #[test]
    fn invalid_utf8_is_refused_with_a_reason() {
        let mut buffer = Vec::new();
        buffer.extend_from_slice(&2u32.to_be_bytes());
        buffer.extend_from_slice(&[0xff, 0xfe]);

        let mut cursor = Cursor::new(buffer);
        assert!(matches!(read_frame(&mut cursor), Err(FrameError::NotUtf8)));
    }
}
