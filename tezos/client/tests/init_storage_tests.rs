// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashSet, convert::TryFrom};

use strum::IntoEnumIterator;

use crypto::hash::{ContextHash, ProtocolHash};
use tezos_api::environment::{TezosEnvironment, TezosEnvironmentConfiguration, TEZOS_ENV};
use tezos_api::ffi::{PatchContext, TezosRuntimeConfiguration};
use tezos_client::client;

mod common;

#[test]
fn test_init_empty_context_for_all_enviroment_nets() {
    // init runtime and turn on/off ocaml logging
    client::change_runtime_configuration(TezosRuntimeConfiguration {
        debug_mode: false,
        compute_context_action_tree_hashes: false,
        log_enabled: common::is_ocaml_log_enabled(),
    })
    .unwrap();

    // prepare data
    let storage_data_dir = "init_storage_tests_01";

    let mut genesis_commit_hashes: Vec<ContextHash> = Vec::new();
    let mut protocol_hashes: HashSet<ProtocolHash> = HashSet::new();

    // run init storage for all nets
    let mut environment_counter = 0;
    TezosEnvironment::iter().for_each(|net| {
        environment_counter += 1;

        let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV
            .get(&net)
            .unwrap_or_else(|| panic!("no tezos environment configured for: {:?}", &net));

        match client::init_protocol_context(
            common::prepare_empty_dir(&storage_data_dir),
            tezos_env.genesis.clone(),
            tezos_env.protocol_overrides.clone(),
            true,
            false,
            false,
            false,
            None,
        ) {
            Err(e) => panic!(
                "Failed to initialize storage for: {:?}, Reason: {:?}",
                net, e
            ),
            Ok(init_info) => {
                if let Some(commit_hash) = &init_info.genesis_commit_hash {
                    genesis_commit_hashes.push(commit_hash.clone());
                }
                init_info
                    .supported_protocol_hashes
                    .iter()
                    .for_each(|protocol_hash| {
                        protocol_hashes.insert(protocol_hash.clone());
                    });
            }
        }
    });

    // check result - we should have
    assert_eq!(environment_counter, genesis_commit_hashes.len());
    assert!(protocol_hashes.len() > 1);
}

#[test]
fn test_init_empty_context_for_sandbox_with_patch_json() -> Result<(), failure::Error> {
    // init runtime and turn on/off ocaml logging
    client::change_runtime_configuration(TezosRuntimeConfiguration {
        debug_mode: false,
        compute_context_action_tree_hashes: false,
        log_enabled: common::is_ocaml_log_enabled(),
    })
    .unwrap();

    // prepare data
    let storage_data_dir = "init_storage_tests_02";

    // run init storage for all nets
    let net = TezosEnvironment::Sandbox;
    let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV
        .get(&net)
        .unwrap_or_else(|| panic!("no tezos environment configured for: {:?}", &net));

    let patch_context = PatchContext {
        key: String::from("sandbox_parameter"),
        json: String::from(
            r#" { "genesis_pubkey": "edpkuSLWfVU1Vq7Jg9FucPyKmma6otcMHac9zG4oU1KMHSTBpJuGQ2"} "#,
        ),
    };

    match client::init_protocol_context(
        common::prepare_empty_dir(&storage_data_dir),
        tezos_env.genesis.clone(),
        tezos_env.protocol_overrides.clone(),
        true,
        false,
        false,
        false,
        Some(patch_context),
    ) {
        Err(e) => panic!(
            "Failed to initialize storage for: {:?}, Reason: {:?}",
            net, e
        ),
        Ok(init_info) => {
            if let Some(commit_hash) = &init_info.genesis_commit_hash {
                assert_eq!(
                    *commit_hash,
                    ContextHash::try_from("CoVBYdAGWBoDTkiVXJEGX6FQvDN1oGCPJu8STMvaTYdeh7N3KGTz")?
                )
            } else {
                panic!("Expected some context hash")
            }
        }
    }

    Ok(())
}

#[test]
fn test_init_empty_context_for_sandbox_without_patch_json() -> Result<(), failure::Error> {
    // init runtime and turn on/off ocaml logging
    client::change_runtime_configuration(TezosRuntimeConfiguration {
        debug_mode: false,
        compute_context_action_tree_hashes: false,
        log_enabled: common::is_ocaml_log_enabled(),
    })
    .unwrap();

    // prepare data
    let storage_data_dir = "init_storage_tests_03";

    // run init storage for all nets
    let net = TezosEnvironment::Sandbox;
    let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV
        .get(&net)
        .unwrap_or_else(|| panic!("no tezos environment configured for: {:?}", &net));

    match client::init_protocol_context(
        common::prepare_empty_dir(&storage_data_dir),
        tezos_env.genesis.clone(),
        tezos_env.protocol_overrides.clone(),
        true,
        false,
        false,
        false,
        None,
    ) {
        Err(e) => panic!(
            "Failed to initialize storage for: {:?}, Reason: {:?}",
            net, e
        ),
        Ok(init_info) => {
            if let Some(commit_hash) = &init_info.genesis_commit_hash {
                assert_eq!(
                    *commit_hash,
                    ContextHash::try_from("CoVewPVcrKctWXSbrRgoGD6NmkdbDhmTFk5oi1FZpEcRT3bmKxdQ")?
                )
            } else {
                panic!("Expected some context hash")
            }
        }
    }

    Ok(())
}
