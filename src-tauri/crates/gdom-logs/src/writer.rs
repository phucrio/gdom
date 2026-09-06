use std::io::{self, Write};

use tracing_subscriber::fmt::MakeWriter;

use crate::redact::redact_secrets;

pub(crate) struct RedactingWriter<W> {
    inner: W,
}

impl<W> RedactingWriter<W> {
    pub const fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W> Write for RedactingWriter<W>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let original_len = buf.len();
        let text = String::from_utf8_lossy(buf);
        let redacted = redact_secrets(&text);
        self.inner.write_all(redacted.as_bytes())?;
        Ok(original_len)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) struct RedactingMakeWriter<M> {
    inner: M,
}

impl<M> RedactingMakeWriter<M> {
    pub const fn new(inner: M) -> Self {
        Self { inner }
    }
}

impl<'a, M> MakeWriter<'a> for RedactingMakeWriter<M>
where
    M: MakeWriter<'a>,
{
    type Writer = RedactingWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter::new(self.inner.make_writer())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::names::REDACTED;

    #[test]
    fn writer_redacts_before_the_sink_sees_the_line() {
        let mut sink = Vec::new();
        {
            let mut writer = RedactingWriter::new(&mut sink);
            writer
                .write_all(b"Authorization: Bearer ya29.live-token\n")
                .expect("write");
            writer.flush().expect("flush");
        }
        let text = String::from_utf8(sink).expect("utf8");
        assert!(text.contains(REDACTED));
        assert!(!text.contains("live-token"));
    }
}
