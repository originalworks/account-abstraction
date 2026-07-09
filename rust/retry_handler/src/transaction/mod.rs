pub mod blob_tx;
pub mod standard_tx;

use anyhow::bail;
use network_db::networks::Network;

pub trait FeeBufferExt {
    fn apply_fee_buffer(&mut self, network: &Network) -> anyhow::Result<()>
    where
        Self: Sized;
}

fn try_option_i64_to_option_u64(input: Option<i64>) -> anyhow::Result<Option<u64>> {
    let Some(output_i64) = input else {
        bail!("Can't parse None value");
    };

    let output = Some(u64::try_from(output_i64)?);

    Ok(output)
}
