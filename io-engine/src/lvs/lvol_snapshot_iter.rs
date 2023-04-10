use async_trait::async_trait;
use spdk_rs::libspdk::spdk_blob;

use crate::{
    core::ListSnapshotParams,
    lvs::{lvs_lvol::LvsLvol, Lvol},
};
#[async_trait(?Send)]
pub trait AsyncIterator {
    type Item;
    async fn next(&mut self) -> Option<ListSnapshotParams>;
}

/// Iterator over Lvol Blobstore for Snapshot.
pub struct LvolSnapshotIter {
    inner: *mut spdk_blob,
    inner_lvol: Lvol,
}

impl LvolSnapshotIter {
    pub fn new(lvol: Lvol) -> Self {
        Self {
            inner: lvol.bs_iter_first(),
            inner_lvol: lvol,
        }
    }
}

#[async_trait(?Send)]
/// Iterator implementation for LvolSnapshot.
impl AsyncIterator for LvolSnapshotIter {
    type Item = ListSnapshotParams;
    async fn next(&mut self) -> Option<Self::Item> {
        if self.inner.is_null() {
            None
        } else {
            let current = self.inner;
            match self.inner_lvol.bs_iter_next(current).await {
                Some(next_blob) => self.inner = next_blob,
                None => self.inner = std::ptr::null_mut(),
            }
            Some(self.inner_lvol.set_snapshot_params(current))
        }
    }
}
