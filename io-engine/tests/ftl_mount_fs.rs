#![cfg(feature = "nvme-pci-tests")]
//! TODO: with spdk v24.09 ftl devices support variable sector size emulation.
//! At that point we can enable testing with aio devices, which do not require a
//! metadata section in the LBA format. For now we are hiding this test case
//! behind the `nvme-pci-tests` feature flag.
use once_cell::sync::OnceCell;
use std::convert::TryFrom;
extern crate libnvme_rs;

use io_engine::{
    bdev::nexus::{nexus_create, nexus_lookup_mut},
    core::{MayastorCliArgs, Protocol, UntypedBdevHandle},
};

pub mod common;
use common::compose::MayastorTest;

// See ftl.rs for uri pattern
static FTL_URI_PREFIX: &str = "ftl:///";
static FTL_BDEV: &str = "ftl0";

// BASE_DEV has to be formated with flbas 4KiB
static BASE_DEV: &str = "pcie:///0000:82:00.0";
// CACHE_DEV has to be formated with flbas 4KiB+64
static CACHE_DEV: &str = "pcie:///0000:83:00.0";

/*
static BASE_DISK: &str = "/tmp/basedev.img";
static BASE_DEV: &str = "aio:///tmp/basedev.img%3Fblk_size=4096";
static CACHE_DISK: &str = "/tmp/cachedev.img";
static CACHE_DEV: &str = "aio:///tmp/cachedev.img%3Fblk_size=4096";

// FTL devices require a minimum of 20 GiB capacity
static DISK_SIZE_MB: u64 = 25000;

macro_rules! prepare_storage {
    () => {
        common::delete_file(&[BASE_DISK.into(), CACHE_DISK.into()]);
        common::truncate_file(BASE_DISK, DISK_SIZE_MB * 1024);
        common::truncate_file(CACHE_DISK, DISK_SIZE_MB * 1024);
    };
}
*/

static MAYASTOR: OnceCell<MayastorTest> = OnceCell::new();

fn get_ms() -> &'static MayastorTest<'static> {
    MAYASTOR.get_or_init(|| MayastorTest::new(MayastorCliArgs::default()))
}

async fn create_connected_nvmf_nexus(
    ms: &'static MayastorTest<'static>,
) -> (libnvme_rs::NvmeTarget, String) {
    let uri = ms
        .spawn(async {
            create_nexus().await;
            // Claim the bdev
            let hdl = UntypedBdevHandle::open(&FTL_BDEV, true, true);
            let nexus = nexus_lookup_mut("nexus").unwrap();
            let ret = nexus.share(Protocol::Nvmf, None).await.unwrap();
            drop(hdl);
            ret
        })
        .await;

    // Create and connect NVMF target.
    let target = libnvme_rs::NvmeTarget::try_from(uri)
        .unwrap()
        .with_rand_hostnqn(true);
    target.connect().unwrap();
    let devices = target.block_devices(2).unwrap();

    assert_eq!(devices.len(), 1);
    (target, devices[0].to_string())
}

#[tokio::test]
async fn ftl_mount_fs_multiple() {
    let ms = get_ms();

    //prepare_storage!();
    let (target, nvmf_dev) = create_connected_nvmf_nexus(ms).await;

    for _i in 0..10 {
        common::mount_umount(&nvmf_dev).unwrap();
    }

    target.disconnect().unwrap();
    ms.spawn(async move {
        let mut nexus = nexus_lookup_mut("nexus").unwrap();
        nexus.as_mut().unshare_nexus().await.unwrap();
        nexus.destroy().await.unwrap();
    })
    .await;
}

pub fn csal_fio_run_verify(device: &str) -> Result<String, String> {
    let (exit, stdout, stderr) = run_script::run(
        r#"
        $FIO --name=randrw --rw=randrw --ioengine=libaio --direct=1 --time_based=1 \
        --runtime=10 --bs=64k --verify=crc32 --group_reporting=1  \
        --verify_fatal=1 --verify_async=2 --filename=$1
        "#,
        &vec![device.into()],
        &run_script::ScriptOptions::new(),
    )
    .unwrap();
    if exit == 0 {
        Ok(stdout)
    } else {
        Err(stderr)
    }
}

#[tokio::test]
async fn ftl_mount_fs_fio() {
    let ms = get_ms();

    //prepare_storage!();
    let (target, nvmf_dev) = create_connected_nvmf_nexus(ms).await;

    let _ = csal_fio_run_verify(&nvmf_dev).unwrap();

    target.disconnect().unwrap();
    ms.spawn(async move {
        let mut nexus = nexus_lookup_mut("nexus").unwrap();
        nexus.as_mut().unshare_nexus().await.unwrap();
        nexus.destroy().await.unwrap();
    })
    .await;
}

async fn create_nexus() {
    let bdev_uri: String = format!("{FTL_URI_PREFIX}{FTL_BDEV}?bbdev={BASE_DEV}&cbdev={CACHE_DEV}");
    let ch = vec![bdev_uri];
    nexus_create("nexus", 8 * 1024 * 1024 * 1024, None, &ch)
        .await
        .unwrap();
}
