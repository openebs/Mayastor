//!
//! Main file to register additional subsystems

pub use config::{
    opts::{NexusOpts, NvmeBdevOpts},
    pool::PoolConfig,
    Config,
    ConfigSubsystem,
};
pub use nvmf::{
    create_snapshot,
    set_snapshot_time,
    Error as NvmfError,
    NvmeCpl,
    NvmfReq,
    NvmfSubsystem,
    SubType,
    Target as NvmfTarget,
};
use spdk_rs::libspdk::{
    spdk_add_subsystem,
    spdk_add_subsystem_depend,
    spdk_subsystem_depend,
};

pub use registration::{
    registration_grpc::Registration,
    RegistrationSubsystem,
};

use crate::subsys::nvmf::Nvmf;

mod config;
mod nvmf;
/// Module for registration of the data-plane with control-plane
pub mod registration;

/// Register initial subsystems
pub(crate) fn register_subsystem() {
    unsafe { spdk_add_subsystem(ConfigSubsystem::new().0) }
    unsafe {
        let mut depend = Box::<spdk_subsystem_depend>::default();
        depend.name = b"mayastor_nvmf_tgt\0" as *const u8 as *mut _;
        depend.depends_on = b"bdev\0" as *const u8 as *mut _;
        spdk_add_subsystem(Nvmf::new().0);
        spdk_add_subsystem_depend(Box::into_raw(depend));
    }
    RegistrationSubsystem::register();
}

/// Makes a subsystem serial number from a subsystem UUID or name.
pub fn make_subsystem_serial<T: AsRef<[u8]>>(uuid: T) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(uuid);
    let s = hasher.finalize().to_vec();

    // SPDK requires serial number string to be no more than 20 chars.
    format!("DCS{:.17}", hex::encode_upper(s))
}
