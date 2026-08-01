//! Turning a duration into something worth reading in a tooltip.

use core::fmt::Write as _;
use core::time::Duration;

/// What is shown when there is no duration to show.
pub const UNKNOWN: &str = "—";

/// Formats a duration the way a person would say it.
///
/// Two units at most, largest first, because "1h 48m" is the answer to "how
/// long?" and "1h 48m 12s" is a stopwatch reading. Anything under a minute is
/// `"<1m"` rather than a count of seconds: the refresh interval is measured in
/// tens of seconds, so a seconds figure would be stale before it was read.
///
/// `None` becomes [`UNKNOWN`] — the provider did not report a reset time, which
/// is not the same as reporting that it is due now.
///
/// ```
/// use claude_tray_core::humanize_duration;
/// use core::time::Duration;
///
/// assert_eq!(humanize_duration(Some(Duration::from_secs(6_480))), "1h 48m");
/// assert_eq!(humanize_duration(Some(Duration::from_secs(720))), "12m");
/// assert_eq!(humanize_duration(Some(Duration::from_secs(30))), "<1m");
/// assert_eq!(humanize_duration(None), "—");
/// ```
#[must_use]
pub fn humanize_duration(duration: Option<Duration>) -> String {
    let Some(duration) = duration else {
        return UNKNOWN.to_owned();
    };

    let minutes = duration.as_secs() / 60;
    if minutes == 0 {
        return "<1m".to_owned();
    }

    let hours = minutes / 60;
    let days = hours / 24;

    let (major, major_unit, minor, minor_unit) = if days > 0 {
        (days, 'd', hours % 24, 'h')
    } else if hours > 0 {
        (hours, 'h', minutes % 60, 'm')
    } else {
        (minutes, 'm', 0, 'm')
    };

    let mut text = String::new();
    // Writing to a `String` cannot fail, and the alternative — `format!` twice
    // — allocates a second time to say the same thing.
    let _ = write!(text, "{major}{major_unit}");
    if minor > 0 {
        let _ = write!(text, " {minor}{minor_unit}");
    }
    text
}

#[cfg(test)]
mod tests {
    use super::{UNKNOWN, humanize_duration};
    use core::time::Duration;

    fn humanize(seconds: u64) -> String {
        humanize_duration(Some(Duration::from_secs(seconds)))
    }

    /// P2-30: across seconds, minutes, hours, days, and zero.
    #[test]
    fn reads_the_way_a_person_would_say_it() {
        assert_eq!(humanize(0), "<1m");
        assert_eq!(humanize(1), "<1m");
        assert_eq!(humanize(59), "<1m");
        assert_eq!(humanize(60), "1m");
        assert_eq!(humanize(720), "12m");
        assert_eq!(humanize(59 * 60), "59m");
        assert_eq!(humanize(60 * 60), "1h");
        assert_eq!(humanize(6_480), "1h 48m");
        assert_eq!(humanize(24 * 60 * 60), "1d");
        assert_eq!(humanize(2 * 24 * 60 * 60 + 3 * 60 * 60), "2d 3h");
    }

    #[test]
    fn a_missing_duration_is_a_dash() {
        assert_eq!(humanize_duration(None), UNKNOWN);
    }

    /// A whole unit drops the second half rather than padding it with a zero.
    #[test]
    fn a_zero_minor_unit_is_left_off() {
        assert_eq!(humanize(2 * 60 * 60), "2h");
        assert_eq!(humanize(3 * 24 * 60 * 60), "3d");
    }

    /// Seconds never reach the tooltip: they would be wrong by the time anyone
    /// read them.
    #[test]
    fn seconds_are_truncated_not_shown() {
        assert_eq!(humanize(6_480 + 59), "1h 48m");
        assert_eq!(humanize(90), "1m");
    }

    #[test]
    fn a_very_long_duration_still_reads_in_days() {
        assert_eq!(humanize(31 * 24 * 60 * 60 + 5 * 60 * 60), "31d 5h");
    }
}
