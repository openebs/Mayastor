pub mod common;
use common::compose::{
    rpc::v0::{
        mayastor::{CreateNexusRequest, DestroyNexusRequest, Nexus},
        GrpcConnect,
        RpcHandle,
    },
    Builder,
};

const NEXUS_COUNT: usize = 10;

/// Create and Destroy multiple Nexuses, one at a time
#[tokio::test]
async fn nexus_create_destroy() {
    common::composer_init();

    let compose = Builder::new()
        .name("cargo-test")
        .network("10.1.0.0/16")
        .unwrap()
        .add_container_dbg("ms1")
        .build()
        .await
        .unwrap();

    let grpc = GrpcConnect::new(&compose);

    let mut hdl = grpc.grpc_handle("ms1").await.unwrap();

    for i in 0 .. NEXUS_COUNT {
        let nexus = hdl
            .mayastor
            .create_nexus(CreateNexusRequest {
                uuid: uuid::Uuid::new_v4().to_string(),
                size: 10 * 1024 * 1024,
                children: vec![format!("malloc:///d{i}?size_mb=10")],
            })
            .await;

        hdl.mayastor
            .destroy_nexus(DestroyNexusRequest {
                uuid: nexus.unwrap().into_inner().uuid.clone(),
            })
            .await
            .unwrap();
    }
}

/// Create multiple Nexuses, and only then destroy them, one at a time
/// Repeat, but destroy them in reverse order
#[tokio::test]
async fn nexus_create_multiple_then_destroy() {
    common::composer_init();

    let compose = Builder::new()
        .name("cargo-test")
        .network("10.1.0.0/16")
        .unwrap()
        .add_container_dbg("ms1")
        .build()
        .await
        .unwrap();

    let grpc = GrpcConnect::new(&compose);

    let mut hdl = grpc.grpc_handle("ms1").await.unwrap();

    let nexuses = create_nexuses(&mut hdl, NEXUS_COUNT).await;
    for (_, nexus) in nexuses.iter().enumerate() {
        hdl.mayastor
            .destroy_nexus(DestroyNexusRequest {
                uuid: nexus.uuid.clone(),
            })
            .await
            .unwrap();
    }

    // now recreate but destroy in the reverse order
    let nexuses = create_nexuses(&mut hdl, NEXUS_COUNT).await;
    for (_, nexus) in nexuses.iter().enumerate().rev() {
        hdl.mayastor
            .destroy_nexus(DestroyNexusRequest {
                uuid: nexus.uuid.clone(),
            })
            .await
            .unwrap();
    }
}

async fn create_nexuses(handle: &mut RpcHandle, count: usize) -> Vec<Nexus> {
    let mut nexuses = vec![];
    for i in 0 .. count {
        let nexus = handle
            .mayastor
            .create_nexus(CreateNexusRequest {
                uuid: uuid::Uuid::new_v4().to_string(),
                size: 10 * 1024 * 1024,
                children: vec![format!("malloc:///d{i}?size_mb=10")],
            })
            .await
            .unwrap();
        nexuses.push(nexus.into_inner());
    }
    nexuses
}
