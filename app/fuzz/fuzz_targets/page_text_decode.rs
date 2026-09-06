#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some(prefix) = data.get(..8) else {
        return;
    };
    let Ok(offset_bytes) = <[u8; 8]>::try_from(prefix) else {
        return;
    };
    let offset = u64::from_le_bytes(offset_bytes);
    let payload = &data[8..];
    let (visible_start, visible_end, next_offset) =
        riteed::fuzzing::decode_page_window_fuzz(offset, payload);
    assert!(visible_start <= visible_end);
    if let Ok(length) = u64::try_from(payload.len())
        && let Some(raw_end) = offset.checked_add(length)
    {
        assert!(visible_start >= offset);
        assert!(visible_end <= raw_end);
        assert!(visible_end <= next_offset);
        assert!(next_offset <= raw_end);
        if !payload.is_empty() {
            assert!(next_offset > offset);
        }
    }
});
