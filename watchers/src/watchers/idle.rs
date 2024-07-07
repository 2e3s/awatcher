use chrono::{DateTime, TimeDelta, Utc};
use std::cmp::max;

pub struct Tracker {
    last_input_time: DateTime<Utc>,
    is_idle: bool,
    is_changed: bool,
    idle_timeout: TimeDelta,

    idle_end: Option<DateTime<Utc>>,
}

pub enum Status {
    Idle {
        changed: bool,
        last_input_time: DateTime<Utc>,
        duration: TimeDelta,
    },
    Active {
        changed: bool,
        last_input_time: DateTime<Utc>,
    },
}

impl Tracker {
    pub fn new(now: DateTime<Utc>, idle_timeout: TimeDelta) -> Self {
        Self {
            last_input_time: now,
            is_idle: false,
            is_changed: false,
            idle_timeout,
            idle_end: None,
        }
    }

    fn set_idle(&mut self, is_idle: bool) {
        self.is_idle = is_idle;
        self.is_changed = true;
    }

    pub fn mark_not_idle(&mut self, now: DateTime<Utc>) {
        debug!("No longer idle");
        self.last_input_time = now;
        self.set_idle(false);

        self.idle_end = Some(now);
    }

    pub fn mark_idle(&mut self, _: DateTime<Utc>) {
        debug!("Idle again");
        self.set_idle(true);
    }

    // The logic is rewritten from the original Python code:
    // https://github.com/ActivityWatch/aw-watcher-afk/blob/ef531605cd8238e00138bbb980e5457054e05248/aw_watcher_afk/afk.py#L73
    pub fn get_with_last_input(
        &mut self,
        now: DateTime<Utc>,
        seconds_since_input: u32,
    ) -> anyhow::Result<Status> {
        let time_since_input = TimeDelta::seconds(i64::from(seconds_since_input));

        self.last_input_time = now - time_since_input;

        if self.is_idle
            && u64::from(seconds_since_input) < self.idle_timeout.num_seconds().try_into().unwrap()
        {
            debug!("No longer idle");
            self.set_idle(false);
        } else if !self.is_idle
            && u64::from(seconds_since_input) >= self.idle_timeout.num_seconds().try_into().unwrap()
        {
            debug!("Idle again");
            self.set_idle(true);
        }

        Ok(self.get_status(now))
    }

    pub fn get_reactive(&mut self, now: DateTime<Utc>) -> anyhow::Result<Status> {
        if !self.is_idle {
            self.last_input_time = max(self.last_input_time, now - self.idle_timeout);

            if let Some(idle_end) = self.idle_end {
                if self.last_input_time < idle_end {
                    self.last_input_time = idle_end;
                }
            }
        }

        Ok(self.get_status(now))
    }

    fn get_status(&mut self, now: DateTime<Utc>) -> Status {
        let result = if self.is_changed {
            if self.is_idle {
                Status::Idle {
                    changed: self.is_changed,
                    last_input_time: self.last_input_time,
                    duration: now - self.last_input_time,
                }
            } else {
                Status::Active {
                    changed: self.is_changed,
                    last_input_time: self.last_input_time,
                }
            }
        } else if self.is_idle {
            Status::Idle {
                changed: self.is_changed,
                last_input_time: self.last_input_time,
                duration: now - self.last_input_time,
            }
        } else {
            Status::Active {
                changed: self.is_changed,
                last_input_time: self.last_input_time,
            }
        };
        self.is_changed = false;

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};
    use rstest::rstest;

    #[rstest]
    fn test_new() {
        let current_time = Utc::now();
        let state = Tracker::new(current_time, Duration::seconds(300));
        assert!(!state.is_idle);
        assert!(!state.is_changed);
    }

    #[rstest]
    fn test_mark_not_idle() {
        let current_time = Utc::now();
        let mut state = Tracker::new(current_time, Duration::seconds(300));

        state.mark_not_idle(current_time);
        assert!(!state.is_idle);
        assert!(state.is_changed);
    }

    #[rstest]
    fn test_mark_idle() {
        let current_time = Utc::now();
        let mut state = Tracker::new(current_time, Duration::seconds(300));

        state.mark_idle(current_time);
        assert!(state.is_idle);
        assert!(state.is_changed);
    }

    #[rstest]
    fn test_send_with_last_input() {
        struct Time {
            now: DateTime<Utc>,
            last_input_ago: u32,
        }

        impl Time {
            fn new() -> Self {
                Self {
                    now: Utc::now(),
                    last_input_ago: 0,
                }
            }

            fn tick_inactive(&mut self) {
                self.now += Duration::seconds(10);
                self.last_input_ago += 10;
            }

            fn tick_active(&mut self) {
                self.now += Duration::seconds(10);
                self.last_input_ago = 0;
            }
        }

        let mut time = Time::new();
        let mut tracker = Tracker::new(time.now, Duration::seconds(30));

        time.tick_inactive();
        let status = tracker
            .get_with_last_input(time.now, time.last_input_ago)
            .unwrap();
        assert!(matches!(status, Status::Active { changed: false, .. }));

        time.tick_inactive();
        let status = tracker
            .get_with_last_input(time.now, time.last_input_ago)
            .unwrap();
        assert!(matches!(status, Status::Active { changed: false, .. }));

        time.tick_inactive();
        let status = tracker
            .get_with_last_input(time.now, time.last_input_ago)
            .unwrap();
        assert!(matches!(status, Status::Idle { changed: true, .. }));

        time.tick_inactive();
        let status = tracker
            .get_with_last_input(time.now, time.last_input_ago)
            .unwrap();
        assert!(matches!(status, Status::Idle { changed: false, .. }));

        time.tick_active();
        let status = tracker
            .get_with_last_input(time.now, time.last_input_ago)
            .unwrap();
        assert!(matches!(status, Status::Active { changed: true, .. }));

        time.tick_active();
        let status = tracker
            .get_with_last_input(time.now, time.last_input_ago)
            .unwrap();
        assert!(matches!(status, Status::Active { changed: false, .. }));
    }

    struct TimeReactive {
        now: DateTime<Utc>,
    }

    impl TimeReactive {
        fn new() -> Self {
            Self {
                now: Utc.with_ymd_and_hms(2021, 3, 1, 13, 30, 0).unwrap(),
            }
        }

        fn tick(&mut self, seconds: i64) {
            self.now += Duration::seconds(seconds);
        }

        fn diff_seconds(&self, other: DateTime<Utc>) -> i64 {
            (self.now - other).num_seconds()
        }

        fn assert_active_status(
            &self,
            status: &Status,
            expected_changed: bool,
            expected_last_input_seconds_ago: i64,
            message: &str,
        ) {
            if let Status::Active {
                changed,
                last_input_time,
            } = status
            {
                assert_eq!(expected_changed, *changed);
                assert_eq!(
                    self.diff_seconds(*last_input_time),
                    expected_last_input_seconds_ago,
                    "{}",
                    message
                );
            } else {
                panic!("Expected active status");
            }
        }

        fn assert_idle_status(
            &self,
            status: &Status,
            expected_changed: bool,
            last_input_ago: i64,
            message: &str,
        ) {
            if let Status::Idle {
                changed,
                last_input_time,
                duration,
            } = status
            {
                assert_eq!(expected_changed, *changed);
                assert_eq!(
                    self.diff_seconds(*last_input_time),
                    last_input_ago,
                    "{}",
                    message
                );
                assert_eq!(duration.num_seconds(), last_input_ago, "{}", message);
            } else {
                panic!("Expected idle status");
            }
        }
    }

    #[rstest]
    fn test_send_reactive() {
        let mut time = TimeReactive::new();
        let mut tracker = Tracker::new(time.now, Duration::seconds(30));

        // 15 seconds of active time
        time.tick(10);
        let status = tracker.get_reactive(time.now).unwrap();
        assert!(matches!(status, Status::Active { changed: false, .. }));

        time.tick(5);
        // 30 seconds of idle time
        tracker.mark_idle(time.now);
        assert!(tracker.is_idle);
        assert!(tracker.is_changed);

        time.tick(5);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_idle_status(
            &status,
            true,
            20,
            "Marked idle 5s ago, last guaranteed activity is on creation as less than 30s interval",
        );

        time.tick(10);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_idle_status(&status, false, 30, "");

        time.tick(10);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_idle_status(&status, false, 40, "");

        time.tick(5);
        tracker.mark_not_idle(time.now);
        assert!(!tracker.is_idle);
        assert!(tracker.is_changed);

        time.tick(5);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_active_status(
            &status,
            true,
            5,
            "Marked active 5s ago which is more recent than 30s interval ago.",
        );
        assert!(
            matches!(status, Status::Active { last_input_time, .. } if last_input_time >= time.now - Duration::seconds(5))
        );

        time.tick(10);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_active_status(&status, false, 15, "");

        time.tick(10);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_active_status(
            &status,
            false,
            25,
            "Marked active 25s ago which is more recent than 30s interval ago.",
        );

        time.tick(10);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_active_status(
            &status,
            false,
            30,
            "Marked active 35s ago, it will be active since 30s ago.",
        );

        time.tick(10);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_active_status(
            &status,
            false,
            30,
            "Marked active 45s ago, it will be active since 30s ago.",
        );

        time.tick(5);
        tracker.mark_idle(time.now);

        time.tick(5);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_idle_status(&status, true, 40, "Last guaranteed activity 5+5+30s ago");

        time.tick(30);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_idle_status(&status, false, 70, "");

        // Short active time
        time.tick(1);
        tracker.mark_not_idle(time.now);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_active_status(&status, true, 0, "");
        assert!(
            matches!(status, Status::Active { last_input_time, .. } if last_input_time == time.now)
        );

        time.tick(5);
        tracker.mark_idle(time.now);
        time.tick(5);
        let status = tracker.get_reactive(time.now).unwrap();
        time.assert_idle_status(&status, true, 10, "");
        assert!(
            matches!(status, Status::Idle { last_input_time, .. } if last_input_time == time.now - Duration::seconds(10))
        );
    }
}
