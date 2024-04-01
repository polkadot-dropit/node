#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::fungibles;
/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

use frame_support::traits::fungible;
use frame_support::PalletId;

pub type AssetIdOf<T> = <<T as Config>::Fungibles as fungibles::Inspect<
	<T as frame_system::Config>::AccountId,
>>::AssetId;

pub type BalanceOf<T> = <<T as Config>::NativeBalance as fungible::Inspect<
	<T as frame_system::Config>::AccountId,
>>::Balance;

pub type AssetBalanceOf<T> = <<T as Config>::Fungibles as fungibles::Inspect<
	<T as frame_system::Config>::AccountId,
>>::Balance;

type DistributionId = u32;

pub const DISTRIBUTION_PALLET_ID: PalletId = PalletId(*b"dropit/d");

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		pallet_prelude::*,
		sp_runtime::traits::{AccountIdConversion, CheckedAdd, CheckedSub, One, Saturating, Zero},
		sp_runtime::{ArithmeticError, Perbill},
		traits::fungible::Mutate as _,
		traits::fungibles::{Inspect as _, Mutate as _},
		traits::{fungible, fungibles, tokens::Preservation},
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
			+ fungible::hold::Mutate<Self::AccountId>
			+ fungible::freeze::Inspect<Self::AccountId>
			+ fungible::freeze::Mutate<Self::AccountId>;

		/// Type to access the Assets Pallet.
		type Fungibles: fungibles::Inspect<Self::AccountId>
			+ fungibles::Mutate<Self::AccountId>
			+ fungibles::Create<Self::AccountId>;
	}

	/// We make the crowdfund a simple state machine to better ensure that all checks are done
	/// when transitioning between phases, and that all calls are gated by checking the right phase.
	#[derive(Encode, Decode, MaxEncodedLen, TypeInfo, PartialEq, Eq, Copy, Clone, Debug)]
	pub enum CrowdfundPhase {
		// The crowdfund is currently raising funds.
		Raising,
		// The crowdfund is currently distributing tokens to users.
		Distributing,
		// The crowdfund has ended successfully.
		Ended,
		// The crowdfund has failed to raise enough funds.
		Failed,
	}

	#[derive(Encode, Decode, MaxEncodedLen, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct DistributionInfo<T: Config> {
		pub creator: T::AccountId,
		pub asset_id: AssetIdOf<T>,
		pub stash_account: T::AccountId,
		pub crowdfunded: bool,
		pub root_hash: Option<T::Hash>,
	}

	#[derive(Encode, Decode, MaxEncodedLen, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct CrowdfundInfo<T: Config> {
		pub phase: CrowdfundPhase,
		pub fund_min: BalanceOf<T>,
		pub fund_max: Option<BalanceOf<T>>,
		pub min_contribution: BalanceOf<T>,
		pub max_contribution: Option<BalanceOf<T>>,
		pub total_contributed: BalanceOf<T>,
		pub total_claimed: BalanceOf<T>,
		pub end_block: RelayChainBlockNumber,
	}

	// `NextDistributionId` keeps track of the next ID available when starting a distribution.
	#[pallet::storage]
	pub type NextDistributionId<T> = StorageValue<_, DistributionId, ValueQuery>;

	// `DistributionInfos` storage maps from a `DistributionId` to the `Info` about that distribution.
	#[pallet::storage]
	pub type DistributionInfos<T: Config> =
		StorageMap<_, Blake2_128Concat, DistributionId, DistributionInfo<T>>;

	// `CrowdfundInfos` storage maps from a `DistributionId` with crowdfunded enabled, to the configuration about the crowdfund.
	#[pallet::storage]
	pub type CrowdfundInfos<T: Config> =
		StorageMap<_, Blake2_128Concat, DistributionId, CrowdfundInfo<T>>;

	// `AssetDistribution` storage maps from a `DistributionId` and `AccountId` to the amount of
	#[pallet::storage]
	pub type CrowdfundContributions<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		DistributionId,
		Blake2_128Concat,
		T::AccountId,
		BalanceOf<T>,
	>;

	// `AssetDistribution` storage maps from a `DistributionId` and `AccountId` to the amount of
	#[pallet::storage]
	pub type AssetDistribution<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		DistributionId,
		Blake2_128Concat,
		T::AccountId,
		AssetBalanceOf<T>,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DistributionCreated {
			id: DistributionId,
			crowdfunded: bool,
		},
		DistributionAdded {
			id: DistributionId,
			recipient: T::AccountId,
			amount: AssetBalanceOf<T>,
		},
		DistributionClaimed {
			id: DistributionId,
			recipient: T::AccountId,
			amount: AssetBalanceOf<T>,
		},
		CrowdfundConfigured {
			id: DistributionId,
			fund_min: BalanceOf<T>,
			fund_max: Option<BalanceOf<T>>,
			min_contribution: BalanceOf<T>,
			max_contribution: Option<BalanceOf<T>>,
		},
		NewCrowdfundPhase {
			id: DistributionId,
			new_phase: CrowdfundPhase,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The provided distribution id does not exist.
		DistributionIdDoesNotExist,
		/// The distribution does not exist.
		DistributionDoesNotExist,
		/// Zero value is not allowed.
		ZeroNotAllowed,
		/// The call is only accessible by the distribution creator.
		CreatorOnly,
		/// The distribution is crowdfunded.
		Crowdfunded,
		/// The distribution is not crowdfunded.
		NotCrowdfunded,
		/// Contribution is larger than the maximum allowed.
		ContributionTooBig,
		/// Contribution is smaller than the minimum allowed.
		ContributionTooSmall,
		/// Contribution limit for the crowdfund has been reached.
		ContributionLimitReached,
		/// Contribution period has ended.
		ContributionPeriodEnded,
		/// Contribution period is still ongoing.
		ContributionPeriodOngoing,
		/// Wrong crowdfund phase.
		WrongPhase,
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
		pub fn create_distribution(
			origin: OriginFor<T>,
			asset_id: AssetIdOf<T>,
			crowdfunded: bool,
		) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let id = NextDistributionId::<T>::get();
			let next_id = id.checked_add(1).ok_or(ArithmeticError::Overflow)?;

			let stash_account = Self::stash_account(id);
			let distribution_info = DistributionInfo::<T> {
				creator,
				asset_id,
				stash_account,
				crowdfunded,
				root_hash: None,
			};

			NextDistributionId::<T>::put(next_id);
			DistributionInfos::<T>::insert(id, distribution_info);

			Self::deposit_event(Event::<T>::DistributionCreated { id, crowdfunded });

			Ok(())
		}

		/// Attempts to transfer an `amount` of the expected asset to the `stash_account`.
		/// This is just a convenience function, and could be done by calling `transfer` with the appropriate pallet to the `stash_account`.
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::default())]
		pub fn fund_distribution(
			origin: OriginFor<T>,
			id: DistributionId,
			amount: AssetBalanceOf<T>,
		) -> DispatchResult {
			let from = ensure_signed(origin)?;
			let DistributionInfo { asset_id, stash_account, .. } =
				DistributionInfos::<T>::get(id).ok_or(Error::<T>::DistributionIdDoesNotExist)?;
			T::Fungibles::transfer(asset_id, &from, &stash_account, amount, Preservation::Protect)?;

			Ok(())
		}

		// TODO: Should be able to add logic which checks distributions are not greater than the amount of tokens available.
		/// This adds a distribution entry for a single account and amount for `id`.
		/// It is possible that this overwrites an existing entry, so the distribution `creator`
		/// should take special care to remove duplicates before calling this function.
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::default())]
		pub fn add_distribution(
			origin: OriginFor<T>,
			id: DistributionId,
			recipient: T::AccountId,
			amount: AssetBalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let DistributionInfo { creator, crowdfunded, .. } =
				DistributionInfos::<T>::get(id).ok_or(Error::<T>::DistributionIdDoesNotExist)?;

			ensure!(!crowdfunded, Error::<T>::Crowdfunded);
			ensure!(who == creator, Error::<T>::CreatorOnly);
			AssetDistribution::<T>::insert(id, recipient.clone(), amount);
			Self::deposit_event(Event::<T>::DistributionAdded { id, recipient, amount });

			Ok(())
		}

		// TODO: Should be able to add logic which checks distributions are not greater than the amount of tokens available.
		/// This adds multiple distribution entries for a accounts and amounts for `id`.
		/// It is possible that this overwrites an existing entries, so the distribution `creator`
		/// should take special care to remove duplicates before calling this function.
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::default())]
		pub fn add_distributions(
			origin: OriginFor<T>,
			id: DistributionId,
			recipients: Vec<(T::AccountId, AssetBalanceOf<T>)>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let DistributionInfo { creator, crowdfunded, .. } =
				DistributionInfos::<T>::get(id).ok_or(Error::<T>::DistributionIdDoesNotExist)?;

			ensure!(!crowdfunded, Error::<T>::Crowdfunded);
			ensure!(who == creator, Error::<T>::CreatorOnly);
			for (recipient, amount) in recipients {
				AssetDistribution::<T>::insert(id, recipient.clone(), amount);
				Self::deposit_event(Event::<T>::DistributionAdded { id, recipient, amount });
			}

			Ok(())
		}

		/// This allows anyone to permissionlessly distribute tokens on behalf of a recipient.
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::default())]
		pub fn claim_distribution(
			origin: OriginFor<T>,
			id: DistributionId,
			recipient: T::AccountId,
		) -> DispatchResult {
			// Anyone can permissionlessly call this function.
			let _ = ensure_signed(origin)?;

			let DistributionInfo { asset_id, stash_account, .. } =
				DistributionInfos::<T>::get(id).ok_or(Error::<T>::DistributionIdDoesNotExist)?;

			let amount = AssetDistribution::<T>::take(id, &recipient)
				.ok_or(Error::<T>::DistributionDoesNotExist)?;
			T::Fungibles::transfer(
				asset_id,
				&stash_account,
				&recipient,
				amount,
				Preservation::Expendable,
			)?;

			Self::deposit_event(Event::<T>::DistributionClaimed { id, recipient, amount });

			Ok(())
		}

		/// This extrinsic creates a crowdfund distribution with settings defined by the `creator`.
		#[pallet::call_index(5)]
		#[pallet::weight(Weight::default())]
		pub fn configure_crowdfund(
			origin: OriginFor<T>,
			id: DistributionId,
			fund_min: BalanceOf<T>,
			fund_max: Option<BalanceOf<T>>,
			min_contribution: BalanceOf<T>,
			max_contribution: Option<BalanceOf<T>>,
			end_block: RelayChainBlockNumber,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(
				max_contribution.map_or(true, |max| max > BalanceOf::<T>::zero()),
				Error::<T>::ZeroNotAllowed
			);
			ensure!(
				fund_max.map_or(true, |max| max > BalanceOf::<T>::zero()),
				Error::<T>::ZeroNotAllowed
			);
			ensure!(min_contribution > BalanceOf::<T>::zero(), Error::<T>::ZeroNotAllowed);
			ensure!(fund_min > BalanceOf::<T>::zero(), Error::<T>::ZeroNotAllowed);
			let DistributionInfo { creator, crowdfunded, .. } =
				DistributionInfos::<T>::get(id).ok_or(Error::<T>::DistributionIdDoesNotExist)?;

			ensure!(crowdfunded, Error::<T>::NotCrowdfunded);
			ensure!(who == creator, Error::<T>::CreatorOnly);

			let crowdfund_info = CrowdfundInfo::<T> {
				phase: CrowdfundPhase::Raising,
				fund_min,
				fund_max,
				min_contribution,
				max_contribution,
				end_block,
				total_contributed: BalanceOf::<T>::zero(),
				total_claimed: BalanceOf::<T>::zero(),
			};

			CrowdfundInfos::<T>::insert(id, crowdfund_info);

			Self::deposit_event(Event::<T>::CrowdfundConfigured {
				id,
				fund_min,
				fund_max,
				min_contribution,
				max_contribution,
			});

			Ok(())
		}

		/// This extrinsic moves a crowdfund to the next phase. This ensures that each phase change is accurately checked,
		/// and subsequent crowdfund extrinsics will execute only at the right phase.
		/// Anyone can permissionlessly call this function.
		#[pallet::call_index(6)]
		#[pallet::weight(Weight::default())]
		pub fn crowdfund_next_phase(origin: OriginFor<T>, id: DistributionId) -> DispatchResult {
			// Anyone can permissionlessly call this function.
			let _ = ensure_signed(origin)?;

			let mut crowdfund_info =
				CrowdfundInfos::<T>::get(id).ok_or(Error::<T>::NotCrowdfunded)?;

			let new_phase = match crowdfund_info.phase {
				// Transition from raising funds to distributing funds or failed, based on the amount contributed before `end_block`.
				CrowdfundPhase::Raising => {
					// TODO: Write this whole block more ergonomically.
					let mut contributions_full = false;

					// First we can check if the crowdfund can end early because it cannot accept anymore contributions.
					if let Some(max_contribution) = crowdfund_info.max_contribution {
						let maybe_total_plus_one = crowdfund_info
							.total_contributed
							.checked_add(&crowdfund_info.min_contribution);
						// Contributions are full if an additional minimum contribution overflows, or is greater than the `max_contribution`.
						contributions_full = maybe_total_plus_one
							.map_or(true, |total_plus_one| total_plus_one > max_contribution);
					}

					// If the contributions are full, we can start distributing early.
					if contributions_full {
						CrowdfundPhase::Distributing
					} else {
						// Then we check if the crowdfund raising period is over.
						let last_relay_block_number =
							cumulus_pallet_parachain_system::Pallet::<T>::last_relay_block_number();
						ensure!(
							last_relay_block_number > crowdfund_info.end_block,
							Error::<T>::ContributionPeriodOngoing
						);

						// If crowdfund didn't raise enough funds, it failed. Otherwise, we can start distributing.
						if crowdfund_info.total_contributed >= crowdfund_info.fund_min {
							CrowdfundPhase::Failed
						} else {
							CrowdfundPhase::Distributing
						}
					}
				},
				_ => crowdfund_info.phase,
			};

			// If needed, update and emit event.
			if crowdfund_info.phase != new_phase {
				crowdfund_info.phase = new_phase;
				CrowdfundInfos::<T>::insert(id, crowdfund_info);
				Self::deposit_event(Event::<T>::NewCrowdfundPhase { id, new_phase });
			}

			Ok(())
		}

		/// This extrinsic allows anyone to permissionlessly contribute to a crowdfund.
		/// This extrinsic is only successful when crowdfund is in `Raising` phase.
		#[pallet::call_index(7)]
		#[pallet::weight(Weight::default())]
		pub fn contribute_crowdfund(
			origin: OriginFor<T>,
			id: DistributionId,
			amount: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut crowdfund_info =
				CrowdfundInfos::<T>::get(id).ok_or(Error::<T>::NotCrowdfunded)?;

			ensure!(crowdfund_info.phase == CrowdfundPhase::Raising, Error::<T>::WrongPhase);

			// Check Contribution Amount
			ensure!(
				crowdfund_info.max_contribution.map_or(true, |max| max >= amount),
				Error::<T>::ContributionTooBig
			);
			ensure!(crowdfund_info.min_contribution <= amount, Error::<T>::ContributionTooSmall);

			let new_total_contributed = crowdfund_info
				.total_contributed
				.checked_add(&amount)
				.ok_or(ArithmeticError::Overflow)?;

			// Check Contribution Limits
			ensure!(
				crowdfund_info.fund_max.map_or(true, |max| max >= new_total_contributed),
				Error::<T>::ContributionLimitReached
			);
			let last_relay_block_number =
				cumulus_pallet_parachain_system::Pallet::<T>::last_relay_block_number();
			ensure!(
				last_relay_block_number <= crowdfund_info.end_block,
				Error::<T>::ContributionPeriodEnded
			);

			let DistributionInfo { stash_account, .. } =
				DistributionInfos::<T>::get(id).ok_or(Error::<T>::DistributionIdDoesNotExist)?;

			// Transfer funds to stash account and record contribution.
			T::NativeBalance::transfer(&who, &stash_account, amount, Preservation::Preserve)?;
			CrowdfundContributions::<T>::insert(&id, &who, amount);

			// Update Crowdfund Total
			crowdfund_info.total_contributed = new_total_contributed;
			CrowdfundInfos::<T>::insert(id, crowdfund_info);

			Ok(())
		}

		/// This extrinsic allows anyone to permissionlessly claim the portion of tokens allocated to a crowdfund contributor.
		#[pallet::call_index(8)]
		#[pallet::weight(Weight::default())]
		pub fn claim_crowdfund_distribution(
			origin: OriginFor<T>,
			id: DistributionId,
			recipient: T::AccountId,
		) -> DispatchResult {
			// Anyone can permissionlessly call this function.
			let _ = ensure_signed(origin)?;

			// Remove contribution record at the same time as reading the info.
			let contribution_amount = CrowdfundContributions::<T>::take(id, &recipient)
				.ok_or(Error::<T>::DistributionDoesNotExist)?;

			let mut crowdfund_info =
				CrowdfundInfos::<T>::get(id).ok_or(Error::<T>::NotCrowdfunded)?;

			// Check that the crowdfund is past the contribution period.
			let last_relay_block_number =
				cumulus_pallet_parachain_system::Pallet::<T>::last_relay_block_number();
			ensure!(
				last_relay_block_number > crowdfund_info.end_block,
				Error::<T>::ContributionPeriodOngoing
			);

			let DistributionInfo { asset_id, stash_account, .. } =
				DistributionInfos::<T>::get(id).ok_or(Error::<T>::DistributionIdDoesNotExist)?;

			// This calculates the amount of contributions which can still claim their distribution.
			let claimable = crowdfund_info
				.total_contributed
				.checked_sub(&crowdfund_info.total_claimed)
				.ok_or(ArithmeticError::Underflow)?;
			ensure!(claimable > BalanceOf::<T>::zero(), Error::<T>::ZeroNotAllowed);
			let new_total_claimed = crowdfund_info
				.total_claimed
				.checked_add(&contribution_amount)
				.ok_or(ArithmeticError::Overflow)?;
			// This will calculate the percentage of tokens the recipient can claim, based on their contribution size, and the outstanding amount to be claimed.
			let claim_percentage = Perbill::from_rational(contribution_amount, claimable);
			// This calculates the total amount of tokens that should be sent. A percentage of the total balance.
			let token_balance = T::Fungibles::total_balance(asset_id.clone(), &stash_account);
			let claimable_tokens = claim_percentage * token_balance;

			// Transfer the funds and update all storage.
			T::Fungibles::transfer(
				asset_id,
				&stash_account,
				&recipient,
				claimable_tokens,
				Preservation::Expendable,
			)?;
			crowdfund_info.total_claimed = new_total_claimed;
			CrowdfundInfos::<T>::insert(id, crowdfund_info);

			Ok(())
		}

		/// This extrinsic allows the crowdfund creator to withdraw their raised funds.
		/// This function only executes successfully after all contributors have been distributed their tokens.
		#[pallet::call_index(9)]
		#[pallet::weight(Weight::default())]
		pub fn claim_crowdfund_raise(origin: OriginFor<T>, id: DistributionId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// The stash `AccountId` used for distributing funds.
		pub fn stash_account(id: DistributionId) -> T::AccountId {
			// only use one byte prefix to support 16 byte account id (used by test)
			// "modl" ++ "dropit/d" ++ "sa" is 14 bytes, and two bytes remaining for distribution index
			DISTRIBUTION_PALLET_ID.into_sub_account_truncating(("sa", id))
		}
	}
}
