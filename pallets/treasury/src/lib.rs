#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use frame_support::traits::fungible;
use frame_support::traits::fungible::Credit;
use frame_support::PalletId;

pub type BalanceOf<T> = <<T as Config>::NativeBalance as fungible::Inspect<
	<T as frame_system::Config>::AccountId,
>>::Balance;

pub type CreditOf<T> = Credit<<T as frame_system::Config>::AccountId, <T as Config>::NativeBalance>;

pub const TREASURY_PALLET_ID: PalletId = PalletId(*b"dropit/t");

/// An index of a proposal. Just a `u32`.
pub type ProposalIndex = u32;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::*,
		sp_runtime::traits::AccountIdConversion,
		sp_runtime::ArithmeticError,
		traits::fungible::{Balanced as _, MutateHold as _},
		traits::{fungible, DefensiveResult, OnUnbalanced},
	};
	use frame_system::pallet_prelude::*;
	use polkadot_parachain_primitives::primitives::RelayChainBlockNumber;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config + cumulus_pallet_parachain_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Type to access the Balances Pallet.
		type NativeBalance: fungible::Inspect<Self::AccountId>
			+ fungible::Mutate<Self::AccountId>
			+ fungible::hold::Inspect<Self::AccountId>
			+ fungible::hold::Mutate<Self::AccountId, Reason = Self::RuntimeHoldReason>
			+ fungible::Balanced<Self::AccountId>;

		/// The hold reason when reserving funds for entering or extending the safe-mode.
		type RuntimeHoldReason: From<HoldReason>;

		/// Minimum amount of funds that should be placed in a deposit for making a proposal.
		#[pallet::constant]
		type ProposalBondMinimum: Get<BalanceOf<Self>>;
	}

	/// A reason for the pallet placing a hold on funds.
	#[pallet::composite_enum]
	pub enum HoldReason {
		/// Funds are held for entering or extending the safe-mode.
		#[codec(index = 0)]
		TreasuryProposal,
	}

	/// A spending proposal.
	#[derive(Encode, Decode, MaxEncodedLen, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Proposal<T: Config> {
		/// The account proposing it.
		proposer: T::AccountId,
		/// The (total) amount that should be paid if the proposal is accepted.
		value: BalanceOf<T>,
		/// The account to whom the payment should be made if the proposal is accepted.
		beneficiary: T::AccountId,
		/// The amount held on deposit (reserved) for making this proposal.
		bond: BalanceOf<T>,
	}

	/// Number of proposals that have been made.
	#[pallet::storage]
	pub(crate) type ProposalCount<T> = StorageValue<_, ProposalIndex, ValueQuery>;

	/// Proposals that have been made.
	#[pallet::storage]
	pub type Proposals<T: Config> = StorageMap<_, Blake2_128Concat, ProposalIndex, Proposal<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New proposal.
		Proposed { proposal_index: ProposalIndex },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Proposer's balance is too low.
		InsufficientProposersBalance,
		/// No proposal, bounty or spend at that index.
		InvalidIndex,
	}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new distribution of `asset_id`.
		/// This extrinsic establishes the `DistributionId` for this distribution and the respective `stash_account`.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::default())]
		pub fn propose_spend(
			origin: OriginFor<T>,
			#[pallet::compact] value: BalanceOf<T>,
			beneficiary: T::AccountId,
		) -> DispatchResult {
			let proposer = ensure_signed(origin)?;

			let bond = Self::calculate_bond(value);
			T::NativeBalance::hold(&HoldReason::TreasuryProposal.into(), &proposer, bond)
				.map_err(|_| Error::<T>::InsufficientProposersBalance)?;

			let proposal_count = ProposalCount::<T>::get();
			let new_proposal_count =
				proposal_count.checked_add(1).ok_or(ArithmeticError::Overflow)?;
			<Proposals<T>>::insert(proposal_count, Proposal { proposer, value, beneficiary, bond });
			ProposalCount::<T>::put(new_proposal_count);

			Self::deposit_event(Event::Proposed { proposal_index: proposal_count });
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// The account ID of the treasury pot.
		///
		/// This actually does computation. If you need to keep using it, then make sure you cache the
		/// value and only call this once.
		pub fn account_id() -> T::AccountId {
			TREASURY_PALLET_ID.into_account_truncating()
		}

		/// The needed bond for a proposal whose spend is `value`.
		fn calculate_bond(_value: BalanceOf<T>) -> BalanceOf<T> {
			// Currently we used a fixed bond amount.
			T::ProposalBondMinimum::get()
		}
	}

	impl<T: Config> OnUnbalanced<CreditOf<T>> for Pallet<T> {
		fn on_nonzero_unbalanced(amount: CreditOf<T>) {
			// Must resolve into existing but better to be safe.
			T::NativeBalance::resolve(&Self::account_id(), amount).defensive_ok();
		}
	}
}
