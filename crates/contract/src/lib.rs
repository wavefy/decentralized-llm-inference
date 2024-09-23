pub mod client;
pub mod storage;

pub use aptos_sdk;
use aptos_sdk::rest_client::{aptos_api_types::Address, error::RestError, Response, Transaction};

pub enum OnChainReq {
    ClientCreateSession(u64, u64, u64, Vec<Address>),
    ClientCommitTokenCount(u64),
    LayerWorkerClaimToken(u64, Address),
}

pub struct OnChainService {
    storage: storage::OnChainStorage,
    client: client::OnChainClient,
}

impl OnChainService {
    pub fn new(account: aptos_sdk::types::LocalAccount, chain: aptos_sdk::rest_client::AptosBaseUrl, contract_address: &str) -> Self {
        let client = client::OnChainClient::new(account, chain, contract_address);
        let storage = storage::OnChainStorage::new();
        Self { client, storage }
    }

    pub async fn init(&mut self) {
        self.client.update_sequence_number().await.expect("Failed to update sequence number");
        log::info!("[OnChainWorker] onchain initialized");
        let current_balance = self.client.get_current_balance().await.expect("Failed to get current balance");

        log::info!("[OpenAIServer] current balance: {current_balance}");
    }

    pub fn increment_chat_token_count(&mut self, chat_id: u64, token_count: u64) {
        self.storage.increment_chat_token_count(chat_id, token_count);
    }

    pub async fn create_session(&mut self, chat_id: u64, exp: u64, max_token: u64, participants: Vec<Address>) -> Result<Response<Transaction>, RestError> {
        self.client.create_session(chat_id, exp, max_token, participants).await
    }

    pub async fn commit_token_count(&mut self, chat_id: u64) -> Result<Response<Transaction>, RestError> {
        let token_count = self.storage.get_chat_token_count(chat_id);

        let onchain_session_id = self.client.get_session_id(self.client.account.address().into(), chat_id).await?;
        self.client.update_token_count(onchain_session_id, token_count).await
    }

    pub async fn claim_tokens(&mut self, chat_id: u64, client_address: Address) -> Result<Response<Transaction>, RestError> {
        let token_count = self.storage.get_chat_token_count(chat_id);
        let onchain_session_id = self.client.get_session_id(client_address, chat_id).await?;
        self.client.claim_tokens(client_address, onchain_session_id, token_count).await
    }
}
