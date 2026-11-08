//! Minimal Server-Sent Events parser: turns a byte stream into the `data:` payload of each event.

use std::collections::VecDeque;

use futures::{Stream, StreamExt};

use crate::{ProviderError, Result};

pub fn data_events<S, B, E>(bytes: S) -> impl Stream<Item = Result<String>>
where
    S: Stream<Item = std::result::Result<B, E>> + Unpin,
    B: AsRef<[u8]>,
    E: std::fmt::Display,
{
    struct State<S> {
        bytes: S,
        buf: Vec<u8>,
        ready: VecDeque<String>,
        ended: bool,
    }
    let state = State {
        bytes,
        buf: Vec::new(),
        ready: VecDeque::new(),
        ended: false,
    };
    futures::stream::unfold(state, |mut st| async move {
        loop {
            if let Some(data) = st.ready.pop_front() {
                return Some((Ok(data), st));
            }
            if st.ended {
                return None;
            }
            match st.bytes.next().await {
                Some(Ok(chunk)) => {
                    st.buf
                        .extend(chunk.as_ref().iter().copied().filter(|b| *b != b'\r'));
                    while let Some(pos) = find(&st.buf, b"\n\n") {
                        let event: Vec<u8> = st.buf.drain(..pos + 2).collect();
                        if let Some(data) = event_data(&event) {
                            st.ready.push_back(data);
                        }
                    }
                }
                Some(Err(e)) => {
                    st.ended = true;
                    return Some((Err(ProviderError::Request(e.to_string())), st));
                }
                None => {
                    st.ended = true;
                    let rest = std::mem::take(&mut st.buf);
                    if let Some(data) = event_data(&rest) {
                        st.ready.push_back(data);
                    }
                }
            }
        }
    })
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Join the `data:` lines of one event. Comments and other fields are ignored.
fn event_data(event: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(event);
    let lines: Vec<&str> = text
        .lines()
        .filter_map(|l| l.strip_prefix("data:"))
        .map(|l| l.strip_prefix(' ').unwrap_or(l))
        .collect();
    (!lines.is_empty()).then(|| lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn splits_events_across_chunks() {
        let chunks: Vec<std::result::Result<&[u8], String>> = vec![
            Ok(b": comment\n\ndata: {\"a\":"),
            Ok(b"1}\r\n\r\ndata: [DONE]"),
            Ok(b"\n\n"),
        ];
        let events: Vec<String> = data_events(futures::stream::iter(chunks))
            .map(|r| r.unwrap())
            .collect()
            .await;
        assert_eq!(events, ["{\"a\":1}", "[DONE]"]);
    }
}
