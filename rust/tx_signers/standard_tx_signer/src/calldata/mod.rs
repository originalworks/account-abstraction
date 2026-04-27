use anyhow::anyhow;

#[cfg(test)]
mod tests;

pub fn parse_calldata(input: &String) -> anyhow::Result<Vec<u8>> {
    let hex = input.strip_prefix("0x").unwrap_or(input);

    if hex.len() % 2 != 0 {
        return Err(anyhow!("CALLDATA has odd-length"));
    }

    if !hex.as_bytes().iter().all(|b| b.is_ascii_hexdigit()) {
        return Err(anyhow!("CALLDATA contains non-hex characters"));
    }

    if hex.len() > 0 && hex.len() < 8 {
        return Err(anyhow!("CALLDATA must be empty or at least 4 bytes"));
    }

    let bytes = hex::decode(hex).map_err(|_| anyhow!("failed to decode CALLDATA hex"))?;

    Ok(bytes)
}
