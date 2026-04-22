#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let _ = mitm2openapi::tnetstring::parse_all_lenient(&mut &data[..]);
});
