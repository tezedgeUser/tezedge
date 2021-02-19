// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::collections::VecDeque;

use slog::{info, Drain, Level, Logger};
use tokio::signal;

mod configuration;
mod deploy_with_compose;
mod display_info;
mod image;
mod monitors;
mod node;
mod slack;
mod rpc;

use crate::image::{Debugger, Explorer, Image};
use crate::monitors::{
    shutdown_and_cleanup, start_deploy_monitoring, start_info_monitoring, start_stack, start_resource_monitoring, start_sandbox, start_sandbox_monitoring
};
use crate::monitors::resource::{ResourceUtilization, MEASUREMENTS_MAX_CAPACITY};

use crate::node::TezedgeNode;

#[tokio::main]
async fn main() {
    // parse and validate program arguments
    let env = configuration::WatchdogEnvironment::from_args();

    // create an slog logger
    let log = create_logger(env.log_level);

    info!(
        log,
        "Tezedge stack watchdog started. Images: {}, {}, {}",
        TezedgeNode::name(),
        Debugger::name(),
        Explorer::name(),
    );

    let slack_server = slack::SlackServer::new(
        env.slack_url,
        env.slack_token,
        env.slack_channel_name,
        log.clone(),
    );

    let running = Arc::new(AtomicBool::new(true));
    let mut thread_handles = Vec::new();

    if env.is_sandbox {
        info!(log, "Starting sandbox launcher monitoring");

        start_sandbox(slack_server.clone(), log.clone())
        .await
        .expect("Sandbox failed to start");

        info!(log, "Creating docker image monitor");
        let deploy_handle = start_sandbox_monitoring(
            slack_server.clone(),
            env.image_monitor_interval,
            log.clone(),
            running.clone(),
        );
        thread_handles.push(deploy_handle);
    } else {
        start_stack(slack_server.clone(), log.clone())
        .await
        .expect("Stack failed to start");

        info!(log, "Creating docker image monitor");
        let deploy_handle = start_deploy_monitoring(
            slack_server.clone(),
            env.image_monitor_interval,
            log.clone(),
            running.clone(),
        );
        thread_handles.push(deploy_handle);

        info!(log, "Creating slack info monitor");
        let monitor_handle = start_info_monitoring(
            slack_server.clone(),
            env.info_interval,
            log.clone(),
            running.clone(),
        );
        thread_handles.push(monitor_handle);

        // create a thread safe VecDeque for each node's resource utilization data
        let ocaml_resource_utilization_storage = Arc::new(RwLock::new(VecDeque::<ResourceUtilization>::with_capacity(MEASUREMENTS_MAX_CAPACITY)));
        let tezedge_resource_utilization_storage = Arc::new(RwLock::new(VecDeque::<ResourceUtilization>::with_capacity(MEASUREMENTS_MAX_CAPACITY)));

        info!(log, "Creating reosurces monitor");
        let resources_handle = start_resource_monitoring(
            env.resource_monitor_interval,
            log.clone(),
            running.clone(),
            ocaml_resource_utilization_storage.clone(),
            tezedge_resource_utilization_storage.clone(),
        );
        thread_handles.push(resources_handle);

        info!(log, "Starting rpc server on port {}", &env.rpc_port);
        let rpc_server_handle = rpc::spawn_rpc_server(env.rpc_port, log.clone(), ocaml_resource_utilization_storage.clone(), tezedge_resource_utilization_storage.clone());
        thread_handles.push(rpc_server_handle);
    }

    // wait for SIGINT
    signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c event");
    info!(log, "Ctrl-c or SIGINT received!");

    // set running to false
    running.store(false, Ordering::Release);

    // drop the looping thread handles (forces exit)
    for handle in thread_handles {
        drop(handle);
    }

    // cleanup
    info!(log, "Cleaning up containers");
    shutdown_and_cleanup(slack_server, log.clone())
        .await
        .expect("Cleanup failed");
    info!(log, "Shutdown complete");
}

/// Creates a slog Logger
fn create_logger(level: Level) -> Logger {
    let drain = slog_async::Async::new(
        slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
            .build()
            .fuse(),
    )
    .chan_size(32768)
    .overflow_strategy(slog_async::OverflowStrategy::Block)
    .build()
    .filter_level(level)
    .fuse();
    Logger::root(drain, slog::o!())
}