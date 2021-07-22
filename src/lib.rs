//! Contains various protocols used by either overline node, or the host communicating with the
//! node, or both.

#![cfg_attr(any(not(feature = "std"), not(test)), no_std)]

pub mod host;
pub mod overline;

// include defmt::Format implementations
// we don't want them derive()d in the modules unless defmt-impl feature is set
#[cfg(feature = "defmt-impl")]
pub mod defmt;
