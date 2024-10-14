pub mod client;
pub mod storage;
pub mod validator;

use anyhow::Result;
pub use aptos_sdk;
use aptos_sdk::{
    crypto::ed25519,
    rest_client::{aptos_api_types::Address, error::RestError, Response, Transaction},
};
use protocol::llm::{EndReq, EndRes, ForwardReq, ForwardRes, StartReq, StartRes};
use std::{ops::Range, sync::Arc};
use usage_service::WorkerUsageService;
use validator::Checkpoint;

pub const CONTRACT_ADDRESS: &str = "0xf4289dca4fe79c4e61fe1255d7f47556c38f512b5cf9ddf727f0e44a5c6a6b00";

pub enum OnChainEvent {
    LayerWorkerClaimToken(u64, Address),
}

pub struct OnChainService {
    earning: storage::OnChainStorage,
    spending: storage::OnChainStorage,
    client: Arc<client::OnChainClient>,
    layers: Range<u32>,
    validator: validator::OnChainValidator,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct OnChainServiceMetadata {
    addresses: Vec<Address>,
    root_address: Address,
    root_pk: ed25519::Ed25519PublicKey,
    layers: Vec<u64>,
    checkpoint: Option<Checkpoint>,
}

impl OnChainService {
    pub fn new(account: aptos_sdk::types::LocalAccount, chain: aptos_sdk::rest_client::AptosBaseUrl, layers: Range<u32>) -> Self {
        let sk_bytes = account.private_key().to_bytes().clone();
        let client = Arc::new(client::OnChainClient::new(account, chain, CONTRACT_ADDRESS));
        let validator = validator::OnChainValidator::new(sk_bytes);

        Self {
            client,
            earning: storage::OnChainStorage::new(),
            spending: storage::OnChainStorage::new(),
            layers,
            validator,
        }
    }

    pub fn address(&self) -> Address {
        self.client.account.address().into()
    }

    pub async fn init(&self) {
        self.client.update_sequence_number().await.expect("Failed to update sequence number");
        log::info!("[OnChainWorker] onchain initialized");
    }

    pub async fn create_session(&self, chat_id: u64, max_token: u64, participants: Vec<Address>, layers: Vec<u64>) -> Result<Response<Transaction>, RestError> {
        self.client.create_session(chat_id, max_token, participants, layers).await
    }

    pub fn commit_session(&self, chat_id: u64) {
        let token_count = self.spending.finish(chat_id);
        log::info!("[OnChainService] commit token: {token_count}");
    }

    pub async fn topup_balance(&self) -> Option<u64> {
        self.client.get_topup_balance().await.ok()
    }

    pub async fn current_balance(&self) -> Option<u64> {
        self.client.get_current_balance().await.ok()
    }

    pub fn spending_token_count(&self) -> u64 {
        self.spending.sum()
    }

    pub fn earning_token_count(&self) -> u64 {
        self.earning.sum()
    }
}

#[async_trait::async_trait]
impl WorkerUsageService for OnChainService {
    async fn pre_start(&self, req: StartReq) -> Result<StartReq> {
        if req.chain_index == 0 {
            let address = self.client.account.address();
            let metadata = OnChainServiceMetadata {
                addresses: vec![],
                root_address: address.into(),
                layers: vec![],
                checkpoint: None,
                root_pk: self.client.account.public_key().clone(),
            };
            Ok(StartReq {
                metadata: bincode::serialize(&metadata).expect("Should be able to serialize metadata").into(),
                ..req
            })
        } else {
            log::info!("[OnChainService] pre_start from worker: {req:?}");
            let pre_metadata = bincode::deserialize::<OnChainServiceMetadata>(&req.metadata).expect("Should be able to parse metadata");
            self.validator.add_root_pk(req.chat_id, pre_metadata.root_pk);
            Ok(req)
        }
    }

    async fn post_start(&self, req: StartReq, res: StartRes) -> StartRes {
        let pre_metadata = bincode::deserialize::<OnChainServiceMetadata>(&res.metadata).expect("Should be able to parse metadata");
        log::info!("[OnChainService] pre_metadata: {pre_metadata:?}");
        if req.chain_index == 0 {
            let addresses = pre_metadata.addresses;
            let layers = pre_metadata.layers;
            let max_tokens = req.max_tokens;
            let onchain_res = self.create_session(req.chat_id, max_tokens as u64, addresses, layers).await;
            match onchain_res {
                Ok(_) => {
                    log::info!("[OnChainService] Successfully created session");
                    res
                }
                Err(e) => {
                    log::error!("[OnChainService] Failed to create session: {e:?}");
                    StartRes { success: false, ..res }
                }
            }
        } else {
            let address = self.client.account.address();
            let mut addresses = pre_metadata.addresses.clone();
            let mut layers = pre_metadata.layers.clone();
            addresses.push(address.into());
            layers.push((self.layers.end - req.from_layer + 1) as u64);

            let new_metadata = OnChainServiceMetadata {
                addresses,
                root_address: pre_metadata.root_address,
                layers,
                checkpoint: None,
                root_pk: pre_metadata.root_pk,
            };

            StartRes {
                metadata: bincode::serialize(&new_metadata).expect("Should be able to serialize metadata").into(),
                ..res
            }
        }
    }

    async fn pre_end(&self, chat_id: u64, req: EndReq) -> Result<EndReq> {
        if req.chain_index == 0 {
            let address = self.client.account.address();
            let metadata = OnChainServiceMetadata {
                addresses: vec![],
                root_address: address.into(),
                layers: vec![],
                checkpoint: None,
                root_pk: self.client.account.public_key().clone(),
            };
            self.commit_session(chat_id);
            log::info!("[OnChainService] Successfully committed token count");
            Ok(EndReq {
                metadata: bincode::serialize(&metadata).expect("Should be able to serialize metadata").into(),
                ..req
            })
        } else {
            Ok(req)
        }
    }

    async fn post_end(&self, chat_id: u64, req: EndReq, res: EndRes) -> EndRes {
        if req.chain_index != 0 {
            // this is earning
            let metadata = bincode::deserialize::<OnChainServiceMetadata>(&res.metadata).expect("Should be able to parse metadata");
            let client = self.client.clone();
            let token_count = self.earning.finish(chat_id);

            if let Some(checkpoint) = self.validator.get_checkpoint(chat_id) {
                tokio::spawn(async move {
                    log::info!("[OnChainService] claim token: {token_count}");
                    match client.claim_tokens(metadata.root_address, chat_id, token_count, checkpoint.signature).await {
                        Ok(_) => {
                            log::info!("[OnChainService] Successfully claim {token_count} tokens");
                        }
                        Err(e) => {
                            log::error!("[OnChainService] Failed to claim tokens: {e:?}");
                        }
                    }
                });
            } else {
                log::error!("[OnChainService] Failed to get checkpoint");
                return EndRes { success: false, ..res };
            }
        }
        res
    }

    async fn pre_forward(&self, chat_id: u64, req: ForwardReq) -> Result<ForwardReq> {
        if req.chain_index == 0 {
            self.spending.increase(chat_id, 1);
            let count = self.spending.get(chat_id);
            let checkpoint = self.validator.create_checkpoint(chat_id, count);
            log::info!("checkpoint: {:?}", checkpoint);

            let metadata = OnChainServiceMetadata {
                addresses: vec![],
                root_address: self.client.account.address().into(),
                layers: vec![],
                checkpoint: Some(checkpoint),
                root_pk: self.client.account.public_key().clone(),
            };

            Ok(ForwardReq {
                metadata: bincode::serialize(&metadata).expect("Should be able to serialize metadata").into(),
                ..req
            })
        } else {
            let pre_metadata = bincode::deserialize::<OnChainServiceMetadata>(&req.metadata).expect("Should be able to parse metadata");
            if let Some(checkpoint) = pre_metadata.checkpoint {
                match self.validator.update_checkpoint(chat_id, checkpoint) {
                    Ok(_) => {
                        self.earning.increase(chat_id, 1);
                        Ok(req)
                    }
                    Err(e) => {
                        log::error!("[OnChainService] Failed to update checkpoint: {e:?}");
                        Err(e)
                    }
                }
            } else {
                Err(anyhow::anyhow!("[OnChainService] Failed to get checkpoint, no checkpoint in metadata"))
            }
        }
    }

    async fn post_forward(&self, _chat_id: u64, _req: ForwardReq, res: ForwardRes) -> ForwardRes {
        res
    }
}
