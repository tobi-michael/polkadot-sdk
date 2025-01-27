// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Cumulus.

// Cumulus is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Cumulus is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Cumulus.  If not, see <http://www.gnu.org/licenses/>.

//! Stock, pure Aura collators.
//!
//! This includes the [`basic`] collator, which only builds on top of the most recently
//! included parachain block, as well as the [`lookahead`] collator, which prospectively
//! builds on parachain blocks which have not yet been included in the relay chain.

use crate::collator::SlotClaim;
use codec::Codec;
use cumulus_client_consensus_common::{
	self as consensus_common, load_abridged_host_configuration, ParentSearchParams,
};
use cumulus_primitives_aura::{AuraUnincludedSegmentApi, Slot};
use cumulus_primitives_core::{relay_chain::Hash as ParaHash, BlockT, ClaimQueueOffset};
use cumulus_relay_chain_interface::RelayChainInterface;
use polkadot_node_subsystem_util::runtime::ClaimQueueSnapshot;
use polkadot_primitives::{
	AsyncBackingParams, CoreIndex, Hash as RelayHash, Id as ParaId, OccupiedCoreAssumption,
	ValidationCodeHash,
};
use sc_consensus_aura::{standalone as aura_internal, AuraApi};
use sp_api::{ApiExt, ProvideRuntimeApi};
use sp_core::Pair;
use sp_keystore::KeystorePtr;
use sp_timestamp::Timestamp;

pub mod basic;
pub mod lookahead;
pub mod slot_based;

// This is an arbitrary value which is likely guaranteed to exceed any reasonable
// limit, as it would correspond to 10 non-included blocks.
//
// Since we only search for parent blocks which have already been imported,
// we can guarantee that all imported blocks respect the unincluded segment
// rules specified by the parachain's runtime and thus will never be too deep.
const PARENT_SEARCH_DEPTH: usize = 10;

/// Check the `local_validation_code_hash` against the validation code hash in the relay chain
/// state.
///
/// If the code hashes do not match, it prints a warning.
async fn check_validation_code_or_log(
	local_validation_code_hash: &ValidationCodeHash,
	para_id: ParaId,
	relay_client: &impl RelayChainInterface,
	relay_parent: RelayHash,
) {
	let state_validation_code_hash = match relay_client
		.validation_code_hash(relay_parent, para_id, OccupiedCoreAssumption::Included)
		.await
	{
		Ok(hash) => hash,
		Err(error) => {
			tracing::debug!(
				target: super::LOG_TARGET,
				%error,
				?relay_parent,
				%para_id,
				"Failed to fetch validation code hash",
			);
			return
		},
	};

	match state_validation_code_hash {
		Some(state) =>
			if state != *local_validation_code_hash {
				tracing::warn!(
					target: super::LOG_TARGET,
					%para_id,
					?relay_parent,
					?local_validation_code_hash,
					relay_validation_code_hash = ?state,
					"Parachain code doesn't match validation code stored in the relay chain state.",
				);
			},
		None => {
			tracing::warn!(
				target: super::LOG_TARGET,
				%para_id,
				?relay_parent,
				"Could not find validation code for parachain in the relay chain state.",
			);
		},
	}
}

/// Reads async backing parameters from the relay chain storage at the given relay parent.
async fn async_backing_params(
	relay_parent: RelayHash,
	relay_client: &impl RelayChainInterface,
) -> Option<AsyncBackingParams> {
	match load_abridged_host_configuration(relay_parent, relay_client).await {
		Ok(Some(config)) => Some(config.async_backing_params),
		Ok(None) => {
			tracing::error!(
				target: crate::LOG_TARGET,
				"Active config is missing in relay chain storage",
			);
			None
		},
		Err(err) => {
			tracing::error!(
				target: crate::LOG_TARGET,
				?err,
				?relay_parent,
				"Failed to read active config from relay chain client",
			);
			None
		},
	}
}

// Return all the cores assigned to the para at the provided relay parent, using the claim queue
// offset.
// Will return an empty vec if the provided offset is higher than the claim queue length (which
// corresponds to the scheduling_lookahead on the relay chain).
async fn cores_scheduled_for_para(
	relay_parent: RelayHash,
	para_id: ParaId,
	relay_client: &impl RelayChainInterface,
	claim_queue_offset: ClaimQueueOffset,
) -> Vec<CoreIndex> {
	// Get `ClaimQueue` from runtime
	let claim_queue: ClaimQueueSnapshot = match relay_client.claim_queue(relay_parent).await {
		Ok(claim_queue) => claim_queue.into(),
		Err(error) => {
			tracing::error!(
				target: crate::LOG_TARGET,
				?error,
				?relay_parent,
				"Failed to query claim queue runtime API",
			);
			return Vec::new()
		},
	};

	claim_queue
		.iter_claims_at_depth(claim_queue_offset.0 as usize)
		.filter_map(|(core_index, core_para_id)| (core_para_id == para_id).then_some(core_index))
		.collect()
}

// Checks if we own the slot at the given block and whether there
// is space in the unincluded segment.
async fn can_build_upon<Block: BlockT, Client, P>(
	para_slot: Slot,
	relay_slot: Slot,
	timestamp: Timestamp,
	parent_hash: Block::Hash,
	included_block: Block::Hash,
	client: &Client,
	keystore: &KeystorePtr,
) -> Option<SlotClaim<P::Public>>
where
	Client: ProvideRuntimeApi<Block>,
	Client::Api: AuraApi<Block, P::Public> + AuraUnincludedSegmentApi<Block> + ApiExt<Block>,
	P: Pair,
	P::Public: Codec,
	P::Signature: Codec,
{
	let runtime_api = client.runtime_api();
	let authorities = runtime_api.authorities(parent_hash).ok()?;
	let author_pub = aura_internal::claim_slot::<P>(para_slot, &authorities, keystore).await?;

	let Ok(Some(api_version)) =
		runtime_api.api_version::<dyn AuraUnincludedSegmentApi<Block>>(parent_hash)
	else {
		return (parent_hash == included_block)
			.then(|| SlotClaim::unchecked::<P>(author_pub, para_slot, timestamp));
	};

	let slot = if api_version > 1 { relay_slot } else { para_slot };

	runtime_api
		.can_build_upon(parent_hash, included_block, slot)
		.ok()?
		.then(|| SlotClaim::unchecked::<P>(author_pub, para_slot, timestamp))
}

/// Use [`cumulus_client_consensus_common::find_potential_parents`] to find parachain blocks that
/// we can build on. Once a list of potential parents is retrieved, return the last one of the
/// longest chain.
async fn find_parent<Block>(
	relay_parent: ParaHash,
	para_id: ParaId,
	para_backend: &impl sc_client_api::Backend<Block>,
	relay_client: &impl RelayChainInterface,
) -> Option<(<Block as BlockT>::Hash, consensus_common::PotentialParent<Block>)>
where
	Block: BlockT,
{
	let parent_search_params = ParentSearchParams {
		relay_parent,
		para_id,
		ancestry_lookback: crate::collators::async_backing_params(relay_parent, relay_client)
			.await
			.map_or(0, |params| params.allowed_ancestry_len as usize),
		max_depth: PARENT_SEARCH_DEPTH,
		ignore_alternative_branches: true,
	};

	let potential_parents = cumulus_client_consensus_common::find_potential_parents::<Block>(
		parent_search_params,
		para_backend,
		relay_client,
	)
	.await;

	let potential_parents = match potential_parents {
		Err(e) => {
			tracing::error!(
				target: crate::LOG_TARGET,
				?relay_parent,
				err = ?e,
				"Could not fetch potential parents to build upon"
			);

			return None
		},
		Ok(x) => x,
	};

	let included_block = potential_parents.iter().find(|x| x.depth == 0)?.hash;
	potential_parents
		.into_iter()
		.max_by_key(|a| a.depth)
		.map(|parent| (included_block, parent))
}
