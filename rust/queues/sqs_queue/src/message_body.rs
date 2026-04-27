use serde::Serialize;

pub trait ToJsonString {
    fn to_json_string(&self) -> anyhow::Result<String>;
}

impl<T> ToJsonString for T
where
    T: Serialize,
{
    fn to_json_string(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string(self)?)
    }
}
