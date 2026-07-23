/// Inserts `script_tag` right after the opening `<head ...>` tag (falling
/// back to right after `<html ...>` if there's no `<head>`). Returns `None`
/// when neither anchor is found, so callers can leave genuinely unusual
/// documents untouched rather than corrupting them.
pub fn inject_after_head_open(body: &[u8], script_tag: &str) -> Option<Vec<u8>> {
    let insert_at = find_tag_close(body, b"<head")
        .or_else(|| find_tag_close(body, b"<html"))?;

    let mut out = Vec::with_capacity(body.len() + script_tag.len());
    out.extend_from_slice(&body[..insert_at]);
    out.extend_from_slice(script_tag.as_bytes());
    out.extend_from_slice(&body[insert_at..]);
    Some(out)
}

/// Finds `needle` (an opening tag prefix, e.g. `<head`) case-insensitively
/// and returns the byte offset right after its closing `>`.
fn find_tag_close(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let tag_start = find_ci(haystack, needle)?;
    let close = haystack[tag_start..].iter().position(|&b| b == b'>')?;
    Some(tag_start + close + 1)
}

fn find_ci(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    (0..=haystack.len() - needle.len()).find(|&i| haystack[i..i + needle.len()].eq_ignore_ascii_case(needle))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_right_after_head_tag() {
        let body = b"<html><head><title>Hi</title></head><body></body></html>";
        let out = inject_after_head_open(body, "<script>1</script>").unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "<html><head><script>1</script><title>Hi</title></head><body></body></html>"
        );
    }

    #[test]
    fn is_case_insensitive_and_handles_attributes() {
        let body = b"<HTML><HEAD lang=\"en\"><title>Hi</title></HEAD></HTML>";
        let out = inject_after_head_open(body, "<script>1</script>").unwrap();
        assert!(String::from_utf8(out).unwrap().contains("<HEAD lang=\"en\"><script>1</script>"));
    }

    #[test]
    fn falls_back_to_html_tag_when_no_head() {
        let body = b"<html><body>hi</body></html>";
        let out = inject_after_head_open(body, "<script>1</script>").unwrap();
        assert!(String::from_utf8(out).unwrap().starts_with("<html><script>1</script>"));
    }

    #[test]
    fn returns_none_when_no_anchor_found() {
        let body = b"not really html at all";
        assert!(inject_after_head_open(body, "<script>1</script>").is_none());
    }
}
