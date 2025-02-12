// Bitcoin Dev Kit
// Written in 2020 by Alekos Filini <alekos.filini@gmail.com>
//
// Copyright (c) 2020-2021 Bitcoin Dev Kit Developers
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Esplora by way of `ureq` HTTP client.

use std::collections::HashMap;
use std::io;
use std::io::Cursor;
use std::str::FromStr;
use std::time::Duration;

use bpstd::{BlockHash, ConsensusDecode, ScriptPubkey, Tx, Txid};

#[allow(unused_imports)]
use log::{debug, error, info, trace};
use sha2::{Digest, Sha256};

use ureq::{Agent, Proxy, Response};

use crate::{BlockStatus, BlockSummary, Builder, Error, OutputStatus, TxStatus, Utxo};

#[derive(Debug, Clone)]
pub struct BlockingClient {
    url: String,
    agent: Agent,
}

impl BlockingClient {
    /// build a blocking client from a [`Builder`]
    pub fn from_builder(builder: Builder) -> Result<Self, Error> {
        let mut agent_builder = ureq::AgentBuilder::new();

        if let Some(timeout) = builder.timeout {
            agent_builder = agent_builder.timeout(Duration::from_secs(timeout));
        }

        if let Some(proxy) = &builder.proxy {
            agent_builder = agent_builder.proxy(Proxy::new(proxy)?);
        }

        Ok(Self::from_agent(builder.base_url, agent_builder.build()))
    }

    /// build a blocking client from an [`Agent`]
    pub fn from_agent(url: String, agent: Agent) -> Self {
        BlockingClient { url, agent }
    }

    /// Get a [`Transaction`] option given its [`Txid`]
    pub fn tx(&self, txid: &Txid) -> Result<Option<Tx>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/tx/{}/raw", self.url, txid))
            .call();

        match resp {
            Ok(resp) => {
                let bytes = into_bytes(resp)?;
                let tx = Tx::consensus_decode(&mut Cursor::new(bytes))
                    .map_err(|_| Error::InvalidServerData)?;
                Ok(Some(tx))
            }
            Err(ureq::Error::Status(code, _)) => {
                if is_status_not_found(code) {
                    return Ok(None);
                }
                Err(Error::HttpResponse(code))
            }
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get a [`Transaction`] given its [`Txid`].
    pub fn tx_no_opt(&self, txid: &Txid) -> Result<Tx, Error> {
        match self.tx(txid) {
            Ok(Some(tx)) => Ok(tx),
            Ok(None) => Err(Error::TransactionNotFound(*txid)),
            Err(e) => Err(e),
        }
    }

    /// Get a [`Txid`] of a transaction given its index in a block with a given hash.
    pub fn txid_at_block_index(
        &self,
        block_hash: &BlockHash,
        index: usize,
    ) -> Result<Option<Txid>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/block/{}/txid/{}", self.url, block_hash, index))
            .call();

        match resp {
            Ok(resp) => Ok(Some(Txid::from_str(&resp.into_string()?)?)),
            Err(ureq::Error::Status(code, _)) => {
                if is_status_not_found(code) {
                    return Ok(None);
                }
                Err(Error::HttpResponse(code))
            }
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get the status of a [`Transaction`] given its [`Txid`].
    pub fn tx_status(&self, txid: &Txid) -> Result<TxStatus, Error> {
        let resp = self
            .agent
            .get(&format!("{}/tx/{}/status", self.url, txid))
            .call();

        match resp {
            Ok(resp) => Ok(resp.into_json()?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /* Uncomment once `bp-primitives` will support consensus serialziation
    /// Get a [`BlockHeader`] given a particular block hash.
    pub fn header_by_hash(&self, block_hash: &BlockHash) -> Result<BlockHeader, Error> {
        let resp = self
            .agent
            .get(&format!("{}/block/{}/header", self.url, block_hash))
            .call();

        match resp {
            Ok(resp) => Ok(deserialize(&Vec::from_hex(&resp.into_string()?)?)?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }
     */

    /// Get the [`BlockStatus`] given a particular [`BlockHash`].
    pub fn block_status(&self, block_hash: &BlockHash) -> Result<BlockStatus, Error> {
        let resp = self
            .agent
            .get(&format!("{}/block/{}/status", self.url, block_hash))
            .call();

        match resp {
            Ok(resp) => Ok(resp.into_json()?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /* TODO: Uncomment once `bp-primitives` will support blocks
    /// Get a [`Block`] given a particular [`BlockHash`].
    pub fn block_by_hash(&self, block_hash: &BlockHash) -> Result<Option<Block>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/block/{}/raw", self.url, block_hash))
            .call();

        match resp {
            Ok(resp) => Ok(Some(deserialize(&into_bytes(resp)?)?)),
            Err(ureq::Error::Status(code, _)) => {
                if is_status_not_found(code) {
                    return Ok(None);
                }
                Err(Error::HttpResponse(code))
            }
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get a merkle inclusion proof for a [`Transaction`] with the given [`Txid`].
    pub fn merkle_proof(&self, txid: &Txid) -> Result<Option<MerkleProof>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/tx/{}/merkle-proof", self.url, txid))
            .call();

        match resp {
            Ok(resp) => Ok(Some(resp.into_json()?)),
            Err(ureq::Error::Status(code, _)) => {
                if is_status_not_found(code) {
                    return Ok(None);
                }
                Err(Error::HttpResponse(code))
            }
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get a [`MerkleBlock`] inclusion proof for a [`Transaction`] with the given [`Txid`].
    pub fn merkle_block(&self, txid: &Txid) -> Result<Option<MerkleBlock>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/tx/{}/merkleblock-proof", self.url, txid))
            .call();

        match resp {
            Ok(resp) => Ok(Some(deserialize(&Vec::from_hex(&resp.into_string()?)?)?)),
            Err(ureq::Error::Status(code, _)) => {
                if is_status_not_found(code) {
                    return Ok(None);
                }
                Err(Error::HttpResponse(code))
            }
            Err(e) => Err(Error::Ureq(e)),
        }
    }
     */

    /// Get the spending status of an output given a [`Txid`] and the output index.
    pub fn output_status(&self, txid: &Txid, index: u64) -> Result<Option<OutputStatus>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/tx/{}/outspend/{}", self.url, txid, index))
            .call();

        match resp {
            Ok(resp) => Ok(Some(resp.into_json()?)),
            Err(ureq::Error::Status(code, _)) => {
                if is_status_not_found(code) {
                    return Ok(None);
                }
                Err(Error::HttpResponse(code))
            }
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Broadcast a [`Transaction`] to Esplora
    pub fn broadcast(&self, tx: &Tx) -> Result<(), Error> {
        let resp = self
            .agent
            .post(&format!("{}/tx", self.url))
            .send_string(&format!("{tx:x}"));

        match resp {
            Ok(_) => Ok(()), // We do not return the txid?
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get the height of the current blockchain tip.
    pub fn height(&self) -> Result<u32, Error> {
        let resp = self
            .agent
            .get(&format!("{}/blocks/tip/height", self.url))
            .call();

        match resp {
            Ok(resp) => Ok(resp.into_string()?.parse()?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get the [`BlockHash`] of the current blockchain tip.
    pub fn tip_hash(&self) -> Result<BlockHash, Error> {
        let resp = self
            .agent
            .get(&format!("{}/blocks/tip/hash", self.url))
            .call();

        Self::process_block_result(resp)
    }

    /// Get the [`BlockHash`] of a specific block height
    pub fn block_hash(&self, block_height: u32) -> Result<BlockHash, Error> {
        let resp = self
            .agent
            .get(&format!("{}/block-height/{}", self.url, block_height))
            .call();

        if let Err(ureq::Error::Status(code, _)) = resp {
            if is_status_not_found(code) {
                return Err(Error::HeaderHeightNotFound(block_height));
            }
        }

        Self::process_block_result(resp)
    }

    fn process_block_result(response: Result<Response, ureq::Error>) -> Result<BlockHash, Error> {
        match response {
            Ok(resp) => Ok(BlockHash::from_str(&resp.into_string()?)?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get an map where the key is the confirmation target (in number of blocks)
    /// and the value is the estimated feerate (in sat/vB).
    pub fn fee_estimates(&self) -> Result<HashMap<String, f64>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/fee-estimates", self.url,))
            .call();

        let map = match resp {
            Ok(resp) => {
                let map: HashMap<String, f64> = resp.into_json()?;
                Ok(map)
            }
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }?;

        Ok(map)
    }

    /// Get confirmed transaction history for the specified address/scripthash,
    /// sorted with newest first. Returns 25 transactions per page.
    /// More can be requested by specifying the last txid seen by the previous query.
    pub fn scripthash_txs(
        &self,
        script: &ScriptPubkey,
        last_seen: Option<Txid>,
    ) -> Result<Vec<crate::Tx>, Error> {
        let mut hasher = Sha256::default();
        hasher.update(script);
        let script_hash = hasher.finalize();
        let url = match last_seen {
            Some(last_seen) => format!(
                "{}/scripthash/{:x}/txs/chain/{}",
                self.url, script_hash, last_seen
            ),
            None => format!("{}/scripthash/{:x}/txs", self.url, script_hash),
        };
        Ok(self.agent.get(&url).call()?.into_json()?)
    }

    /// Get confirmed transaction history for the specified address/scripthash,
    /// sorted with newest first. Returns 25 transactions per page.
    /// More can be requested by specifying the last txid seen by the previous query.
    pub fn scripthash_utxo(&self, script: &ScriptPubkey) -> Result<Vec<Utxo>, Error> {
        let mut hasher = Sha256::default();
        hasher.update(script);
        let script_hash = hasher.finalize();
        let url = format!("{}/scripthash/{:x}/utxo", self.url, script_hash);
        Ok(self.agent.get(&url).call()?.into_json()?)
    }

    /// Gets some recent block summaries starting at the tip or at `height` if provided.
    ///
    /// The maximum number of summaries returned depends on the backend itself: esplora returns `10`
    /// while [mempool.space](https://mempool.space/docs/api) returns `15`.
    pub fn blocks(&self, height: Option<u32>) -> Result<Vec<BlockSummary>, Error> {
        let url = match height {
            Some(height) => format!("{}/blocks/{}", self.url, height),
            None => format!("{}/blocks", self.url),
        };

        Ok(self.agent.get(&url).call()?.into_json()?)
    }

    /// Get the underlying base URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the underlying [`Agent`].
    pub fn agent(&self) -> &Agent {
        &self.agent
    }
}

fn is_status_not_found(status: u16) -> bool {
    status == 404
}

fn into_bytes(resp: Response) -> Result<Vec<u8>, std::io::Error> {
    use std::io::Read;
    const BYTES_LIMIT: usize = 10 * 1_024 * 1_024;

    let mut buf: Vec<u8> = vec![];
    resp.into_reader()
        .take((BYTES_LIMIT + 1) as u64)
        .read_to_end(&mut buf)?;
    if buf.len() > BYTES_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "response too big for into_bytes",
        ));
    }

    Ok(buf)
}
