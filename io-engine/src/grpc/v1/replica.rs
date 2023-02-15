use crate::{
    bdev::PtplFileOps,
    bdev_api::BdevError,
    core::{Bdev, Protocol, Share, ShareProps, UntypedBdev, UpdateProps},
    grpc::{rpc_submit, GrpcClientContext, GrpcResult, Serializer},
    lvs::{Error as LvsError, Lvol, LvolSpaceUsage, Lvs},
};
use ::function_name::named;
use futures::FutureExt;
use mayastor_api::v1::replica::*;
use nix::errno::Errno;
use std::{convert::TryFrom, panic::AssertUnwindSafe, pin::Pin};
use tonic::{Request, Response, Status};

#[derive(Debug)]
#[allow(dead_code)]
pub struct ReplicaService {
    name: String,
    client_context: tokio::sync::Mutex<Option<GrpcClientContext>>,
}

#[async_trait::async_trait]
impl<F, T> Serializer<F, T> for ReplicaService
where
    T: Send + 'static,
    F: core::future::Future<Output = Result<T, Status>> + Send + 'static,
{
    async fn locked(&self, ctx: GrpcClientContext, f: F) -> Result<T, Status> {
        let mut context_guard = self.client_context.lock().await;

        // Store context as a marker of to detect abnormal termination of the
        // request. Even though AssertUnwindSafe() allows us to
        // intercept asserts in underlying method strategies, such a
        // situation can still happen when the high-level future that
        // represents gRPC call at the highest level (i.e. the one created
        // by gRPC server) gets cancelled (due to timeout or somehow else).
        // This can't be properly intercepted by 'locked' function itself in the
        // first place, so the state needs to be cleaned up properly
        // upon subsequent gRPC calls.
        if let Some(c) = context_guard.replace(ctx) {
            warn!("{}: gRPC method timed out, args: {}", c.id, c.args);
        }

        let fut = AssertUnwindSafe(f).catch_unwind();
        let r = fut.await;

        // Request completed, remove the marker.
        let ctx = context_guard.take().expect("gRPC context disappeared");

        match r {
            Ok(r) => r,
            Err(_e) => {
                warn!("{}: gRPC method panicked, args: {}", ctx.id, ctx.args);
                Err(Status::cancelled(format!(
                    "{}: gRPC method panicked",
                    ctx.id
                )))
            }
        }
    }
}

impl From<LvolSpaceUsage> for ReplicaSpaceUsage {
    fn from(u: LvolSpaceUsage) -> Self {
        Self {
            capacity_bytes: u.capacity_bytes,
            allocated_bytes: u.allocated_bytes,
            cluster_size: u.cluster_size,
            num_clusters: u.num_clusters,
            num_allocated_clusters: u.num_allocated_clusters,
        }
    }
}

impl From<Lvol> for Replica {
    fn from(l: Lvol) -> Self {
        let usage = l.usage();
        Self {
            name: l.name(),
            uuid: l.uuid(),
            pooluuid: l.pool_uuid(),
            size: usage.capacity_bytes,
            thin: l.is_thin(),
            share: l.shared().unwrap().into(),
            uri: l.share_uri().unwrap(),
            poolname: l.pool_name(),
            usage: Some(usage.into()),
            allowed_hosts: l.allowed_hosts(),
        }
    }
}

impl Default for ReplicaService {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplicaService {
    pub fn new() -> Self {
        Self {
            name: String::from("ReplicaSvc"),
            client_context: tokio::sync::Mutex::new(None),
        }
    }
}

#[tonic::async_trait]
impl ReplicaRpc for ReplicaService {
    #[named]
    async fn create_replica(
        &self,
        request: Request<CreateReplicaRequest>,
    ) -> GrpcResult<Replica> {
        self.locked(GrpcClientContext::new(&request, function_name!()), async move {

            let args = request.into_inner();
            info!("{:?}", args);
            if !matches!(
                Protocol::try_from(args.share)?,
                Protocol::Off | Protocol::Nvmf
            ) {
                return Err(LvsError::ReplicaShareProtocol {
                    value: args.share,
                }).map_err(Status::from);
            }

            let rx = rpc_submit(async move {
                let lvs = match Lvs::lookup_by_uuid(&args.pooluuid) {
                    Some(lvs) => lvs,
                    None => {
                        // lookup takes care of backward compatibility
                        match Lvs::lookup(&args.pooluuid) {
                            Some(lvs) => lvs,
                            None => {
                                return Err(LvsError::Invalid {
                                    source: Errno::ENOSYS,
                                    msg: format!("Pool {} not found", args.pooluuid),
                                })
                            }
                        }
                    }
                };
                // if pooltype is not Lvs, the provided replica uuid need to be added as
                // a metadata on the volume.
                match lvs.create_lvol(&args.name, args.size, Some(&args.uuid), args.thin).await {
                    Ok(mut lvol)
                    if Protocol::try_from(args.share)? == Protocol::Nvmf => {
                        let props = ShareProps::new()
                            .with_allowed_hosts(args.allowed_hosts)
                            .with_ptpl(lvol.ptpl().create().map_err(
                                |source| LvsError::LvolShare {
                                    source: crate::core::CoreError::Ptpl {
                                        reason: source.to_string(),
                                    },
                                    name: lvol.name(),
                                },
                            )?);
                        match Pin::new(&mut lvol).share_nvmf(Some(props)).await {
                            Ok(s) => {
                                debug!("created and shared {:?} as {}", lvol, s);
                                Ok(Replica::from(lvol))
                            }
                            Err(e) => {
                                debug!(
                                    "failed to share created lvol {:?}: {} (destroying)",
                                    lvol,
                                    e.to_string()
                                );
                                let _ = lvol.destroy().await;
                                Err(e)
                            }
                        }
                    }
                    Ok(lvol) => {
                        debug!("created lvol {:?}", lvol);
                        Ok(Replica::from(lvol))
                    }
                    Err(e) => Err(e),
                }
            })?;
            rx.await
                .map_err(|_| Status::cancelled("cancelled"))?
                .map_err(Status::from)
                .map(Response::new)
        }).await
    }

    #[named]
    async fn destroy_replica(
        &self,
        request: Request<DestroyReplicaRequest>,
    ) -> GrpcResult<()> {
        self.locked(GrpcClientContext::new(&request, function_name!()), async {
            let args = request.into_inner();
            info!("{:?}", args);
            let rx = rpc_submit::<_, _, LvsError>(async move {
                if let Some(b) = Bdev::lookup_by_uuid_str(&args.uuid) {
                    return if b.driver() == "lvol" {
                        let lvol = Lvol::try_from(b)?;
                        lvol.destroy().await?;
                        Ok(())
                    } else {
                        Err(LvsError::RepDestroy {
                            source: Errno::ENOENT,
                            name: args.uuid.clone(),
                        })
                    };
                }
                Err(LvsError::RepDestroy {
                    source: Errno::ENOENT,
                    name: args.uuid,
                })
            })?;

            rx.await
                .map_err(|_| Status::cancelled("cancelled"))?
                .map_err(Status::from)
                .map(Response::new)
        })
        .await
    }

    #[named]
    async fn list_replicas(
        &self,
        request: Request<ListReplicaOptions>,
    ) -> GrpcResult<ListReplicasResponse> {
        self.locked(GrpcClientContext::new(&request, function_name!()), async {
            let args = request.into_inner();
            trace!("{:?}", args);
            let rx = rpc_submit::<_, _, LvsError>(async move {
                let mut lvols = Vec::new();
                if let Some(bdev) = UntypedBdev::bdev_first() {
                    lvols = bdev
                        .into_iter()
                        .filter(|b| b.driver() == "lvol")
                        .map(|b| Lvol::try_from(b).unwrap())
                        .collect();
                }

                // perform filtering on lvols
                if let Some(pool_name) = args.poolname {
                    lvols.retain(|l| l.pool_name() == pool_name);
                }

                // convert lvols to replicas
                let mut replicas: Vec<Replica> =
                    lvols.into_iter().map(Replica::from).collect();

                // perform the filtering on the replica list
                if let Some(name) = args.name {
                    replicas.retain(|r| r.name == name);
                } else if let Some(uuid) = args.uuid {
                    replicas.retain(|r| r.uuid == uuid);
                }

                Ok(ListReplicasResponse {
                    replicas,
                })
            })?;

            rx.await
                .map_err(|_| Status::cancelled("cancelled"))?
                .map_err(Status::from)
                .map(Response::new)
        })
        .await
    }

    #[named]
    async fn share_replica(
        &self,
        request: Request<ShareReplicaRequest>,
    ) -> GrpcResult<Replica> {
        self.locked(
            GrpcClientContext::new(&request, function_name!()),
            async move {
                let args = request.into_inner();
                info!("{:?}", args);
                let rx = rpc_submit(async move {
                    match Bdev::lookup_by_uuid_str(&args.uuid) {
                        Some(bdev) => {
                            let mut lvol = Lvol::try_from(bdev)?;

                            // if we are already shared with the same protocol
                            if lvol.shared()
                                == Some(Protocol::try_from(args.share)?)
                            {
                                Pin::new(&mut lvol)
                                    .update_properties(
                                        UpdateProps::new().with_allowed_hosts(
                                            args.allowed_hosts,
                                        ),
                                    )
                                    .await?;
                                return Ok(Replica::from(lvol));
                            }

                            match Protocol::try_from(args.share)? {
                                Protocol::Off => {
                                    return Err(LvsError::Invalid {
                                        source: Errno::EINVAL,
                                        msg: "invalid share protocol NONE"
                                            .to_string(),
                                    })
                                }
                                Protocol::Nvmf => {
                                    let props = ShareProps::new()
                                        .with_allowed_hosts(args.allowed_hosts)
                                        .with_ptpl(lvol.ptpl().create().map_err(
                                            |source| LvsError::LvolShare {
                                                source: crate::core::CoreError::Ptpl {
                                                    reason: source.to_string(),
                                                },
                                                name: lvol.name(),
                                            },
                                        )?);
                                    Pin::new(&mut lvol)
                                        .share_nvmf(Some(props))
                                        .await?;
                                }
                            }

                            Ok(Replica::from(lvol))
                        }

                        None => Err(LvsError::InvalidBdev {
                            source: BdevError::BdevNotFound {
                                name: args.uuid.clone(),
                            },
                            name: args.uuid,
                        }),
                    }
                })?;

                rx.await
                    .map_err(|_| Status::cancelled("cancelled"))?
                    .map_err(Status::from)
                    .map(Response::new)
            },
        )
        .await
    }

    #[named]
    async fn unshare_replica(
        &self,
        request: Request<UnshareReplicaRequest>,
    ) -> GrpcResult<Replica> {
        self.locked(
            GrpcClientContext::new(&request, function_name!()),
            async move {
                let args = request.into_inner();
                info!("{:?}", args);
                let rx = rpc_submit(async move {
                    match Bdev::lookup_by_uuid_str(&args.uuid) {
                        Some(bdev) => {
                            let mut lvol = Lvol::try_from(bdev)?;
                            if lvol.shared().is_some() {
                                Pin::new(&mut lvol).unshare().await?;
                            }
                            Ok(Replica::from(lvol))
                        }
                        None => Err(LvsError::InvalidBdev {
                            source: BdevError::BdevNotFound {
                                name: args.uuid.clone(),
                            },
                            name: args.uuid,
                        }),
                    }
                })?;
                rx.await
                    .map_err(|_| Status::cancelled("cancelled"))?
                    .map_err(Status::from)
                    .map(Response::new)
            },
        )
        .await
    }
}
