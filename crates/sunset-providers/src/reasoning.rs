//! Hide a model's leading `<think>` block without delaying ordinary answer text.
//! Only the response prefix is special; literal tags later in an explanation are preserved.

#[derive(Default)]
pub(crate) struct AnswerFilter {
    pending: String,
    thinking: bool,
    answering: bool,
}

impl AnswerFilter {
    pub fn push(&mut self, text: &str) -> String {
        if self.answering {
            return text.to_owned();
        }
        self.pending.push_str(text);
        loop {
            if self.thinking {
                if let Some(end) = self.pending.find("</think>") {
                    self.pending.drain(..end + "</think>".len());
                    self.thinking = false;
                } else {
                    // Retain only the possible closing-tag prefix, not the reasoning itself.
                    let keep = suffix_prefix(&self.pending, "</think>");
                    self.pending.drain(..self.pending.len() - keep);
                    return String::new();
                }
            } else {
                let trimmed = self.pending.trim_start();
                if let Some(rest) = trimmed.strip_prefix("<think>") {
                    self.pending = rest.to_owned();
                    self.thinking = true;
                } else if "<think>".starts_with(trimmed) {
                    // Whitespace alone must not grow without bound.
                    self.pending = trimmed.to_owned();
                    return String::new();
                } else {
                    self.answering = true;
                    return std::mem::take(&mut self.pending);
                }
            }
        }
    }

    pub fn finish(&mut self) -> String {
        if self.thinking {
            self.pending.clear();
            String::new()
        } else {
            std::mem::take(&mut self.pending)
        }
    }
}

fn suffix_prefix(text: &str, tag: &str) -> usize {
    (1..tag.len())
        .rev()
        .find(|&n| text.ends_with(&tag[..n]))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_reasoning_at_every_utf8_chunk_boundary() {
        let text = " \n<think>諗一諗</think>\n<think>again</think>答案 <think>literal</think>";
        for split in text.char_indices().map(|(i, _)| i) {
            let mut filter = AnswerFilter::default();
            let mut answer = filter.push(&text[..split]);
            answer.push_str(&filter.push(&text[split..]));
            answer.push_str(&filter.finish());
            assert_eq!(answer, "答案 <think>literal</think>", "split {split}");
        }
    }

    #[test]
    fn plain_text_and_unclosed_reasoning() {
        let mut filter = AnswerFilter::default();
        assert_eq!(filter.push("2 < 3"), "2 < 3");
        let mut filter = AnswerFilter::default();
        assert_eq!(filter.push("<think>private reasoning"), "");
        assert_eq!(filter.finish(), "");
    }
}
