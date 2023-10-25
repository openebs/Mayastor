//! The structure copying is needed because we cannot impl things for foreign
//! types. Naturally this is a good reason, but it means we have to copy things
//! around. If the structures change, we will know about it because we use the
//! from trait, and we are not allowed to skip or use different types.

use serde::{Deserialize, Serialize};

use spdk_rs::{
    ffihelper::copy_str_with_null,
    libspdk::{
        bdev_nvme_get_opts,
        bdev_nvme_set_opts,
        spdk_bdev_get_opts,
        spdk_bdev_nvme_opts,
        spdk_bdev_opts,
        spdk_bdev_set_opts,
        spdk_iobuf_get_opts,
        spdk_iobuf_opts,
        spdk_iobuf_set_opts,
        spdk_nvmf_target_opts,
        spdk_nvmf_transport_opts,
        spdk_sock_impl_get_opts,
        spdk_sock_impl_opts,
        spdk_sock_impl_set_opts,
    },
};

use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use crate::core::MayastorEnvironment;

pub trait GetOpts {
    fn get(&self) -> Self;
    fn set(&self) -> bool {
        true
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NexusOpts {
    /// enable nvmf target
    pub nvmf_enable: bool,
    /// enable the nvmf discovery subsystem
    pub nvmf_discovery_enable: bool,
    /// nvmf port over which we export
    pub nvmf_nexus_port: u16,
    /// NOTE: we do not (yet) differentiate between
    /// the nexus and replica nvmf target
    pub nvmf_replica_port: u16,
}

/// Default nvmf port used for replicas.
/// It's different from the standard nvmf port 4420 because we don't want
/// to conflict with nexus exported over nvmf running on the same node.
const NVMF_PORT_REPLICA: u16 = 8420;
const NVMF_PORT_NEXUS: u16 = 4421;

impl Default for NexusOpts {
    fn default() -> Self {
        Self {
            nvmf_enable: true,
            nvmf_discovery_enable: true,
            nvmf_nexus_port: NVMF_PORT_NEXUS,
            nvmf_replica_port: NVMF_PORT_REPLICA,
        }
    }
}

impl GetOpts for NexusOpts {
    fn get(&self) -> Self {
        self.clone()
    }
}

/// Length of target Command Retry Delay configuration array.
/// Must be equal to the size of `spdk_nvmf_target_opts.crdt`.
pub const TARGET_CRDT_LEN: usize = 3;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NvmfTgtConfig {
    /// name of the target to be created
    pub name: String,
    /// the max number of namespaces this target should allow for
    pub max_namespaces: u32,
    /// NVMF target Command Retry Delay in x100 ms.
    pub crdt: [u16; TARGET_CRDT_LEN],
    /// TCP transport options
    pub opts: NvmfTcpTransportOpts,
}

impl From<NvmfTgtConfig> for Box<spdk_nvmf_target_opts> {
    fn from(o: NvmfTgtConfig) -> Self {
        let mut out = Self::default();
        copy_str_with_null(&o.name, &mut out.name);
        out.max_subsystems = o.max_namespaces;
        out.crdt = o.crdt;
        out
    }
}

impl Default for NvmfTgtConfig {
    fn default() -> Self {
        let args = MayastorEnvironment::global_or_default();
        Self {
            name: "mayastor_target".to_string(),
            max_namespaces: 2048,
            crdt: args.nvmf_tgt_crdt,
            opts: NvmfTcpTransportOpts::default(),
        }
    }
}

impl GetOpts for NvmfTgtConfig {
    fn get(&self) -> Self {
        self.clone()
    }
}

/// Settings for the TCP transport
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NvmfTcpTransportOpts {
    /// max queue depth
    max_queue_depth: u16,
    /// max qpairs per controller
    max_qpairs_per_ctrl: u16,
    /// encapsulated data size
    in_capsule_data_size: u32,
    /// max IO size
    max_io_size: u32,
    /// IO unit size
    io_unit_size: u32,
    /// max admin queue depth per admin queue
    max_aq_depth: u32,
    /// num of shared buffers
    num_shared_buf: u32,
    /// cache size
    buf_cache_size: u32,
    /// dif
    dif_insert_or_strip: bool,
    /// abort execution timeout
    abort_timeout_sec: u32,
    /// acceptor poll rate, microseconds
    acceptor_poll_rate: u32,
    /// Use zero-copy operations if the underlying bdev supports them
    zcopy: bool,
}

/// try to read an env variable or returns the default when not found
fn try_from_env<T>(name: &str, default: T) -> T
where
    T: FromStr + Display + Copy,
    <T as FromStr>::Err: Debug + Display,
{
    std::env::var(name).map_or_else(
        |_| default,
        |v| {
            match v.parse::<T>() {
               Ok(val) => {
                   info!("Overriding {} value to '{}'", name, val);
                   val
               },
               Err(e) => {
                   error!("Invalid value: {} (error {}) specified for {}. Reverting to default ({})", v, e, name, default);
                   default
               }
            }
        },
    )
}

impl Default for NvmfTcpTransportOpts {
    fn default() -> Self {
        Self {
            max_queue_depth: try_from_env("NVMF_TCP_MAX_QUEUE_DEPTH", 32),
            in_capsule_data_size: 4096,
            max_io_size: 131_072,
            io_unit_size: 131_072,
            max_qpairs_per_ctrl: try_from_env(
                "NVMF_TCP_MAX_QPAIRS_PER_CTRL",
                32,
            ),
            num_shared_buf: try_from_env("NVMF_TCP_NUM_SHARED_BUF", 2048),
            buf_cache_size: try_from_env("NVMF_TCP_BUF_CACHE_SIZE", 64),
            dif_insert_or_strip: false,
            max_aq_depth: 32,
            abort_timeout_sec: 1,
            acceptor_poll_rate: try_from_env("NVMF_ACCEPTOR_POLL_RATE", 10_000),
            zcopy: try_from_env("NVMF_ZCOPY", 1) == 1,
        }
    }
}

/// we cannot add derives for YAML to these structs directly, so we need to
/// copy them. The upside though, is that if the FFI structures change, we will
/// know about it during compile time.
impl From<NvmfTcpTransportOpts> for spdk_nvmf_transport_opts {
    fn from(o: NvmfTcpTransportOpts) -> Self {
        Self {
            max_queue_depth: o.max_queue_depth,
            max_qpairs_per_ctrlr: o.max_qpairs_per_ctrl,
            in_capsule_data_size: o.in_capsule_data_size,
            max_io_size: o.max_io_size,
            io_unit_size: o.io_unit_size,
            max_aq_depth: o.max_aq_depth,
            num_shared_buffers: o.num_shared_buf,
            buf_cache_size: o.buf_cache_size,
            dif_insert_or_strip: o.dif_insert_or_strip,
            reserved29: Default::default(),
            abort_timeout_sec: o.abort_timeout_sec,
            association_timeout: 120000,
            transport_specific: std::ptr::null(),
            opts_size: std::mem::size_of::<spdk_nvmf_transport_opts>() as u64,
            acceptor_poll_rate: o.acceptor_poll_rate,
            zcopy: o.zcopy,
            reserved61: Default::default(),
        }
    }
}

/// generic settings for the NVMe bdev (all our replicas)
#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NvmeBdevOpts {
    /// action take on timeout
    pub action_on_timeout: u32,
    /// timeout for IO commands
    pub timeout_us: u64,
    /// timeout for admin commands
    pub timeout_admin_us: u64,
    /// keep-alive timeout
    pub keep_alive_timeout_ms: u32,
    /// transport retry count
    pub transport_retry_count: u32,
    /// TODO
    pub arbitration_burst: u32,
    /// max number of low priority cmds a controller may launch at one time
    pub low_priority_weight: u32,
    /// max number of medium priority cmds a controller may launch at one time
    pub medium_priority_weight: u32,
    /// max number of high priority cmds a controller may launch at one time
    pub high_priority_weight: u32,
    /// admin queue polling period
    pub nvme_adminq_poll_period_us: u64,
    /// ioq polling period
    pub nvme_ioq_poll_period_us: u64,
    /// number of requests per nvme IO queue
    pub io_queue_requests: u32,
    /// allow for batching of commands
    pub delay_cmd_submit: bool,
    /// attempts per I/O in bdev layer before I/O fails
    pub bdev_retry_count: i32,
    /// TODO
    pub transport_ack_timeout: u8,
    /// TODO
    pub ctrlr_loss_timeout_sec: i32,
    /// TODO
    pub reconnect_delay_sec: u32,
    /// TODO
    pub fast_io_fail_timeout_sec: u32,
    /// TODO
    pub disable_auto_failback: bool,
    /// enable creation of submission and completion queues asynchronously.
    pub async_mode: bool,
}

impl GetOpts for NvmeBdevOpts {
    fn get(&self) -> Self {
        let opts = spdk_bdev_nvme_opts::default();
        unsafe {
            bdev_nvme_get_opts(&opts as *const _ as *mut spdk_bdev_nvme_opts)
        };
        opts.into()
    }

    fn set(&self) -> bool {
        let opts = spdk_bdev_nvme_opts::from(self);
        debug!("{opts:?}");
        (unsafe { bdev_nvme_set_opts(&opts as *const _) } == 0)
    }
}

impl Default for NvmeBdevOpts {
    fn default() -> Self {
        Self {
            action_on_timeout: 4,
            timeout_us: try_from_env("NVME_TIMEOUT_US", 5_000_000),
            timeout_admin_us: try_from_env("NVME_TIMEOUT_ADMIN_US", 5_000_000),
            keep_alive_timeout_ms: try_from_env("NVME_KATO_MS", 1_000),
            transport_retry_count: try_from_env("NVME_RETRY_COUNT", 0),
            arbitration_burst: 0,
            low_priority_weight: 0,
            medium_priority_weight: 0,
            high_priority_weight: 0,
            nvme_adminq_poll_period_us: try_from_env(
                "NVME_ADMINQ_POLL_PERIOD_US",
                1_000,
            ),
            nvme_ioq_poll_period_us: try_from_env("NVME_IOQ_POLL_PERIOD_US", 0),
            io_queue_requests: 0,
            delay_cmd_submit: true,
            bdev_retry_count: try_from_env("NVME_BDEV_RETRY_COUNT", 0),
            transport_ack_timeout: 0,
            ctrlr_loss_timeout_sec: 0,
            reconnect_delay_sec: 0,
            fast_io_fail_timeout_sec: 0,
            disable_auto_failback: false,
            async_mode: try_from_env("NVME_QPAIR_CONNECT_ASYNC", false),
        }
    }
}

impl From<spdk_bdev_nvme_opts> for NvmeBdevOpts {
    fn from(o: spdk_bdev_nvme_opts) -> Self {
        Self {
            action_on_timeout: o.action_on_timeout,
            timeout_us: o.timeout_us,
            timeout_admin_us: o.timeout_admin_us,
            keep_alive_timeout_ms: o.keep_alive_timeout_ms,
            transport_retry_count: o.transport_retry_count,
            arbitration_burst: o.arbitration_burst,
            low_priority_weight: o.low_priority_weight,
            medium_priority_weight: o.medium_priority_weight,
            high_priority_weight: o.high_priority_weight,
            nvme_adminq_poll_period_us: o.nvme_adminq_poll_period_us,
            nvme_ioq_poll_period_us: o.nvme_ioq_poll_period_us,
            io_queue_requests: o.io_queue_requests,
            delay_cmd_submit: o.delay_cmd_submit,
            bdev_retry_count: o.bdev_retry_count,
            transport_ack_timeout: o.transport_ack_timeout,
            ctrlr_loss_timeout_sec: o.ctrlr_loss_timeout_sec,
            reconnect_delay_sec: o.reconnect_delay_sec,
            fast_io_fail_timeout_sec: o.fast_io_fail_timeout_sec,
            disable_auto_failback: o.disable_auto_failback,
            async_mode: NvmeBdevOpts::default().async_mode,
        }
    }
}

impl From<&NvmeBdevOpts> for spdk_bdev_nvme_opts {
    fn from(o: &NvmeBdevOpts) -> Self {
        Self {
            action_on_timeout: o.action_on_timeout,
            timeout_us: o.timeout_us,
            timeout_admin_us: o.timeout_admin_us,
            keep_alive_timeout_ms: o.keep_alive_timeout_ms,
            transport_retry_count: o.transport_retry_count,
            arbitration_burst: o.arbitration_burst,
            low_priority_weight: o.low_priority_weight,
            medium_priority_weight: o.medium_priority_weight,
            high_priority_weight: o.high_priority_weight,
            nvme_adminq_poll_period_us: o.nvme_adminq_poll_period_us,
            nvme_ioq_poll_period_us: o.nvme_ioq_poll_period_us,
            io_queue_requests: o.io_queue_requests,
            delay_cmd_submit: o.delay_cmd_submit,
            bdev_retry_count: o.bdev_retry_count,
            transport_ack_timeout: o.transport_ack_timeout,
            ctrlr_loss_timeout_sec: o.ctrlr_loss_timeout_sec,
            reconnect_delay_sec: o.reconnect_delay_sec,
            fast_io_fail_timeout_sec: o.fast_io_fail_timeout_sec,
            disable_auto_failback: o.disable_auto_failback,
            generate_uuids: false,
            transport_tos: 0,
            nvme_error_stat: false,
            rdma_srq_size: 0,
            io_path_stat: false,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BdevOpts {
    /// number of bdev IO structures in the shared mempool
    bdev_io_pool_size: u32,
    /// number of bdev IO structures cached per thread
    bdev_io_cache_size: u32,
}

impl GetOpts for BdevOpts {
    fn get(&self) -> Self {
        let mut opts = spdk_bdev_opts::default();
        unsafe {
            spdk_bdev_get_opts(
                &mut opts,
                std::mem::size_of::<spdk_bdev_opts>() as u64,
            )
        };
        opts.into()
    }

    fn set(&self) -> bool {
        let mut opts = spdk_bdev_opts::from(self);
        (unsafe { spdk_bdev_set_opts(&mut opts as *mut _) } == 0)
    }
}

impl Default for BdevOpts {
    fn default() -> Self {
        Self {
            bdev_io_pool_size: try_from_env("BDEV_IO_POOL_SIZE", 65535),
            bdev_io_cache_size: try_from_env("BDEV_IO_CACHE_SIZE", 512),
        }
    }
}

impl From<spdk_bdev_opts> for BdevOpts {
    fn from(o: spdk_bdev_opts) -> Self {
        Self {
            bdev_io_pool_size: o.bdev_io_pool_size,
            bdev_io_cache_size: o.bdev_io_cache_size,
        }
    }
}

impl From<&BdevOpts> for spdk_bdev_opts {
    fn from(o: &BdevOpts) -> Self {
        Self {
            bdev_io_pool_size: o.bdev_io_pool_size,
            bdev_io_cache_size: o.bdev_io_cache_size,
            bdev_auto_examine: false,
            reserved9: Default::default(),
            opts_size: std::mem::size_of::<spdk_bdev_opts>() as u64,
            reserved: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PosixSocketOpts {
    recv_buf_size: u32,
    send_buf_size: u32,
    enable_recv_pipe: bool,
    /// deprecated, use use enable_zerocopy_send_server or
    /// enable_zerocopy_send_client instead
    enable_zero_copy_send: bool,
    enable_quickack: bool,
    enable_placement_id: u32,
    enable_zerocopy_send_server: bool,
    enable_zerocopy_send_client: bool,
    zerocopy_threshold: u32,
}

impl Default for PosixSocketOpts {
    fn default() -> Self {
        Self {
            recv_buf_size: try_from_env("SOCK_RECV_BUF_SIZE", 2097152),
            send_buf_size: try_from_env("SOCK_SEND_BUF_SIZE", 2097152),
            enable_recv_pipe: try_from_env("SOCK_ENABLE_RECV_PIPE", true),
            enable_zero_copy_send: true,
            enable_quickack: try_from_env("SOCK_ENABLE_QUICKACK", true),
            enable_placement_id: try_from_env("SOCK_ENABLE_PLACEMENT_ID", 0),
            enable_zerocopy_send_server: try_from_env(
                "SOCK_ZEROCOPY_SEND_SERVER",
                true,
            ),
            enable_zerocopy_send_client: try_from_env(
                "SOCK_ZEROCOPY_SEND_CLIENT",
                false,
            ),
            zerocopy_threshold: 0,
        }
    }
}

impl GetOpts for PosixSocketOpts {
    fn get(&self) -> Self {
        let opts = spdk_sock_impl_opts::default();

        unsafe {
            let name = std::ffi::CString::new("posix").unwrap();
            let mut size = std::mem::size_of::<spdk_sock_impl_opts>() as u64;
            let rc = spdk_sock_impl_get_opts(
                name.as_ptr(),
                &opts as *const _ as *mut spdk_sock_impl_opts,
                &mut size,
            );
            assert_eq!(rc, 0);
        };

        Self {
            recv_buf_size: opts.recv_buf_size,
            send_buf_size: opts.send_buf_size,
            enable_recv_pipe: opts.enable_recv_pipe,
            enable_zero_copy_send: opts.enable_zerocopy_send,
            enable_quickack: opts.enable_quickack,
            enable_placement_id: opts.enable_placement_id,
            enable_zerocopy_send_server: opts.enable_zerocopy_send_server,
            enable_zerocopy_send_client: opts.enable_zerocopy_send_client,
            zerocopy_threshold: opts.zerocopy_threshold,
        }
    }

    fn set(&self) -> bool {
        let opts = spdk_sock_impl_opts {
            recv_buf_size: self.recv_buf_size,
            send_buf_size: self.send_buf_size,
            enable_recv_pipe: self.enable_recv_pipe,
            enable_zerocopy_send: self.enable_zero_copy_send,
            enable_quickack: self.enable_quickack,
            enable_placement_id: self.enable_placement_id,
            enable_zerocopy_send_server: self.enable_zerocopy_send_server,
            enable_zerocopy_send_client: self.enable_zerocopy_send_client,
            zerocopy_threshold: self.zerocopy_threshold,
            tls_version: 0,
            enable_ktls: false,
            psk_key: std::ptr::null_mut(),
            psk_key_size: 0,
            psk_identity: std::ptr::null_mut(),
            get_key: None,
            get_key_ctx: std::ptr::null_mut(),
            tls_cipher_suites: std::ptr::null_mut(),
        };

        let size = std::mem::size_of::<spdk_sock_impl_opts>() as u64;
        unsafe {
            let name = std::ffi::CString::new("posix").unwrap();
            let rc = spdk_sock_impl_set_opts(
                name.as_ptr(),
                &opts as *const _ as *mut spdk_sock_impl_opts,
                size,
            );
            rc == 0
        }
    }
}

/// I/O buffer pool options.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct IoBufOpts {
    /// Maximum number of small buffers.
    pub small_pool_count: u64,
    /// Maximum number of large buffers.
    pub large_pool_count: u64,
    /// Size of a single small buffer.
    pub small_bufsize: u32,
    /// Size of a single large buffer.
    pub large_bufsize: u32,
}

impl GetOpts for IoBufOpts {
    fn get(&self) -> Self {
        let mut opts = spdk_iobuf_opts::default();
        unsafe { spdk_iobuf_get_opts(&mut opts) };
        opts.into()
    }

    fn set(&self) -> bool {
        unsafe { spdk_iobuf_set_opts(&self.into()) == 0 }
    }
}

impl Default for IoBufOpts {
    fn default() -> Self {
        Self {
            small_pool_count: try_from_env("IOBUF_SMALL_POOL_COUNT", 8192),
            large_pool_count: try_from_env("IOBUF_LARGE_POOL_COUNT", 1024),
            small_bufsize: try_from_env("IOBUF_SMALL_BUFSIZE", 8 * 1024),
            large_bufsize: try_from_env("IOBUF_LARGE_BUFSIZE", 132 * 1024),
        }
    }
}

impl From<spdk_iobuf_opts> for IoBufOpts {
    fn from(o: spdk_iobuf_opts) -> Self {
        Self {
            small_pool_count: o.small_pool_count,
            large_pool_count: o.large_pool_count,
            small_bufsize: o.small_bufsize,
            large_bufsize: o.large_bufsize,
        }
    }
}

impl From<&IoBufOpts> for spdk_iobuf_opts {
    fn from(o: &IoBufOpts) -> Self {
        Self {
            small_pool_count: o.small_pool_count,
            large_pool_count: o.large_pool_count,
            small_bufsize: o.small_bufsize,
            large_bufsize: o.large_bufsize,
        }
    }
}
