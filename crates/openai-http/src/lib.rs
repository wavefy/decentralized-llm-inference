use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use api_chat::{chat_completions, get_model, list_models};
use poem::{EndpointExt, Route};
use protocol::Model;
use tokio::sync::mpsc::{channel, Receiver, Sender};

mod api_chat;

pub use api_chat::ChatStartRequest;

#[derive(Debug, Clone)]
struct ModelStoreElement {
    model: Model,
    tx: Sender<ChatStartRequest>,
}

#[derive(Debug, Clone, Default)]
pub struct ModelStore {
    models: Arc<RwLock<HashMap<String, ModelStoreElement>>>,
}

impl ModelStore {
    pub fn add_model(&self, model: Model) -> Receiver<ChatStartRequest> {
        let (tx, rx) = channel(1);
        self.models.write().expect("").insert(model.id.clone(), ModelStoreElement { model, tx });
        rx
    }

    pub fn remove_model(&self, model_id: &str) {
        self.models.write().expect("").remove(model_id);
    }

    pub fn models(&self) -> Vec<Model> {
        self.models.read().expect("").values().map(|v| v.model.clone()).collect::<Vec<_>>()
    }

    pub fn model(&self, model_id: &str) -> Option<Model> {
        self.models.read().expect("").get(model_id).map(|v| v.model.clone())
    }

    pub async fn send_model(&self, req: ChatStartRequest) -> Option<()> {
        let elm = self.models.read().expect("").get(&req.req.model).cloned()?;
        elm.tx.send(req).await.ok()?;
        Some(())
    }
}

pub fn create_route(tx: Sender<ChatStartRequest>, models: ModelStore) -> Route {
    Route::new()
        .at("/v1/chat/completions", poem::post(chat_completions).data(tx))
        .at("/v1/models", poem::get(list_models).data(models.clone()))
        .at("/v1/models/:model_id", poem::get(get_model).data(models.clone()))
}
