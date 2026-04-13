use unicode_segmentation::UnicodeSegmentation;

pub(crate) fn truncate_graphemes(text: &str, max_graphemes: usize) -> String {
    let graphemes = text.graphemes(true).collect::<Vec<_>>();
    if graphemes.len() <= max_graphemes {
        return text.to_string();
    }
    if max_graphemes <= 3 {
        return graphemes.into_iter().take(max_graphemes).collect();
    }
    let mut out = graphemes
        .into_iter()
        .take(max_graphemes - 3)
        .collect::<String>();
    out.push_str("...");
    out
}

pub(crate) fn format_json_compact(text: &str) -> Option<String> {
    let json = serde_json::from_str::<serde_json::Value>(text).ok()?;
    let pretty = serde_json::to_string_pretty(&json).ok()?;
    let mut result = String::new();
    let mut chars = pretty.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if !escape_next => {
                in_string = !in_string;
                result.push(ch);
            }
            '\\' if in_string => {
                escape_next = !escape_next;
                result.push(ch);
            }
            '\n' | '\r' if !in_string => {}
            ' ' | '\t' if !in_string => {
                if let Some(&next_ch) = chars.peek()
                    && let Some(last_ch) = result.chars().last()
                    && (last_ch == ':' || last_ch == ',')
                    && !matches!(next_ch, '}' | ']')
                {
                    result.push(' ');
                }
            }
            _ => {
                if escape_next && in_string {
                    escape_next = false;
                }
                result.push(ch);
            }
        }
    }

    Some(result)
}
