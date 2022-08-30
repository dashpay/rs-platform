use super::validation_result::TimeWindowValidationResult;

const BLOCK_TIME_WINDOW_MINUTES: u64 = 5;
const BLOCK_TIME_WINDOW_MILLIS: u64 = BLOCK_TIME_WINDOW_MINUTES * 60 * 1000;

pub fn validate_time_in_block_time_window(
    last_block_header_time_millis: u64,
    time_to_check_millis: u64,
) -> TimeWindowValidationResult {
    let time_window_start = last_block_header_time_millis - BLOCK_TIME_WINDOW_MILLIS;
    let time_window_end = last_block_header_time_millis + BLOCK_TIME_WINDOW_MILLIS;

    let valid =
        time_to_check_millis >= time_window_start && time_to_check_millis <= time_window_end;

    TimeWindowValidationResult {
        time_window_start,
        time_window_end,
        valid,
    }
}
