use crate::errors::StorageError;
use alloy_eips::BlockNumHash;
use alloy_primitives::{map::B256HashMap, BlockHash, BlockNumber, Bytes, B256};
use ress_protocol::RessProtocolProvider;
use reth_chainspec::ChainSpec;
use reth_primitives::{Header, SealedHeader};
use reth_revm::primitives::Bytecode;
use reth_storage_errors::provider::{ProviderError, ProviderResult};
use std::{collections::HashMap, sync::Arc};

pub mod backends;
use backends::{disk::DiskStorage, memory::MemoryStorage};

/// Wrapper around in-memory and on-disk storages.
#[derive(Clone, Debug)]
pub struct Storage {
    chain_spec: Arc<ChainSpec>,
    pub(crate) memory: MemoryStorage,
    pub(crate) disk: DiskStorage,
}

impl Storage {
    /// Instantiate new storage.
    pub fn new(chain_spec: Arc<ChainSpec>, current_head: BlockNumHash) -> Self {
        let memory = MemoryStorage::new(current_head);
        let disk = DiskStorage::new("test.db");
        Self {
            chain_spec,
            memory,
            disk,
        }
    }

    /// Get chain spec.
    pub fn chain_spec(&self) -> Arc<ChainSpec> {
        self.chain_spec.clone()
    }

    /// Update canonical state.
    pub fn on_chain_update(&self, new: Vec<SealedHeader>, old: Vec<SealedHeader>) {
        for header in old {
            self.memory
                .remove_canonical_hash(header.number, header.hash());
        }
        for header in new {
            self.memory
                .insert_canonical_hash(header.number, header.hash());
        }
    }

    /// Remove old canonical blocks on new finalized hash.
    pub fn on_new_finalized_hash(&self, finalized_hash: B256) -> Result<(), StorageError> {
        if !finalized_hash.is_zero() {
            let upper_bound = self.memory.get_block_number(finalized_hash)?;
            self.memory
                .remove_canonical_until(upper_bound, finalized_hash);
        }
        Ok(())
    }

    /// Return block header by hash.
    pub fn header(&self, hash: B256) -> Option<Header> {
        self.memory.header_by_hash(hash)
    }

    /// Return sealed block header by hash.
    pub fn sealed_header(&self, hash: B256) -> Option<SealedHeader> {
        Some(SealedHeader::new(self.memory.header_by_hash(hash)?, hash))
    }

    /// Insert header into the state.
    pub fn insert_header(&self, header: Header) {
        self.memory.insert_header(header);
    }

    /// Return whether or not the hash is part of the canonical chain.
    pub fn is_canonical(&self, hash: B256) -> bool {
        self.memory.is_canonical_lookup(hash)
    }

    /// Update current canonical head.
    pub fn set_canonical_head(&self, new_head: BlockNumHash) {
        self.memory.set_canonical_head(new_head);
    }

    /// Return current canonical head.
    pub fn get_canonical_head(&self) -> BlockNumHash {
        self.memory.get_canonical_head()
    }

    /// Get contract bytecode from given codehash from the disk
    pub fn get_contract_bytecode(&self, code_hash: B256) -> Result<Bytecode, StorageError> {
        if let Some(bytecode) = self.disk.get_bytecode(code_hash)? {
            return Ok(bytecode);
        }
        Err(StorageError::NoCodeForCodeHash(code_hash))
    }

    /// Filter code hashes only not stored in disk already
    pub fn filter_code_hashes(&self, code_hashes: Vec<B256>) -> Vec<B256> {
        self.disk.filter_code_hashes(code_hashes)
    }

    /// Set safe canonical block hash that historical 256 blocks from canonical head
    ///
    /// It have canonical hash validation check
    pub fn set_canonical_hash(
        &self,
        block_hash: B256,
        block_number: BlockNumber,
    ) -> Result<(), StorageError> {
        self.memory.set_canonical_hash(block_hash, block_number)?;
        Ok(())
    }

    /// Overwrite block hashes mapping
    pub fn overwrite_block_hashes(&self, block_hashes: HashMap<BlockNumber, B256>) {
        self.memory.overwrite_block_hashes(block_hashes);
    }

    /// Overwrite block hashes by passing block headers
    pub fn overwrite_block_hashes_by_headers(&self, block_headers: Vec<Header>) {
        let mut block_hashes = HashMap::new();

        for header in block_headers {
            let block_number = header.number;
            let block_hash = header.hash_slow();
            block_hashes.insert(block_number, block_hash);
        }

        self.memory.overwrite_block_hashes(block_hashes);
    }

    /// Get block hash from memory of target block number
    pub fn get_block_hash(&self, block_number: BlockNumber) -> Result<BlockHash, StorageError> {
        self.memory
            .get_block_hash(block_number)
            .map_err(StorageError::Memory)
    }
}

// TODO: implement
impl RessProtocolProvider for Storage {
    fn header(&self, block_hash: B256) -> ProviderResult<Option<Header>> {
        Ok(self.header(block_hash))
    }
    fn bytecode(&self, code_hash: B256) -> ProviderResult<Option<Bytes>> {
        let bytes = self
            .get_contract_bytecode(code_hash)
            .map_err(|_| ProviderError::StateForHashNotFound(code_hash))?
            .bytes();
        Ok(Some(bytes))
    }

    fn witness(&self, _block_hash: B256) -> ProviderResult<Option<B256HashMap<Bytes>>> {
        Ok(None)
    }
}
