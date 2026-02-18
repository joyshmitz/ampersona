#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Split input into two halves for two JSON inputs
    let mid = data.len() / 2;
    let (left, right) = data.split_at(mid);
    if let (Ok(a), Ok(b)) = (
        serde_json::from_slice::<serde_json::Value>(left),
        serde_json::from_slice::<serde_json::Value>(right),
    ) {
        let _ = ampersona_core::compose::merge_personas(&a, &b);
    }
});
