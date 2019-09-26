// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::cmp;
use std::sync::Arc;

use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockMetaStorageDatabase, BlockStorage, BlockStorageDatabase, BlockStorageReader, IteratorMode, StorageError};
use tezos_encoding::hash::{BlockHash, ChainId};

pub struct BlockState {
    block_storage: BlockStorage,
    block_meta_storage: BlockMetaStorage,
    missing_blocks: Vec<BlockHash>,
    chain_id: ChainId,
}

impl BlockState {
    pub fn new(db: Arc<BlockStorageDatabase>, meta_db: Arc<BlockMetaStorageDatabase>, chain_id: &ChainId) -> Self {
        BlockState {
            block_storage: BlockStorage::new(db),
            block_meta_storage: BlockMetaStorage::new(meta_db),
            missing_blocks: Vec::new(),
            chain_id: chain_id.clone()
        }
    }

    pub fn process_block_header(&mut self, block_header: BlockHeaderWithHash) -> Result<(), StorageError> {
        // check if we already have seen predecessor
        self.push_missing_block(block_header.header.predecessor.clone())?;

        // store block
        self.block_storage.put_block_header(&block_header)?;
        // update meta
        self.block_meta_storage.put_block_header(&block_header)?;

        Ok(())
    }

    #[inline]
    pub fn push_missing_block(&mut self, block_hash: BlockHash) -> Result<(), StorageError> {
        if !self.block_storage.contains(&block_hash)? {
            self.missing_blocks.push(block_hash);
        }
        Ok(())
    }

    #[inline]
    pub fn drain_missing_blocks(&mut self, n: usize) -> Vec<BlockHash> {
        self.missing_blocks
            .drain(0..cmp::min(self.missing_blocks.len(), n))
            .collect()
    }

    #[inline]
    pub fn has_missing_blocks(&self) -> bool {
        !self.missing_blocks.is_empty()
    }

    pub fn hydrate(&mut self) -> Result<(), StorageError> {
        for (key, value) in self.block_meta_storage.iter(IteratorMode::Start)? {
            if value?.predecessor.is_none() {
                self.missing_blocks.push(key?);
            }
        }

        Ok(())
    }

    #[inline]
    pub fn get_chain_id(&self) -> &ChainId {
        &self.chain_id
    }
}