// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::Serialize;

use tezos_encoding::{encoding::HasEncoding, nom::NomReader, types::Zarith};

#[derive(Serialize, Debug, Clone, Getters, HasEncoding, NomReader)]
pub struct Counter {
    #[get = "pub"]
    counter: Zarith,
}

impl Counter {
    pub fn to_numeric_string(&self) -> String {
        self.counter.0.to_str_radix(10)
    }
}
