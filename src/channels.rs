// channels.rs - generic channel/DM helpers. No distro or community defaults.
// The app is now a general-purpose IRC client for any server and any channels.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinTarget {
    Channel(String),
    DirectMessage(String),
}

pub fn is_channel_target(name: &str) -> bool {
    let name = name.trim();
    name.starts_with('#')
        || name.starts_with('&')
        || name.starts_with('+')
        || name.starts_with('!')
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

/// Parse a `/join` argument (always treated as a channel).
pub fn parse_join_command(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        None
    } else {
        Some(normalize_channel_name(raw))
    }
}

/// Parse the sidebar join entry: `#channel` joins a channel, plain text opens a DM.
pub fn parse_join_entry(raw: &str) -> Option<JoinTarget> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    if is_channel_target(raw) {
        Some(JoinTarget::Channel(raw.to_string()))
    } else {
        Some(JoinTarget::DirectMessage(raw.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JoinTarget {
    Channel(String),
    DirectMessage(String),
}

pub fn is_channel_target(name: &str) -> bool {
    let name = name.trim();
    name.starts_with('#')
        || name.starts_with('&')
        || name.starts_with('+')
        || name.starts_with('!')
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

/// Parse a `/join` argument (always treated as a channel).
pub fn parse_join_command(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        None
    } else {
        Some(normalize_channel_name(raw))
    }
}

/// Parse the sidebar join entry: `#channel` joins a channel, plain text opens a DM.
pub fn parse_join_entry(raw: &str) -> Option<JoinTarget> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
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
}