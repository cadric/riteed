#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _result = riteed::fuzzing::compute_diff_bytes(data);
});
