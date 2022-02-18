use common::bdev_io;
use mayastor::{
    bdev::nexus::nexus_create,
    core::{BdevHandle, CoreError, MayastorCliArgs},
    lvs::{Lvol, Lvs},
    pool::PoolArgs,
};
use rpc::mayastor::{
    CreatePoolRequest,
    CreateReplicaRequest,
    ShareProtocolReplica,
    ShareReplicaRequest,
};
use tracing::info;

pub mod common;
use common::{compose::Builder, MayastorTest};

static DISKNAME1: &str = "/tmp/disk1.img";
static POOL1_NAME: &str = "pool1";
static POOL2_NAME: &str = "pool2";

static DISKSIZE_KB: u64 = 96 * 1024;

static UUID1: &str = "00000000-76b6-4fcf-864d-1027d4038756";

static NXNAME: &str = "replica_snapshot_test";
static NXNAME_SNAP: &str = "replica_snapshot_test-snap";

#[tokio::test]
#[ignore]
async fn replica_snapshot() {
    // Start with fresh pools
    common::delete_file(&[DISKNAME1.to_string()]);
    common::truncate_file(DISKNAME1, DISKSIZE_KB);

    let test = Builder::new()
        .name("replica_snapshot_test")
        .network("10.1.0.0/16")
        .add_container("ms1")
        .with_clean(true)
        .build()
        .await
        .unwrap();

    let mut hdls = test.grpc_handles().await.unwrap();

    // create a pool on remote node
    hdls[0]
        .mayastor
        .create_pool(CreatePoolRequest {
            name: POOL2_NAME.to_string(),
            disks: vec!["malloc:///disk0?size_mb=96".into()],
        })
        .await
        .unwrap();

    // create replica, shared over nvmf
    hdls[0]
        .mayastor
        .create_replica(CreateReplicaRequest {
            uuid: UUID1.to_string(),
            pool: POOL2_NAME.to_string(),
            size: 64 * 1024 * 1024,
            thin: false,
            share: ShareProtocolReplica::ReplicaNvmf as i32,
        })
        .await
        .unwrap();

    let mayastor = MayastorTest::new(MayastorCliArgs::default());
    let ip0 = hdls[0].endpoint.ip();

    let t = mayastor
        .spawn(async move {
            Lvs::create_or_import(PoolArgs {
                name: POOL1_NAME.to_string(),
                disks: vec![format!("aio://{}", DISKNAME1)],
                uuid: None,
            })
            .await
            .unwrap();
            let pool = Lvs::lookup(POOL1_NAME).unwrap();
            pool.create_lvol(UUID1, 64 * 1024 * 1024, None, true)
                .await
                .unwrap();
            create_nexus(0, &ip0).await;
            bdev_io::write_some(NXNAME, 0, 0xff).await.unwrap();
            // Issue an unimplemented vendor command
            // This checks that the target is correctly rejecting such commands
            // In practice the nexus will not send such commands
            custom_nvme_admin(0xc1).await.expect_err(
                "unexpectedly succeeded invalid nvme admin command",
            );
            bdev_io::read_some(NXNAME, 0, 0xff).await.unwrap();
            let ts = create_snapshot().await.unwrap();
            // Check that IO to the replica still works after creating a
            // snapshot
            info!("testing IO to nexus");
            bdev_io::read_some(NXNAME, 0, 0xff).await.unwrap();
            bdev_io::write_some(NXNAME, 0, 0xff).await.unwrap();
            bdev_io::read_some(NXNAME, 0, 0xff).await.unwrap();
            bdev_io::write_some(NXNAME, 1024, 0xaa).await.unwrap();
            bdev_io::read_some(NXNAME, 1024, 0xaa).await.unwrap();
            ts
        })
        .await;

    // Share the snapshot and create a new nexus
    info!("sharing snapshot {}", t);
    hdls[0]
        .mayastor
        .share_replica(ShareReplicaRequest {
            uuid: format!("{}-snap-{}", UUID1, t),
            share: ShareProtocolReplica::ReplicaNvmf as i32,
        })
        .await
        .unwrap();

    mayastor
        .spawn(async move {
            info!("creating nexus for snapshot");
            create_nexus(t, &ip0).await;
            // FIXME: Re-enable when addressing read-only aspect of snapshots
            //bdev_io::write_some(NXNAME_SNAP, 0, 0xff)
            //    .await
            //    .expect_err("writing to snapshot should fail");
            // Verify that data read from snapshot remains unchanged
            info!("testing IO to nexus for snapshot");
            bdev_io::write_some(NXNAME, 0, 0x55).await.unwrap();
            bdev_io::read_some(NXNAME, 0, 0x55).await.unwrap();
            bdev_io::read_some(NXNAME_SNAP, 0, 0xff).await.unwrap();
            bdev_io::read_some(NXNAME_SNAP, 1024, 0).await.unwrap();
        })
        .await;

    common::delete_file(&[DISKNAME1.to_string()]);
}

async fn create_nexus(t: u64, ip: &std::net::IpAddr) {
    let mut children = vec![
        "loopback:///".to_string() + &UUID1.to_string(),
        format!("nvmf://{}:8420/nqn.2019-05.io.openebs:{}", &ip, UUID1),
    ];
    let mut nexus_name = NXNAME;
    if t > 0 {
        children
            .iter_mut()
            .for_each(|c| *c = Lvol::format_snapshot_name(c, t));
        nexus_name = NXNAME_SNAP;
    }

    nexus_create(nexus_name, 64 * 1024 * 1024, None, &children)
        .await
        .unwrap();
}

async fn create_snapshot() -> Result<u64, CoreError> {
    let h = BdevHandle::open(NXNAME, true, false).unwrap();
    let t = h
        .create_snapshot()
        .await
        .expect("failed to create snapshot");
    Ok(t)
}

async fn custom_nvme_admin(opc: u8) -> Result<(), CoreError> {
    let h = BdevHandle::open(NXNAME, true, false).unwrap();
    h.nvme_admin_custom(opc).await?;
    Ok(())
}
