pub fn public_fn() {}

pub(crate) fn crate_fn() {}

pub(super) fn super_fn() {}

pub(in crate::inner) fn restricted_fn() {}

fn private_fn() {}

pub struct PubStruct {}

struct PrivateStruct {}
