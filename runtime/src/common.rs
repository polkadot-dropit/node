use sp_runtime::Perbill;

use crate::{Balances, Runtime, Treasury};
use frame_support::{
	traits::{Currency, Imbalance, OnUnbalanced},
	weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight},
};
use pallet_balances::NegativeImbalance;

// Cumulus types re-export
//https://github.com/paritytech/cumulus/tree/master/parachains/common
pub use parachains_common::{AccountId, AuraId, Balance, Block, BlockNumber, Hash, Signature};

/// Nonce for an account
pub type Nonce = u32;

/// This determines the average expected block time that we are targeting.
/// Blocks will be produced at a minimum duration defined by `SLOT_DURATION`.
/// `SLOT_DURATION` is picked up by `pallet_timestamp` which is in turn picked
/// up by `pallet_aura` to implement `fn slot_duration()`.
///
/// Change this to adjust the block time.
//
// TODO: Update comment and/or usage to reflect we don't use aura.
pub const MILLISECS_PER_BLOCK: u64 = 6000;

// NOTE: Currently it is not possible to change the slot duration after the chain has started.
// Attempting to do so will brick block production.
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
/// used to limit the maximal weight of a single extrinsic.
pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
/// `Operational` extrinsics.
pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// We allow for 0.5 of a second of compute with a 12 second average block time.
pub const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
	WEIGHT_REF_TIME_PER_SECOND.saturating_mul(2),
	polkadot_primitives::MAX_POV_SIZE as u64,
);

/// Logic for the author to get a portion of fees.
pub struct ToAuthor;
impl OnUnbalanced<NegativeImbalance<Runtime>> for ToAuthor {
	fn on_nonzero_unbalanced(amount: NegativeImbalance<Runtime>) {
		if let Some(author) = <pallet_author_reward_dest::Pallet<Runtime>>::author() {
			Balances::resolve_creating(&author, amount);
		} else {
			log::error!(
				"Author reward destination not available, but is mandatory, fall back to treasury"
			);
			<Treasury as OnUnbalanced<_>>::on_unbalanced(amount);
		}
	}
}

pub struct DealWithFees;
impl OnUnbalanced<NegativeImbalance<Runtime>> for DealWithFees {
	fn on_unbalanceds<B>(mut fees_then_tips: impl Iterator<Item = NegativeImbalance<Runtime>>) {
		if let Some(fees) = fees_then_tips.next() {
			// for fees, 80% to treasury, 20% to author
			// TODO: adjust the split
			let mut split = fees.ration(80, 20);
			if let Some(tips) = fees_then_tips.next() {
				// for tips, if any, 100% to author
				// TODO: does part of tip go to treasury?
				tips.merge_into(&mut split.1);
			}
			<Treasury as OnUnbalanced<_>>::on_unbalanced(split.0);
			<ToAuthor as OnUnbalanced<_>>::on_unbalanced(split.1);
		}
	}
}

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarks {
	use sp_runtime::AccountId32;

	/// Provide factory methods for the benchmarks of treasury.
	pub struct TreasuryArguments;
	impl pallet_treasury::ArgumentsFactory<(), AccountId32> for TreasuryArguments {
		fn create_asset_kind(_seed: u32) -> () {
			()
		}
		fn create_beneficiary(seed: [u8; 32]) -> AccountId32 {
			AccountId32::from(seed)
		}
	}
}
