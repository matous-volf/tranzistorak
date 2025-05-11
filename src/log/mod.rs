use ftail::Ftail;
use ftail::error::FtailError;
use iana_time_zone::get_timezone;
use log::LevelFilter;
use std::fs;
use std::path::Path;

const DAILY_FILE_DIRECTORY_PATH: &str = "logs";
const DAILY_RETENTION_DAYS_COUNT: u64 = 7;

pub(crate) fn initialize_logger() -> Result<(), FtailError> {
    let daily_file_path = Path::new(DAILY_FILE_DIRECTORY_PATH);
    if !daily_file_path.exists() {
        fs::create_dir_all(daily_file_path)
            .unwrap_or_else(|error| panic!("on the daily file log directory creation: {}", error));
    }

    let ftail = Ftail::new()
        .timezone(
            get_timezone()
                .unwrap_or_else(|error| panic!("on getting the timezone: {}", error))
                .parse()
                .unwrap(),
        )
        .console(LevelFilter::Warn)
        .daily_file(DAILY_FILE_DIRECTORY_PATH, LevelFilter::Warn)
        .retention_days(DAILY_RETENTION_DAYS_COUNT);

    ftail.init()
}
