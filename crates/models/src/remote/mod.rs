mod layers_worker;
mod msg;

pub use layers_worker::LayersWorkerRequester;
pub use msg::*;

use crate::Session;

#[allow(async_fn_in_trait)]
pub trait LayersRequester {
    async fn request(&self, session: Session, input: RpcRequest) -> Result<RpcResponse, String>;
    async fn finish(&self, session: Session) -> Result<(), String>;
}

#[allow(async_fn_in_trait)]
pub trait LayersHandler {
    async fn on_request(&self, session: Session, input: RpcRequest) -> Result<RpcResponse, String>;
    async fn on_finish(&self, session: Session) -> Result<(), String>;
}
