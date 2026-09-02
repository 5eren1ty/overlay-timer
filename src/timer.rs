use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerState {
    Paused,
    Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimerReading {
    Remaining(Duration),
    Overtime(Duration),
}

impl TimerReading {
    pub fn is_overtime(self) -> bool {
        matches!(self, Self::Overtime(_))
    }
}

pub struct CountdownTimer {
    total: Duration,
    paused_reading: TimerReading,
    deadline: Option<Instant>,
}

impl CountdownTimer {
    pub fn new(total: Duration) -> Self {
        Self {
            total,
            paused_reading: TimerReading::Remaining(total),
            deadline: None,
        }
    }

    pub fn state(&self) -> TimerState {
        if self.deadline.is_some() {
            TimerState::Running
        } else {
            TimerState::Paused
        }
    }

    pub fn is_running(&self) -> bool {
        self.deadline.is_some()
    }

    pub fn set_total(&mut self, total: Duration) {
        if self.is_running() {
            return;
        }

        self.total = total;
        self.paused_reading = TimerReading::Remaining(total);
    }

    pub fn toggle(&mut self, now: Instant) {
        if self.is_running() {
            self.pause(now);
        } else {
            self.start(now);
        }
    }

    pub fn reset(&mut self) {
        self.paused_reading = TimerReading::Remaining(self.total);
        self.deadline = None;
    }

    pub fn reading(&self, now: Instant) -> TimerReading {
        let Some(deadline) = self.deadline else {
            return self.paused_reading;
        };

        if now < deadline {
            TimerReading::Remaining(deadline.duration_since(now))
        } else {
            TimerReading::Overtime(now.duration_since(deadline))
        }
    }

    fn start(&mut self, now: Instant) {
        self.deadline = Some(match self.paused_reading {
            TimerReading::Remaining(remaining) => now.checked_add(remaining).unwrap_or(now),
            TimerReading::Overtime(overtime) => now.checked_sub(overtime).unwrap_or(now),
        });
    }

    fn pause(&mut self, now: Instant) {
        self.paused_reading = self.reading(now);
        self.deadline = None;
    }
}

pub fn format_duration(duration: Duration) -> String {
    let seconds = duration
        .as_secs()
        .saturating_add(u64::from(duration.subsec_nanos() > 0));
    format_seconds(seconds)
}

pub fn format_reading(reading: TimerReading) -> String {
    match reading {
        TimerReading::Remaining(duration) => format_duration(duration),
        TimerReading::Overtime(duration) => format!("+{}", format_seconds(duration.as_secs())),
    }
}

fn format_seconds(seconds: u64) -> String {
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_changes_to_overtime_at_the_deadline() {
        let start = Instant::now();
        let mut timer = CountdownTimer::new(Duration::from_secs(10));

        timer.toggle(start);
        assert_eq!(timer.state(), TimerState::Running);
        assert_eq!(
            timer.reading(start + Duration::from_secs(3)),
            TimerReading::Remaining(Duration::from_secs(7))
        );
        assert_eq!(
            timer.reading(start + Duration::from_secs(10)),
            TimerReading::Overtime(Duration::ZERO)
        );
        assert_eq!(
            timer.reading(start + Duration::from_secs(13)),
            TimerReading::Overtime(Duration::from_secs(3))
        );
    }

    #[test]
    fn pause_and_resume_preserve_the_remaining_duration() {
        let start = Instant::now();
        let mut timer = CountdownTimer::new(Duration::from_secs(10));

        timer.toggle(start);
        timer.toggle(start + Duration::from_secs(4));
        assert_eq!(timer.state(), TimerState::Paused);
        assert_eq!(
            timer.reading(start + Duration::from_secs(20)),
            TimerReading::Remaining(Duration::from_secs(6))
        );

        timer.toggle(start + Duration::from_secs(20));
        assert_eq!(
            timer.reading(start + Duration::from_secs(22)),
            TimerReading::Remaining(Duration::from_secs(4))
        );
    }

    #[test]
    fn pause_and_resume_preserve_overtime() {
        let start = Instant::now();
        let mut timer = CountdownTimer::new(Duration::from_secs(10));

        timer.toggle(start);
        timer.toggle(start + Duration::from_secs(15));
        assert_eq!(timer.state(), TimerState::Paused);
        assert_eq!(
            timer.reading(start + Duration::from_secs(40)),
            TimerReading::Overtime(Duration::from_secs(5))
        );

        timer.toggle(start + Duration::from_secs(40));
        assert_eq!(timer.state(), TimerState::Running);
        assert_eq!(
            timer.reading(start + Duration::from_secs(43)),
            TimerReading::Overtime(Duration::from_secs(8))
        );
    }

    #[test]
    fn reset_after_overtime_restores_the_configured_total_and_pauses() {
        let start = Instant::now();
        let mut timer = CountdownTimer::new(Duration::from_secs(90));
        timer.toggle(start);
        assert!(
            timer
                .reading(start + Duration::from_secs(120))
                .is_overtime()
        );

        timer.reset();
        assert_eq!(timer.state(), TimerState::Paused);
        assert_eq!(
            timer.reading(start + Duration::from_secs(300)),
            TimerReading::Remaining(Duration::from_secs(90))
        );
    }

    #[test]
    fn formatting_rounds_countdown_up_but_overtime_down() {
        assert_eq!(format_duration(Duration::from_millis(1)), "00:01");
        assert_eq!(format_duration(Duration::from_secs(65)), "01:05");
        assert_eq!(format_duration(Duration::from_secs(3_600)), "60:00");
        assert_eq!(
            format_reading(TimerReading::Overtime(Duration::from_millis(999))),
            "+00:00"
        );
        assert_eq!(
            format_reading(TimerReading::Overtime(Duration::from_secs(65))),
            "+01:05"
        );
    }
}
