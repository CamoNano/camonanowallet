pub use log::LevelFilter;
use log::{Metadata, Record, SetLoggerError};

pub struct Logger {
    pub level: LevelFilter,
}
impl Logger {
    pub fn start_logging(self) -> Result<(), SetLoggerError> {
        log::set_max_level(self.level);
        log::set_boxed_logger(Box::new(self))
    }
}
impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }
    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }
    fn flush(&self) {}
}
impl From<LevelFilter> for Logger {
    fn from(value: LevelFilter) -> Self {
        Logger { level: value }
    }
}
