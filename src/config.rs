//! User configuration: the on-disk form ([`disk`]) and the resolved form
//! ([`resolved`]) built from it once at startup via [`resolved::load`].

pub mod disk;
pub mod resolved;
