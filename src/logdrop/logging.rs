use chrono;
use log;
use log::{LogRecord, LogLevel, LogMetadata, SetLoggerError};

struct Logger {
    level: LogLevel,
}

impl Logger {
    fn new(level: LogLevel) -> Logger {
        Logger {
            level: level,
        }
    }
}

fn verbosity<'r>(level: LogLevel) -> &'r str {
    match level {
        LogLevel::Trace => "T",
        LogLevel::Debug => "D",
        LogLevel::Info  => "I",
        LogLevel::Warn  => "W",
        LogLevel::Error => "E",
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &LogMetadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &LogRecord) {
        if self.enabled(record.metadata()) {
            let now = chrono::Local::now();
            println!("{}, [{}] -- {} : {}",
                verbosity(record.level()),
                now,
                record.target(),
                record.args()
            );
        }
    }
}

pub fn init(level: LogLevel) -> Result<(), SetLoggerError> {
    log::set_logger(|max| {
        max.set(level.to_log_level_filter());
        Box::new(Logger::new(level))
    })
}
