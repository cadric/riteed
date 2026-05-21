#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _offset = riteed::fuzzing::split_frontmatter_bytes(data);
});
