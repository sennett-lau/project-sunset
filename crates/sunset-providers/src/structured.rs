//! Helpers for structured (JSON) output from models that may wrap it in prose.

/// Find the JSON object in a model reply. Drops `<think>…</think>` reasoning blocks and
/// Markdown code fences, then takes the outermost `{ … }`.
pub fn extract_json(text: &str) -> &str {
    let mut rest = text;
    while let Some(start) = rest.find("<think>") {
        match rest[start..].find("</think>") {
            Some(end) => rest = &rest[start + end + "</think>".len()..],
            None => break,
        }
    }
    match (rest.find('{'), rest.rfind('}')) {
        (Some(a), Some(b)) if a < b => &rest[a..=b],
        _ => rest.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::extract_json;

    #[test]
    fn extracts() {
        assert_eq!(extract_json("{\"a\":1}"), "{\"a\":1}");
        assert_eq!(extract_json("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(
            extract_json("<think>{no}</think>\nHere: {\"a\":{\"b\":2}} done"),
            "{\"a\":{\"b\":2}}"
        );
        assert_eq!(extract_json("no json"), "no json");
    }
}
