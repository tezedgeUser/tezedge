// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::persistent::database::{DBError, RocksDBStats};
use failure::Fail;
use serde::Serialize;
use std::collections::HashSet;
use std::array::TryFromSliceError;
use std::mem;

use crate::merkle_storage::{ContextValue, EntryHash};

pub fn size_of_vec<T>(v: &Vec<T>) -> usize {
    mem::size_of::<Vec<T>>() + mem::size_of::<T>() * v.capacity()
}

#[derive(Debug, Fail)]
pub enum StorageBackendError {
    #[fail(display = "RocksDB error: {}", error)]
    RocksDBError { error: rocksdb::Error },
    #[fail(display = "Column family {} is missing", name)]
    MissingColumnFamily { name: &'static str },
    #[fail(display = "Backend Error")]
    BackendError,
    #[fail(display = "SledDB error: {}", error)]
    SledDBError { error: sled::Error },
    #[fail(display = "Guard Poison {} ", error)]
    GuardPoison { error: String },
    #[fail(display = "Serialization error: {:?}", error)]
    SerializationError { error: bincode::Error },
    #[fail(display = "DBError error: {:?}", error)]
    DBError { error: DBError },
    #[fail(display = "Failed to convert hash to array: {}", error)]
    HashConversionError { error: TryFromSliceError },
}

impl From<rocksdb::Error> for StorageBackendError {
    fn from(error: rocksdb::Error) -> Self {
        StorageBackendError::RocksDBError { error }
    }
}

impl From<sled::Error> for StorageBackendError {
    fn from(error: sled::Error) -> Self {
        StorageBackendError::SledDBError { error }
    }
}

impl From<DBError> for StorageBackendError {
    fn from(error: DBError) -> Self {
        StorageBackendError::DBError { error }
    }
}

impl From<bincode::Error> for StorageBackendError {
    fn from(error: bincode::Error) -> Self {
        StorageBackendError::SerializationError { error }
    }
}

impl From<TryFromSliceError> for StorageBackendError {
    fn from(error: TryFromSliceError) -> Self {
        StorageBackendError::HashConversionError { error }
    }
}

impl slog::Value for StorageBackendError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

//TODO TE-432 - create single abstraction for StorageBackend and KeyValueWithSchema
// should be only GC realted trait
pub trait StorageBackend: Send + Sync {
    fn is_persisted(&self) -> bool;
    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError>;
    fn put(&mut self, key: &EntryHash, value: ContextValue) -> Result<bool, StorageBackendError>;
    fn put_batch(
        &mut self,
        batch: Vec<(EntryHash, ContextValue)>,
    ) -> Result<(), StorageBackendError>{
        for (k, v) in batch.into_iter() {
            self.put(&k, v)?;
        }
        Ok(())
    }
    fn merge(&mut self, key: &EntryHash, value: ContextValue) -> Result<(), StorageBackendError>;
    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError>;
    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError>;

    //TODO: gc specific - split this trait
    fn retain(&mut self, pred: HashSet<EntryHash>) -> Result<(), StorageBackendError>{Ok(())}
    fn mark_reused(&mut self, key: EntryHash){}
    fn start_new_cycle(&mut self, last_commit_hash: Option<EntryHash>){}
    fn wait_for_gc_finish(&self){}
    fn total_get_mem_usage(&self) -> Result<usize,StorageBackendError>;
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct StorageBackendStats {
    pub key_bytes: usize,
    pub value_bytes: usize,
    pub reused_keys_bytes: usize,
}

impl StorageBackendStats {
    /// increases `reused_keys_bytes` based on `key`
    pub fn update_reused_keys(&mut self, list: &HashSet<EntryHash>) {
        self.reused_keys_bytes = list.capacity() * mem::size_of::<EntryHash>();
    }

    pub fn total_as_bytes(&self) -> usize {
        self.key_bytes + self.value_bytes + self.reused_keys_bytes
    }
}

impl<'a> std::ops::Add<&'a Self> for StorageBackendStats {
    type Output = Self;

    fn add(self, other: &'a Self) -> Self::Output {
        Self {
            key_bytes: self.key_bytes + other.key_bytes,
            value_bytes: self.value_bytes + other.value_bytes,
            reused_keys_bytes: self.reused_keys_bytes + other.reused_keys_bytes,
        }
    }
}

impl std::ops::Add for StorageBackendStats {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        self + &other
    }
}

impl<'a> std::ops::AddAssign<&'a Self> for StorageBackendStats {
    fn add_assign(&mut self, other: &'a Self) {
        *self = *self + other;
    }
}

impl std::ops::AddAssign for StorageBackendStats {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl<'a> std::ops::Sub<&'a Self> for StorageBackendStats {
    type Output = Self;

    fn sub(self, other: &'a Self) -> Self::Output {
        Self {
            key_bytes: self.key_bytes - other.key_bytes,
            value_bytes: self.value_bytes - other.value_bytes,
            reused_keys_bytes: self.reused_keys_bytes - other.reused_keys_bytes,
        }
    }
}

impl std::ops::Sub for StorageBackendStats {
    type Output = Self;

    fn sub(self, other: Self) -> Self::Output {
        self - &other
    }
}

impl<'a> std::ops::SubAssign<&'a Self> for StorageBackendStats {
    fn sub_assign(&mut self, other: &'a Self) {
        *self = *self - other;
    }
}

impl std::ops::SubAssign for StorageBackendStats {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl<'a> std::iter::Sum<&'a StorageBackendStats> for StorageBackendStats {
    fn sum<I: Iterator<Item = &'a Self>>(iter: I) -> Self {
        iter.fold(StorageBackendStats::default(), |acc, cur| acc + cur)
    }
}

impl From<(&EntryHash, &ContextValue)> for StorageBackendStats {
    fn from((entry_hash, value): (&EntryHash, &ContextValue)) -> Self {
        StorageBackendStats {
            key_bytes: mem::size_of::<EntryHash>(),
            value_bytes: size_of_vec(&value),
            reused_keys_bytes: 0,
        }
    }
}