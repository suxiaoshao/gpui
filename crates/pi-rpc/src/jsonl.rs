use crate::Error;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

/// The buffer belongs to the reader, so cancelling next() never loses a prefix.
pub(crate) struct Jsonl<R> {
    reader: BufReader<R>,
    buffer: Vec<u8>,
}
impl<R: AsyncRead + Unpin> Jsonl<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::new(reader),
            buffer: Vec::new(),
        }
    }
    pub async fn next(&mut self) -> Result<Option<Vec<u8>>, Error> {
        loop {
            let chunk = self.reader.fill_buf().await?;
            if chunk.is_empty() {
                return if self.buffer.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(self.take()))
                };
            }
            let newline = chunk.iter().position(|&b| b == b'\n');
            let len = newline.unwrap_or(chunk.len());
            self.buffer.extend_from_slice(&chunk[..len]);
            self.reader.consume(len + usize::from(newline.is_some()));
            if newline.is_some() {
                return Ok(Some(self.take()));
            }
        }
    }
    fn take(&mut self) -> Vec<u8> {
        let mut bytes = std::mem::take(&mut self.buffer);
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        bytes
    }
}
