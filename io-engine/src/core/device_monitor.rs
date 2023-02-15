use std::{
    fmt::{Debug, Display, Formatter},
    time::Duration,
};

use super::{work_queue::WorkQueue, Reactor};
use crate::{bdev::nexus::nexus_lookup_mut, core::VerboseError};

/// TODO
#[derive(Debug, Clone)]
pub enum DeviceCommand {
    RemoveDevice {
        nexus_name: String,
        child_device: String,
    },
}

impl Display for DeviceCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::RemoveDevice {
                nexus_name,
                child_device,
            } => write!(
                f,
                "remove device '{child_device}' from nexus '{nexus_name}'",
            ),
        }
    }
}

/// TODO
static DEV_CMD_QUEUE: once_cell::sync::Lazy<WorkQueue<DeviceCommand>> =
    once_cell::sync::Lazy::new(WorkQueue::new);

/// TODO
pub fn device_cmd_queue() -> &'static WorkQueue<DeviceCommand> {
    &DEV_CMD_QUEUE
}

/// TODO
pub async fn device_monitor_loop() {
    let mut interval = tokio::time::interval(Duration::from_millis(10));
    loop {
        interval.tick().await;
        if let Some(w) = device_cmd_queue().take() {
            info!("Device monitor executing command: {}", w);
            match w {
                DeviceCommand::RemoveDevice {
                    nexus_name,
                    child_device,
                } => {
                    let rx = Reactor::spawn_at_primary(async move {
                        if let Some(n) = nexus_lookup_mut(&nexus_name) {
                            if let Err(e) =
                                n.destroy_child_device(&child_device).await
                            {
                                error!(
                                    "{:?}: destroy child failed: {}",
                                    n,
                                    e.verbose()
                                );
                            }
                        }
                    });

                    match rx {
                        Err(e) => {
                            error!(
                                "Failed to schedule removal request: {}",
                                e.verbose()
                            )
                        }
                        Ok(rx) => rx.await.unwrap(),
                    }
                }
            }
        }
    }
}
