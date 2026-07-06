//! Generic channel/DM helpers. No distro or community defaults.
//! Boulder Relay is a general-purpose IRC client for any server and any channels.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinTarget {
    Channel(String),
    DirectMessage(String),
    /// Multiple targets from a comma-separated list e.g. "#foo,#bar"
    Multi(Vec<JoinTarget>),
}

/// Returns true for any valid IRC channel prefix: #, &, +, !
pub fn is_channel_target(name: &str) -> bool {
    let name = name.trim();
    matches!(
        name.chars().next(),
        Some('#') | Some('&') | Some('+') | Some('!')
    )
}

pub fn normalize_channel_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if is_channel_target(trimmed) {
        trimmed.to_string()
    } else {
        format!("#{trimmed}")
    }
}

/// Parse a `/join` argument. Supports comma-separated multi-join: `/join #a,#b`.
pub fn parse_join_command(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        None
    } else {
        Some(normalize_channel_name(raw))
    }
}

/// Parse comma-separated join commands into a list of channel names.
pub fn parse_join_command_multi(raw: &str) -> Vec<String> {
    raw.split(',')
        .filter_map(|part| parse_join_command(part.trim()))
        .collect()
}

/// Parse the sidebar join entry: `#channel` joins a channel, plain text opens a DM.
/// Supports comma-separated multi-join.
pub fn parse_join_entry(raw: &str) -> Option<JoinTarget> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    // Multi-join: "#foo,#bar" or "#foo, alice"
    if raw.contains(',') {
        let targets: Vec<JoinTarget> = raw
            .split(',')
            .filter_map(|part| parse_join_entry(part.trim()))
            .collect();
        return if targets.is_empty() {
            None
        } else if targets.len() == 1 {
            Some(targets.into_iter().next().unwrap())
        } else {
            Some(JoinTarget::Multi(targets))
        };
    }

    if is_channel_target(raw) {
        Some(JoinTarget::Channel(raw.to_string()))
    } else {
        Some(JoinTarget::DirectMessage(raw.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_join_command_targets() {
        assert_eq!(parse_join_command("gentoo"), Some("#gentoo".into()));
        assert_eq!(parse_join_command("#fedora"), Some("#fedora".into()));
        assert_eq!(parse_join_command("##unofficial"), Some("##unofficial".into()));
        assert_eq!(parse_join_command("&local"), Some("&local".into()));
        assert_eq!(parse_join_command("!service"), Some("!service".into()));
    }

    #[test]
    fn parses_join_entry_channels_and_dms() {
        assert_eq!(
            parse_join_entry("#archlinux"),
            Some(JoinTarget::Channel("#archlinux".into()))
        );
        assert_eq!(
            parse_join_entry("alice"),
            Some(JoinTarget::DirectMessage("alice".into()))
        );
    }

    #[test]
    fn parses_multi_join() {
        let result = parse_join_entry("#foo,#bar");
        assert!(matches!(result, Some(JoinTarget::Multi(_))));
        if let Some(JoinTarget::Multi(targets)) = result {
            assert_eq!(targets.len(), 2);
            assert_eq!(targets[0], JoinTarget::Channel("#foo".into()));
            assert_eq!(targets[1], JoinTarget::Channel("#bar".into()));
        }
    }

    #[test]
    fn multi_join_single_normalizes() {
        // Single entry in comma list should not wrap in Multi
        let result = parse_join_entry("#single,");
        assert_eq!(result, Some(JoinTarget::Channel("#single".into())));
    }

    #[test]
    fn multi_join_command() {
        let results = parse_join_command_multi("#foo,bar,#baz");
        assert_eq!(results, vec!["#foo", "#bar", "#baz"]);
    }

    #[test]
    fn is_channel_target_all_prefixes() {
        assert!(is_channel_target("#general"));
        assert!(is_channel_target("&local"));
        assert!(is_channel_target("+moderated"));
        assert!(is_channel_target("!service"));
        assert!(!is_channel_target("alice"));
        assert!(!is_channel_target(""));
    }
}
