//! Contains various protocols used by either overline node, or the host communicating with the
//! node, or both.

#![cfg_attr(not(test), no_std)]
pub mod host;
pub mod overline;
pub mod p2p;
