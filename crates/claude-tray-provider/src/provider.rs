//! The contract every usage data source implements.

use async_trait::async_trait;
use claude_tray_core::{UsageSnapshot, WindowKind};

use crate::ProviderError;

/// A source of Claude usage data.
///
/// The rest of the application depends on this trait and on the domain types it
/// returns, never on a particular source. That is what lets the entire user
/// interface be built against [`MockProvider`](crate::MockProvider) while the
/// question of where real data comes from is still open, and what confines the
/// damage to one file when a source changes underneath us.
///
/// See `docs/adr/0002-provider-abstraction.md`.
///
/// # Implementing one
///
/// - `fetch` must not panic on any input. A source that returns something
///   unexpected is a [`ProviderError::parse_failed`], not a crash.
/// - Report honestly through [`Self::capabilities`]. The tooltip hides rows a
///   provider cannot fill, so overstating them produces empty rows rather than
///   a richer display.
/// - Set [`UsageSnapshot::source`] to [`Self::id`], so the user can always see
///   which source produced the number they are looking at.
#[async_trait]
pub trait UsageProvider: core::fmt::Debug + Send + Sync {
    /// Returns the provider's id: lowercase, stable across releases, and what
    /// configuration and [`UsageSnapshot::source`] refer to it by.
    fn id(&self) -> &str;

    /// Describes what this provider can actually populate.
    fn capabilities(&self) -> ProviderCapabilities;

    /// Reads current usage.
    ///
    /// # Errors
    ///
    /// Returns a [`ProviderError`] describing what went wrong and whether
    /// trying again could help.
    async fn fetch(&self) -> Result<UsageSnapshot, ProviderError>;

    /// Reports whether the provider is in a position to serve a fetch.
    ///
    /// Cheap by contract — this is for the menu's status line and for choosing
    /// between providers, so it must not perform the fetch it is describing.
    /// The default answers optimistically, which suits any provider whose only
    /// way to find out is to try.
    async fn health(&self) -> Health {
        Health::Ready
    }
}

/// What a provider is able to report.
///
/// The tooltip and menu use this to omit what the active provider cannot
/// supply. Without it a source that knows only consumption, with no quota
/// limit, would render a percentage row that stays permanently blank.
// Four independent yes/no facts about one provider. The lint's suggested
// remedies — a state machine, or two-variant enums — would replace `limits:
// true` with ceremony that reads worse and says the same thing.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderCapabilities {
    /// The quota windows this provider can report on.
    pub windows: Vec<WindowKind>,
    /// Whether it knows the size of an allowance, and not merely how much has
    /// been consumed. Without this there is no percentage to draw.
    pub limits: bool,
    /// Whether it knows when a window refills.
    pub reset_times: bool,
    /// Whether it knows the subscription plan.
    pub plan: bool,
    /// Whether it knows which model is in use.
    pub model: bool,
}

impl ProviderCapabilities {
    /// A provider that can report nothing — the starting point for building one
    /// up, and what a source reduced to a bare token count would declare.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Whether this provider reports on the given window.
    #[must_use]
    pub fn supports_window(&self, kind: WindowKind) -> bool {
        self.windows.contains(&kind)
    }

    /// Whether this provider can produce a remaining-usage percentage at all,
    /// which needs both a window and a limit for it.
    #[must_use]
    pub fn reports_percentage(&self) -> bool {
        self.limits && !self.windows.is_empty()
    }
}

/// Whether a provider is in a position to be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    /// Ready to serve a fetch.
    Ready,
    /// Usable, but something is wrong — an expiring credential, a source
    /// answering slowly, a partial reading.
    Degraded {
        /// What is wrong, for the menu's status line.
        reason: String,
    },
    /// Not usable until something changes.
    Unavailable {
        /// Why, for the menu's status line.
        reason: String,
    },
}

impl Health {
    /// Whether a fetch is worth attempting.
    #[must_use]
    pub fn is_usable(&self) -> bool {
        !matches!(self, Self::Unavailable { .. })
    }

    /// Returns the explanation, if there is one to give.
    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Ready => None,
            Self::Degraded { reason } | Self::Unavailable { reason } => Some(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Health, ProviderCapabilities};
    use claude_tray_core::WindowKind;

    #[test]
    fn a_provider_reporting_nothing_declares_nothing() {
        let capabilities = ProviderCapabilities::none();

        assert!(!capabilities.reports_percentage());
        assert!(!capabilities.supports_window(WindowKind::Session));
    }

    /// A source that counts tokens but does not know the allowance cannot
    /// produce a percentage, however many windows it lists.
    #[test]
    fn windows_without_limits_are_not_a_percentage() {
        let counts_only = ProviderCapabilities {
            windows: vec![WindowKind::Session, WindowKind::Weekly],
            limits: false,
            ..ProviderCapabilities::none()
        };

        assert!(counts_only.supports_window(WindowKind::Weekly));
        assert!(!counts_only.reports_percentage());
    }

    #[test]
    fn a_window_with_a_limit_is_a_percentage() {
        let full = ProviderCapabilities {
            windows: vec![WindowKind::Session],
            limits: true,
            reset_times: true,
            plan: true,
            model: true,
        };

        assert!(full.reports_percentage());
    }

    #[test]
    fn degraded_is_still_worth_trying() {
        let degraded = Health::Degraded {
            reason: "credential expires tomorrow".to_owned(),
        };

        assert!(degraded.is_usable());
        assert_eq!(degraded.reason(), Some("credential expires tomorrow"));
        assert_eq!(Health::Ready.reason(), None);
        assert!(
            !Health::Unavailable {
                reason: "not signed in".to_owned()
            }
            .is_usable()
        );
    }
}
