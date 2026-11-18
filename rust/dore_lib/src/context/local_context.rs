use async_trait::async_trait;

use crate::Context;

pub(crate) struct LocalContext;

#[async_trait]
impl Context for LocalContext {
    async fn build(&self, _service: &[String]) -> anyhow::Result<()> {
        todo!()
    }

    async fn up(&self, _detach: bool, _service: &[String]) -> anyhow::Result<()> {
        todo!()
    }

    async fn down(&self, _timeout_sec: Option<u32>, _drop_volumes: bool, _services: &[String]) -> anyhow::Result<()> {
        todo!()
    }
}