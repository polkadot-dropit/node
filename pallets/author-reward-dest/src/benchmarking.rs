#![cfg(feature = "runtime-benchmarks")]

use super::*;
#[allow(unused)]
use crate::Pallet as AuthorRewardDest;
use frame_benchmarking::v2::*;
use frame_system::RawOrigin;

const SEED: u32 = 0;

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn set_author_reward_dest() {
		let dest: T::AccountId = account("recipient", 0, SEED);

		#[extrinsic_call]
		_(RawOrigin::None, dest);

		assert!(AuthorRewardDest::<T>::author().is_some());
	}

	impl_benchmark_test_suite!(AuthorRewardDest, crate::tests::new_test_ext(), crate::tests::Test);
}
