pub mod common;

#[cfg(feature = "fault_injection")]
#[tokio::test]
async fn nexus_fault_injection() {
    use common::{
        compose::{
            rpc::v1::{
                nexus::{ChildState, ChildStateReason},
                GrpcConnect,
            },
            Binary,
            Builder,
        },
        nexus::{test_write_to_nexus, NexusBuilder},
        pool::PoolBuilder,
        replica::ReplicaBuilder,
    };

    static POOL_SIZE: u64 = 60;
    static REPL_SIZE: u64 = 50;

    common::composer_init();

    let test = Builder::new()
        .name("cargo-test")
        .network("10.1.0.0/16")
        .unwrap()
        .add_container_bin(
            "ms_0",
            Binary::from_dbg("io-engine").with_args(vec!["-l", "1"]),
        )
        .add_container_bin(
            "ms_1",
            Binary::from_dbg("io-engine").with_args(vec!["-l", "2"]),
        )
        .add_container_bin(
            "ms_nex",
            Binary::from_dbg("io-engine").with_args(vec!["-l", "3", "-Fcolor"]),
        )
        .with_clean(true)
        .build()
        .await
        .unwrap();

    let conn = GrpcConnect::new(&test);

    let ms_0 = conn.grpc_handle_shared("ms_0").await.unwrap();
    let ms_1 = conn.grpc_handle_shared("ms_1").await.unwrap();
    let ms_nex = conn.grpc_handle_shared("ms_nex").await.unwrap();

    let mut pool_0 = PoolBuilder::new(ms_0.clone())
        .with_name("pool0")
        .with_new_uuid()
        .with_malloc("mem0", POOL_SIZE);

    let mut repl_0 = ReplicaBuilder::new(ms_0.clone())
        .with_pool(&pool_0)
        .with_name("r0")
        .with_new_uuid()
        .with_size_mb(REPL_SIZE)
        .with_thin(true);

    pool_0.create().await.unwrap();
    repl_0.create().await.unwrap();
    repl_0.share().await.unwrap();

    let mut pool_1 = PoolBuilder::new(ms_1.clone())
        .with_name("pool1")
        .with_new_uuid()
        .with_malloc("mem0", POOL_SIZE);

    let mut repl_1 = ReplicaBuilder::new(ms_1.clone())
        .with_pool(&pool_1)
        .with_name("r1")
        .with_new_uuid()
        .with_size_mb(REPL_SIZE)
        .with_thin(true);

    pool_1.create().await.unwrap();
    repl_1.create().await.unwrap();
    repl_1.share().await.unwrap();

    let mut nex_0 = NexusBuilder::new(ms_nex.clone())
        .with_name("nexus0")
        .with_new_uuid()
        .with_size_mb(REPL_SIZE)
        .with_replica(&repl_0)
        .with_replica(&repl_1);

    nex_0.create().await.unwrap();
    nex_0.publish().await.unwrap();

    //
    let children = nex_0.get_nexus().await.unwrap().children;
    assert_eq!(children.len(), 2);
    let dev_name = children[0].device_name.as_ref().unwrap();

    let inj_uri = format!("inject://{}?op=write&start_cnt=50", dev_name);
    nex_0.inject_nexus_fault(&inj_uri).await.unwrap();

    // List injected fault.
    let lst = nex_0.list_injected_faults().await.unwrap();
    assert_eq!(lst.len(), 1);
    assert_eq!(&lst[0].device_name, dev_name);

    // Write less than pool size.
    test_write_to_nexus(&nex_0, 30, 1).await.unwrap();

    //
    let children = nex_0.get_nexus().await.unwrap().children;
    assert_eq!(children[0].state, ChildState::Faulted as i32);
    assert_eq!(children[0].state, ChildStateReason::CannotOpen as i32);
}
