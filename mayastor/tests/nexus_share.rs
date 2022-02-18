use mayastor::{
    bdev::nexus::{nexus_create, nexus_lookup_mut},
    core::{
        mayastor_env_stop,
        Bdev,
        MayastorCliArgs,
        Protocol,
        Reactor,
        Share,
    },
};

pub mod common;
use common::MayastorTest;

#[tokio::test]
async fn nexus_share_test() {
    let args = MayastorCliArgs {
        reactor_mask: "0x3".into(),
        ..Default::default()
    };

    MayastorTest::new(args)
        .spawn(async {
            // create a nexus and share it via iSCSI
            Reactor::block_on(async {
                nexus_create(
                    "nexus0",
                    48 * 1024 * 1024,
                    None,
                    &[
                        "malloc:///malloc0?size_mb=64".into(),
                        "malloc:///malloc1?size_mb=64".into(),
                    ],
                )
                .await
                .unwrap();

                let nexus = nexus_lookup_mut("nexus0").unwrap();

                // this should be idempotent so validate that sharing the
                // same thing over the same protocol
                // works
                let share = nexus.share_iscsi().await.unwrap();
                let share2 = nexus.share_iscsi().await.unwrap();
                assert_eq!(share, share2);
                assert_eq!(nexus.shared(), Some(Protocol::Iscsi));
            });

            // sharing the nexus over nvmf should fail
            Reactor::block_on(async {
                let nexus = nexus_lookup_mut("nexus0").unwrap();
                assert!(nexus.share_nvmf(None).await.is_err());
                assert_eq!(nexus.shared(), Some(Protocol::Iscsi));
            });

            // unshare the nexus and then share over nvmf
            Reactor::block_on(async {
                let nexus = nexus_lookup_mut("nexus0").unwrap();
                nexus.unshare().await.unwrap();
                let shared = nexus.shared();
                assert_eq!(shared, Some(Protocol::Off));

                let shared = nexus.share_nvmf(None).await.unwrap();
                let shared2 = nexus.share_nvmf(None).await.unwrap();

                assert_eq!(shared, shared2);
                assert_eq!(nexus.shared(), Some(Protocol::Nvmf));
            });

            // sharing the bdev directly, over iSCSI or nvmf should result
            // in an error
            Reactor::block_on(async {
                let bdev = Bdev::lookup_by_name("nexus0").unwrap();
                assert!(bdev.share_iscsi().await.is_err());
                assert!(bdev.share_nvmf(None).await.is_err());
            });

            // unshare the nexus
            Reactor::block_on(async {
                let nexus = nexus_lookup_mut("nexus0").unwrap();
                nexus.unshare().await.unwrap();
            });

            Reactor::block_on(async {
                let nexus = nexus_lookup_mut("nexus0").unwrap();
                assert_eq!(nexus.shared(), Some(Protocol::Off));
                let bdev = Bdev::lookup_by_name("nexus0").unwrap();
                assert_eq!(bdev.shared(), Some(Protocol::Off));
                nexus.destroy().await.unwrap();
            });

            mayastor_env_stop(0);
        })
        .await;
}
