//! Cleaning up domains typed by the user.

/// Turns `https://www.YouTube.com/watch?v=1` into `youtube.com`.
/// Returns `None` if nothing domain-like is left.
pub fn normalize_domain(input: &str) -> Option<String> {
    let mut s = input.trim().to_lowercase();
    if let Some(i) = s.find("://") {
        s = s[i + 3..].to_string();
    }
    let s = s.split(['/', '?', '#']).next().unwrap_or("");
    // Drop credentials and port.
    let s = s.rsplit('@').next().unwrap_or("");
    let s = s.split(':').next().unwrap_or("");
    let s = s.trim_start_matches("*.").trim_matches('.');
    let s = s.strip_prefix("www.").unwrap_or(s);
    let valid = !s.is_empty()
        && s.contains('.')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        && !s.split('.').any(str::is_empty);
    valid.then(|| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::normalize_domain as n;

    #[test]
    fn strips_scheme_path_www_and_port() {
        assert_eq!(n("https://www.YouTube.com/watch?v=1").as_deref(), Some("youtube.com"));
        assert_eq!(n("reddit.com:443").as_deref(), Some("reddit.com"));
        assert_eq!(n("*.twitch.tv").as_deref(), Some("twitch.tv"));
        assert_eq!(n("  m.facebook.com  ").as_deref(), Some("m.facebook.com"));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(n(""), None);
        assert_eq!(n("localhost"), None);
        assert_eq!(n("not a domain.com"), None);
        assert_eq!(n("a..b"), None);
    }
}
