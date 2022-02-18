use std::ptr::NonNull;

use spdk_rs::libspdk::{
    spdk_nvme_ns,
    spdk_nvme_ns_get_extended_sector_size,
    spdk_nvme_ns_get_flags,
    spdk_nvme_ns_get_md_size,
    spdk_nvme_ns_get_num_sectors,
    spdk_nvme_ns_get_optimal_io_boundary,
    spdk_nvme_ns_get_size,
    spdk_nvme_ns_get_uuid,
    spdk_nvme_ns_supports_compare,
    SPDK_NVME_NS_DEALLOCATE_SUPPORTED,
    SPDK_NVME_NS_WRITE_ZEROES_SUPPORTED,
};

#[derive(Debug)]
pub struct NvmeNamespace(NonNull<spdk_nvme_ns>);

// TODO: is `NvmeNamespace` really a Sync/Send type?
unsafe impl Sync for NvmeNamespace {}
unsafe impl Send for NvmeNamespace {}

impl NvmeNamespace {
    pub fn size_in_bytes(&self) -> u64 {
        unsafe { spdk_nvme_ns_get_size(self.0.as_ptr()) }
    }

    pub fn block_len(&self) -> u64 {
        unsafe { spdk_nvme_ns_get_extended_sector_size(self.0.as_ptr()) as u64 }
    }

    pub fn num_blocks(&self) -> u64 {
        unsafe { spdk_nvme_ns_get_num_sectors(self.0.as_ptr()) }
    }

    pub fn uuid(&self) -> uuid::Uuid {
        spdk_rs::Uuid::legacy_from_ptr(unsafe {
            spdk_nvme_ns_get_uuid(self.0.as_ptr())
        })
        .into()
    }

    pub fn supports_compare(&self) -> bool {
        unsafe { spdk_nvme_ns_supports_compare(self.0.as_ptr()) }
    }

    pub fn supports_deallocate(&self) -> bool {
        unsafe {
            spdk_nvme_ns_get_flags(self.0.as_ptr())
                & SPDK_NVME_NS_DEALLOCATE_SUPPORTED
                > 0
        }
    }

    pub fn supports_write_zeroes(&self) -> bool {
        unsafe {
            spdk_nvme_ns_get_flags(self.0.as_ptr())
                & SPDK_NVME_NS_WRITE_ZEROES_SUPPORTED
                > 0
        }
    }

    pub fn alignment(&self) -> u64 {
        unsafe { spdk_nvme_ns_get_optimal_io_boundary(self.0.as_ptr()) as u64 }
    }

    pub fn md_size(&self) -> u64 {
        unsafe { spdk_nvme_ns_get_md_size(self.0.as_ptr()) as u64 }
    }

    pub fn from_ptr(ns: *mut spdk_nvme_ns) -> NvmeNamespace {
        NonNull::new(ns)
            .map(NvmeNamespace)
            .expect("nullptr dereference while constructing NVMe namespace")
    }

    pub fn as_ptr(&self) -> *mut spdk_nvme_ns {
        self.0.as_ptr()
    }
}
