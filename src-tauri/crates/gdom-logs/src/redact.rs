use crate::names::{
    BEARER_PREFIX, GOOGLE_ACCESS_PREFIX, GOOGLE_AUTH_CODE_PREFIX, GOOGLE_REFRESH_PREFIX,
    REDACTED, SECRET_KEYS,
};

pub fn redact_secrets(input: &str) -> String {
    let with_bearer = redact_bearer(input);
    let with_prefixes = redact_prefixed_tokens(&with_bearer);
    redact_keyed_values(&with_prefixes)
}

fn redact_bearer(input: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut pos = 0;
    while let Some(found) = lower[pos..].find(BEARER_PREFIX) {
        let start = pos + found;
        let token_start = start + BEARER_PREFIX.len();
        out.push_str(&input[pos..token_start]);
        let token_len = input[token_start..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .unwrap_or(input.len() - token_start);
        out.push_str(REDACTED);
        pos = token_start + token_len;
    }
    out.push_str(&input[pos..]);
    out
}

fn redact_prefixed_tokens(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while !rest.is_empty() {
        if is_token_left_boundary(out.chars().last()) {
            if let Some(consumed) = take_prefixed_secret(rest, GOOGLE_ACCESS_PREFIX) {
                out.push_str(GOOGLE_ACCESS_PREFIX);
                out.push_str(REDACTED);
                rest = &rest[consumed..];
                continue;
            }
            if let Some(consumed) = take_prefixed_secret(rest, GOOGLE_REFRESH_PREFIX) {
                out.push_str(GOOGLE_REFRESH_PREFIX);
                out.push_str(REDACTED);
                rest = &rest[consumed..];
                continue;
            }
            if let Some(consumed) = take_prefixed_secret(rest, GOOGLE_AUTH_CODE_PREFIX) {
                out.push_str(GOOGLE_AUTH_CODE_PREFIX);
                out.push_str(REDACTED);
                rest = &rest[consumed..];
                continue;
            }
        }
        let Some(next) = rest.chars().next() else {
            break;
        };
        out.push(next);
        rest = &rest[next.len_utf8()..];
    }
    out
}

fn is_token_left_boundary(previous: Option<char>) -> bool {
    match previous {
        None => true,
        Some(c) => {
            c.is_whitespace()
                || matches!(c, '"' | '\'' | '=' | ':' | ',' | '{' | '[' | '(' | '?' | '&')
        }
    }
}

fn take_prefixed_secret(input: &str, prefix: &str) -> Option<usize> {
    let head = input.get(..prefix.len())?;
    if !head.eq_ignore_ascii_case(prefix) {
        return None;
    }
    let consumed = token_char_len(&input[prefix.len()..]);
    Some(prefix.len() + consumed)
}

fn token_char_len(input: &str) -> usize {
    input
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(*c, '-' | '_' | '.'))
        .map(char::len_utf8)
        .sum()
}

fn redact_keyed_values(input: &str) -> String {
    let mut text = input.to_string();
    for key in SECRET_KEYS {
        text = redact_one_key(&text, key);
    }
    text
}

fn redact_one_key(input: &str, key: &str) -> String {
    let lower = input.to_ascii_lowercase();
    let key_lower = key.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut pos = 0;
    while let Some(found) = lower[pos..].find(&key_lower) {
        let key_start = pos + found;
        if !is_key_boundary(&lower, key_start, key_lower.len()) {
            out.push_str(&input[pos..key_start + key_lower.len()]);
            pos = key_start + key_lower.len();
            continue;
        }
        let after_key = key_start + key_lower.len();
        let (sep_end, value_end) = match value_span(&input[after_key..]) {
            Some(span) => span,
            None => {
                out.push_str(&input[pos..after_key]);
                pos = after_key;
                continue;
            }
        };
        out.push_str(&input[pos..after_key + sep_end]);
        out.push_str(REDACTED);
        pos = after_key + value_end;
    }
    out.push_str(&input[pos..]);
    out
}

fn is_key_boundary(lower: &str, key_start: usize, key_len: usize) -> bool {
    let before_ok = key_start == 0
        || !lower.as_bytes()[key_start - 1].is_ascii_alphanumeric();
    let after_index = key_start + key_len;
    let after_ok = after_index >= lower.len()
        || !lower.as_bytes().get(after_index).is_some_and(u8::is_ascii_alphanumeric);
    before_ok && after_ok
}

fn value_span(after_key: &str) -> Option<(usize, usize)> {
    let mut index = 0;
    let bytes = after_key.as_bytes();
    while index < bytes.len() && (bytes[index].is_ascii_whitespace() || bytes[index] == b'"') {
        index += 1;
    }
    if index >= bytes.len() || (bytes[index] != b'=' && bytes[index] != b':') {
        return None;
    }
    index += 1;
    while index < bytes.len() && (bytes[index].is_ascii_whitespace() || bytes[index] == b'"') {
        index += 1;
    }
    let sep_end = index;
    let rest = &after_key[index..];
    let value_len = rest
        .find(|c: char| c.is_whitespace() || matches!(c, '&' | '"' | '\'' | ',' | '}' | ']'))
        .unwrap_or(rest.len());
    if value_len == 0 {
        return None;
    }
    Some((sep_end, sep_end + value_len))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_bearer_access_and_refresh_material() {
        let input = "Authorization: Bearer ya29.abc-secret Authorization: bearer tok123";
        let redacted = redact_secrets(input);
        assert!(redacted.contains(REDACTED));
        assert!(!redacted.contains("abc-secret"));
        assert!(!redacted.contains("tok123"));
        assert!(redacted.contains("Bearer"));
    }

    #[test]
    fn redacts_keyed_json_and_form_secrets() {
        let input = r#"{"access_token":"secret-a","email":"ada@gmail.com"} refresh_token=secret-b clientSecret: "secret-c""#;
        let redacted = redact_secrets(input);
        assert!(!redacted.contains("secret-a"));
        assert!(!redacted.contains("secret-b"));
        assert!(!redacted.contains("secret-c"));
        assert!(redacted.contains("ada@gmail.com"));
        assert!(redacted.contains(REDACTED));
    }

    #[test]
    fn redacts_google_refresh_prefix_without_touching_http_urls() {
        let input = "token=1//0refresh http://127.0.0.1//callback http://example.com/file";
        let redacted = redact_secrets(input);
        assert!(!redacted.contains("0refresh"));
        assert!(redacted.contains("http://127.0.0.1//callback"));
        assert!(redacted.contains("http://example.com/file"));
    }

    #[test]
    fn redacts_google_authorization_code_query() {
        let input = "redirect ?code=4/0Asecret-value&state=xyz encode=4/not-a-code code=200";
        let redacted = redact_secrets(input);
        assert!(!redacted.contains("0Asecret-value"));
        assert!(redacted.contains("encode=4/not-a-code"));
        assert!(redacted.contains("code=200"));
        assert!(redacted.contains("state=xyz"));
    }

    #[test]
    fn keeps_job_metadata() {
        let input = "job=22 file_id=1AbC status=READY_FOR_REVIEW source=ada@gmail.com";
        assert_eq!(redact_secrets(input), input);
    }
}
