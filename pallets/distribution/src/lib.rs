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
		sp_runtime::traits::AccountIdConversion,
		sp_runtime::ArithmeticError,
		traits::{fungible, fungibles},
	};
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
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

	#[derive(Encode, Decode, MaxEncodedLen, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Info<T: Config> {
		pub creator: T::AccountId,
		pub asset_id: AssetIdOf<T>,
		pub stash_account: T::AccountId,
		pub root_hash: Option<T::Hash>,
	}

	// `NextDistributionId` keeps track of the next ID available when starting a distribution.
	#[pallet::storage]
	pub type NextDistributionId<T> = StorageValue<_, DistributionId, ValueQuery>;

	// `DistributionInfo` storage maps from a `DistributionId` to the `Info` about that distribution.
	#[pallet::storage]
	pub type DistributionInfo<T: Config> = StorageMap<_, Blake2_128Concat, DistributionId, Info<T>>;

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
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {}

	// Dispatchable functions allows users to interact with the pallet and invoke state changes.
	// These functions materialize as "extrinsics", which are often compared to transactions.
	// Dispatchable functions must be annotated with a weight and must return a DispatchResult.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new distribution of `asset_id`.
		/// This extrinsic establishes the `DistributionId` for this distribution and the respective `stash_account`.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::default())]
		pub fn create_distribution(origin: OriginFor<T>, asset_id: AssetIdOf<T>) -> DispatchResult {
			let creator = ensure_signed(origin)?;
			let id = NextDistributionId::<T>::get();
			let next_id = id.checked_add(1).ok_or(ArithmeticError::Overflow)?;

			let stash_account = Self::stash_account(id);
			let info = Info::<T> { creator, asset_id, stash_account, root_hash: None };

			NextDistributionId::<T>::put(next_id);
			DistributionInfo::<T>::insert(id, info);

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// The account ID of a bounty account
		pub fn stash_account(id: DistributionId) -> T::AccountId {
			// only use one byte prefix to support 16 byte account id (used by test)
			// "modl" ++ "dropit/d" ++ "sa" is 14 bytes, and two bytes remaining for distribution index
			DISTRIBUTION_PALLET_ID.into_sub_account_truncating(("sa", id))
		}
	}
}
