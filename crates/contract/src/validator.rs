use std::sync::RwLock;

use anyhow::Result;
use aptos_sdk::{bcs, crypto::ed25519};
use ed25519_dalek::{ed25519::signature::SignerMut, Signature, SigningKey, VerifyingKey};
use utils::shared_map::SharedHashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Checkpoint {
    pub token_count: u64,
    pub signature: Vec<u8>,
}

pub struct OnChainValidator {
    root_pks: SharedHashMap<u64, ed25519::Ed25519PublicKey>,
    checkpoints: SharedHashMap<u64, Checkpoint>,
    root_sk: RwLock<SigningKey>,
}

impl OnChainValidator {
    pub fn new(root_sk: [u8; 32]) -> Self {
        Self {
            root_pks: SharedHashMap::new(),
            checkpoints: SharedHashMap::new(),
            root_sk: RwLock::new(SigningKey::from_bytes(&root_sk)),
        }
    }

    pub fn add_root_pk(&self, chat_id: u64, pk: ed25519::Ed25519PublicKey) {
        self.root_pks.insert(chat_id, pk);
    }

    pub fn update_checkpoint(&self, chat_id: u64, checkpoint: Checkpoint) -> Result<()> {
        if self.verify_checkpoint(chat_id, &checkpoint) {
            self.checkpoints.insert(chat_id, checkpoint);
            Ok(())
        } else {
            Err(anyhow::anyhow!("Invalid checkpoint"))
        }
    }

    pub fn get_checkpoint(&self, chat_id: u64) -> Option<Checkpoint> {
        self.checkpoints.get_clone(&chat_id)
    }

    pub fn create_checkpoint(&self, chat_id: u64, token_count: u64) -> Checkpoint {
        log::info!("[OnChainValidator] create checkpoint: {token_count} for {chat_id}");
        let signature = self.sign_checkpoint(token_count);
        self.checkpoints.insert(chat_id, Checkpoint { token_count, signature });
        self.checkpoints.get_clone(&chat_id).unwrap()
    }

    pub fn sign_checkpoint(&self, token_count: u64) -> Vec<u8> {
        log::info!("[OnChainValidator] sign checkpoint: {token_count}");
        let msg = bcs::to_bytes(&token_count).unwrap();
        self.root_sk.write().expect("Should get root signingkey lock").sign(&msg).to_bytes().to_vec()
    }

    pub fn verify_checkpoint(&self, chat_id: u64, checkpoint: &Checkpoint) -> bool {
        log::info!("[OnChainValidator] verify checkpoint: {checkpoint:?} for {chat_id}");
        if let Some(pk_raw) = self.root_pks.get_clone(&chat_id).map(|pk| pk.to_bytes()) {
            log::info!("[OnChainValidator] verify checkpoint: {checkpoint:?}");
            let pk = VerifyingKey::from_bytes(&pk_raw).expect("Failed to parse public key");
            let msg = bcs::to_bytes(&checkpoint.token_count).unwrap();
            let mut sig = [0u8; 64];
            sig.copy_from_slice(checkpoint.signature.as_slice());
            pk.verify_strict(&msg, &Signature::from_bytes(&sig)).is_ok()
        } else {
            false
        }
    }
}
