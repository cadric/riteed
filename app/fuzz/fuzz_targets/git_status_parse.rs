#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _entries = riteed::fuzzing::parse_git_status_bytes(data);
});
