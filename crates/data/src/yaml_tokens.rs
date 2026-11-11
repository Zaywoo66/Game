/// Detect numeric YAML scalars whose spelling is not the content contract's
/// decimal form. yaml_serde normalizes e.g. `+250` before integer deserialization.
/// Quoted strings and comments are left to field validation.
pub(crate) fn noncanonical_integer(source: &str) -> Option<(usize, String)> {
    for (line_number, line) in source.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut quote = None;
        let mut escaped = false;
        let mut start = None;
        for index in 0..=bytes.len() {
            let byte = bytes.get(index).copied().unwrap_or(b' ');
            if let Some(mark) = quote {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' && mark == b'"' {
                    escaped = true;
                } else if byte == mark {
                    quote = None;
                }
                continue;
            }
            if byte == b'"' || byte == b'\'' {
                if let Some(token) = invalid_token(line, start.take(), index) {
                    return Some((line_number + 1, token));
                }
                quote = Some(byte);
                continue;
            }
            if byte == b'#' {
                if let Some(token) = invalid_token(line, start.take(), index) {
                    return Some((line_number + 1, token));
                }
                break;
            }
            if byte.is_ascii_whitespace() || b":,[]{}".contains(&byte) {
                if let Some(token) = invalid_token(line, start.take(), index) {
                    return Some((line_number + 1, token));
                }
            } else if start.is_none() {
                start = Some(index);
            }
        }
    }
    None
}

fn invalid_token(line: &str, start: Option<usize>, end: usize) -> Option<String> {
    let start = start?;
    let token = &line[start..end];
    let bytes = token.as_bytes();
    let numeric = bytes.first().is_some_and(u8::is_ascii_digit)
        || bytes.get(1).is_some_and(u8::is_ascii_digit)
            && matches!(bytes.first(), Some(b'+' | b'-'));
    if !numeric {
        return None;
    }
    let digits = token.strip_prefix('-').unwrap_or(token);
    if digits == "0"
        || digits.starts_with(|ch: char| ('1'..='9').contains(&ch))
            && digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        None
    } else {
        Some(token.to_owned())
    }
}
