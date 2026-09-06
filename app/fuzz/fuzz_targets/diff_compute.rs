#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let (_skipped, _changed_rows, mappings_valid) = riteed::fuzzing::compute_diff_bytes(data);
    assert!(mappings_valid);
});
