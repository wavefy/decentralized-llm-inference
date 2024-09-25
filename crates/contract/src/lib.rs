pub mod client;
pub mod storage;

use anyhow::{Error, Result};
pub use aptos_sdk;
use aptos_sdk::rest_client::{aptos_api_types::Address, error::RestError, Response, Transaction};
use protocol::llm::{EndReq, EndRes, ForwardReq, ForwardRes, StartReq, StartRes};
use tokio::sync::RwLock;
use usage_service::WorkerUsageService;

pub const CONTRACT_ADDRESS: &str = "0x9123e2561d81ba5f77473b8dc664fa75179c841061d12264508894610b9d0b7a";

pub enum OnChainReq {
    ClientCreateSession(u64, u64, u64, Vec<Address>),
    ClientCommitTokenCount(u64),
    LayerWorkerClaimToken(u64, Address),
}

pub struct OnChainService {
    storage: RwLock<storage::OnChainStorage>,
    client: client::OnChainClient,
}

impl OnChainService {
    pub fn new(account: aptos_sdk::types::LocalAccount, chain: aptos_sdk::rest_client::AptosBaseUrl) -> Self {
        let client = client::OnChainClient::new(account, chain, CONTRACT_ADDRESS);
        let storage = RwLock::new(storage::OnChainStorage::new());
        Self { client, storage }
    }

    pub async fn init(&self) {
        self.client.update_sequence_number().await.expect("Failed to update sequence number");
        log::info!("[OnChainWorker] onchain initialized");
        let current_balance = self.client.get_current_balance().await.expect("Failed to get current balance");

        log::info!("[OpenAIServer] current balance: {current_balance}");
    }

    pub async fn increment_chat_token_count(&self, chat_id: u64, token_count: u64) {
        self.storage.write().await.increment_chat_token_count(chat_id, token_count);
    }

    pub async fn create_session(&self, chat_id: u64, exp: u64, max_token: u64, participants: Vec<Address>) -> Result<Response<Transaction>, RestError> {
        self.client.create_session(chat_id, exp, max_token, participants).await
    }

    pub async fn commit_token_count(&self, chat_id: u64) -> Result<Response<Transaction>, RestError> {
        let token_count = self.storage.read().await.get_chat_token_count(chat_id);
        log::info!("[OnChainService] commit_token_count: {token_count}");

        let onchain_session_id = self.client.get_session_id(self.client.account.address().into(), chat_id).await?;
        self.client.update_token_count(onchain_session_id, token_count).await
    }

    pub async fn claim_tokens(&self, chat_id: u64, client_address: Address) -> Result<Response<Transaction>, RestError> {
        let token_count = self.storage.read().await.get_chat_token_count(chat_id);
        log::info!("[OnChainService] claim token: {token_count}");
        let onchain_session_id = self.client.get_session_id(client_address, chat_id).await?;
        self.client.claim_tokens(client_address, onchain_session_id, token_count).await
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct OnChainServiceMetadata {
    addresses: Vec<Address>,
    root_address: Address,
}

#[async_trait::async_trait]
impl WorkerUsageService for OnChainService {
    async fn pre_start(&self, req: StartReq) -> Result<StartReq> {
        if req.chain_index == 0 {
            let address = self.client.account.address();
            let metadata = OnChainServiceMetadata {
                addresses: vec![address.into()],
                root_address: address.into(),
            };
            Ok(StartReq {
                metadata: serde_json::to_string(&metadata).expect("Should be able to serialize metadata").into(),
                ..req
            })
        } else {
            log::info!("[OnChainService] pre_start from worker: {req:?}");
            Ok(req)
        }
    }

    async fn post_start(&self, req: StartReq, res: StartRes) -> StartRes {
        let pre_metadata = serde_json::from_str::<OnChainServiceMetadata>(&String::from_utf8(res.metadata.clone()).unwrap()).expect("Should be able to parse metadata");
        log::info!("[OnChainService] pre_metadata: {pre_metadata:?}");
        if req.chain_index == 0 {
            let addresses = pre_metadata.addresses;
            let onchain_res = self.create_session(req.chat_id, 100, 100, addresses).await;
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
            addresses.push(address.into());
            let new_metadata = OnChainServiceMetadata {
                addresses,
                root_address: address.into(),
            };

            StartRes {
                metadata: serde_json::to_string(&new_metadata).expect("Should be able to serialize metadata").into(),
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
            };
            if let Err(e) = self.commit_token_count(chat_id).await {
                log::error!("[OnChainService] Failed to commit token count: {e:?}");
                return Err(Error::from(e));
            }
            log::info!("[OnChainService] Successfully committed token count");

            Ok(EndReq {
                metadata: serde_json::to_string(&metadata).expect("Should be able to serialize metadata").into(),
                ..req
            })
        } else {
            Ok(req)
        }
    }

    async fn post_end(&self, chat_id: u64, req: EndReq, res: EndRes) -> EndRes {
        log::info!("[OnChainService] post_end: {:?}", req.chain_index);
        if req.chain_index != 0 {
            let metadata = serde_json::from_str::<OnChainServiceMetadata>(&String::from_utf8(req.metadata).unwrap()).expect("Should be able to parse metadata");
            match self.claim_tokens(chat_id, metadata.root_address).await {
                Ok(_) => {
                    log::info!("[OnChainService] Successfully claimed tokens");
                    res
                }
                Err(e) => {
                    log::error!("[OnChainService] Failed to claim tokens: {e:?}");
                    EndRes { success: false, ..res }
                }
            }
        } else {
            res
        }
    }

    async fn pre_forward(&self, chat_id: u64, req: ForwardReq) -> Result<ForwardReq> {
        self.increment_chat_token_count(chat_id, 1).await;
        Ok(req)
    }

    async fn post_forward(&self, _chat_id: u64, _req: ForwardReq, res: ForwardRes) -> ForwardRes {
        res
    }
}
