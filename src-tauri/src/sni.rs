/// Parses the Server Name Indication (SNI) from a TLS ClientHello packet.
/// Returns Some(domain_name) if successfully found, or None.
pub fn parse_tls_sni(payload: &[u8]) -> Option<String> {
    // Basic TLS Record Check:
    // Byte 0: 0x16 (Handshake record)
    // Byte 1..3: Version (e.g. 0x03 0x01, 0x03 0x03)
    // Byte 3..5: Record Length
    if payload.len() < 5 || payload[0] != 0x16 {
        return None;
    }

    let record_len = u16::from_be_bytes([payload[3], payload[4]]) as usize;
    let handshake = &payload[5..std::cmp::min(payload.len(), 5 + record_len)];

    // Byte 0 of Handshake: 0x01 (ClientHello)
    if handshake.is_empty() || handshake[0] != 0x01 {
        return None;
    }

    // Handshake length (3 bytes)
    if handshake.len() < 4 {
        return None;
    }

    let mut cursor = 4; // Skip type (1) + length (3)

    // Skip client version (2 bytes) + random (32 bytes)
    cursor += 2 + 32;
    if cursor >= handshake.len() {
        return None;
    }

    // Skip Session ID
    let session_id_len = handshake[cursor] as usize;
    cursor += 1 + session_id_len;
    if cursor + 2 > handshake.len() {
        return None;
    }

    // Skip Cipher Suites
    let cipher_suites_len = u16::from_be_bytes([handshake[cursor], handshake[cursor + 1]]) as usize;
    cursor += 2 + cipher_suites_len;
    if cursor + 1 > handshake.len() {
        return None;
    }

    // Skip Compression Methods
    let compression_methods_len = handshake[cursor] as usize;
    cursor += 1 + compression_methods_len;
    if cursor + 2 > handshake.len() {
        return None;
    }

    // Extensions length
    let extensions_len = u16::from_be_bytes([handshake[cursor], handshake[cursor + 1]]) as usize;
    cursor += 2;

    let extensions_end = std::cmp::min(handshake.len(), cursor + extensions_len);

    while cursor + 4 <= extensions_end {
        let ext_type = u16::from_be_bytes([handshake[cursor], handshake[cursor + 1]]);
        let ext_len = u16::from_be_bytes([handshake[cursor + 2], handshake[cursor + 3]]) as usize;
        cursor += 4;

        if cursor + ext_len > extensions_end {
            break;
        }

        // ext_type == 0x0000 is Server Name (SNI)
        if ext_type == 0 {
            let mut sni_cursor = cursor;
            if sni_cursor + 2 > cursor + ext_len {
                break;
            }
            let server_name_list_len =
                u16::from_be_bytes([handshake[sni_cursor], handshake[sni_cursor + 1]]) as usize;
            sni_cursor += 2;

            let list_end = std::cmp::min(cursor + ext_len, sni_cursor + server_name_list_len);

            while sni_cursor + 3 <= list_end {
                let name_type = handshake[sni_cursor]; // 0 = host_name
                let name_len =
                    u16::from_be_bytes([handshake[sni_cursor + 1], handshake[sni_cursor + 2]])
                        as usize;
                sni_cursor += 3;

                if sni_cursor + name_len > list_end {
                    break;
                }

                if name_type == 0 {
                    if let Ok(host_name) =
                        std::str::from_utf8(&handshake[sni_cursor..sni_cursor + name_len])
                    {
                        return Some(host_name.to_lowercase());
                    }
                }
                sni_cursor += name_len;
            }
        }

        cursor += ext_len;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tls_sni() {
        // Sample TLS 1.2 / 1.3 ClientHello packet containing SNI for "api.anthropic.com"
        let hostname = b"api.anthropic.com";
        let mut packet = Vec::new();

        // 1. TLS Record Header: [0x16, 0x03, 0x01, len_hi, len_lo]
        packet.push(0x16); // Handshake
        packet.extend_from_slice(&[0x03, 0x01]); // TLS 1.0 (Record layer)
        packet.extend_from_slice(&[0x00, 0x00]); // Placeholder for record length

        let handshake_start = packet.len();

        // 2. Handshake Header: Type = 0x01 (ClientHello), Length (3 bytes)
        packet.push(0x01);
        packet.extend_from_slice(&[0x00, 0x00, 0x00]); // Placeholder for handshake length

        let client_hello_start = packet.len();

        // Client Version (2 bytes) + Random (32 bytes)
        packet.extend_from_slice(&[0x03, 0x03]); // TLS 1.2
        packet.extend_from_slice(&[0u8; 32]);

        // Session ID: len = 0
        packet.push(0x00);

        // Cipher Suites: len = 2, [0xC0, 0x2F]
        packet.extend_from_slice(&[0x00, 0x02, 0xC0, 0x2F]);

        // Compression Methods: len = 1, [0x00]
        packet.extend_from_slice(&[0x01, 0x00]);

        // Extensions length placeholder
        let ext_len_offset = packet.len();
        packet.extend_from_slice(&[0x00, 0x00]);
        let ext_start = packet.len();

        // Extension: SNI (Type 0x0000)
        packet.extend_from_slice(&[0x00, 0x00]); // Type
        let sni_ext_len = 2 + 1 + 2 + hostname.len();
        packet.extend_from_slice(&(sni_ext_len as u16).to_be_bytes()); // Ext len
        packet.extend_from_slice(&((hostname.len() + 3) as u16).to_be_bytes()); // Server name list len
        packet.push(0x00); // Host name type: 0 (host_name)
        packet.extend_from_slice(&(hostname.len() as u16).to_be_bytes());
        packet.extend_from_slice(hostname);

        let ext_end = packet.len();
        let total_ext_len = (ext_end - ext_start) as u16;
        packet[ext_len_offset..ext_len_offset + 2].copy_from_slice(&total_ext_len.to_be_bytes());

        let client_hello_len = (packet.len() - client_hello_start) as u32;
        let client_hello_len_bytes = client_hello_len.to_be_bytes();
        packet[handshake_start + 1..handshake_start + 4].copy_from_slice(&client_hello_len_bytes[1..4]);

        let record_len = (packet.len() - handshake_start) as u16;
        packet[3..5].copy_from_slice(&record_len.to_be_bytes());

        let parsed = parse_tls_sni(&packet);
        assert_eq!(parsed, Some("api.anthropic.com".to_string()));
    }

    #[test]
    fn test_non_tls_packet() {
        let dummy = [0x47, 0x45, 0x54, 0x20, 0x2F, 0x20]; // "GET / "
        assert_eq!(parse_tls_sni(&dummy), None);
    }
}
