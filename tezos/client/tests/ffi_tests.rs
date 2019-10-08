use tezos_client::environment;
use tezos_client::environment::TezosEnvironment;
use tezos_interop::ffi;
use tezos_interop::ffi::{OcamlRuntimeConfiguration, OcamlStorageInitInfo};

mod common;

pub const CHAIN_ID: &str = "8eceda2f";

#[test]
fn test_init_storage_and_change_configuration() {
    // change cfg
    ffi::change_runtime_configuration(OcamlRuntimeConfiguration { log_enabled: common::is_ocaml_log_enabled() }).unwrap().unwrap();

    // init empty storage for test
    let OcamlStorageInitInfo { chain_id, genesis_block_header_hash, genesis_block_header, current_block_header_hash } = prepare_empty_storage("test_storage_01");
    assert!(!current_block_header_hash.is_empty());
    assert!(!genesis_block_header.is_empty());
    assert_eq!(genesis_block_header_hash, current_block_header_hash);

    // has current head (genesis)
    let current_head = ffi::get_current_block_header(chain_id.to_string()).unwrap().unwrap();
    assert!(!current_head.is_empty());

    // get header - genesis
    let block_header = ffi::get_block_header(chain_id.to_string(), genesis_block_header_hash).unwrap().unwrap();

    // check header found
    assert!(block_header.is_some());
    assert_eq!(current_head, block_header.unwrap());
    assert_eq!(current_head, genesis_block_header);
}

#[test]
fn test_fn_get_block_header_not_found_return_none() {
    ffi::change_runtime_configuration(OcamlRuntimeConfiguration { log_enabled: common::is_ocaml_log_enabled() }).unwrap().unwrap();

    // init empty storage for test
    let OcamlStorageInitInfo { chain_id, .. } = prepare_empty_storage("test_storage_02");

    // get unknown header
    let block_header_hash = "3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a";
    let block_header = ffi::get_block_header(chain_id.to_string(), block_header_hash.to_string()).unwrap().unwrap();

    // check not found
    assert!(block_header.is_none());
}

/// Initializes empty dir for ocaml storage
fn prepare_empty_storage(dir_name: &str) -> OcamlStorageInitInfo {
    // init empty storage for test
    let storage_data_dir_path = common::prepare_empty_dir(dir_name);
    let storage_init_info = ffi::init_storage(
        storage_data_dir_path.to_string(),
        &environment::TEZOS_ENV.get(&TezosEnvironment::Alphanet)
            .expect("no tezos environment configured")
            .genesis,
    ).unwrap().unwrap();
    assert_eq!(CHAIN_ID, &storage_init_info.chain_id);
    storage_init_info
}