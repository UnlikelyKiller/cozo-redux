/*
 * Copyright 2023, The Cozo Project Authors.
 *
 * This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0.
 * If a copy of the MPL was not distributed with this file,
 * You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use crate::data::expr::{eval_bytecode_pred, Bytecode};
use crate::data::program::HnswSearch;
use crate::data::relation::VecElementType;
use crate::data::tuple::{Tuple, ENCODED_KEY_MIN_LEN};
use crate::data::value::Vector;
use crate::parse::sys::HnswDistance;
use crate::runtime::relation::RelationHandle;
use crate::runtime::transact::SessionTx;
use crate::{DataValue, SourceSpan};
use itertools::Itertools;
use miette::{bail, ensure, miette, Result};
use ordered_float::OrderedFloat;
use priority_queue::PriorityQueue;
use rand::seq::SliceRandom;
use rand::Rng;
use rustc_hash::{FxHashMap, FxHashSet};
use smartstring::{LazyCompact, SmartString};
use std::cmp::{max, Reverse};

#[cfg(feature = "rayon")]
use rayon::prelude::*;

const HNSW_PAR_DIST_THRESHOLD: usize = 8;

#[derive(Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
pub(crate) struct PqConfig {
    pub(crate) num_subspaces: usize,
    pub(crate) num_centroids: usize,
}

#[derive(Debug, Clone, serde_derive::Serialize, serde_derive::Deserialize)]
pub(crate) struct PqCodebook {
    pub(crate) num_subspaces: usize,
    pub(crate) num_centroids: usize,
    pub(crate) sub_dim: usize,
    pub(crate) centroids: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, serde_derive::Serialize, serde_derive::Deserialize)]
pub(crate) struct HnswIndexManifest {
    pub(crate) base_relation: SmartString<LazyCompact>,
    pub(crate) index_name: SmartString<LazyCompact>,
    pub(crate) vec_dim: usize,
    pub(crate) dtype: VecElementType,
    pub(crate) vec_fields: Vec<usize>,
    pub(crate) distance: HnswDistance,
    pub(crate) ef_construction: usize,
    pub(crate) m_neighbours: usize,
    pub(crate) m_max: usize,
    pub(crate) m_max0: usize,
    pub(crate) level_multiplier: f64,
    pub(crate) index_filter: Option<String>,
    pub(crate) extend_candidates: bool,
    pub(crate) keep_pruned_connections: bool,
    #[serde(default)]
    pub(crate) pq: Option<PqConfig>,
}

impl HnswIndexManifest {
    fn get_random_level(&self) -> i64 {
        let mut rng = rand::thread_rng();
        let uniform_num: f64 = rng.gen_range(0.0..1.0);
        let r = -uniform_num.ln() * self.level_multiplier;
        // the level is the largest integer smaller than r
        -(r.floor() as i64)
    }
}

type CompoundKey = (Tuple, usize, i32);

fn decode_metadata(bytes: &[u8]) -> Result<Vec<DataValue>> {
    if bytes.len() < ENCODED_KEY_MIN_LEN {
        return Ok(Vec::new());
    }
    rmp_serde::from_slice(&bytes[ENCODED_KEY_MIN_LEN..])
        .map_err(|e| miette!("Failed to deserialize HNSW metadata: {e}"))
}

fn kmeans_lloyd(data: &[Vec<f32>], k: usize, max_iter: usize) -> Vec<Vec<f32>> {
    let n = data.len();
    let d = data[0].len();
    let mut rng = rand::thread_rng();
    let mut idx_pool: Vec<usize> = (0..n).collect();
    idx_pool.shuffle(&mut rng);
    let mut centroids: Vec<Vec<f32>> = idx_pool[..k].iter().map(|&i| data[i].clone()).collect();
    let mut assignments = vec![0usize; n];
    for _ in 0..max_iter {
        let mut changed = false;
        for i in 0..n {
            let mut best_c = 0;
            let mut best_dist = f32::MAX;
            for (c, centroid) in centroids.iter().enumerate() {
                let dist: f32 = data[i]
                    .iter()
                    .zip(centroid.iter())
                    .map(|(a, b)| (a - b) * (a - b))
                    .sum();
                if dist < best_dist {
                    best_dist = dist;
                    best_c = c;
                }
            }
            if assignments[i] != best_c {
                assignments[i] = best_c;
                changed = true;
            }
        }
        if !changed {
            break;
        }
        let mut sums = vec![vec![0.0f32; d]; k];
        let mut counts = vec![0usize; k];
        for i in 0..n {
            let c = assignments[i];
            counts[c] += 1;
            for (j, &v) in data[i].iter().enumerate() {
                sums[c][j] += v;
            }
        }
        for c in 0..k {
            if counts[c] > 0 {
                for j in 0..d {
                    centroids[c][j] = sums[c][j] / counts[c] as f32;
                }
            } else {
                centroids[c] = data[rng.gen_range(0..n)].clone();
            }
        }
    }
    centroids
}

fn encode_vector_pq(vector: &Vector, codebook: &PqCodebook) -> Result<Vec<u8>> {
    let Vector::F32(arr) = vector else {
        bail!("encode_vector_pq only supports F32 vectors");
    };
    let dim = codebook.num_subspaces * codebook.sub_dim;
    ensure!(
        arr.len() == dim,
        "vector dimension {} does not match codebook dimension {}",
        arr.len(),
        dim
    );
    let slice = arr
        .as_slice()
        .ok_or_else(|| miette!("Invalid vector slice"))?;
    let mut codes = Vec::with_capacity(codebook.num_subspaces);
    for m in 0..codebook.num_subspaces {
        let start = m * codebook.sub_dim;
        let end = start + codebook.sub_dim;
        let subvec = &slice[start..end];
        let mut best_c = 0usize;
        let mut best_dist = f32::MAX;
        for c in 0..codebook.num_centroids {
            let c_start = (m * codebook.num_centroids + c) * codebook.sub_dim;
            let centroid = &codebook.centroids[c_start..c_start + codebook.sub_dim];
            let dist: f32 = subvec
                .iter()
                .zip(centroid.iter())
                .map(|(a, b)| (a - b) * (a - b))
                .sum();
            if dist < best_dist {
                best_dist = dist;
                best_c = c;
            }
        }
        codes.push(best_c as u8);
    }
    Ok(codes)
}

struct VectorCache {
    cache: FxHashMap<CompoundKey, Vector>,
    distance: HnswDistance,
    pq_codebook: Option<PqCodebook>,
    pq_codes: FxHashMap<CompoundKey, Vec<u8>>,
}

impl VectorCache {
    fn insert(&mut self, k: CompoundKey, v: Vector) {
        self.cache.insert(k, v);
    }
    fn dist(&self, v1: &Vector, v2: &Vector) -> Result<f64> {
        match self.distance {
            HnswDistance::L2 => match (v1, v2) {
                (Vector::F32(a), Vector::F32(b)) => {
                    let diff = a - b;
                    Ok(diff.dot(&diff) as f64)
                }
                (Vector::F64(a), Vector::F64(b)) => {
                    let diff = a - b;
                    Ok(diff.dot(&diff))
                }
                _ => bail!("Cannot compute L2 distance between {:?} and {:?}", v1, v2),
            },
            HnswDistance::Cosine => match (v1, v2) {
                (Vector::F32(a), Vector::F32(b)) => {
                    let a_norm = a.dot(a) as f64;
                    let b_norm = b.dot(b) as f64;
                    let dot = a.dot(b) as f64;
                    Ok(1.0 - dot / (a_norm * b_norm).sqrt())
                }
                (Vector::F64(a), Vector::F64(b)) => {
                    let a_norm = a.dot(a);
                    let b_norm = b.dot(b);
                    let dot = a.dot(b);
                    Ok(1.0 - dot / (a_norm * b_norm).sqrt())
                }
                _ => bail!(
                    "Cannot compute cosine distance between {:?} and {:?}",
                    v1,
                    v2
                ),
            },
            HnswDistance::InnerProduct => match (v1, v2) {
                (Vector::F32(a), Vector::F32(b)) => {
                    let dot = a.dot(b);
                    Ok(1. - dot as f64)
                }
                (Vector::F64(a), Vector::F64(b)) => {
                    let dot = a.dot(b);
                    Ok(1. - dot)
                }
                _ => bail!("Cannot compute inner product between {:?} and {:?}", v1, v2),
            },
        }
    }
    fn v_dist(&self, v: &Vector, key: &CompoundKey) -> Result<f64> {
        let v2 = self
            .cache
            .get(key)
            .ok_or_else(|| miette!("Vector not found in cache: {:?}", key))?;
        self.dist(v, v2)
    }
    fn k_dist(&self, k1: &CompoundKey, k2: &CompoundKey) -> Result<f64> {
        let v1 = self
            .cache
            .get(k1)
            .ok_or_else(|| miette!("Vector not found in cache: {:?}", k1))?;
        let v2 = self
            .cache
            .get(k2)
            .ok_or_else(|| miette!("Vector not found in cache: {:?}", k2))?;
        self.dist(v1, v2)
    }
    fn get_key(&self, key: &CompoundKey) -> Result<&Vector> {
        self.cache
            .get(key)
            .ok_or_else(|| miette!("Vector not found in cache: {:?}", key))
    }
    fn ensure_key(
        &mut self,
        key: &CompoundKey,
        handle: &RelationHandle,
        tx: &SessionTx<'_>,
    ) -> Result<()> {
        if !self.cache.contains_key(key) {
            match handle.get(tx, &key.0)? {
                Some(tuple) => {
                    let mut field = &tuple[key.1];
                    if key.2 >= 0 {
                        match field {
                            DataValue::List(l) => {
                                field = &l[key.2 as usize];
                            }
                            _ => bail!("Cannot interpret {} as list", field),
                        }
                    }
                    match field {
                        DataValue::Vec(v) => {
                            self.cache.insert(key.clone(), *v.clone());
                        }
                        _ => bail!("Cannot interpret {} as vector", field),
                    }
                }
                None => bail!("Cannot find compound key for HNSW in relation {:?}: {:?}. Cache size: {}. Sample keys: {:?}", handle.id, key, self.cache.len(), self.cache.keys().take(3).collect::<Vec<_>>()),
            }
        }
        Ok(())
    }
    fn ensure_pq_code(
        &mut self,
        key: &CompoundKey,
        idx_handle: &RelationHandle,
        tx: &SessionTx<'_>,
    ) -> Result<()> {
        if self.pq_codes.contains_key(key) {
            return Ok(());
        }
        let mut pq_key = vec![DataValue::from(i64::MAX - 1)];
        pq_key.extend_from_slice(&key.0);
        pq_key.push(DataValue::from(key.1 as i64));
        pq_key.push(DataValue::from(key.2 as i64));
        pq_key.extend_from_slice(&key.0);
        pq_key.push(DataValue::from(key.1 as i64));
        pq_key.push(DataValue::from(key.2 as i64));
        let key_bytes = idx_handle.encode_key_for_store(&pq_key, Default::default())?;
        match tx.store_tx.get(&key_bytes, false)? {
            None => {
                self.pq_codes.insert(key.clone(), Vec::new());
            }
            Some(val_bytes) => {
                let val_tuple: Vec<DataValue> = decode_metadata(&val_bytes)?;
                match val_tuple.first() {
                    Some(DataValue::Bytes(bytes)) => {
                        self.pq_codes.insert(key.clone(), bytes.clone());
                    }
                    _ => {
                        self.pq_codes.insert(key.clone(), Vec::new());
                    }
                }
            }
        }
        Ok(())
    }
    fn pq_dist(&self, dist_table: &[Vec<f64>], key: &CompoundKey) -> Option<f64> {
        let codes = self.pq_codes.get(key)?;
        if codes.is_empty() {
            return None;
        }
        let mut sum = 0.0;
        for (m, &code) in codes.iter().enumerate() {
            sum += dist_table[m][code as usize];
        }
        Some(sum)
    }
}

impl<'a> SessionTx<'a> {
    fn hnsw_put_vector(
        &mut self,
        tuple: &[DataValue],
        q: &Vector,
        idx: usize,
        subidx: i32,
        manifest: &HnswIndexManifest,
        orig_table: &RelationHandle,
        idx_table: &RelationHandle,
        vec_cache: &mut VectorCache,
    ) -> Result<()> {
        let tuple_key = &tuple[..orig_table.metadata.keys.len()];
        let ck = (tuple_key.to_vec().into(), idx, subidx);
        vec_cache.insert(ck, q.clone());
        let hash = q.get_hash();
        let mut canary_tuple = vec![DataValue::from(0)];
        for _ in 0..2 {
            canary_tuple.extend_from_slice(tuple_key);
            canary_tuple.push(DataValue::from(idx as i64));
            canary_tuple.push(DataValue::from(subidx as i64));
        }
        if let Some(v) = idx_table.get(self, &canary_tuple)? {
            if let DataValue::Bytes(b) = &v[tuple_key.len() * 2 + 6] {
                if b == hash.as_ref() {
                    return Ok(());
                }
            }
            self.hnsw_remove_vec(
                tuple_key, idx, subidx, manifest, orig_table, idx_table, vec_cache,
            )?;
        }

        let ep_res = idx_table
            .scan_bounded_prefix(
                self,
                &[],
                &[DataValue::from(i64::MIN)],
                &[DataValue::from(0)],
            )
            .next();
        if let Some(ep) = ep_res {
            let ep = ep?;
            // bottom level since we are going up
            let bottom_level = ep[0]
                .get_int()
                .ok_or_else(|| miette!("Invalid entry point level"))?;
            let ep_t_key = ep[1..orig_table.metadata.keys.len() + 1].to_vec().into();
            let ep_idx = ep[orig_table.metadata.keys.len() + 1]
                .get_int()
                .ok_or_else(|| miette!("Invalid entry point index"))?
                as usize;
            let ep_subidx = ep[orig_table.metadata.keys.len() + 2]
                .get_int()
                .ok_or_else(|| miette!("Invalid entry point subindex"))?
                as i32;
            let ep_key = (ep_t_key, ep_idx, ep_subidx);
            vec_cache.ensure_key(&ep_key, orig_table, self)?;
            let ep_distance = vec_cache.v_dist(q, &ep_key)?;
            // max queue
            let mut found_nn = PriorityQueue::new();
            found_nn.push(ep_key, OrderedFloat(ep_distance));
            let target_level = manifest.get_random_level();
            if target_level < bottom_level {
                // this becomes the entry point
                self.hnsw_put_fresh_at_levels(
                    hash.as_ref(),
                    tuple_key,
                    idx,
                    subidx,
                    orig_table,
                    idx_table,
                    target_level,
                    bottom_level - 1,
                )?;
            }
            for current_level in bottom_level..target_level {
                self.hnsw_search_level(
                    q,
                    1,
                    current_level,
                    orig_table,
                    idx_table,
                    &mut found_nn,
                    vec_cache,
                    None,
                    None,
                )?;
            }
            let mut self_tuple_key = Vec::with_capacity(orig_table.metadata.keys.len() * 2 + 5);
            self_tuple_key.push(DataValue::from(0));
            for _ in 0..2 {
                self_tuple_key.extend_from_slice(tuple_key);
                self_tuple_key.push(DataValue::from(idx as i64));
                self_tuple_key.push(DataValue::from(subidx as i64));
            }
            let mut self_tuple_val = vec![
                DataValue::from(0.0),
                DataValue::Bytes(hash.as_ref().to_vec()),
                DataValue::from(false),
            ];
            for current_level in max(target_level, bottom_level)..=0 {
                let m_max = if current_level == 0 {
                    manifest.m_max0
                } else {
                    manifest.m_max
                };
                self.hnsw_search_level(
                    q,
                    manifest.ef_construction,
                    current_level,
                    orig_table,
                    idx_table,
                    &mut found_nn,
                    vec_cache,
                    None,
                    None,
                )?;
                // add bidirectional links to the nearest neighbors
                let neighbours = self.hnsw_select_neighbours_heuristic(
                    q,
                    &found_nn,
                    m_max,
                    current_level,
                    manifest,
                    idx_table,
                    orig_table,
                    vec_cache,
                )?;
                // add self-link
                self_tuple_key[0] = DataValue::from(current_level);
                self_tuple_val[0] = DataValue::from(neighbours.len() as f64);

                let self_tuple_key_bytes =
                    idx_table.encode_key_for_store(&self_tuple_key, Default::default())?;
                let self_tuple_val_bytes =
                    idx_table.encode_val_only_for_store(&self_tuple_val, Default::default())?;
                self.store_tx
                    .put(&self_tuple_key_bytes, &self_tuple_val_bytes)?;

                // add bidirectional links
                for (neighbour, Reverse(OrderedFloat(dist))) in neighbours.iter() {
                    let mut out_key = Vec::with_capacity(orig_table.metadata.keys.len() * 2 + 5);
                    let out_val = vec![
                        DataValue::from(*dist),
                        DataValue::Null,
                        DataValue::from(false),
                    ];
                    out_key.push(DataValue::from(current_level));
                    out_key.extend_from_slice(tuple_key);
                    out_key.push(DataValue::from(idx as i64));
                    out_key.push(DataValue::from(subidx as i64));
                    out_key.extend_from_slice(&neighbour.0);
                    out_key.push(DataValue::from(neighbour.1 as i64));
                    out_key.push(DataValue::from(neighbour.2 as i64));
                    let out_key_bytes =
                        idx_table.encode_key_for_store(&out_key, Default::default())?;
                    let out_val_bytes =
                        idx_table.encode_val_only_for_store(&out_val, Default::default())?;
                    self.store_tx.put(&out_key_bytes, &out_val_bytes)?;

                    let mut in_key = Vec::with_capacity(orig_table.metadata.keys.len() * 2 + 5);
                    let in_val = vec![
                        DataValue::from(*dist),
                        DataValue::Null,
                        DataValue::from(false),
                    ];
                    in_key.push(DataValue::from(current_level));
                    in_key.extend_from_slice(&neighbour.0);
                    in_key.push(DataValue::from(neighbour.1 as i64));
                    in_key.push(DataValue::from(neighbour.2 as i64));
                    in_key.extend_from_slice(tuple_key);
                    in_key.push(DataValue::from(idx as i64));
                    in_key.push(DataValue::from(subidx as i64));

                    let in_key_bytes =
                        idx_table.encode_key_for_store(&in_key, Default::default())?;
                    let in_val_bytes =
                        idx_table.encode_val_only_for_store(&in_val, Default::default())?;
                    self.store_tx.put(&in_key_bytes, &in_val_bytes)?;

                    // shrink links if necessary
                    let mut target_self_key =
                        Vec::with_capacity(orig_table.metadata.keys.len() * 2 + 5);
                    target_self_key.push(DataValue::from(current_level));
                    for _ in 0..2 {
                        target_self_key.extend_from_slice(&neighbour.0);
                        target_self_key.push(DataValue::from(neighbour.1 as i64));
                        target_self_key.push(DataValue::from(neighbour.2 as i64));
                    }
                    let target_self_key_bytes =
                        idx_table.encode_key_for_store(&target_self_key, Default::default())?;
                    let target_self_val_bytes = match self.store_tx.get(&target_self_key_bytes, false)? {
                        Some(bytes) => bytes,
                        None => bail!("Indexed vector not found, this signifies a bug in the index implementation"),
                    };
                    let mut target_self_val: Vec<DataValue> =
                        decode_metadata(&target_self_val_bytes)?;
                    let mut target_degree = target_self_val
                        .first()
                        .and_then(|v| v.get_float())
                        .ok_or_else(|| {
                            miette!("Invalid neighbor degree (metadata too short or corrupted)")
                        })? as usize
                        + 1;
                    if target_degree > m_max {
                        // shrink links
                        target_degree = self.hnsw_shrink_neighbour(
                            neighbour,
                            m_max,
                            current_level,
                            manifest,
                            idx_table,
                            orig_table,
                            vec_cache,
                        )?;
                    }
                    // update degree
                    target_self_val[0] = DataValue::from(target_degree as f64);
                    self.store_tx.put(
                        &target_self_key_bytes,
                        &idx_table
                            .encode_val_only_for_store(&target_self_val, Default::default())?,
                    )?;
                }
            }
        } else {
            // This is the first vector in the index.
            let level = manifest.get_random_level();
            self.hnsw_put_fresh_at_levels(
                hash.as_ref(),
                tuple_key,
                idx,
                subidx,
                orig_table,
                idx_table,
                level,
                0,
            )?;
        }
        // Store PQ codes if configured.
        if manifest.pq.is_some() {
            if let Some(codebook) =
                self.hnsw_get_pq_codebook(orig_table.metadata.keys.len(), idx_table)?
            {
                let codes = encode_vector_pq(q, &codebook)?;
                let compound_key = (tuple_key.to_vec().into(), idx, subidx);
                self.hnsw_store_pq_codes(idx_table, &compound_key, &codes)?;
            }
        }
        Ok(())
    }
    fn hnsw_shrink_neighbour(
        &mut self,
        target_key: &CompoundKey,
        m: usize,
        level: i64,
        manifest: &HnswIndexManifest,
        idx_table: &RelationHandle,
        orig_table: &RelationHandle,
        vec_cache: &mut VectorCache,
    ) -> Result<usize> {
        vec_cache.ensure_key(target_key, orig_table, self)?;
        let vec = vec_cache.get_key(target_key)?.clone();
        let mut candidates = PriorityQueue::new();
        for (neighbour_key, neighbour_dist) in
            self.hnsw_get_neighbours(target_key, level, idx_table, false)?
        {
            candidates.push(neighbour_key, OrderedFloat(neighbour_dist));
        }
        let new_candidates = self.hnsw_select_neighbours_heuristic(
            &vec,
            &candidates,
            m,
            level,
            manifest,
            idx_table,
            orig_table,
            vec_cache,
        )?;
        let mut old_candidate_set = FxHashSet::default();
        for (old, _) in &candidates {
            old_candidate_set.insert(old.clone());
        }
        let mut new_candidate_set = FxHashSet::default();
        for (new, _) in &new_candidates {
            new_candidate_set.insert(new.clone());
        }
        let new_degree = new_candidates.len();
        for (new, Reverse(OrderedFloat(new_dist))) in new_candidates {
            if !old_candidate_set.contains(&new) {
                let mut new_key = Vec::with_capacity(orig_table.metadata.keys.len() * 2 + 5);
                let new_val = vec![
                    DataValue::from(new_dist),
                    DataValue::Null,
                    DataValue::from(false),
                ];
                new_key.push(DataValue::from(level));
                new_key.extend_from_slice(&target_key.0);
                new_key.push(DataValue::from(target_key.1 as i64));
                new_key.push(DataValue::from(target_key.2 as i64));
                new_key.extend_from_slice(&new.0);
                new_key.push(DataValue::from(new.1 as i64));
                new_key.push(DataValue::from(new.2 as i64));
                let new_key_bytes = idx_table.encode_key_for_store(&new_key, Default::default())?;
                let new_val_bytes =
                    idx_table.encode_val_only_for_store(&new_val, Default::default())?;
                self.store_tx.put(&new_key_bytes, &new_val_bytes)?;
            }
        }
        for (old, OrderedFloat(old_dist)) in candidates {
            if !new_candidate_set.contains(&old) {
                let mut old_key = Vec::with_capacity(orig_table.metadata.keys.len() * 2 + 5);
                old_key.push(DataValue::from(level));
                old_key.extend_from_slice(&target_key.0);
                old_key.push(DataValue::from(target_key.1 as i64));
                old_key.push(DataValue::from(target_key.2 as i64));
                old_key.extend_from_slice(&old.0);
                old_key.push(DataValue::from(old.1 as i64));
                old_key.push(DataValue::from(old.2 as i64));
                let old_key_bytes = idx_table.encode_key_for_store(&old_key, Default::default())?;
                let old_existing_val = match self.store_tx.get(&old_key_bytes, false)? {
                    Some(bytes) => bytes,
                    None => {
                        bail!("Indexed vector not found, this signifies a bug in the index implementation")
                    }
                };
                let old_existing_val: Vec<DataValue> = decode_metadata(&old_existing_val)?;
                if old_existing_val
                    .get(2)
                    .and_then(|v| v.get_bool())
                    .unwrap_or(false)
                {
                    self.store_tx.del(&old_key_bytes)?;
                } else {
                    let old_val = vec![
                        DataValue::from(old_dist),
                        DataValue::Null,
                        DataValue::from(true),
                    ];
                    let old_val_bytes =
                        idx_table.encode_val_only_for_store(&old_val, Default::default())?;
                    self.store_tx.put(&old_key_bytes, &old_val_bytes)?;
                }
            }
        }

        Ok(new_degree)
    }
    fn hnsw_select_neighbours_heuristic(
        &self,
        q: &Vector,
        found: &PriorityQueue<CompoundKey, OrderedFloat<f64>>,
        m: usize,
        level: i64,
        manifest: &HnswIndexManifest,
        idx_table: &RelationHandle,
        orig_table: &RelationHandle,
        vec_cache: &mut VectorCache,
    ) -> Result<PriorityQueue<CompoundKey, Reverse<OrderedFloat<f64>>>> {
        let mut candidates = PriorityQueue::new();
        // Simple non-heuristic selection
        // let mut temp = found.clone();
        // while temp.len() > m {
        //     temp.pop();
        // }
        // for (item, dist) in temp.iter() {
        //     candidates.push(item.clone(), Reverse(*dist));
        // }
        // return Ok(candidates);
        // End of simple non-heuristic selection

        let mut ret: PriorityQueue<CompoundKey, Reverse<OrderedFloat<_>>> = PriorityQueue::new();
        let mut discarded: PriorityQueue<_, Reverse<OrderedFloat<_>>> = PriorityQueue::new();
        for (item, dist) in found.iter() {
            // Add to candidates
            candidates.push(item.clone(), Reverse(*dist));
        }
        if manifest.extend_candidates {
            for (item, _) in found.iter() {
                // Extend by neighbours
                for (neighbour_key, _) in self.hnsw_get_neighbours(item, level, idx_table, false)? {
                    vec_cache.ensure_key(&neighbour_key, orig_table, self)?;
                    let dist = vec_cache.v_dist(q, &neighbour_key)?;
                    candidates.push(
                        (neighbour_key.0, neighbour_key.1, neighbour_key.2),
                        Reverse(OrderedFloat(dist)),
                    );
                }
            }
        }
        while !candidates.is_empty() && ret.len() < m {
            let (cand_key, Reverse(OrderedFloat(cand_dist_to_q))) = candidates
                .pop()
                .ok_or_else(|| miette!("Candidates heap empty"))?;
            let mut should_add = true;
            for (existing, _) in ret.iter() {
                vec_cache.ensure_key(&cand_key, orig_table, self)?;
                vec_cache.ensure_key(existing, orig_table, self)?;
                let dist_to_existing = vec_cache.k_dist(existing, &cand_key)?;
                if dist_to_existing < cand_dist_to_q {
                    should_add = false;
                    break;
                }
            }
            if should_add {
                ret.push(cand_key, Reverse(OrderedFloat(cand_dist_to_q)));
            } else if manifest.keep_pruned_connections {
                discarded.push(cand_key, Reverse(OrderedFloat(cand_dist_to_q)));
            }
        }
        if manifest.keep_pruned_connections {
            while !discarded.is_empty() && ret.len() < m {
                let (nearest_triple, Reverse(OrderedFloat(nearest_dist))) = discarded
                    .pop()
                    .ok_or_else(|| miette!("Discarded heap empty"))?;
                ret.push(nearest_triple, Reverse(OrderedFloat(nearest_dist)));
            }
        }
        Ok(ret)
    }
    fn hnsw_search_level(
        &self,
        q: &Vector,
        ef: usize,
        cur_level: i64,
        orig_table: &RelationHandle,
        idx_table: &RelationHandle,
        found_nn: &mut PriorityQueue<CompoundKey, OrderedFloat<f64>>,
        vec_cache: &mut VectorCache,
        filter: Option<(&[Bytecode], SourceSpan)>,
        pq_dist_table: Option<&[Vec<f64>]>,
    ) -> Result<()> {
        let mut visited: FxHashSet<CompoundKey> = FxHashSet::default();
        // min queue
        let mut candidates: PriorityQueue<CompoundKey, Reverse<OrderedFloat<f64>>> =
            PriorityQueue::new();

        for item in found_nn.iter() {
            visited.insert(item.0.clone());
            candidates.push(item.0.clone(), Reverse(*item.1));
        }

        // When a predicate filter is active, maintain a separate ef-bounded traversal
        // frontier so nodes failing the predicate still guide graph navigation without
        // distorting found_nn's worst-distance bound. Without a filter this is None
        // and all logic reduces to the original single-heap behaviour with no overhead.
        let mut traversal_nn: Option<PriorityQueue<CompoundKey, OrderedFloat<f64>>> =
            if filter.is_some() {
                let mut pq = PriorityQueue::new();
                for item in found_nn.iter() {
                    pq.push(item.0.clone(), *item.1);
                }
                Some(pq)
            } else {
                None
            };

        let mut pred_stack: Vec<DataValue> = vec![];

        while let Some((candidate, Reverse(OrderedFloat(candidate_dist)))) = candidates.pop() {
            let furthest_dist = match traversal_nn.as_ref() {
                Some(tn) => tn.peek().map(|(_, OrderedFloat(d))| *d).unwrap_or(f64::MAX),
                None => {
                    let (_, OrderedFloat(d)) = found_nn
                        .peek()
                        .ok_or_else(|| miette!("Search heap empty"))?;
                    *d
                }
            };
            if candidate_dist > furthest_dist {
                break;
            }

            // Collect unvisited neighbors for this candidate.
            let unvisited: Vec<CompoundKey> = self
                .hnsw_get_neighbours(&candidate, cur_level, idx_table, false)?
                .filter(|(k, _)| !visited.contains(k))
                .map(|(k, _)| k)
                .collect();

            // Mark all as visited before processing so cross-candidate dedup is
            // preserved even if the same node appears in multiple neighbor lists.
            visited.extend(unvisited.iter().cloned());

            // Load vectors sequentially (requires store access via &mut vec_cache).
            for key in &unvisited {
                vec_cache.ensure_key(key, orig_table, self)?;
            }

            // Load PQ codes if available.
            if pq_dist_table.is_some() {
                for key in &unvisited {
                    vec_cache.ensure_pq_code(key, idx_table, self)?;
                }
            }

            // Compute distances. The immutable reborrow of vec_cache is safe here
            // because all ensure_key mutations for this batch are complete.
            let distances: Vec<f64> = {
                let cache_ref: &VectorCache = &*vec_cache;
                let pq_compute = |k: &CompoundKey| {
                    if let Some(dt) = pq_dist_table {
                        match cache_ref.pq_dist(dt, k) {
                            Some(d) => Ok(d),
                            None => cache_ref.v_dist(q, k),
                        }
                    } else {
                        cache_ref.v_dist(q, k)
                    }
                };
                #[cfg(feature = "rayon")]
                if unvisited.len() >= HNSW_PAR_DIST_THRESHOLD {
                    unvisited
                        .par_iter()
                        .map(pq_compute)
                        .collect::<Result<Vec<_>>>()?
                } else {
                    unvisited
                        .iter()
                        .map(pq_compute)
                        .collect::<Result<Vec<_>>>()?
                }
                #[cfg(not(feature = "rayon"))]
                unvisited
                    .iter()
                    .map(pq_compute)
                    .collect::<Result<Vec<_>>>()?
            };

            // Update heaps sequentially.
            for (key, dist) in unvisited.into_iter().zip(distances) {
                let (frontier_len, frontier_furthest) = match traversal_nn.as_ref() {
                    Some(tn) => (
                        tn.len(),
                        tn.peek().map(|(_, OrderedFloat(d))| *d).unwrap_or(f64::MAX),
                    ),
                    None => {
                        let (_, OrderedFloat(d)) = found_nn
                            .peek()
                            .ok_or_else(|| miette!("Search heap empty"))?;
                        (found_nn.len(), *d)
                    }
                };

                if frontier_len < ef || dist < frontier_furthest {
                    candidates.push(key.clone(), Reverse(OrderedFloat(dist)));

                    // Keep traversal frontier ef-bounded when filter is active.
                    if let Some(ref mut tn) = traversal_nn {
                        tn.push(key.clone(), OrderedFloat(dist));
                        if tn.len() > ef {
                            tn.pop();
                        }
                    }

                    // Evaluate in-loop predicate on the base tuple. Nodes that fail
                    // still expand their neighbors via candidates; they just don't
                    // enter found_nn (biased traversal).
                    let passes_filter = match filter {
                        None => true,
                        Some((code, span)) => match orig_table.get(self, &key.0)? {
                            None => false,
                            Some(tuple) => eval_bytecode_pred(code, &tuple, &mut pred_stack, span)?,
                        },
                    };

                    if passes_filter {
                        found_nn.push(key, OrderedFloat(dist));
                        // Without filter: cap found_nn at ef (standard HNSW).
                        // With filter: leave uncapped; caller truncates to k after
                        // applying any remaining post-hoc checks on extra bindings.
                        if traversal_nn.is_none() && found_nn.len() > ef {
                            found_nn.pop();
                        }
                    }
                }
            }
        }

        Ok(())
    }
    fn hnsw_get_neighbours<'b>(
        &'b self,
        cand_key: &'b CompoundKey,
        level: i64,
        idx_handle: &RelationHandle,
        include_deleted: bool,
    ) -> Result<impl Iterator<Item = (CompoundKey, f64)> + 'b> {
        let mut start_tuple = Vec::with_capacity(cand_key.0.len() + 3);
        start_tuple.push(DataValue::from(level));
        start_tuple.extend_from_slice(&cand_key.0);
        start_tuple.push(DataValue::from(cand_key.1 as i64));
        start_tuple.push(DataValue::from(cand_key.2 as i64));
        let key_len = cand_key.0.len();
        Ok(idx_handle
            .scan_prefix(self, &start_tuple)
            .filter_map(move |res| {
                let tuple = res.ok()?;

                // Defensive check: ensure the tuple has at least the key parts.
                if tuple.len() < 2 * key_len + 5 {
                    log::warn!("HNSW index row too short: {} fields. Expected at least {}.", tuple.len(), 2 * key_len + 5);
                    return None;
                }

                // If it's a self-link or missing values, return None.
                if tuple.len() < 2 * key_len + 8 {
                    if tuple.len() == 2 * key_len + 5 {
                        return None;
                    }
                    log::warn!("HNSW index row has unexpected length {}. Expected {}. This may indicate a stale index or dimension mismatch.", tuple.len(), 2 * key_len + 8);
                    return None;
                }

                let key_idx = tuple[2 * key_len + 3].get_int()? as usize;
                let key_subidx = tuple[2 * key_len + 4].get_int()? as i32;
                let key_tup: Tuple = tuple[key_len + 3..2 * key_len + 3].to_vec().into();
                if key_tup == cand_key.0 {
                    None
                } else {
                    if include_deleted {
                        return Some((
                            (key_tup, key_idx, key_subidx),
                            tuple[2 * key_len + 5].get_float()?,
                        ));
                    }
                    let is_deleted = tuple[2 * key_len + 7].get_bool()?;
                    if is_deleted {
                        None
                    } else {
                        Some((
                            (key_tup, key_idx, key_subidx),
                            tuple[2 * key_len + 5].get_float()?,
                        ))
                    }
                }
            }))
    }
    fn hnsw_put_fresh_at_levels(
        &mut self,
        hash: &[u8],
        tuple: &[DataValue],
        idx: usize,
        subidx: i32,
        orig_table: &RelationHandle,
        idx_table: &RelationHandle,
        bottom_level: i64,
        top_level: i64,
    ) -> Result<()> {
        let mut target_key = vec![DataValue::Null];
        let mut canary_key = vec![DataValue::from(1)];
        for _ in 0..2 {
            for i in 0..orig_table.metadata.keys.len() {
                target_key.push(
                    tuple
                        .get(i)
                        .ok_or_else(|| miette!("Base relation row too short"))?
                        .clone(),
                );
                canary_key.push(DataValue::Null);
            }
            target_key.push(DataValue::from(idx as i64));
            target_key.push(DataValue::from(subidx as i64));
            canary_key.push(DataValue::Null);
            canary_key.push(DataValue::Null);
        }
        let target_value = [
            DataValue::from(0.0),
            DataValue::Bytes(hash.to_vec()),
            DataValue::from(false),
        ];
        let target_key_bytes = idx_table.encode_key_for_store(&target_key, Default::default())?;

        // canary value is for conflict detection: prevent the scenario of disconnected graphs at all levels
        let canary_value = [
            DataValue::from(bottom_level),
            DataValue::Bytes(target_key_bytes),
            DataValue::from(false),
        ];
        let canary_key_bytes = idx_table.encode_key_for_store(&canary_key, Default::default())?;
        let canary_value_bytes =
            idx_table.encode_val_only_for_store(&canary_value, Default::default())?;
        self.store_tx.put(&canary_key_bytes, &canary_value_bytes)?;

        for cur_level in bottom_level..=top_level {
            target_key[0] = DataValue::from(cur_level);
            let key = idx_table.encode_key_for_store(&target_key, Default::default())?;
            let val = idx_table.encode_val_only_for_store(&target_value, Default::default())?;
            self.store_tx.put(&key, &val)?;
        }
        Ok(())
    }
    pub(crate) fn hnsw_put(
        &mut self,
        manifest: &HnswIndexManifest,
        orig_table: &RelationHandle,
        idx_table: &RelationHandle,
        filter: Option<&Vec<Bytecode>>,
        stack: &mut Vec<DataValue>,
        tuple: &[DataValue],
    ) -> Result<bool> {
        if let Some(code) = filter {
            if !eval_bytecode_pred(code, tuple, stack, Default::default())? {
                self.hnsw_remove(orig_table, idx_table, manifest, tuple)?;
                return Ok(false);
            }
        }
        let mut extracted_vectors = vec![];
        for idx in &manifest.vec_fields {
            let val = tuple
                .get(*idx)
                .ok_or_else(|| miette!("Base relation row too short"))?;
            if let DataValue::Vec(v) = val {
                extracted_vectors.push((v, *idx, -1));
            } else if let DataValue::List(l) = val {
                for (sidx, v) in l.iter().enumerate() {
                    if let DataValue::Vec(v) = v {
                        extracted_vectors.push((v, *idx, sidx as i32));
                    }
                }
            }
        }
        if extracted_vectors.is_empty() {
            return Ok(false);
        }
        let mut vec_cache = VectorCache {
            cache: FxHashMap::default(),
            distance: manifest.distance,
            pq_codebook: None,
            pq_codes: Default::default(),
        };
        for (vec, idx, sub) in extracted_vectors {
            self.hnsw_put_vector(
                tuple,
                vec,
                idx,
                sub,
                manifest,
                orig_table,
                idx_table,
                &mut vec_cache,
            )?;
        }
        Ok(true)
    }
    pub(crate) fn hnsw_remove(
        &mut self,
        orig_table: &RelationHandle,
        idx_table: &RelationHandle,
        manifest: &HnswIndexManifest,
        tuple: &[DataValue],
    ) -> Result<()> {
        let mut prefix = vec![DataValue::from(0)];
        prefix.extend_from_slice(&tuple[0..orig_table.metadata.keys.len()]);
        let candidates: FxHashSet<CompoundKey> = idx_table
            .scan_prefix(self, &prefix)
            .filter_map(|t| match t {
                Ok(t) => {
                    let k_len = orig_table.metadata.keys.len();
                    if t.len() < k_len + 3 {
                        return None;
                    }
                    Some((
                        t[1..k_len + 1].to_vec().into(),
                        t[k_len + 1].get_int()? as usize,
                        t[k_len + 2].get_int()? as i32,
                    ))
                }
                Err(_) => None,
            })
            .collect();
        let mut vec_cache = VectorCache {
            cache: FxHashMap::default(),
            distance: manifest.distance,
            pq_codebook: None,
            pq_codes: Default::default(),
        };
        for (tuple_key, idx, subidx) in candidates {
            self.hnsw_remove_vec(
                &tuple_key,
                idx,
                subidx,
                manifest,
                orig_table,
                idx_table,
                &mut vec_cache,
            )?;
        }
        Ok(())
    }
    fn hnsw_repair_node(
        &mut self,
        target_key: &CompoundKey,
        layer: i64,
        manifest: &HnswIndexManifest,
        orig_table: &RelationHandle,
        idx_table: &RelationHandle,
        vec_cache: &mut VectorCache,
    ) -> Result<()> {
        let m_max = if layer == 0 {
            manifest.m_max0
        } else {
            manifest.m_max
        };
        let repair_threshold = max(1, m_max / 2);

        let current_nbrs: Vec<(CompoundKey, f64)> = self
            .hnsw_get_neighbours(target_key, layer, idx_table, false)?
            .collect();

        if current_nbrs.len() >= repair_threshold {
            return Ok(());
        }

        vec_cache.ensure_key(target_key, orig_table, self)?;
        let target_vec = vec_cache.get_key(target_key)?.clone();

        // Candidate pool: current neighbors + their neighbors (1-hop expansion)
        let mut candidates: PriorityQueue<CompoundKey, OrderedFloat<f64>> = PriorityQueue::new();
        let current_set: FxHashSet<CompoundKey> =
            current_nbrs.iter().map(|(k, _)| k.clone()).collect();

        for (nk, dist) in &current_nbrs {
            candidates.push(nk.clone(), OrderedFloat(*dist));
        }

        // Collect expansion keys first to avoid borrow conflicts
        let mut expansion: Vec<CompoundKey> = vec![];
        for (nk, _) in &current_nbrs {
            for (nn, _) in self.hnsw_get_neighbours(nk, layer, idx_table, false)? {
                if &nn != target_key && !current_set.contains(&nn) {
                    expansion.push(nn);
                }
            }
        }
        for nn in expansion {
            vec_cache.ensure_key(&nn, orig_table, self)?;
            let d = vec_cache.v_dist(&target_vec, &nn)?;
            candidates.push(nn, OrderedFloat(d));
        }

        if candidates.len() <= current_nbrs.len() {
            return Ok(());
        }

        let selected = self.hnsw_select_neighbours_heuristic(
            &target_vec,
            &candidates,
            m_max,
            layer,
            manifest,
            idx_table,
            orig_table,
            vec_cache,
        )?;

        let new_degree = selected.len();

        for (new_nbr, Reverse(OrderedFloat(dist))) in &selected {
            if current_set.contains(new_nbr) {
                continue;
            }
            // target → new_nbr
            let mut out_key = vec![DataValue::from(layer)];
            out_key.extend_from_slice(&target_key.0);
            out_key.push(DataValue::from(target_key.1 as i64));
            out_key.push(DataValue::from(target_key.2 as i64));
            out_key.extend_from_slice(&new_nbr.0);
            out_key.push(DataValue::from(new_nbr.1 as i64));
            out_key.push(DataValue::from(new_nbr.2 as i64));
            let edge_val = vec![
                DataValue::from(*dist),
                DataValue::Null,
                DataValue::from(false),
            ];
            self.store_tx.put(
                &idx_table.encode_key_for_store(&out_key, Default::default())?,
                &idx_table.encode_val_only_for_store(&edge_val, Default::default())?,
            )?;

            // new_nbr → target
            let mut in_key = vec![DataValue::from(layer)];
            in_key.extend_from_slice(&new_nbr.0);
            in_key.push(DataValue::from(new_nbr.1 as i64));
            in_key.push(DataValue::from(new_nbr.2 as i64));
            in_key.extend_from_slice(&target_key.0);
            in_key.push(DataValue::from(target_key.1 as i64));
            in_key.push(DataValue::from(target_key.2 as i64));
            self.store_tx.put(
                &idx_table.encode_key_for_store(&in_key, Default::default())?,
                &idx_table.encode_val_only_for_store(&edge_val, Default::default())?,
            )?;

            // Update new_nbr's degree; shrink if over limit
            let mut nbr_self_key = vec![DataValue::from(layer)];
            for _ in 0..2 {
                nbr_self_key.extend_from_slice(&new_nbr.0);
                nbr_self_key.push(DataValue::from(new_nbr.1 as i64));
                nbr_self_key.push(DataValue::from(new_nbr.2 as i64));
            }
            let nbr_self_key_bytes =
                idx_table.encode_key_for_store(&nbr_self_key, Default::default())?;
            if let Some(existing) = self.store_tx.get(&nbr_self_key_bytes, false)? {
                let mut val: Vec<DataValue> = decode_metadata(&existing)?;
                let new_nbr_degree = val
                    .first()
                    .and_then(|v| v.get_float())
                    .ok_or_else(|| miette!("Invalid neighbor degree"))?
                    as usize
                    + 1;
                let actual = if new_nbr_degree > m_max {
                    self.hnsw_shrink_neighbour(
                        new_nbr, m_max, layer, manifest, idx_table, orig_table, vec_cache,
                    )?
                } else {
                    new_nbr_degree
                };
                val[0] = DataValue::from(actual as f64);
                self.store_tx.put(
                    &nbr_self_key_bytes,
                    &idx_table.encode_val_only_for_store(&val, Default::default())?,
                )?;
            }
        }

        // Update target's degree
        let mut target_self_key = vec![DataValue::from(layer)];
        for _ in 0..2 {
            target_self_key.extend_from_slice(&target_key.0);
            target_self_key.push(DataValue::from(target_key.1 as i64));
            target_self_key.push(DataValue::from(target_key.2 as i64));
        }
        let target_self_key_bytes =
            idx_table.encode_key_for_store(&target_self_key, Default::default())?;
        if let Some(existing) = self.store_tx.get(&target_self_key_bytes, false)? {
            let mut val: Vec<DataValue> = decode_metadata(&existing)?;
            ensure!(
                !val.is_empty(),
                "Node metadata is empty or corrupted during degree update"
            );
            val[0] = DataValue::from(new_degree as f64);
            self.store_tx.put(
                &target_self_key_bytes,
                &idx_table.encode_val_only_for_store(&val, Default::default())?,
            )?;
        }

        Ok(())
    }
    fn hnsw_remove_vec(
        &mut self,
        tuple_key: &[DataValue],
        idx: usize,
        subidx: i32,
        manifest: &HnswIndexManifest,
        orig_table: &RelationHandle,
        idx_table: &RelationHandle,
        vec_cache: &mut VectorCache,
    ) -> Result<()> {
        let compound_key = (tuple_key.to_vec().into(), idx, subidx);

        // Phase 1: Delete ALL edges and self-links across all layers, collecting
        // neighbors that need repair. We must finish deleting every reference to
        // the removed node before any repair runs, because repair does a 2-hop
        // expansion that would otherwise discover stale edges to this node via
        // un-processed neighbors.
        let mut encountered_singletons = false;
        let mut neighbours_to_repair: Vec<(CompoundKey, i64)> = vec![];

        for neg_layer in 0i64.. {
            let layer = -neg_layer;
            let mut self_key = vec![DataValue::from(layer)];
            for _ in 0..2 {
                self_key.extend_from_slice(tuple_key);
                self_key.push(DataValue::from(idx as i64));
                self_key.push(DataValue::from(subidx as i64));
            }
            let self_key_bytes = idx_table.encode_key_for_store(&self_key, Default::default())?;
            if self.store_tx.exists(&self_key_bytes, false)? {
                self.store_tx.del(&self_key_bytes)?;
            } else {
                break;
            }

            let neigbours = self
                .hnsw_get_neighbours(&compound_key, layer, idx_table, true)?
                .collect_vec();
            encountered_singletons |= neigbours.is_empty();
            for (neighbour_key, _) in neigbours {
                // Delete outgoing edge: deleted_node → neighbour
                let mut out_key = vec![DataValue::from(layer)];
                out_key.extend_from_slice(tuple_key);
                out_key.push(DataValue::from(idx as i64));
                out_key.push(DataValue::from(subidx as i64));
                out_key.extend_from_slice(&neighbour_key.0);
                out_key.push(DataValue::from(neighbour_key.1 as i64));
                out_key.push(DataValue::from(neighbour_key.2 as i64));
                let out_key_bytes = idx_table.encode_key_for_store(&out_key, Default::default())?;
                self.store_tx.del(&out_key_bytes)?;

                // Delete incoming edge: neighbour → deleted_node
                let mut in_key = vec![DataValue::from(layer)];
                in_key.extend_from_slice(&neighbour_key.0);
                in_key.push(DataValue::from(neighbour_key.1 as i64));
                in_key.push(DataValue::from(neighbour_key.2 as i64));
                in_key.extend_from_slice(tuple_key);
                in_key.push(DataValue::from(idx as i64));
                in_key.push(DataValue::from(subidx as i64));
                let in_key_bytes = idx_table.encode_key_for_store(&in_key, Default::default())?;
                self.store_tx.del(&in_key_bytes)?;

                // Decrement neighbour's degree
                let mut neighbour_self_key = vec![DataValue::from(layer)];
                for _ in 0..2 {
                    neighbour_self_key.extend_from_slice(&neighbour_key.0);
                    neighbour_self_key.push(DataValue::from(neighbour_key.1 as i64));
                    neighbour_self_key.push(DataValue::from(neighbour_key.2 as i64));
                }
                let neighbour_val_bytes = self
                    .store_tx
                    .get(
                        &idx_table.encode_key_for_store(&neighbour_self_key, Default::default())?,
                        false,
                    )?
                    .ok_or_else(|| miette!("Neighbor metadata not found"))?;
                let mut neighbour_val: Vec<DataValue> = decode_metadata(&neighbour_val_bytes)?;
                ensure!(
                    !neighbour_val.is_empty(),
                    "Neighbor metadata is empty or corrupted"
                );
                neighbour_val[0] = DataValue::from(
                    neighbour_val[0]
                        .get_float()
                        .ok_or_else(|| miette!("Invalid degree"))?
                        - 1.,
                );
                self.store_tx.put(
                    &idx_table.encode_key_for_store(&neighbour_self_key, Default::default())?,
                    &idx_table.encode_val_only_for_store(&neighbour_val, Default::default())?,
                )?;

                neighbours_to_repair.push((neighbour_key, layer));
            }
        }

        // Phase 2: Now that ALL edges to/from the deleted node are gone, repair
        // former neighbors that may have too few connections.
        for (neighbour_key, layer) in neighbours_to_repair {
            self.hnsw_repair_node(
                &neighbour_key,
                layer,
                manifest,
                orig_table,
                idx_table,
                vec_cache,
            )?;
        }

        // Update entry point if needed
        if encountered_singletons {
            let ep_res = idx_table
                .scan_bounded_prefix(
                    self,
                    &[],
                    &[DataValue::from(i64::MIN)],
                    &[DataValue::from(1)],
                )
                .next();
            let mut canary_key = vec![DataValue::from(1)];
            for _ in 0..2 {
                for _ in 0..orig_table.metadata.keys.len() {
                    canary_key.push(DataValue::Null);
                }
                canary_key.push(DataValue::Null);
                canary_key.push(DataValue::Null);
            }
            let canary_key_bytes =
                idx_table.encode_key_for_store(&canary_key, Default::default())?;
            if let Some(ep) = ep_res {
                let ep = ep?;
                let target_key_bytes = idx_table.encode_key_for_store(&ep, Default::default())?;
                let bottom_level = ep[0]
                    .get_int()
                    .ok_or_else(|| miette!("Invalid entry point level"))?;
                let canary_value = [
                    DataValue::from(bottom_level),
                    DataValue::Bytes(target_key_bytes),
                    DataValue::from(false),
                ];
                let canary_value_bytes =
                    idx_table.encode_val_only_for_store(&canary_value, Default::default())?;
                self.store_tx.put(&canary_key_bytes, &canary_value_bytes)?;
            } else {
                self.store_tx.del(&canary_key_bytes)?;
            }
        }

        self.hnsw_remove_pq_codes(idx_table, &(tuple_key.to_vec().into(), idx, subidx))?;

        Ok(())
    }
    pub(crate) fn hnsw_knn(
        &self,
        q: Vector,
        config: &HnswSearch,
        filter_bytecode: &Option<(Vec<Bytecode>, SourceSpan)>,
        stack: &mut Vec<DataValue>,
    ) -> Result<Vec<Tuple>> {
        if q.len() != config.manifest.vec_dim {
            bail!("query vector dimension mismatch");
        }
        let q = match (q, config.manifest.dtype) {
            (v @ Vector::F32(_), VecElementType::F32) => v,
            (v @ Vector::F64(_), VecElementType::F64) => v,
            (Vector::F32(v), VecElementType::F64) => Vector::F64(v.mapv(|x| x as f64)),
            (Vector::F64(v), VecElementType::F32) => Vector::F32(v.mapv(|x| x as f32)),
        };

        let mut vec_cache = VectorCache {
            cache: Default::default(),
            distance: config.manifest.distance,
            pq_codebook: None,
            pq_codes: Default::default(),
        };

        // Load PQ codebook if configured.
        if config.manifest.pq.is_some() {
            if let Some(codebook) = self
                .hnsw_get_pq_codebook(config.base_handle.metadata.keys.len(), &config.idx_handle)?
            {
                vec_cache.pq_codebook = Some(codebook);
            }
        }

        let ep_res = config
            .idx_handle
            .scan_bounded_prefix(
                self,
                &[],
                &[DataValue::from(i64::MIN)],
                &[DataValue::from(1)],
            )
            .next();
        if let Some(ep) = ep_res {
            let ep = ep?;
            let bottom_level = ep[0]
                .get_int()
                .ok_or_else(|| miette!("Invalid entry point level"))?;
            let ep_idx = match ep[config.base_handle.metadata.keys.len() + 1].get_int() {
                Some(x) => x as usize,
                None => {
                    // this occurs if the index is empty
                    return Ok(vec![]);
                }
            };
            let ep_t_key = ep[1..config.base_handle.metadata.keys.len() + 1]
                .to_vec()
                .into();
            let ep_subidx = ep[config.base_handle.metadata.keys.len() + 2]
                .get_int()
                .ok_or_else(|| miette!("Invalid entry point subindex"))?
                as i32;
            let ep_key = (ep_t_key, ep_idx, ep_subidx);
            vec_cache.ensure_key(&ep_key, &config.base_handle, self)?;
            let ep_distance = vec_cache.v_dist(&q, &ep_key)?;
            let mut found_nn = PriorityQueue::new();
            found_nn.push(ep_key, OrderedFloat(ep_distance));
            let pq_dist_table: Option<Vec<Vec<f64>>> = if let Some(ref codebook) =
                vec_cache.pq_codebook
            {
                let q_slice = match &q {
                    Vector::F32(arr) => arr
                        .as_slice()
                        .ok_or_else(|| miette!("Invalid query vector slice"))?,
                    _ => bail!("PQ search only supported for F32 vectors"),
                };
                let mut table = vec![vec![0.0f64; codebook.num_centroids]; codebook.num_subspaces];
                for (m, table_m) in table.iter_mut().enumerate() {
                    let start = m * codebook.sub_dim;
                    let q_sub = &q_slice[start..start + codebook.sub_dim];
                    for (c, cell) in table_m.iter_mut().enumerate() {
                        let c_start = (m * codebook.num_centroids + c) * codebook.sub_dim;
                        let centroid = &codebook.centroids[c_start..c_start + codebook.sub_dim];
                        let dist: f32 = q_sub
                            .iter()
                            .zip(centroid.iter())
                            .map(|(a, b)| (a - b) * (a - b))
                            .sum();
                        *cell = dist as f64;
                    }
                }
                Some(table)
            } else {
                None
            };
            for current_level in bottom_level..0 {
                self.hnsw_search_level(
                    &q,
                    1,
                    current_level,
                    &config.base_handle,
                    &config.idx_handle,
                    &mut found_nn,
                    &mut vec_cache,
                    None,
                    pq_dist_table.as_deref(),
                )?;
            }
            // Double ef when a filter is active to compensate for expected rejections.
            let ef_actual = if filter_bytecode.is_some() {
                config.ef * 2
            } else {
                config.ef
            };
            let filter_ref = filter_bytecode
                .as_ref()
                .map(|(code, span)| (code.as_slice(), *span));
            self.hnsw_search_level(
                &q,
                ef_actual,
                0,
                &config.base_handle,
                &config.idx_handle,
                &mut found_nn,
                &mut vec_cache,
                filter_ref,
                pq_dist_table.as_deref(),
            )?;
            if found_nn.is_empty() {
                return Ok(vec![]);
            }

            if config.filter.is_none() {
                while found_nn.len() > config.k {
                    found_nn.pop();
                }
            }

            let mut ret = vec![];

            while let Some((cand_key, OrderedFloat(distance))) = found_nn.pop() {
                if let Some(r) = config.radius {
                    if distance > r {
                        continue;
                    }
                }

                let mut cand_tuple = config
                    .base_handle
                    .get(self, &cand_key.0)?
                    .ok_or_else(|| miette!("corrupted index"))?;

                // make sure the order is the same as in all_bindings()!!!
                if config.bind_field.is_some() {
                    let field = if cand_key.1 < config.base_handle.metadata.keys.len() {
                        config.base_handle.metadata.keys[cand_key.1].name.clone()
                    } else {
                        config.base_handle.metadata.non_keys
                            [cand_key.1 - config.base_handle.metadata.keys.len()]
                        .name
                        .clone()
                    };
                    cand_tuple.push(DataValue::Str(field));
                }
                if config.bind_field_idx.is_some() {
                    cand_tuple.push(if cand_key.2 < 0 {
                        DataValue::Null
                    } else {
                        DataValue::from(cand_key.2 as i64)
                    });
                }
                if config.bind_distance.is_some() {
                    cand_tuple.push(DataValue::from(distance));
                }
                if config.bind_vector.is_some() {
                    let vec = if cand_key.2 < 0 {
                        cand_tuple[cand_key.1].clone()
                    } else {
                        match &cand_tuple[cand_key.1] {
                            DataValue::List(v) => v[cand_key.2 as usize].clone(),
                            v => bail!("corrupted index value {:?}", v),
                        }
                    };
                    cand_tuple.push(vec);
                }

                if let Some((code, span)) = filter_bytecode {
                    if !eval_bytecode_pred(code, &cand_tuple, stack, *span)? {
                        continue;
                    }
                }

                ret.push(cand_tuple);
            }
            ret.reverse();
            ret.truncate(config.k);

            Ok(ret)
        } else {
            Ok(vec![])
        }
    }

    pub(crate) fn hnsw_train_pq(
        &mut self,
        base_relation: &str,
        index_name: &str,
        num_subspaces: usize,
        num_centroids: usize,
        num_samples: usize,
    ) -> Result<()> {
        let base_handle = self.get_relation(base_relation, false)?;
        let (idx_handle, manifest) = base_handle
            .hnsw_indices
            .get(index_name)
            .ok_or_else(|| miette!("HNSW index {} not found on {}", index_name, base_relation))?;
        let idx_handle = idx_handle.clone();
        let mut manifest = manifest.clone();
        let key_len = base_handle.metadata.keys.len();
        let field_idx = *manifest
            .vec_fields
            .first()
            .ok_or_else(|| miette!("HNSW index has no vector fields"))?;

        ensure!(
            manifest.dtype == VecElementType::F32,
            "PQ training only supported for F32 vectors"
        );
        let dim = manifest.vec_dim;
        ensure!(
            dim % num_subspaces == 0,
            "vec_dim {} must be divisible by num_subspaces {}",
            dim,
            num_subspaces
        );
        ensure!(num_centroids >= 1, "num_centroids must be at least 1");

        let sub_dim = dim / num_subspaces;

        let mut all_samples: Vec<Vec<f32>> = base_handle
            .scan_all(self)
            .filter_map(|t| {
                let tuple = t.ok()?;
                match tuple.get(field_idx) {
                    Some(DataValue::Vec(v)) => {
                        if let Vector::F32(arr) = v.as_ref() {
                            arr.as_slice().map(|s| s.to_vec())
                        } else {
                            None
                        }
                    }
                    Some(DataValue::List(l)) => l.iter().find_map(|item| {
                        if let DataValue::Vec(v) = item {
                            if let Vector::F32(arr) = v.as_ref() {
                                arr.as_slice().map(|s| s.to_vec())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }),
                    _ => None,
                }
            })
            .collect();

        if all_samples.len() > num_samples {
            let mut rng = rand::thread_rng();
            let n = all_samples.len();
            for i in 0..num_samples.min(n) {
                let j = rng.gen_range(i..n);
                all_samples.swap(i, j);
            }
            all_samples.truncate(num_samples);
        }

        ensure!(
            !all_samples.is_empty(),
            "no vectors found in {} to train PQ",
            base_relation
        );
        ensure!(
            all_samples.len() >= num_centroids,
            "need at least {} vectors to train {} centroids, got {}",
            num_centroids,
            num_centroids,
            all_samples.len()
        );

        let mut centroids_flat: Vec<f32> =
            Vec::with_capacity(num_subspaces * num_centroids * sub_dim);
        for m in 0..num_subspaces {
            let start = m * sub_dim;
            let end = start + sub_dim;
            let subspace_data: Vec<Vec<f32>> =
                all_samples.iter().map(|v| v[start..end].to_vec()).collect();
            let centroids = kmeans_lloyd(&subspace_data, num_centroids, 25);
            for c in &centroids {
                centroids_flat.extend_from_slice(c);
            }
        }

        let codebook = PqCodebook {
            num_subspaces,
            num_centroids,
            sub_dim,
            centroids: centroids_flat,
        };

        self.hnsw_store_pq_codebook(key_len, &idx_handle, &codebook)?;

        // Encode all existing vectors and store their PQ codes.
        let all_tuples: Vec<Tuple> = base_handle.scan_all(self).filter_map(|r| r.ok()).collect();
        for tuple in all_tuples {
            let tuple_key = &tuple[..key_len];
            if let Some(DataValue::Vec(v)) = tuple.get(field_idx) {
                if let Vector::F32(_) = v.as_ref() {
                    let codes = encode_vector_pq(v, &codebook)?;
                    let compound_key = (tuple_key.to_vec().into(), field_idx, -1i32);
                    self.hnsw_store_pq_codes(&idx_handle, &compound_key, &codes)?;
                }
            } else if let Some(DataValue::List(l)) = tuple.get(field_idx) {
                for (sidx, item) in l.iter().enumerate() {
                    if let DataValue::Vec(v) = item {
                        if let Vector::F32(_) = v.as_ref() {
                            let codes = encode_vector_pq(v, &codebook)?;
                            let compound_key = (tuple_key.to_vec().into(), field_idx, sidx as i32);
                            self.hnsw_store_pq_codes(&idx_handle, &compound_key, &codes)?;
                        }
                    }
                }
            }
        }

        manifest.pq = Some(PqConfig {
            num_subspaces,
            num_centroids,
        });
        self.update_hnsw_manifest(base_relation, index_name, manifest)?;
        Ok(())
    }

    fn hnsw_store_pq_codebook(
        &mut self,
        orig_key_len: usize,
        idx_handle: &RelationHandle,
        codebook: &PqCodebook,
    ) -> Result<()> {
        let mut cb_key = vec![DataValue::from(i64::MAX)];
        for _ in 0..2 {
            for _ in 0..orig_key_len {
                cb_key.push(DataValue::Null);
            }
            cb_key.push(DataValue::Null);
            cb_key.push(DataValue::Null);
        }
        let codebook_bytes = rmp_serde::to_vec(codebook)
            .map_err(|e| miette!("failed to serialize PQ codebook: {e}"))?;
        let cb_val = [
            DataValue::from(0.0),
            DataValue::Bytes(codebook_bytes),
            DataValue::from(false),
        ];
        let key_bytes = idx_handle.encode_key_for_store(&cb_key, Default::default())?;
        let val_bytes = idx_handle.encode_val_only_for_store(&cb_val, Default::default())?;
        self.store_tx.put(&key_bytes, &val_bytes)?;
        Ok(())
    }

    pub(crate) fn hnsw_get_pq_codebook(
        &self,
        orig_key_len: usize,
        idx_handle: &RelationHandle,
    ) -> Result<Option<PqCodebook>> {
        let mut cb_key = vec![DataValue::from(i64::MAX)];
        for _ in 0..2 {
            for _ in 0..orig_key_len {
                cb_key.push(DataValue::Null);
            }
            cb_key.push(DataValue::Null);
            cb_key.push(DataValue::Null);
        }
        let key_bytes = idx_handle.encode_key_for_store(&cb_key, Default::default())?;
        match self.store_tx.get(&key_bytes, false)? {
            None => Ok(None),
            Some(val_bytes) => {
                let val_tuple: Vec<DataValue> = decode_metadata(&val_bytes)?;
                match val_tuple.get(1) {
                    Some(DataValue::Bytes(bytes)) => {
                        let codebook = rmp_serde::from_slice(bytes)
                            .map_err(|e| miette!("failed to deserialize PQ codebook: {e}"))?;
                        Ok(Some(codebook))
                    }
                    _ => Ok(None),
                }
            }
        }
    }

    fn hnsw_store_pq_codes(
        &mut self,
        idx_handle: &RelationHandle,
        key: &CompoundKey,
        codes: &[u8],
    ) -> Result<()> {
        let mut pq_key = vec![DataValue::from(i64::MAX - 1)];
        pq_key.extend_from_slice(&key.0);
        pq_key.push(DataValue::from(key.1 as i64));
        pq_key.push(DataValue::from(key.2 as i64));
        pq_key.extend_from_slice(&key.0);
        pq_key.push(DataValue::from(key.1 as i64));
        pq_key.push(DataValue::from(key.2 as i64));
        let pq_val = [DataValue::Bytes(codes.to_vec())];
        let key_bytes = idx_handle.encode_key_for_store(&pq_key, Default::default())?;
        let val_bytes = idx_handle.encode_val_only_for_store(&pq_val, Default::default())?;
        self.store_tx.put(&key_bytes, &val_bytes)?;
        Ok(())
    }

    fn hnsw_remove_pq_codes(
        &mut self,
        idx_handle: &RelationHandle,
        key: &CompoundKey,
    ) -> Result<()> {
        let mut pq_key = vec![DataValue::from(i64::MAX - 1)];
        pq_key.extend_from_slice(&key.0);
        pq_key.push(DataValue::from(key.1 as i64));
        pq_key.push(DataValue::from(key.2 as i64));
        pq_key.extend_from_slice(&key.0);
        pq_key.push(DataValue::from(key.1 as i64));
        pq_key.push(DataValue::from(key.2 as i64));
        if let Ok(key_bytes) = idx_handle.encode_key_for_store(&pq_key, Default::default()) {
            let _ = self.store_tx.del(&key_bytes);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;
    use std::collections::BTreeMap;

    #[test]
    fn test_random_level() {
        let m = 20;
        let mult = 1. / (m as f64).ln();
        let mut rng = rand::thread_rng();
        let mut collected = BTreeMap::new();
        for _ in 0..10000 {
            let uniform_num: f64 = rng.gen_range(0.0..1.0);
            let r = -uniform_num.ln() * mult;
            // the level is the largest integer smaller than r
            let level = -(r.floor() as i64);
            collected.entry(level).and_modify(|x| *x += 1).or_insert(1);
        }
        println!("{:?}", collected);
    }
}
