use anyhow::{Context, Result};
use aptos_sdk::{
    coin_client::CoinClient,
    move_types::{
        ident_str,
        identifier::Identifier,
        language_storage::ModuleId,
        value::{serialize_values, MoveValue},
    },
    rest_client::{
        aptos_api_types::{Address, ViewFunction},
        error::RestError,
        AptosBaseUrl, Client, ClientBuilder, Response, Transaction,
    },
    transaction_builder::TransactionBuilder,
    types::{
        chain_id::ChainId,
        transaction::{EntryFunction, TransactionPayload},
        LocalAccount,
    },
};
use std::{
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

pub struct OnChainClient {
    pub account: LocalAccount,
    client: Client,
    contract_address: Address,
}

impl OnChainClient {
    pub fn new(account: LocalAccount, chain: AptosBaseUrl, contract_address: &str) -> Self {
        let client = ClientBuilder::new(chain).build();
        let account = account;
        let contract_address = Address::from_str(contract_address).unwrap();
        Self { account, client, contract_address }
    }

    pub async fn get_sequence_number(&self) -> Result<u64, RestError> {
        self.client.get_account(self.account.address()).await.map(|account| account.inner().sequence_number)
    }

    pub async fn get_current_balance(&self) -> Result<u64> {
        let client = self.client.clone();
        let coin_client = CoinClient::new(&client);
        coin_client.get_account_balance(&self.account.address()).await
    }

    pub async fn update_sequence_number(&self) -> Result<(), RestError> {
        let sequence_number: u64 = self.get_sequence_number().await?;
        log::info!("[OnChainClient] update sequence number to {}", sequence_number);
        self.account.set_sequence_number(sequence_number);
        Ok(())
    }

    async fn client_send(&self, payload: TransactionPayload) -> Result<Response<Transaction>, RestError> {
        self.update_sequence_number().await?;
        let chain_id = self.client.get_index().await.context("Failed to get chain ID")?.inner().chain_id;
        let exp_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() + 30;
        let transaction = TransactionBuilder::new(payload, exp_timestamp, ChainId::new(chain_id)).max_gas_amount(100000);
        let txn = self.account.sign_with_transaction_builder(transaction);
        self.client.submit_and_wait(&txn).await
    }

    pub async fn create_session(&self, uuid: u64, session_exp: u64, max_tokens: u64, addresses: Vec<Address>) -> Result<Response<Transaction>, RestError> {
        let payload = TransactionPayload::EntryFunction(EntryFunction::new(
            ModuleId::new(self.contract_address.clone().into(), Identifier::from_str("dllm").unwrap()),
            Identifier::from_str("create_session").unwrap(),
            vec![],
            serialize_values(vec![
                &MoveValue::U64(uuid),
                &MoveValue::U64(session_exp),
                &MoveValue::U64(max_tokens),
                &MoveValue::Vector(addresses.iter().map(|a| MoveValue::Address(a.into())).collect::<Vec<MoveValue>>()),
            ]),
        ));
        log::info!("create_session: {:?}", payload);
        self.client_send(payload).await
    }

    pub async fn my_session_id(&self, uuid: u64) -> Result<u64, RestError> {
        self.get_session_id(self.account.address().into(), uuid).await
    }

    pub async fn get_session_id(&self, client_address: Address, uuid: u64) -> Result<u64, RestError> {
        let view_func = ViewFunction {
            module: ModuleId::new(self.contract_address.clone().into(), ident_str!("dllm").into()),
            function: ident_str!("get_session_id_from_uuid").into(),
            ty_args: vec![],
            args: serialize_values(vec![&MoveValue::Address(client_address.into()), &MoveValue::U64(uuid)]),
        };
        log::info!("get_session_id: {:?}", view_func);

        let response = self.client.view_bcs_with_json_response(&view_func, None).await?;
        log::info!("get_session_id response: {:?}", response);
        let session_id: String = serde_json::from_value(response.inner()[0].clone()).map_err(|e| RestError::Json(e))?;
        return Ok(u64::from_str(session_id.as_str()).unwrap());
    }

    pub async fn update_session_addresses(&self, session_id: u64, addresses: Vec<Address>) -> Result<Response<Transaction>, RestError> {
        let payload = TransactionPayload::EntryFunction(EntryFunction::new(
            ModuleId::new(self.contract_address.clone().into(), Identifier::from_str("dllm").unwrap()),
            Identifier::from_str("update_session_addresses").unwrap(),
            vec![],
            serialize_values(vec![
                &MoveValue::U64(session_id),
                &MoveValue::Vector(addresses.iter().map(|a| MoveValue::Address(a.into())).collect::<Vec<MoveValue>>()),
            ]),
        ));
        self.client_send(payload).await
    }

    pub async fn update_token_count(&self, session_id: u64, token_count: u64) -> Result<Response<Transaction>, RestError> {
        let payload = TransactionPayload::EntryFunction(EntryFunction::new(
            ModuleId::new(self.contract_address.clone().into(), Identifier::from_str("dllm").unwrap()),
            Identifier::from_str("update_token_count").unwrap(),
            vec![],
            serialize_values(vec![&MoveValue::U64(session_id), &MoveValue::U64(token_count)]),
        ));
        self.client_send(payload).await
    }

    pub async fn claim_tokens(&self, client_address: Address, session_id: u64, token_count: u64) -> Result<Response<Transaction>, RestError> {
        let payload = TransactionPayload::EntryFunction(EntryFunction::new(
            ModuleId::new(self.contract_address.clone().into(), Identifier::from_str("dllm").unwrap()),
            Identifier::from_str("claim_tokens").unwrap(),
            vec![],
            serialize_values(vec![&MoveValue::Address(client_address.into()), &MoveValue::U64(session_id), &MoveValue::U64(token_count)]),
        ));
        self.client_send(payload).await
    }
}
