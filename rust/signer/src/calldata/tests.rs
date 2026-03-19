#[cfg(test)]
mod tests {
    use crate::calldata::parse_calldata;

    #[test]
    fn parses_valid_calldata_with_prefix() {
        let input = "0xdeadbeef".to_string();
        let result = parse_calldata(&input).unwrap();

        assert_eq!(result, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn parses_valid_calldata_without_prefix() {
        let input = "deadbeef".to_string();
        let result = parse_calldata(&input).unwrap();

        assert_eq!(result, vec![0xde, 0xad, 0xbe, 0xef]);
    }

    #[test]
    fn fails_if_empty() {
        let input = "0x".to_string();
        let err = parse_calldata(&input).unwrap_err();

        assert_eq!(err.to_string(), "CALLDATA is empty");
    }

    #[test]
    fn fails_if_odd_length() {
        let input = "0xabc".to_string();
        let err = parse_calldata(&input).unwrap_err();

        assert_eq!(err.to_string(), "CALLDATA has odd-length");
    }

    #[test]
    fn fails_if_non_hex_characters() {
        let input = "0xzzzzzzzz".to_string();
        let err = parse_calldata(&input).unwrap_err();

        assert_eq!(err.to_string(), "CALLDATA contains non-hex characters");
    }

    #[test]
    fn fails_if_shorter_than_selector() {
        let input = "0xdead".to_string(); // 2 bytes only
        let err = parse_calldata(&input).unwrap_err();

        assert_eq!(err.to_string(), "CALLDATA shorter than 4-byte selector");
    }
}
