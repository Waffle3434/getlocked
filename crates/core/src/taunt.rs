//! The roast shown when a user hits a block. `{site}` is left in templates
//! for the extension to fill with the blocked domain.

pub const SITE: &str = "{site}";
const MAX_NAME: usize = 40;

fn clean_name(name: Option<&str>) -> Option<String> {
    let n = name?.trim();
    (!n.is_empty()).then(|| n.chars().take(MAX_NAME).collect())
}

/// A taunt with `{site}` where the target goes. `bails` is the number of
/// sessions ended early this week, or `None` if it couldn't be read.
pub fn taunt_template(name: Option<&str>, bails: Option<u32>, variant: u64) -> String {
    let Some(n) = clean_name(name) else {
        return match bails {
            None => format!("Come on. {SITE}? LOCK IN."),
            Some(0) => format!("Zero bails this week. Don't ruin it for {SITE}."),
            Some(1) => format!("You already bailed once this week. {SITE}? Really? LOCK IN."),
            Some(b) => {
                format!("You've bailed early {b} times this week, and now you're trying to open {SITE}? LOCK IN.")
            }
        };
    };
    let lines: [String; 2] = match bails {
        None => [format!("Come on {n}, {SITE}? LOCK IN."), format!("{n}. {SITE}. Seriously? LOCK IN.")],
        Some(0) => [
            format!("{n}. Zero bails this week. Don't ruin it for {SITE}."),
            format!("Clean week so far, {n}. {SITE} isn't worth breaking it. LOCK IN."),
        ],
        Some(1) => [
            format!("Come on {n}, you already bailed once this week. {SITE}? Really? LOCK IN."),
            format!("{n}, one early exit this week is plenty. Close {SITE} and LOCK IN."),
        ],
        Some(b) => [
            format!(
                "Come on {n}, you said you'd lock in. You've bailed early {b} times this week, and now you're trying to open {SITE}? LOCK IN."
            ),
            format!("{b} early exits this week, {n}, and now {SITE}? LOCK IN."),
        ],
    };
    lines[(variant % 2) as usize].clone()
}

pub fn taunt(name: Option<&str>, bails: Option<u32>, target: &str, variant: u64) -> String {
    taunt_template(name, bails, variant).replace(SITE, target)
}

/// "hirsh" → "Hirsh", for when the user hasn't set a name.
pub fn default_name(account: &str) -> Option<String> {
    let a = account.trim();
    let mut chars = a.chars();
    let first = chars.next()?;
    Some(first.to_uppercase().chain(chars).collect())
}

/// "Steam.exe" → "Steam".
pub fn app_label(exe: &str) -> String {
    let lower = exe.to_ascii_lowercase();
    match lower.strip_suffix(".exe") {
        Some(_) => exe[..exe.len() - 4].to_string(),
        None => exe.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_line_names_the_target_and_mentions_the_count() {
        for v in 0..4 {
            let t = taunt(Some("Hirsh"), Some(3), "Steam", v);
            assert!(t.contains("Steam") && t.contains('3') && t.contains("Hirsh"), "{t}");
            let one = taunt(Some("Hirsh"), Some(1), "youtube.com", v);
            assert!(one.contains("once") || one.contains("one"), "{one}");
            let zero = taunt(Some("Hirsh"), Some(0), "x.com", v);
            assert!(zero.to_lowercase().contains("zero") || zero.contains("Clean week"), "{zero}");
            assert!(!taunt(Some("Hirsh"), None, "Steam", v).chars().any(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn templates_keep_the_site_gap_and_fit_the_limit() {
        let long = "N".repeat(40);
        for bails in [None, Some(0), Some(1), Some(42)] {
            for name in [None, Some("Hirsh"), Some(long.as_str())] {
                for v in 0..4 {
                    let t = taunt_template(name, bails, v);
                    assert!(t.contains(SITE), "{t}");
                    assert!(t.chars().count() <= 300, "{t}");
                }
            }
        }
    }

    #[test]
    fn works_without_a_name() {
        let t = taunt(None, Some(3), "Steam", 0);
        assert!(t.starts_with(char::is_uppercase) && t.contains("Steam") && t.contains('3'), "{t}");
        assert_eq!(taunt(Some("  "), Some(3), "Steam", 0), t);
    }

    #[test]
    fn default_name_capitalizes_the_account() {
        assert_eq!(default_name("hirsh").as_deref(), Some("Hirsh"));
        assert_eq!(default_name("  ").as_deref(), None);
    }

    #[test]
    fn app_label_drops_the_extension() {
        assert_eq!(app_label("Steam.exe"), "Steam");
        assert_eq!(app_label("Discord.EXE"), "Discord");
        assert_eq!(app_label("tool"), "tool");
    }
}
