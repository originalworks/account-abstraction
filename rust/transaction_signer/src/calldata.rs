use anyhow::anyhow;

pub fn parse_calldata(input: &String) -> anyhow::Result<Vec<u8>> {
    let hex = input.strip_prefix("0x").unwrap_or(input);

    if hex.is_empty() {
        return Err(anyhow!("CALLDATA is empty"));
    }

    if hex.len() % 2 != 0 {
        return Err(anyhow!("CALLDATA has odd-length"));
    }

    if !hex.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        return Err(anyhow!("CALLDATA contains non-hex characters"));
    }

    if hex.len() < 8 {
        return Err(anyhow!("CALLDATA shorter than 4-byte selector"));
    }

    let bytes = hex::decode(hex).map_err(|_| anyhow!("failed to decode CALLDATA hex"))?;

    Ok(bytes)
}
