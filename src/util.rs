use std::marker::PhantomData;

use bevy::{asset::uuid::Uuid, prelude::*};

/// Creates a constant handle from a random `u64`.
/// Use `scripts/random_u64.sh` to generate new ones.
pub(crate) const fn weak_from_u64<A: Asset>(id: u64) -> Handle<A> {
    Handle::Uuid(Uuid::from_u64_pair(0, id), PhantomData)
}
