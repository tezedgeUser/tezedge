// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]

//! This crate exists to handle linking against libtezos.so, this file is empty on purpose.

#[link(name = "tezos")]
extern "C" {
    pub fn initialize_ml_context_functions(
        ml_context_send_action: unsafe extern "C" fn(isize) -> isize,
    );
}

/// This function does nothing. It exists to force cargo to link libtezos to crates
/// that depend on tezos-sys.
////
/// To do so, define a `pub` function in the crate that requires libtezos linking that
/// calls this function.
///
/// The problem is that cargo seems to not link in stuff that doesn't get called
/// (but I have only experienced these issues when running the tests with ASAN enabled, and not always).
/// What is done with this function is to add a call from the crates that require libtezos to be linked
/// (the call does nothing), that way cargo sees that the crate is used which forces it to be linked.
///
/// It doesn't seem to be required under normal compilation conditions, but when
/// running the tests with the address sanitizer enabled, linking fails without
/// this extra step.
pub fn force_libtezos_linking() {}
