#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _diagnostics = riteed::fuzzing::unsupported_diagnostics_bytes(data);
});
