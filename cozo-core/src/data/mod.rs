/*
 * Copyright 2022, The Cozo Project Authors.
 *
 * This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
 * If a copy of the MPL was not distributed with this file,
 * You can obtain one at https://mozilla.org/MPL/2.0/.
 */

/// Aggregation implementations.
pub(crate) mod aggr;
/// Expression evaluation logic.
pub(crate) mod expr;
/// Public functions available in CozoScript.
pub mod functions;
/// JSON conversion utilities.
pub(crate) mod json;
/// Memory comparison utilities.
pub(crate) mod memcmp;
/// Datalog program structures and normalization.
pub mod program;
/// Relation management and metadata.
pub(crate) mod relation;
/// Symbol and identifier management.
pub mod symb;
/// Tuple encoding and decoding.
pub(crate) mod tuple;
/// Value types and operations.
pub(crate) mod value;

#[cfg(test)]
mod tests;
