//! Keep expensive gameplay trace logging opt-in on slow storage.
pub fn level_from_setting(value: Option<&str>) -> log::LevelFilter {
    value.and_then(|v| v.parse().ok()).unwrap_or(log::LevelFilter::Warn)
}

pub fn input_trace_from_setting(value: Option<&str>) -> bool {
    matches!(value, Some("1") | Some("true"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn zero_and_unset_disable_input_trace() {
        for v in [None, Some("0"), Some("false"), Some("invalid")] {
            assert!(!input_trace_from_setting(v));
        }
        assert!(input_trace_from_setting(Some("1")));
        assert!(input_trace_from_setting(Some("true")));
    }
    #[test] fn default_preserves_warnings_and_errors() {
        assert_eq!(level_from_setting(None), log::LevelFilter::Warn);
        assert_eq!(level_from_setting(Some("invalid")), log::LevelFilter::Warn);
    }
    #[test] fn diagnostics_are_reversible() {
        assert_eq!(level_from_setting(Some("info")), log::LevelFilter::Info);
        assert_eq!(level_from_setting(Some("debug")), log::LevelFilter::Debug);
        assert_eq!(level_from_setting(Some("off")), log::LevelFilter::Off);
    }
}
