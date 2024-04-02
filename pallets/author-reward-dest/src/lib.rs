//! Pallet for setting and getting author reward destination.
//!
//! The author of a block can provide an account using inherents, this account is meant to be used
//! for reward within the block such as tips and part of transaction fees.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;
use weights::WeightInfo;

use primitives_author_reward_dest::{InherentError, InherentType, INHERENT_IDENTIFIER};

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The type `Self::AccountId` but with additional constraint.
		type AccountIdType: IsType<<Self as frame_system::Config>::AccountId> + From<InherentType>;
		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_: BlockNumberFor<T>) {
			// ensure we never go to trie with these values.
			<Author<T>>::take().expect("`set_author_reward_dest` inherent must be present");
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The inherent set author reward dest has already been submitted.
		AlreadySet,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set the author reward destination for the block.
		#[pallet::call_index(0)]
		#[pallet::weight((
			T::WeightInfo::set_author_reward_dest(),
			DispatchClass::Mandatory
		))]
		pub fn set_author_reward_dest(origin: OriginFor<T>, dest: T::AccountId) -> DispatchResult {
			ensure_none(origin)?;
			ensure!(!<Author<T>>::exists(), Error::<T>::AlreadySet);
			<Author<T>>::put(dest);
			Ok(())
		}
	}

	#[pallet::inherent]
	impl<T: Config> ProvideInherent for Pallet<T> {
		type Call = Call<T>;
		type Error = InherentError;
		const INHERENT_IDENTIFIER: InherentIdentifier = INHERENT_IDENTIFIER;

		fn create_inherent(data: &InherentData) -> Option<Self::Call> {
			let inherent_data = data
				.get_data::<InherentType>(&INHERENT_IDENTIFIER)
				.expect("author reward dest inherent data not correctly encoded")
				.expect("author reward dest inherent data must be provided");
			Some(Call::set_author_reward_dest {
				dest: T::AccountIdType::from(inherent_data).into(),
			})
		}

		fn check_inherent(_call: &Self::Call, _data: &InherentData) -> Result<(), Self::Error> {
			Ok(())
		}

		fn is_inherent(call: &Self::Call) -> bool {
			matches!(call, Call::set_author_reward_dest { .. })
		}
	}

	#[pallet::storage]
	/// Author of current block.
	pub(super) type Author<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;
}

impl<T: Config> Pallet<T> {
	/// Fetch the author of the block.
	///
	/// This is safe to invoke after inherent extrinsics and before `on_finalize`
	pub fn author() -> Option<T::AccountId> {
		<Author<T>>::get()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate as pallet_author_reward_dest;
	use frame_support::{
		assert_noop, derive_impl,
		inherent::{InherentData, ProvideInherent},
		traits::{OnFinalize, UnfilteredDispatchable},
	};
	use sp_runtime::{traits::IdentityLookup, AccountId32, BuildStorage};

	type Block = frame_system::mocking::MockBlock<Test>;

	frame_support::construct_runtime!(
		pub enum Test
		{
			System: frame_system,
			AuthorRewardDest: pallet_author_reward_dest,
		}
	);

	#[derive_impl(frame_system::config_preludes::TestDefaultConfig as frame_system::DefaultConfig)]
	impl frame_system::Config for Test {
		type AccountId = AccountId32;
		type Lookup = IdentityLookup<Self::AccountId>;
		type Block = Block;
	}

	impl pallet::Config for Test {
		type AccountIdType = AccountId32;
		type WeightInfo = ();
	}

	pub fn new_test_ext() -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
		t.into()
	}

	#[test]
	fn set_author_test() {
		new_test_ext().execute_with(|| {
			System::reset_events();

			let mut inherent_data = InherentData::new();

			let author = AccountId32::new([1; 32]);
			inherent_data.put_data(INHERENT_IDENTIFIER, &author).unwrap();

			let inherent_extrinsic = AuthorRewardDest::create_inherent(&inherent_data);
			let inherent_extrinsic = inherent_extrinsic.expect("inherent extrinsic is mandatory");
			assert!(AuthorRewardDest::is_inherent(&inherent_extrinsic));

			assert_eq!(Author::<Test>::get(), None);
			assert_eq!(AuthorRewardDest::author(), None);

			inherent_extrinsic
				.clone()
				.dispatch_bypass_filter(frame_system::RawOrigin::None.into())
				.unwrap();

			assert_eq!(Author::<Test>::get(), Some(author.clone()));
			assert_eq!(AuthorRewardDest::author(), Some(author.clone()));

			assert_noop!(
				inherent_extrinsic.dispatch_bypass_filter(frame_system::RawOrigin::None.into()),
				crate::Error::<Test>::AlreadySet,
			);

			assert_eq!(Author::<Test>::get(), Some(author.clone()));
			assert_eq!(AuthorRewardDest::author(), Some(author.clone()));

			AuthorRewardDest::on_finalize(System::block_number());

			assert_eq!(Author::<Test>::get(), None);
			assert_eq!(AuthorRewardDest::author(), None);
		});
	}

	#[test]
	#[should_panic = "`set_author_reward_dest` inherent must be present"]
	fn author_not_set() {
		new_test_ext().execute_with(|| {
			System::reset_events();
			AuthorRewardDest::on_finalize(System::block_number());
		});
	}
}
