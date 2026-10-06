/// Linux ping sockets supply the identifier and ICMPv6 pseudo-header checksum.
pub fn build_echo_request(sequence: u16, payload_size: usize) -> Vec<u8> {
    let mut packet = vec![0; 8 + payload_size];
    packet[0] = 128;
    packet[6..8].copy_from_slice(&sequence.to_be_bytes());
    for (index, byte) in packet[8..].iter_mut().enumerate() {
        *byte = (index & 0xff) as u8;
    }
    packet
}

#[cfg(test)]
mod tests {
    #[test]
    fn echo_header_and_payload() {
        let p = super::build_echo_request(0x1234, 3);
        assert_eq!(&p[..8], &[128, 0, 0, 0, 0, 0, 0x12, 0x34]);
        assert_eq!(&p[8..], &[0, 1, 2]);
        assert_eq!(super::build_echo_request(0, 0).len(), 8);
    }
}
