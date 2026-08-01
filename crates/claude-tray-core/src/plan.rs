//! The subscription plan a snapshot belongs to.

use core::fmt;

use serde::{Deserialize, Serialize};

/// A Claude subscription plan.
///
/// The set of plans is Anthropic's to change, so an unrecognised name is
/// preserved verbatim in [`Plan::Unknown`] rather than rejected. A new plan
/// appearing upstream must never stop the tray from showing a usage figure —
/// worst case the tooltip prints a name the application has not been taught
/// about.
///
/// ```
/// use claude_tray_core::Plan;
///
/// assert_eq!(Plan::from("Pro"), Plan::Pro);
/// assert_eq!(Plan::from("enterprise"), Plan::Unknown("enterprise".into()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(from = "String", into = "String")]
pub enum Plan {
    /// No subscription.
    Free,
    /// Claude Pro.
    Pro,
    /// Claude Max.
    Max,
    /// A team seat.
    Team,
    /// Pay-as-you-go API access rather than a subscription.
    Api,
    /// A plan this version does not recognise, kept as reported.
    Unknown(String),
}

impl Plan {
    /// Returns the plan's wire name — lowercase, stable, and what gets
    /// persisted.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Free => "free",
            Self::Pro => "pro",
            Self::Max => "max",
            Self::Team => "team",
            Self::Api => "api",
            Self::Unknown(name) => name,
        }
    }

    /// Whether this plan was recognised by this version of the application.
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }
}

impl From<&str> for Plan {
    /// Parses a plan name case-insensitively, ignoring surrounding whitespace.
    fn from(name: &str) -> Self {
        let trimmed = name.trim();
        match trimmed.to_ascii_lowercase().as_str() {
            "free" => Self::Free,
            "pro" => Self::Pro,
            "max" => Self::Max,
            "team" => Self::Team,
            "api" => Self::Api,
            _ => Self::Unknown(trimmed.to_owned()),
        }
    }
}

impl From<String> for Plan {
    fn from(name: String) -> Self {
        Self::from(name.as_str())
    }
}

impl From<Plan> for String {
    fn from(plan: Plan) -> Self {
        match plan {
            Plan::Unknown(name) => name,
            known => known.as_str().to_owned(),
        }
    }
}

impl fmt::Display for Plan {
    /// Renders the plan for a human — `Pro`, `API`, or the raw name of a plan
    /// this version does not know.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Free => "Free",
            Self::Pro => "Pro",
            Self::Max => "Max",
            Self::Team => "Team",
            Self::Api => "API",
            Self::Unknown(name) => name,
        };
        f.write_str(label)
    }
}

#[cfg(test)]
mod tests {
    use super::Plan;

    #[test]
    fn parses_known_names_case_insensitively() {
        assert_eq!(Plan::from("pro"), Plan::Pro);
        assert_eq!(Plan::from("PRO"), Plan::Pro);
        assert_eq!(Plan::from("  Max "), Plan::Max);
        assert_eq!(Plan::from("api"), Plan::Api);
    }

    #[test]
    fn keeps_unknown_names_verbatim() {
        let plan = Plan::from(" Enterprise ");
        assert_eq!(plan, Plan::Unknown("Enterprise".to_owned()));
        assert!(!plan.is_known());
        assert_eq!(plan.to_string(), "Enterprise");
    }

    #[test]
    fn round_trips_through_json_as_a_bare_string() {
        for plan in [
            Plan::Free,
            Plan::Pro,
            Plan::Max,
            Plan::Team,
            Plan::Api,
            Plan::Unknown("enterprise".to_owned()),
        ] {
            let json = serde_json::to_string(&plan).unwrap();
            assert_eq!(json, format!("\"{}\"", plan.as_str()));
            assert_eq!(serde_json::from_str::<Plan>(&json).unwrap(), plan);
        }
    }

    /// A plan added upstream after this release must survive a persist/reload
    /// cycle unchanged, so that a later version can recognise it.
    #[test]
    fn an_unrecognised_plan_survives_persistence() {
        let json = "\"some_future_plan\"";
        let plan: Plan = serde_json::from_str(json).unwrap();
        assert_eq!(serde_json::to_string(&plan).unwrap(), json);
    }

    #[test]
    fn display_and_wire_names_differ_where_it_matters() {
        assert_eq!(Plan::Api.as_str(), "api");
        assert_eq!(Plan::Api.to_string(), "API");
    }
}
