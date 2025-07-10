use chrono::FixedOffset;

pub mod log;
pub mod version;

pub fn format_timestamp(timestamp: i64, offset: i32) -> String {
    // format 1751933700 -> 2002-01-01 00:00:00
    let offset = FixedOffset::east_opt(offset * 60 * 60).unwrap();
    let datetime = chrono::DateTime::from_timestamp(timestamp, 0)
        .unwrap()
        .with_timezone(&offset);
    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}
