// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::Path;

use failure::Error;
use slog::{error, info};

use crate::gc::NotGarbageCollected;
use crate::hash::EntryHash;
use crate::persistent::database::DBError;
use crate::persistent::{Flushable, KeyValueStoreBackend, Persistable};
use crate::{ContextKeyValueStoreSchema, ContextValue};

pub struct ReadonlyIpcBackend {
    client: IpcContextClient,
}

// TODO - TE-261: quick hack to make the initializer happy, but must be fixed.
// Probably needs a separate thread for the controller, and communication
// should happen through a channel.
unsafe impl Send for ReadonlyIpcBackend {}
unsafe impl Sync for ReadonlyIpcBackend {}

impl ReadonlyIpcBackend {
    /// Connects the IPC backend to a socket in `socket_path`. This operation is blocking.
    /// Will wait for a few seconds if the socket file is not found yet.
    pub fn connect<P: AsRef<Path>>(socket_path: P) -> Self {
        // TODO - TE-261: remove this expect and return `Result`
        let err_msg = format!(
            "Failed to connect IPC client with path={:?}",
            socket_path.as_ref()
        );
        let client = IpcContextClient::try_connect(socket_path).expect(&err_msg);
        Self { client }
    }
}

impl NotGarbageCollected for ReadonlyIpcBackend {}

impl KeyValueStoreBackend<ContextKeyValueStoreSchema> for ReadonlyIpcBackend {
    fn retain(&self, _predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError> {
        // This context is readonly
        Ok(())
    }

    fn put(&self, _key: &EntryHash, _value: &ContextValue) -> Result<(), DBError> {
        // This context is readonly
        Ok(())
    }

    fn delete(&self, _key: &EntryHash) -> Result<(), DBError> {
        // This context is readonly
        Ok(())
    }

    fn merge(&self, _key: &EntryHash, _value: &ContextValue) -> Result<(), DBError> {
        // This context is readonly
        Ok(())
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        self.client
            .get_entry(key.clone())
            .map_err(|reason| DBError::IpcAccessError { reason })
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        self.client
            .contains_entry(key.clone())
            .map_err(|reason| DBError::IpcAccessError { reason })
    }

    fn write_batch(&self, _batch: Vec<(EntryHash, ContextValue)>) -> Result<(), DBError> {
        // This context is readonly
        Ok(())
    }

    fn total_get_mem_usage(&self) -> Result<usize, DBError> {
        Ok(0)
    }
}

impl Flushable for ReadonlyIpcBackend {
    fn flush(&self) -> Result<(), Error> {
        Ok(())
    }
}

impl Persistable for ReadonlyIpcBackend {
    fn is_persistent(&self) -> bool {
        false
    }
}

// IPC communication

use std::{cell::RefCell, time::Duration};

use failure::Fail;
use ipc::{IpcClient, IpcError, IpcReceiver, IpcSender, IpcServer};
use serde::{Deserialize, Serialize};
use slog::{warn, Logger};
use strum_macros::IntoStaticStr;

/// This request is generated by a readonly protool runner and is received by the writable protocol runner.
#[derive(Serialize, Deserialize, Debug, IntoStaticStr)]
enum ContextRequest {
    GetEntry(EntryHash),
    ContainsEntry(EntryHash),

    ShutdownCall, // TODO: is this required?
}

/// This is generated as a response to the `ContextRequest` command.
#[derive(Serialize, Deserialize, Debug, IntoStaticStr)]
enum ContextResponse {
    GetEntryResponse(Result<Option<ContextValue>, String>),
    ContainsEntryResponse(Result<bool, String>),

    ShutdownResult,
}

#[derive(Fail, Debug)]
pub enum ContextError {
    #[fail(display = "Context get entry error: {}", reason)]
    GetEntryError { reason: String },
    #[fail(display = "Context contains entry error: {}", reason)]
    ContainsEntryError { reason: String },
}

/// Errors generated by `protocol_runner`.
#[derive(Fail, Debug)]
pub enum ContextServiceError {
    /// Generic IPC communication error. See `reason` for more details.
    #[fail(display = "IPC error: {}", reason)]
    IpcError { reason: IpcError },
    /// Tezos protocol error.
    #[fail(display = "Protocol error: {}", reason)]
    ContextError { reason: ContextError },
    /// Unexpected message was received from IPC channel
    #[fail(display = "Received unexpected message: {}", message)]
    UnexpectedMessage { message: &'static str },
    /// Lock error
    #[fail(display = "Lock error: {:?}", message)]
    LockPoisonError { message: String },
}

impl<T> From<std::sync::PoisonError<T>> for ContextServiceError {
    fn from(source: std::sync::PoisonError<T>) -> Self {
        Self::LockPoisonError {
            message: source.to_string(),
        }
    }
}

impl slog::Value for ContextServiceError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

impl From<IpcError> for ContextServiceError {
    fn from(error: IpcError) -> Self {
        ContextServiceError::IpcError { reason: error }
    }
}

impl From<ContextError> for ContextServiceError {
    fn from(error: ContextError) -> Self {
        ContextServiceError::ContextError { reason: error }
    }
}

/// IPC context server that listens for new connections.
pub struct IpcContextListener(IpcServer<ContextRequest, ContextResponse>);

pub struct ContextIncoming<'a> {
    listener: &'a mut IpcContextListener,
}

struct IpcClientIO {
    rx: IpcReceiver<ContextResponse>,
    tx: IpcSender<ContextRequest>,
}

struct IpcServerIO {
    rx: IpcReceiver<ContextRequest>,
    tx: IpcSender<ContextResponse>,
}

/// Encapsulate IPC communication.
pub struct IpcContextClient {
    io: RefCell<IpcClientIO>,
}

pub struct IpcContextServer {
    io: RefCell<IpcServerIO>,
}

/// IPC context client for readers.
impl IpcContextClient {
    const TIMEOUT: Duration = Duration::from_secs(30);

    pub fn try_connect<P: AsRef<Path>>(socket_path: P) -> Result<Self, IpcError> {
        // TODO - TE-261: do this in a better way
        for _ in 0..5 {
            if socket_path.as_ref().exists() {
                break;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        let ipc_client: IpcClient<ContextResponse, ContextRequest> = IpcClient::new(socket_path);
        let (rx, tx) = ipc_client.connect()?;
        let io = RefCell::new(IpcClientIO { rx, tx });
        Ok(Self { io })
    }

    /// Get entry by hash
    pub fn get_entry(
        &self,
        entry_hash: EntryHash,
    ) -> Result<Option<ContextValue>, ContextServiceError> {
        let mut io = self.io.borrow_mut();
        io.tx.send(&ContextRequest::GetEntry(entry_hash))?;

        // this might take a while, so we will use unusually long timeout
        match io
            .rx
            .try_receive(Some(Self::TIMEOUT), Some(IpcContextListener::IO_TIMEOUT))?
        {
            ContextResponse::GetEntryResponse(result) => {
                result.map_err(|err| ContextError::GetEntryError { reason: err }.into())
            }
            message => Err(ContextServiceError::UnexpectedMessage {
                message: message.into(),
            }),
        }
    }

    /// Check if entry with hash exists
    pub fn contains_entry(&self, entry_hash: EntryHash) -> Result<bool, ContextServiceError> {
        let mut io = self.io.borrow_mut();
        io.tx.send(&ContextRequest::ContainsEntry(entry_hash))?;

        // this might take a while, so we will use unusually long timeout
        match io
            .rx
            .try_receive(Some(Self::TIMEOUT), Some(IpcContextListener::IO_TIMEOUT))?
        {
            ContextResponse::ContainsEntryResponse(result) => {
                result.map_err(|err| ContextError::ContainsEntryError { reason: err }.into())
            }
            message => Err(ContextServiceError::UnexpectedMessage {
                message: message.into(),
            }),
        }
    }
}

impl<'a> Iterator for ContextIncoming<'a> {
    type Item = Result<IpcContextServer, IpcError>;
    fn next(&mut self) -> Option<Result<IpcContextServer, IpcError>> {
        Some(self.listener.accept())
    }
}

impl IpcContextListener {
    const IO_TIMEOUT: Duration = Duration::from_secs(10);

    /// Create new IPC endpoint
    pub fn try_new<P: AsRef<Path>>(socket_path: P) -> Result<Self, IpcError> {
        Ok(IpcContextListener(IpcServer::bind_path(socket_path)?))
    }

    /// Start accepting incoming IPC connections.
    ///
    /// Returns an [`ipc context server`](IpcContextServer) if new IPC channel is successfully created.
    /// This is a blocking operation.
    pub fn accept(&mut self) -> Result<IpcContextServer, IpcError> {
        let (rx, tx) = self.0.accept()?;

        Ok(IpcContextServer {
            io: RefCell::new(IpcServerIO { rx, tx }),
        })
    }

    /// Returns an iterator over the connections being received on this context IPC listener.
    pub fn incoming(&mut self) -> ContextIncoming<'_> {
        ContextIncoming { listener: self }
    }

    /// Starts accepting connections.
    ///
    /// A new thread is launched to serve each connection.
    pub fn handle_incoming_connections(&mut self, log: &Logger) {
        for connection in self.incoming() {
            match connection {
                Err(err) => {
                    error!(&log, "Error accepting IPC connection: {:?}", err)
                }
                Ok(server) => {
                    info!(&log, "Accepted context IPC connection");
                    let log = log.clone();
                    std::thread::spawn(move || {
                        if let Err(err) = server.process_context_requests(&log) {
                            error!(
                                &log,
                                "Error when processing context IPC requests: {:?}", err
                            );
                        }
                    });
                }
            }
        }
    }
}

impl IpcContextServer {
    /// Listen to new connections from context readers.
    /// Begin receiving commands from context readers until `ShutdownCall` command is received.
    pub fn process_context_requests(&self, log: &Logger) -> Result<(), IpcError> {
        let mut io = self.io.borrow_mut();
        loop {
            let cmd = io.rx.receive()?;
            match cmd {
                ContextRequest::GetEntry(hash) => {
                    // TODO - TE-261: remove unwrap
                    let index = crate::ffi::get_context_index().unwrap();
                    let res = index
                        .find_entry_bytes(&hash)
                        .map_err(|err| format!("Context error: {:?}", err));
                    io.tx.send(&ContextResponse::GetEntryResponse(res))?;
                }
                ContextRequest::ContainsEntry(hash) => {
                    // TODO - TE-261: remove unwrap
                    let index = crate::ffi::get_context_index().unwrap();
                    let res = index
                        .contains(&hash)
                        .map_err(|err| format!("Context error: {:?}", err));
                    io.tx.send(&ContextResponse::ContainsEntryResponse(res))?;
                }

                ContextRequest::ShutdownCall => {
                    if let Err(e) = io.tx.send(&ContextResponse::ShutdownResult) {
                        warn!(log, "Failed to send shutdown response"; "reason" => format!("{}", e));
                    }

                    break;
                }
            }
        }

        Ok(())
    }
}