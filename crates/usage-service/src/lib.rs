use anyhow::Result;
use protocol::llm::{EndReq, EndRes, ForwardReq, ForwardRes, StartReq, StartRes};

#[async_trait::async_trait]
pub trait WorkerUsageService: Send + Sync + 'static {
    async fn pre_start(&self, req: StartReq) -> Result<StartReq>;
    async fn post_start(&self, req: StartReq, res: StartRes) -> StartRes;

    async fn pre_end(&self, chat_id: u64, req: EndReq) -> Result<EndReq>;
    async fn post_end(&self, chat_id: u64, req: EndReq, res: EndRes) -> EndRes;

    async fn pre_forward(&self, chat_id: u64, req: ForwardReq) -> Result<ForwardReq>;
    async fn post_forward(&self, chat_id: u64, req: ForwardReq, res: ForwardRes) -> ForwardRes;
}

pub struct PassthroughUsageService;

#[async_trait::async_trait]
impl WorkerUsageService for PassthroughUsageService {
    async fn pre_start(&self, req: StartReq) -> Result<StartReq> {
        Ok(req)
    }

    async fn post_start(&self, _req: StartReq, res: StartRes) -> StartRes {
        res
    }

    async fn pre_end(&self, _chat_id: u64, req: EndReq) -> Result<EndReq> {
        Ok(req)
    }

    async fn post_end(&self, _chat_id: u64, _req: EndReq, res: EndRes) -> EndRes {
        res
    }

    async fn pre_forward(&self, _chat_id: u64, req: ForwardReq) -> Result<ForwardReq> {
        Ok(req)
    }

    async fn post_forward(&self, _chat_id: u64, _req: ForwardReq, res: ForwardRes) -> ForwardRes {
        res
    }
}
