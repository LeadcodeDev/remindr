use std::cell::Cell;
use std::ffi::OsStr;
use std::time::{Duration, Instant};

const SWITCH: &str = "REMINDR_TIMING";

pub struct FirstPaint {
    started: Instant,
    enabled: bool,
    done: Cell<bool>,
}

impl FirstPaint {
    pub fn start() -> Self {
        Self::with(enabled(std::env::var_os(SWITCH).as_deref()))
    }

    pub fn with(enabled: bool) -> Self {
        Self {
            started: Instant::now(),
            enabled,
            done: Cell::new(false),
        }
    }

    pub fn finish(&self) -> Option<String> {
        if !self.enabled || self.done.replace(true) {
            return None;
        }
        Some(line(self.started.elapsed()))
    }
}

pub fn enabled(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| value == "1")
}

pub fn line(elapsed: Duration) -> String {
    format!(
        "editor first paint: {:.1} ms",
        elapsed.as_secs_f64() * 1000.
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::time::Duration;

    use super::{FirstPaint, enabled, line};

    #[test]
    fn only_the_value_one_switches_timing_on() {
        assert!(enabled(Some(OsStr::new("1"))));
        assert!(!enabled(Some(OsStr::new("0"))));
        assert!(!enabled(Some(OsStr::new(""))));
        assert!(!enabled(None));
    }

    #[test]
    fn the_line_names_the_milliseconds() {
        assert_eq!(
            line(Duration::from_micros(123_456)),
            "editor first paint: 123.5 ms"
        );
    }

    #[test]
    fn the_first_paint_is_reported_once() {
        let timer = FirstPaint::with(true);
        assert!(timer.finish().is_some_and(|text| text.ends_with(" ms")));
        assert_eq!(timer.finish(), None);
        let silent = FirstPaint::with(false);
        assert_eq!(silent.finish(), None);
    }
}
