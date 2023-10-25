use std::{
    fmt::{Debug, Formatter},
    ops::{Deref, DerefMut},
    pin::Pin,
};

use libc::c_void;
use nix::errno::Errno;

use spdk_rs::{
    libspdk::{
        spdk_bdev_io,
        spdk_bdev_io_complete_nvme_status,
        spdk_io_channel,
        SPDK_NVME_SC_ABORTED_SQ_DELETION,
        SPDK_NVME_SC_CAPACITY_EXCEEDED,
        SPDK_NVME_SC_INVALID_OPCODE,
        SPDK_NVME_SC_RESERVATION_CONFLICT,
    },
    BdevIo,
};

use super::{FaultReason, IOLogChannel, Nexus, NexusChannel, NEXUS_PRODUCT_ID};

use crate::core::{
    BlockDevice,
    BlockDeviceHandle,
    CoreError,
    Cores,
    IoCompletionStatus,
    IoStatus,
    IoSubmissionFailure,
    IoType,
    LvolFailure,
    Mthread,
    NvmeStatus,
    ReadOptions,
};

#[cfg(feature = "nexus-io-tracing")]
mod debug_nexus_io {
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    pub(super) fn new_serial() -> u64 {
        SERIAL.fetch_add(1, Ordering::SeqCst)
    }
}

#[cfg(feature = "nexus-io-tracing")]
macro_rules! trace_nexus_io {
    ($($arg:tt)*) => {{ trace!($($arg)*); }}
}

#[cfg(not(feature = "nexus-io-tracing"))]
macro_rules! trace_nexus_io {
    ($($arg:tt)*) => {};
}

/// TODO
#[repr(C)]
pub(super) struct NioCtx<'n> {
    /// Number of I/O's submitted. Nexus I/O's may never be freed until this
    /// counter drops to zero.
    in_flight: u8,
    /// Intermediate status of the I/O.
    status: IoStatus,
    /// Reference to the channel.
    channel: spdk_rs::IoChannel<NexusChannel<'n>>,
    /// Counter for successfully completed child I/Os.
    successful: u8,
    /// Counter for failed child I/Os.
    failed: u8,
    /// Number of resubmissions. Incremented with each resubmission.
    resubmits: u8,
    /// Debug serial number.
    #[cfg(feature = "nexus-io-tracing")]
    serial: u64,
}

impl<'n> Debug for NioCtx<'n> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        #[cfg(feature = "nexus-io-tracing")]
        let serial = format!("#{s} {self:p} ", s = self.serial);

        #[cfg(not(feature = "nexus-io-tracing"))]
        let serial = "";

        write!(
            f,
            "{serial}[{re}{status:?} {sc}:{fc}{infl}]",
            re = if self.resubmits > 0 {
                format!("re:{} ", self.resubmits)
            } else {
                "".to_string()
            },
            status = self.status,
            infl = if self.in_flight > 0 {
                format!(" (infl:{})", self.in_flight)
            } else {
                "".to_string()
            },
            sc = self.successful,
            fc = self.failed,
        )
    }
}

/// TODO
#[repr(transparent)]
#[derive(Clone)]
pub(super) struct NexusBio<'n>(BdevIo<Nexus<'n>>);

impl<'n> Debug for NexusBio<'n> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "I/O {type:?} at {eoff}({off})/{num}: {ctx:?} ({chan:?})",
            type = self.io_type(),
            eoff = self.effective_offset(),
            off = self.offset(),
            num = self.num_blocks(),
            ctx = self.ctx(),
            chan = self.channel(),
        )
    }
}

impl<'n> Deref for NexusBio<'n> {
    type Target = BdevIo<Nexus<'n>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for NexusBio<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<'n> From<*mut spdk_bdev_io> for NexusBio<'n> {
    fn from(ptr: *mut spdk_bdev_io) -> Self {
        Self(BdevIo::<Nexus<'n>>::legacy_from_ptr(ptr))
    }
}

impl<'n> NexusBio<'n> {
    fn as_ptr(&self) -> *mut spdk_bdev_io {
        self.0.legacy_as_ptr()
    }

    /// Makes a new instance of `NexusBio` from a channel and `BdevIo`.
    pub(super) fn new(
        channel: spdk_rs::IoChannel<NexusChannel<'n>>,
        io: BdevIo<Nexus<'n>>,
    ) -> Self {
        let mut bio = Self(io);
        let ctx = bio.ctx_mut();
        ctx.channel = channel;
        ctx.status = IoStatus::Pending;
        ctx.in_flight = 0;
        ctx.resubmits = 0;
        ctx.successful = 0;
        ctx.failed = 0;

        #[cfg(feature = "nexus-io-tracing")]
        {
            ctx.serial = debug_nexus_io::new_serial();
        }

        trace_nexus_io!("New: {bio:?}");

        bio
    }

    /// TODO
    pub(super) fn submit_request(mut self) {
        if self.channel().is_frozen() {
            let s = self.clone();
            self.channel_mut().freeze_io_submission(s);
            return;
        }

        if let Err(_e) = match self.io_type() {
            IoType::Read => self.readv(),
            // these IOs are submitted to all the underlying children
            IoType::Write
            | IoType::WriteZeros
            | IoType::Reset
            | IoType::Unmap
            | IoType::Flush => self.submit_all(),
            IoType::NvmeAdmin => {
                self.fail();
                Err(CoreError::NotSupported {
                    source: Errno::EINVAL,
                })
            }
            _ => {
                trace!(?self, "not supported");
                self.fail();
                Err(CoreError::NotSupported {
                    source: Errno::EOPNOTSUPP,
                })
            }
        } {
            trace_nexus_io!("Submission error: {self:?}: {_e}");
        }
    }

    /// Obtains a reference to the Nexus struct embedded within the bdev.
    #[inline(always)]
    pub(crate) fn nexus(&self) -> &Nexus<'n> {
        self.bdev_checked(NEXUS_PRODUCT_ID).data()
    }

    /// Obtains a mutable reference to the Nexus struct embedded within the
    /// bdev.
    #[inline(always)]
    fn nexus_mut(&mut self) -> Pin<&mut Nexus<'n>> {
        self.bdev_checked(NEXUS_PRODUCT_ID).data_mut()
    }

    /// Invoked when a nexus IO completes.
    fn child_completion(
        device: &dyn BlockDevice,
        status: IoCompletionStatus,
        ctx: *mut c_void,
    ) {
        let mut nexus_io = NexusBio::from(ctx as *mut spdk_bdev_io);
        nexus_io.complete(device, status);
    }

    /// immutable reference to the IO context
    #[inline(always)]
    fn ctx(&self) -> &NioCtx<'n> {
        self.driver_ctx::<NioCtx>()
    }

    /// a mutable reference to the IO context
    #[inline(always)]
    fn ctx_mut(&mut self) -> &mut NioCtx<'n> {
        self.driver_ctx_mut::<NioCtx>()
    }

    /// Completion handler for the nexus when a child I/O completes.
    fn complete(
        &mut self,
        child: &dyn BlockDevice,
        status: IoCompletionStatus,
    ) {
        #[cfg(feature = "fault-injection")]
        let status = self.inject_completion_error(child, status);

        debug_assert!(self.ctx().in_flight > 0);
        self.ctx_mut().in_flight -= 1;

        if status == IoCompletionStatus::Success {
            self.ctx_mut().successful += 1;
        } else {
            self.ctx_mut().status = IoStatus::Failed;
            self.ctx_mut().failed += 1;

            self.completion_error(child, status);
        }

        if self.ctx().in_flight > 0 {
            // More child I/Os to complete, not yet ready to complete nexus I/O.
            trace_nexus_io!("Inflight: {self:?}");
            return;
        }

        if self.ctx().failed == 0 {
            // No child failures, complete nexus I/O with success.
            trace_nexus_io!("Success: {self:?}");
            self.ok();
        } else if self.ctx().successful > 0 {
            // Having some child failures, resubmit the I/O.
            self.resubmit();
        } else {
            error!("{self:?}: failing nexus I/O: all child I/Os failed");

            unsafe {
                self.nexus_mut().get_unchecked_mut().last_error = status;
            }

            self.fail();
        }
    }

    /// Fails the current I/O with a generic internal error. If the nexus
    /// already had a last child error, it fails with it.
    fn fail(&self) {
        match self.nexus().last_error {
            IoCompletionStatus::NvmeError(s) => self.fail_nvme_status(s),
            IoCompletionStatus::LvolError(LvolFailure::NoSpace) => self
                .fail_nvme_status(NvmeStatus::Generic(
                    SPDK_NVME_SC_CAPACITY_EXCEEDED,
                )),
            _ => self.0.fail(),
        }
    }

    /// Completes the I/O with the given `NvmeStatus`.
    #[inline(always)]
    fn fail_nvme_status(&self, status: NvmeStatus) {
        let (sct, sc) = status.as_sct_sc_codes();
        unsafe {
            spdk_bdev_io_complete_nvme_status(self.as_ptr(), 0, sct, sc);
        }
    }

    /// Resubmits the I/O.
    fn resubmit(&mut self) {
        warn!("{self:?}: resubmitting nexus I/O due to a child I/O failure");

        let ctx = self.ctx_mut();

        debug_assert_eq!(ctx.in_flight, 0);
        debug_assert!(ctx.failed > 0);
        debug_assert!(ctx.successful > 0);

        ctx.status = IoStatus::Pending;
        ctx.resubmits += 1;
        ctx.successful = 0;
        ctx.failed = 0;

        let bio = self.clone();
        trace_nexus_io!("New resubmit: {bio:?}");
        bio.submit_request();
    }

    /// reference to the channel. The channel contains the specific
    /// per-core data structures.
    #[inline(always)]
    fn channel(&self) -> &NexusChannel<'n> {
        self.ctx().channel.channel_data()
    }

    /// mutable reference to the channels. The channel contains the
    /// specific per-core data structures.
    #[inline(always)]
    fn channel_mut(&mut self) -> &mut NexusChannel<'n> {
        self.ctx_mut().channel.channel_data_mut()
    }

    /// Returns the offset in num blocks where the data partition starts.
    #[inline]
    fn data_ent_offset(&self) -> u64 {
        self.nexus().data_ent_offset
    }

    /// Returns the effictive offset in num blocks where the I/O operation
    /// starts.
    #[inline]
    fn effective_offset(&self) -> u64 {
        self.offset() + self.data_ent_offset()
    }

    /// submit a read operation to one of the children of this nexus
    #[inline]
    fn submit_read(
        &self,
        hdl: &dyn BlockDeviceHandle,
    ) -> Result<(), CoreError> {
        #[cfg(feature = "fault-injection")]
        self.inject_submission_error(hdl)?;

        hdl.readv_blocks(
            self.iovs_mut(),
            self.effective_offset(),
            self.num_blocks(),
            ReadOptions::None,
            Self::child_completion,
            self.as_ptr().cast(),
        )
    }

    /// Submit a Read operation to the next available replica.
    fn __do_readv_one(&mut self) -> Result<(), CoreError> {
        if let Some(hdl) = self.channel().select_reader() {
            let r = self.submit_read(hdl);

            if r.is_err() {
                // Such a situation can happen when there is no active I/O in
                // the queues, but error on qpair is observed
                // due to network timeout, which initiates
                // controller reset. During controller reset all
                // I/O channels are de-initialized, so no I/O
                // submission is possible (spdk returns -6/ENXIO), so we have to
                // start device retire.
                // TODO: ENOMEM and ENXIO should be handled differently and
                // device should not be retired in case of ENOMEM.
                let device = hdl.get_device().device_name();
                error!(
                    "{self:?}: read I/O to '{device}' submission failed: {r:?}"
                );

                self.fault_device(
                    &device,
                    IoCompletionStatus::IoSubmissionError(
                        IoSubmissionFailure::Read,
                    ),
                );
                r
            } else {
                self.ctx_mut().in_flight = 1;
                r
            }
        } else {
            error!(
                "{self:?}: read I/O submission failed: no children available"
            );

            Err(CoreError::NoDevicesAvailable {})
        }
    }

    /// Submit a read operation to the next suitable replica.
    /// In case of submission error the requiest is transparently resubmitted
    /// to the next available replica.
    fn do_readv(&mut self) -> Result<(), CoreError> {
        match self.__do_readv_one() {
            Err(e) => {
                match e {
                    // No readers available - bail out.
                    CoreError::NoDevicesAvailable {} => {
                        self.fail();
                        Err(e)
                    }
                    // Failed to submit Read I/O request to the current replica,
                    // try to resumbit request to the next available replica.
                    _ => {
                        let mut num_readers = self.channel().num_readers();

                        let r = {
                            if num_readers <= 1 {
                                // No more readers available (taking into
                                // account the failed one).
                                Err(e)
                            } else {
                                num_readers -= 1; // Account the failed reader.

                                // Resubmission loop to find a next available
                                // replica for this Read I/O operation.
                                loop {
                                    match self.__do_readv_one() {
                                        Ok(_) => break Ok(()),
                                        Err(e) => {
                                            num_readers -= 1;

                                            if num_readers == 0 {
                                                break Err(e);
                                            }
                                        }
                                    }
                                }
                            }
                        };

                        if r.is_err() {
                            self.fail();
                        }
                        r
                    }
                }
            }
            Ok(_) => Ok(()),
        }
    }

    extern "C" fn nexus_get_buf_cb(
        _ch: *mut spdk_io_channel,
        io: *mut spdk_bdev_io,
        success: bool,
    ) {
        let mut bio = NexusBio::from(io);

        if !success {
            trace!(
                "(core: {core} thread: {thread}): get_buf() failed",
                core = Cores::current(),
                thread = Mthread::current().unwrap().name()
            );
            bio.no_mem();
        }

        let _ = bio.do_readv();
    }

    /// submit read IO to some child
    fn readv(&mut self) -> Result<(), CoreError> {
        if self.need_buf() {
            unsafe {
                self.alloc_buffer(Self::nexus_get_buf_cb);
            }
            Ok(())
        } else {
            self.do_readv()
        }
    }

    #[inline]
    fn submit_write(
        &self,
        hdl: &dyn BlockDeviceHandle,
    ) -> Result<(), CoreError> {
        trace_nexus_io!(
            "Submitting: {self:?} -> {name}",
            name = hdl.get_device().device_name()
        );

        #[cfg(feature = "fault-injection")]
        self.inject_submission_error(hdl)?;

        hdl.writev_blocks(
            self.iovs(),
            self.effective_offset(),
            self.num_blocks(),
            Self::child_completion,
            self.as_ptr().cast(),
        )
    }

    #[inline]
    fn submit_unmap(
        &self,
        hdl: &dyn BlockDeviceHandle,
    ) -> Result<(), CoreError> {
        trace_nexus_io!(
            "Submitting: {self:?} -> {name}",
            name = hdl.get_device().device_name()
        );

        hdl.unmap_blocks(
            self.effective_offset(),
            self.num_blocks(),
            Self::child_completion,
            self.as_ptr().cast(),
        )
    }

    #[inline]
    fn submit_write_zeroes(
        &self,
        hdl: &dyn BlockDeviceHandle,
    ) -> Result<(), CoreError> {
        trace_nexus_io!(
            "Submitting: {self:?} -> {name}",
            name = hdl.get_device().device_name()
        );

        #[cfg(feature = "fault-injection")]
        self.inject_submission_error(hdl)?;

        hdl.write_zeroes(
            self.effective_offset(),
            self.num_blocks(),
            Self::child_completion,
            self.as_ptr().cast(),
        )
    }

    #[inline]
    fn submit_reset(
        &self,
        hdl: &dyn BlockDeviceHandle,
    ) -> Result<(), CoreError> {
        trace_nexus_io!(
            "Submitting: {self:?} -> {name}",
            name = hdl.get_device().device_name()
        );

        hdl.reset(Self::child_completion, self.as_ptr().cast())
    }

    #[inline]
    fn submit_flush(
        &self,
        hdl: &dyn BlockDeviceHandle,
    ) -> Result<(), CoreError> {
        trace_nexus_io!(
            "Submitting: {self:?} -> {name}",
            name = hdl.get_device().device_name()
        );

        hdl.flush_io(Self::child_completion, self.as_ptr().cast())
    }

    /// Submit the IO to all underlying children, failing on the first error we
    /// find. When an IO is partially submitted -- we must wait until all
    /// the child IOs have completed before we mark the whole IO failed to
    /// avoid double frees. This function handles IO for a subset that must
    /// be submitted to all the underlying children.
    fn submit_all(&mut self) -> Result<(), CoreError> {
        let mut inflight = 0;
        // Name of the device which experiences I/O submission failures.
        let mut failed_device = None;

        let result = self.channel().for_each_writer(|h| {
            match self.io_type() {
                IoType::Write => self.submit_write(h),
                IoType::Unmap => self.submit_unmap(h),
                IoType::WriteZeros => self.submit_write_zeroes(h),
                IoType::Reset => self.submit_reset(h),
                IoType::Flush => self.submit_flush(h),
                // we should never reach here, if we do it is a bug.
                _ => unreachable!(),
            }
            .map(|_| {
                inflight += 1;
            })
            .map_err(|err| {
                error!(
                    "(core: {core} thread: {thread}): IO submission \
                        failed with error {err:?}, I/Os submitted: {inflight}",
                    core = Cores::current(),
                    thread = Mthread::current().unwrap().name()
                );

                // Record the name of the device for immediate retire.
                failed_device = Some(h.get_device().device_name());
                err
            })
        });

        // Submission errors can also trigger device retire.
        // Such a situation can happen when there is no active I/O in the
        // queues, but error on qpair is observed due to network
        // timeout, which initiates controller reset. During controller
        // reset all I/O channels are de-initialized, so no I/O
        // submission is possible (spdk returns -6/ENXIO), so we have to
        // start device retire.

        // TODO:
        // ENOMEM and ENXIO should be handled differently and
        // device should not be retired in case of ENOMEM.
        if result.is_err() {
            let device = failed_device.unwrap();
            // set the IO as failed in the submission stage.
            self.ctx_mut().failed += 1;

            self.channel_mut().disconnect_device(&device);

            if let Some(log) = self.fault_device(
                &device,
                IoCompletionStatus::IoSubmissionError(
                    IoSubmissionFailure::Write,
                ),
            ) {
                self.log_io(&log);
            }
        }

        self.channel().for_each_io_log(|log| self.log_io(log));

        if inflight > 0 {
            // TODO: fix comment:
            // An error was experienced during submission.
            // Some IO however, has been submitted successfully
            // prior to the error condition.
            self.ctx_mut().in_flight = inflight;
            self.ctx_mut().status = IoStatus::Success;
        } else {
            debug_assert_eq!(self.ctx().in_flight, 0);
            error!(
                "{self:?}: failing nexus I/O: all child I/O submissions failed"
            );
            self.fail();
        }

        result
    }

    /// Logs all write-like operation in the rebuild logs, if any exist.
    #[inline]
    fn log_io(&self, log: &IOLogChannel) {
        log.log_io(self.io_type(), self.effective_offset(), self.num_blocks());
    }

    /// Initiate shutdown of the nexus associated with this BIO request.
    fn try_self_shutdown_nexus(&mut self) {
        if self
            .channel_mut()
            .nexus_mut()
            .shutdown_requested
            .compare_exchange(false, true)
            .is_ok()
        {
            self.channel().nexus().try_self_shutdown();
        }
    }

    /// Faults the device by its name, with the given I/O error.
    /// The faulted device is scheduled to be retired.
    fn fault_device(
        &mut self,
        child_device: &str,
        io_status: IoCompletionStatus,
    ) -> Option<IOLogChannel> {
        let reason = match io_status {
            IoCompletionStatus::LvolError(LvolFailure::NoSpace) => {
                FaultReason::NoSpace
            }
            _ => FaultReason::IoError,
        };

        self.channel_mut().fault_device(child_device, reason)
    }

    /// TODO
    fn completion_error(
        &mut self,
        child: &dyn BlockDevice,
        status: IoCompletionStatus,
    ) {
        // We have experienced a failure on one of the child devices. We need to
        // ensure we do not submit more IOs to this child. We do not
        // need to tell other cores about this because
        // they will experience the same errors on their own channels, and
        // handle it on their own.
        //
        // We differentiate between errors in the submission and completion.
        // When we have a completion error, it typically means that the
        // child has lost the connection to the nexus. In order for
        // outstanding IO to complete, the IO's to that child must be aborted.
        // The abortion is implicit when removing the device.

        if matches!(
            status,
            IoCompletionStatus::NvmeError(NvmeStatus::Generic(
                SPDK_NVME_SC_INVALID_OPCODE
            ))
        ) {
            warn!(
                "{self:?}: invalid opcode error on '{dev}', skipping retire",
                dev = child.device_name()
            );
            return;
        }

        // Reservation conflicts should trigger shutdown of the nexus but
        // replica should not be retired.
        if matches!(
            status,
            IoCompletionStatus::NvmeError(NvmeStatus::Generic(
                SPDK_NVME_SC_RESERVATION_CONFLICT
            ))
        ) {
            warn!(
                "{self:?}: reservation conflict on '{dev}', shutdown nexus",
                dev = child.device_name()
            );
            self.try_self_shutdown_nexus();
            return;
        }

        if matches!(
            status,
            IoCompletionStatus::NvmeError(NvmeStatus::Generic(
                SPDK_NVME_SC_ABORTED_SQ_DELETION
            ))
        ) {
            warn!(
                "{self:?}: aborted submission queue deleted on '{dev}'",
                dev = child.device_name(),
            );
        } else {
            error!(
                "{self:?}: child I/O failed on '{dev}' with {err:?}",
                dev = child.device_name(),
                err = status,
            );
        }

        if let Some(log) = self.fault_device(&child.device_name(), status) {
            self.log_io(&log);
        }
    }

    /// Checks if an error is to be injected upon submission.
    #[cfg(feature = "fault-injection")]
    #[inline]
    fn inject_submission_error(
        &self,
        hdl: &dyn BlockDeviceHandle,
    ) -> Result<(), CoreError> {
        use crate::core::fault_injection::{
            inject_submission_error,
            FaultDomain::NexusChild,
            InjectIoCtx,
        };

        inject_submission_error(&InjectIoCtx::with_iovs(
            NexusChild,
            hdl.get_device(),
            self.io_type(),
            self.offset(),
            self.num_blocks(),
            self.iovs(),
        ))
    }

    /// Checks if an error is to be injected upon completion.
    #[cfg(feature = "fault-injection")]
    #[inline]
    fn inject_completion_error(
        &self,
        child: &dyn BlockDevice,
        status: IoCompletionStatus,
    ) -> IoCompletionStatus {
        use crate::core::fault_injection::{
            inject_completion_error,
            FaultDomain::NexusChild,
            InjectIoCtx,
        };

        inject_completion_error(
            &InjectIoCtx::with_iovs(
                NexusChild,
                child,
                self.io_type(),
                self.offset(),
                self.num_blocks(),
                self.iovs(),
            ),
            status,
        )
    }
}
