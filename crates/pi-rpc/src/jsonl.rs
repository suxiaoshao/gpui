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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    #[tokio::test]
    async fn cancellation_preserves_partial_utf8_and_only_lf_splits() {
        let (mut write, read) = tokio::io::duplex(32);
        let mut reader = Jsonl::new(read);
        write.write_all(b"{\"text\":\"\xe4").await.unwrap();
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(10), reader.next())
                .await
                .is_err()
        );
        write.write_all(b"\xb8\xad\"}\r\n").await.unwrap();
        let bytes = reader.next().await.unwrap().unwrap();
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()["text"],
            "中"
        );
        write
            .write_all("\"中\u{2028}\u{2029}\"\r\n\"tail\"".as_bytes())
            .await
            .unwrap();
        drop(write);
        assert_eq!(
            reader.next().await.unwrap().unwrap(),
            "\"中\u{2028}\u{2029}\"".as_bytes()
        );
        assert_eq!(reader.next().await.unwrap().unwrap(), b"\"tail\"");
        assert!(reader.next().await.unwrap().is_none());
    }
}
