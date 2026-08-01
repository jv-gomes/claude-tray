//! What the tray currently knows, including the ways it can know nothing.

use chrono::{DateTime, Utc};

use crate::UsageSnapshot;

/// Why usage could not be read.
///
/// Deliberately coarse. The variants exist to drive three decisions — what the
/// icon shows, what the tooltip says, and whether a retry is worth attempting —
/// so a finer taxonomy would add cases nobody branches on. The underlying cause
/// belongs in the log, not in this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, thiserror::Error)]
pub enum ErrorKind {
    /// No usable credentials. Only the user can fix this, so the tray should
    /// say so rather than retry quietly.
    #[error("not signed in")]
    NotAuthenticated,

    /// The machine is offline or the host could not be reached. Expected during
    /// a suspend or a network change, and worth retrying.
    #[error("network unavailable")]
    NetworkUnavailable,

    /// The provider asked us to slow down. Retrying sooner makes it worse.
    #[error("rate limited")]
    RateLimited,

    /// The provider is reachable but cannot serve usage right now.
    #[error("usage provider unavailable")]
    ProviderUnavailable,

    /// A response arrived but did not have the expected shape — usually an
    /// undocumented source changing underneath us.
    #[error("could not parse the provider's response")]
    ParseFailed,

    /// A fault on our side.
    #[error("internal error")]
    Internal,
}

impl ErrorKind {
    /// Whether retrying on the normal refresh schedule could plausibly succeed.
    ///
    /// A retry that cannot succeed is worse than no retry: it burns a request
    /// and, for [`Self::RateLimited`], digs the hole deeper.
    #[must_use]
    pub fn is_retryable(self) -> bool {
        match self {
            Self::NetworkUnavailable | Self::ProviderUnavailable | Self::Internal => true,
            Self::NotAuthenticated | Self::RateLimited | Self::ParseFailed => false,
        }
    }

    /// Whether the user has to do something before usage can be read again.
    #[must_use]
    pub fn needs_user_action(self) -> bool {
        matches!(self, Self::NotAuthenticated)
    }
}

/// The application's view of usage at any moment.
///
/// Every state the tray can be in is one of these, which is what keeps the icon
/// and the tooltip from having to invent a rendering for a half-populated
/// struct.
///
/// [`Self::Stale`] is separate from [`Self::Ok`] on purpose: showing an old
/// reading greyed out is more useful than showing nothing, but it must not be
/// mistaken for a current one.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum UsageState {
    /// Nothing has been read yet, and no refresh is in flight — the state at
    /// startup before the first attempt.
    #[default]
    Unknown,
    /// A refresh is in flight and there is no earlier reading to show.
    Loading,
    /// A current reading.
    Ok(UsageSnapshot),
    /// The last reading, now too old to be trusted.
    Stale {
        /// The reading, kept because an old number beats no number.
        snapshot: UsageSnapshot,
        /// When it stopped being considered current.
        since: DateTime<Utc>,
    },
    /// Usage could not be read and there is no earlier reading to fall back on.
    Error(ErrorKind),
}

impl UsageState {
    /// Returns the snapshot behind this state, current or stale.
    #[must_use]
    pub fn snapshot(&self) -> Option<&UsageSnapshot> {
        match self {
            Self::Ok(snapshot) | Self::Stale { snapshot, .. } => Some(snapshot),
            Self::Unknown | Self::Loading | Self::Error(_) => None,
        }
    }

    /// Returns the error behind this state, if it is an error.
    #[must_use]
    pub fn error(&self) -> Option<ErrorKind> {
        match self {
            Self::Error(kind) => Some(*kind),
            Self::Unknown | Self::Loading | Self::Ok(_) | Self::Stale { .. } => None,
        }
    }

    /// Whether the reading being shown is current.
    #[must_use]
    pub fn is_current(&self) -> bool {
        matches!(self, Self::Ok(_))
    }

    /// Marks the current reading stale as of `since`, leaving every other state
    /// untouched.
    #[must_use]
    pub fn into_stale(self, since: DateTime<Utc>) -> Self {
        match self {
            Self::Ok(snapshot) => Self::Stale { snapshot, since },
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorKind, UsageState};
    use crate::{Plan, Source, UsageSnapshot};
    use chrono::{DateTime, Utc};

    fn instant(text: &str) -> DateTime<Utc> {
        text.parse().unwrap()
    }

    fn snapshot() -> UsageSnapshot {
        UsageSnapshot::new(
            instant("2026-08-01T10:42:30Z"),
            Plan::Pro,
            Vec::new(),
            Source::new("mock"),
        )
    }

    #[test]
    fn starts_out_knowing_nothing() {
        assert_eq!(UsageState::default(), UsageState::Unknown);
        assert_eq!(UsageState::default().snapshot(), None);
    }

    #[test]
    fn both_ok_and_stale_expose_their_snapshot() {
        let ok = UsageState::Ok(snapshot());
        let stale = ok.clone().into_stale(instant("2026-08-01T11:00:00Z"));

        assert_eq!(ok.snapshot(), Some(&snapshot()));
        assert_eq!(stale.snapshot(), Some(&snapshot()));
        assert!(ok.is_current());
        assert!(!stale.is_current(), "a stale reading is not a current one");
    }

    #[test]
    fn going_stale_leaves_other_states_alone() {
        let since = instant("2026-08-01T11:00:00Z");
        for state in [
            UsageState::Unknown,
            UsageState::Loading,
            UsageState::Error(ErrorKind::RateLimited),
        ] {
            assert_eq!(state.clone().into_stale(since), state);
        }
    }

    #[test]
    fn only_the_error_state_carries_an_error() {
        assert_eq!(
            UsageState::Error(ErrorKind::NotAuthenticated).error(),
            Some(ErrorKind::NotAuthenticated)
        );
        assert_eq!(UsageState::Ok(snapshot()).error(), None);
        assert_eq!(UsageState::Loading.error(), None);
    }

    #[test]
    fn retrying_a_rate_limit_or_a_missing_login_is_pointless() {
        assert!(!ErrorKind::RateLimited.is_retryable());
        assert!(!ErrorKind::NotAuthenticated.is_retryable());
        assert!(ErrorKind::NetworkUnavailable.is_retryable());
        assert!(ErrorKind::ProviderUnavailable.is_retryable());
    }

    #[test]
    fn only_a_missing_login_needs_the_user() {
        assert!(ErrorKind::NotAuthenticated.needs_user_action());
        assert!(!ErrorKind::NetworkUnavailable.needs_user_action());
    }

    #[test]
    fn errors_read_as_plain_sentences() {
        assert_eq!(ErrorKind::NotAuthenticated.to_string(), "not signed in");
        assert_eq!(
            ErrorKind::ParseFailed.to_string(),
            "could not parse the provider's response"
        );
    }
}
