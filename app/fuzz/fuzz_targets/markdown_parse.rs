#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _blocks = riteed::fuzzing::parse_markdown_bytes(data);
});
