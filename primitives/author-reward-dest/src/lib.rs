//! Core types for author reward dest inherents.

#![cfg_attr(not(feature = "std"), no_std)]

use parity_scale_codec::{Decode, Encode};
use sp_inherents::{InherentData, InherentIdentifier, IsFatalError};

/// The identifier for the `set_author_reward_dest` inherent.
pub const INHERENT_IDENTIFIER: InherentIdentifier = *b"authrdst";

/// The type of the inherent.
pub type InherentType = sp_core::crypto::AccountId32;

/// Errors that can occur while checking the `set author reward dest` inherent.
#[derive(Encode, sp_runtime::RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Decode, thiserror::Error))]
pub struct InherentError;

impl IsFatalError for InherentError {
	fn is_fatal_error(&self) -> bool {
		true
	}
}

impl sp_std::fmt::Display for InherentError {
	fn fmt(&self, f: &mut sp_std::fmt::Formatter) -> sp_std::fmt::Result {
		write!(f, "Error for author reward destination inherent")
	}
}

impl InherentError {
	/// Try to create an instance ouf of the given identifier and data.
	#[cfg(feature = "std")]
	pub fn try_from(id: &InherentIdentifier, mut data: &[u8]) -> Option<Self> {
		if id == &INHERENT_IDENTIFIER {
			<InherentError as Decode>::decode(&mut data).ok()
		} else {
			None
		}
	}
}

#[cfg(feature = "std")]
pub struct InherentDataProvider {
	author_reward_dest: InherentType,
}

#[cfg(feature = "std")]
impl InherentDataProvider {
	/// Create `Self` using the given author reward destination address.
	pub fn new(dest: InherentType) -> Self {
		Self { author_reward_dest: dest }
	}
}

#[cfg(feature = "std")]
impl core::ops::Deref for InherentDataProvider {
	type Target = InherentType;

	fn deref(&self) -> &Self::Target {
		&self.author_reward_dest
	}
}

#[cfg(feature = "std")]
#[async_trait::async_trait]
impl sp_inherents::InherentDataProvider for InherentDataProvider {
	async fn provide_inherent_data(
		&self,
		inherent_data: &mut InherentData,
	) -> Result<(), sp_inherents::Error> {
		inherent_data.put_data(INHERENT_IDENTIFIER, &self.author_reward_dest)
	}

	async fn try_handle_error(
		&self,
		identifier: &InherentIdentifier,
		error: &[u8],
	) -> Option<Result<(), sp_inherents::Error>> {
		Some(Err(sp_inherents::Error::Application(Box::from(InherentError::try_from(
			identifier, error,
		)?))))
	}
}
