// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use shell::shell_channel::BlockApplied;

use crate::helpers::FullBlockInfo;
use tezos_context::channel::ContextAction;

/// Request/Response to access the Current Head data from RpcActor
#[derive(Debug, Clone)]
pub enum GetCurrentHead {
    Request,
    Response(Option<BlockApplied>),
}

/// Request/Response to access the Current Head data from RpcActor
#[derive(Debug, Clone)]
pub enum GetFullCurrentHead {
    Request,
    Response(Option<FullBlockInfo>),
}

/// Request list of block header hashes. Will retrieve only applied blocks.
#[derive(Debug, Clone)]
pub enum GetBlocks {
    Request {
        /// Optional starting block hash (formatted string), if left out then we will
        /// assume genesis block hash.
        block_hash: Option<String>,
        /// Required limit of blocks to retrieve.
        limit: usize
    },
    Response(Vec<FullBlockInfo>)
}

/// Request actions for a specific block
#[derive(Debug, Clone)]
pub enum GetBlockActions {
    Request {
        /// Block hash formatted as a string
        block_hash: String,
    },
    Response(Vec<ContextAction>),
}