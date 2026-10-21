#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|bytes: &[u8]| {
    let _ = aegis_protocol::decode_header(bytes);
    if bytes.len() >= aegis_protocol::HEADER_LEN {
        let _ = aegis_protocol::decode_header(&bytes[..aegis_protocol::HEADER_LEN]);
    }
    let _ = aegis_protocol::Hello::decode(bytes);
    let _ = aegis_protocol::Welcome::decode(bytes);
});
