#[macro_use]
extern crate tracing;

use std::{env, path::Path};

use futures::future::FutureExt;
use structopt::StructOpt;

use io_engine::{
    bdev::util::uring,
    core::{
        device_monitor_loop,
        diagnostics::process_diagnostics_cli,
        lock::{
            ProtectedSubsystems,
            ResourceLockManager,
            ResourceLockManagerConfig,
        },
        reactor_monitor_loop,
        runtime,
        MayastorCliArgs,
        MayastorEnvironment,
        Mthread,
        Reactors,
    },
    grpc,
    logger,
    persistent_store::PersistentStore,
    subsys::Registration,
};
use version_info::fmt_package_info;

const PAGES_NEEDED: u32 = 1024;

io_engine::CPS_INIT!();
fn start_tokio_runtime(args: &MayastorCliArgs) {
    let grpc_address = grpc::endpoint(args.grpc_endpoint.clone());
    let registration_addr = args.registration_endpoint.clone();
    let rpc_address = args.rpc_address.clone();
    let api_versions = args.api_versions.clone();
    let node_name = grpc::node_name(&args.node_name);
    let node_nqn = args.make_hostnqn();

    let persistent_store_endpoint = args.persistent_store_endpoint.clone();

    let reactor_freeze_detection = args.reactor_freeze_detection;
    let reactor_freeze_timeout = args.reactor_freeze_timeout;

    // Initialize Lock manager.
    let cfg = ResourceLockManagerConfig::default()
        .with_subsystem(ProtectedSubsystems::NEXUS, 512);
    ResourceLockManager::initialize(cfg);

    Mthread::spawn_unaffinitized(move || {
        runtime::block_on(async move {
            let mut futures = Vec::new();
            PersistentStore::init(persistent_store_endpoint).await;
            runtime::spawn(device_monitor_loop());

            // Launch reactor health monitor if diagnostics is enabled.
            if reactor_freeze_detection {
                runtime::spawn(reactor_monitor_loop(reactor_freeze_timeout));
            }

            futures.push(
                grpc::MayastorGrpcServer::run(
                    &node_name,
                    &node_nqn,
                    grpc_address,
                    rpc_address,
                    api_versions.clone(),
                )
                .boxed(),
            );

            if let Some(registration_addr) = registration_addr {
                Registration::init(
                    &node_name,
                    &node_nqn,
                    &grpc_address.to_string(),
                    registration_addr,
                    api_versions,
                );
                futures.push(Registration::run().boxed());
            }

            futures::future::try_join_all(futures)
                .await
                .expect("runtime exited in the normal state");
        });
    });
}

fn hugepage_get_nr(hugepage_path: &Path) -> (u32, u32) {
    let nr_pages = match sysfs::parse_value(hugepage_path, "nr_hugepages") {
        Ok(nr_pages) => nr_pages,
        Err(error) => {
            warn!(
                %error,
                "Failed to read the number of pages at {}",
                hugepage_path.display()
            );
            // NOTE: We check for 1g pages but the directory for 1g pages won't
            // exist if they are not enabled. Effectively this means
            // that zero 1g hugepages are available, so it
            // seems sensible to fall back to 0 if reads fail. See discussion
            // here: https://github.com/openebs/mayastor/issues/1273.
            0
        }
    };

    let free_pages = match sysfs::parse_value(hugepage_path, "free_hugepages") {
        Ok(free_pages) => free_pages,
        Err(error) => {
            warn!(
                %error,
                "Failed to read the number of free pages at {}",
                hugepage_path.display()
            );
            0
        }
    };

    (nr_pages, free_pages)
}

fn hugepage_check() {
    let (nr_pages, free_pages) =
        hugepage_get_nr(Path::new("/sys/kernel/mm/hugepages/hugepages-2048kB"));
    let (nr_1g_pages, free_1g_pages) = hugepage_get_nr(Path::new(
        "/sys/kernel/mm/hugepages/hugepages-1048576kB",
    ));

    if nr_pages + nr_1g_pages * 512 < PAGES_NEEDED {
        error!(
            ?PAGES_NEEDED,
            ?nr_pages,
            ?nr_1g_pages,
            "insufficient pages available"
        );
        if !cfg!(debug_assertions) {
            std::process::exit(1)
        }
    }

    if free_pages + free_1g_pages * 512 < PAGES_NEEDED {
        error!(
            ?PAGES_NEEDED,
            ?free_pages,
            ?free_1g_pages,
            "insufficient free pages available"
        );
        if !cfg!(debug_assertions) {
            std::process::exit(1)
        }
    }

    info!("free_pages 2MB: {} nr_pages 2MB: {}", free_pages, nr_pages);
    info!(
        "free_pages 1GB: {} nr_pages 1GB: {}",
        free_1g_pages, nr_1g_pages
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = MayastorCliArgs::from_args();

    let log_format = args.log_format.unwrap_or_default();

    // setup our logger first if -L is passed, raise the log level
    // automatically. trace maps to debug at FFI level. If RUST_LOG is
    // passed, we will use it regardless.
    if !args.log_components.is_empty() {
        logger::init_ex("TRACE", log_format);
    } else {
        logger::init_ex("INFO", log_format);
    }

    spdk_rs::some_test_foo("abc");

    info!("{}", fmt_package_info!());

    // Handle diagnostics-related commands before initializing the agent.
    // Once diagnostics command is executed (regardless of status), exit the
    // agent.
    if let Some(res) = process_diagnostics_cli(&args) {
        return res;
    }

    hugepage_check();

    let nvme_core_path = Path::new("/sys/module/nvme_core/parameters");
    let nvme_mp: String =
        match sysfs::parse_value::<String>(nvme_core_path, "multipath") {
            Ok(s) => match s.as_str() {
                "Y" => "yes".to_string(),
                "N" => "disabled".to_string(),
                u => format!("unknown value {}", u),
            },
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    if nvme_core_path.exists() {
                        "not built".to_string()
                    } else {
                        "nvme not loaded".to_string()
                    }
                } else {
                    format!("unknown error: {}", e)
                }
            }
        };

    info!(
        "kernel io_uring support: {}",
        if uring::kernel_support() { "yes" } else { "no" }
    );
    info!("kernel nvme initiator multipath support: {}", nvme_mp);

    let ms = MayastorEnvironment::new(args.clone()).init();
    start_tokio_runtime(&args);

    Reactors::current().running();
    Reactors::current().poll_reactor();

    ms.fini();
    Ok(())
}
