use std;

use std::fmt::Display;

use std::time::Duration;

#[derive(Default)]
pub(crate) struct Timer {
    pub(crate) writing: Duration,
    pub(crate) reading: Duration,
    pub(crate) sleeping: Duration,
    pub(crate) removing: Duration,
}

impl Display for Timer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.1} s reading ok, {:.1} s writing, {:.1} s sleeping, \
	     {:.1} s removing",
            self.reading.as_millis() as f64 / 1000.0,
            self.writing.as_millis() as f64 / 1000.0,
            self.sleeping.as_millis() as f64 / 1000.0,
            self.removing.as_millis() as f64 / 1000.0,
        )
    }
}
