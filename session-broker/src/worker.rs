use std::{
    sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    thread::{self, JoinHandle},
    time::Duration,
};

use ksa64_presentation::{
    ActionReceiptView, CursorError, DispositionView, OperationalSnapshot, PredictionPathView,
    PresentationActionIntent, PresentationBatch, PresentationCursors, PresentationEventView,
    PresentationLifecycle, PresentationPace, PresentationRole, PresentationSession, ProcedureView,
    ReleaseSampleView, SealedEvidenceMetadata, TimelineEventView, TransportStatusView,
    SNAPSHOT_VALID_ACTION,
};

pub const DEFAULT_WORKER_COMMAND_CAPACITY: usize = 32;
pub const MAX_WORKER_COMMAND_CAPACITY: usize = 256;
pub const DEFAULT_WORKER_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_FAST_RELEASE_BATCH: u32 = 8;
pub const MAX_POLL_RECORDS: usize = 256;
pub const MAX_ATTACHED_CLIENTS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorityError {
    pub code: u32,
}

pub trait BrokerAuthority: PresentationSession<Error = AuthorityError> + Send + 'static {
    fn session_nonce(&self) -> u64;
    fn advance_bounded(&mut self, max_releases: u32) -> Result<u32, AuthorityError>;
    fn set_pace(&mut self, pace: PresentationPace) -> Result<(), AuthorityError>;
    fn request_lifecycle(&mut self, requested: PresentationLifecycle)
        -> Result<(), AuthorityError>;
    fn step_one_release(&mut self) -> Result<u32, AuthorityError>;
    fn sealed_evidence(&self) -> Option<Vec<u8>> {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkerConfig {
    pub command_capacity: usize,
    pub response_timeout: Duration,
    pub autonomous_pacing: bool,
    pub fast_release_batch: u32,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            command_capacity: DEFAULT_WORKER_COMMAND_CAPACITY,
            response_timeout: DEFAULT_WORKER_RESPONSE_TIMEOUT,
            autonomous_pacing: true,
            fast_release_batch: DEFAULT_FAST_RELEASE_BATCH,
        }
    }
}

impl WorkerConfig {
    fn validate(self) -> Result<(), BrokerError> {
        if self.command_capacity == 0
            || self.command_capacity > MAX_WORKER_COMMAND_CAPACITY
            || self.response_timeout.is_zero()
            || self.fast_release_batch == 0
        {
            return Err(BrokerError::InvalidConfig);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrokerError {
    InvalidConfig,
    SessionNonce,
    Role,
    Client,
    ClientAlreadyAttached,
    ClientLimit,
    Controller,
    QueueFull,
    WorkerGone,
    Timeout,
    InvalidLimit,
    Cursor(CursorError),
    Authority(AuthorityError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttachReply {
    pub controlling: bool,
    pub session_nonce: u64,
    pub cursors: PresentationCursors,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrokerPublication {
    pub snapshot: OperationalSnapshot,
    pub procedure: Option<ProcedureView>,
    pub disposition: Option<DispositionView>,
    pub prediction_paths: Vec<PredictionPathView>,
    pub transport: TransportStatusView,
    pub evidence: Option<SealedEvidenceMetadata>,
    pub cursors: PresentationCursors,
    pub events: PresentationBatch<PresentationEventView>,
    pub timeline: PresentationBatch<TimelineEventView>,
    pub action_receipts: PresentationBatch<ActionReceiptView>,
    pub release_samples: PresentationBatch<ReleaseSampleView>,
}

enum WorkerCommand {
    Attach {
        client_id: u64,
        session_nonce: u64,
        role: PresentationRole,
        cursors: PresentationCursors,
        reply: SyncSender<Result<AttachReply, BrokerError>>,
    },
    Disconnect {
        client_id: u64,
        reply: SyncSender<Result<(), BrokerError>>,
    },
    Poll {
        client_id: u64,
        cursors: PresentationCursors,
        limit: usize,
        reply: SyncSender<Result<BrokerPublication, BrokerError>>,
    },
    Advance {
        client_id: u64,
        releases: u32,
        reply: SyncSender<Result<u32, BrokerError>>,
    },
    Pace {
        client_id: u64,
        pace: PresentationPace,
        reply: SyncSender<Result<(), BrokerError>>,
    },
    Lifecycle {
        client_id: u64,
        requested: PresentationLifecycle,
        reply: SyncSender<Result<(), BrokerError>>,
    },
    Step {
        client_id: u64,
        reply: SyncSender<Result<u32, BrokerError>>,
    },
    Action {
        client_id: u64,
        intent: PresentationActionIntent,
        reply: SyncSender<Result<ActionReceiptView, BrokerError>>,
    },
    Evidence {
        client_id: u64,
        reply: SyncSender<Result<Option<Vec<u8>>, BrokerError>>,
    },
    Shutdown,
}

pub struct SessionBrokerHandle {
    sender: SyncSender<WorkerCommand>,
    worker: Option<JoinHandle<()>>,
    response_timeout: Duration,
}

impl SessionBrokerHandle {
    pub fn spawn<A: BrokerAuthority>(
        authority: A,
        config: WorkerConfig,
    ) -> Result<Self, BrokerError> {
        config.validate()?;
        if authority.session_nonce() == 0 {
            return Err(BrokerError::SessionNonce);
        }
        let (sender, receiver) = mpsc::sync_channel(config.command_capacity);
        let worker = thread::Builder::new()
            .name(format!(
                "ksa64-authority-{:016x}",
                authority.session_nonce()
            ))
            .spawn(move || worker_loop(authority, receiver, config))
            .map_err(|_| BrokerError::WorkerGone)?;
        Ok(Self {
            sender,
            worker: Some(worker),
            response_timeout: config.response_timeout,
        })
    }

    pub fn attach(
        &self,
        client_id: u64,
        session_nonce: u64,
        role: PresentationRole,
        cursors: PresentationCursors,
    ) -> Result<AttachReply, BrokerError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(
            WorkerCommand::Attach {
                client_id,
                session_nonce,
                role,
                cursors,
                reply,
            },
            receiver,
        )
    }

    pub fn disconnect(&self, client_id: u64) -> Result<(), BrokerError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(WorkerCommand::Disconnect { client_id, reply }, receiver)
    }

    pub fn poll(
        &self,
        client_id: u64,
        cursors: PresentationCursors,
        limit: usize,
    ) -> Result<BrokerPublication, BrokerError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(
            WorkerCommand::Poll {
                client_id,
                cursors,
                limit,
                reply,
            },
            receiver,
        )
    }

    pub fn advance(&self, client_id: u64, releases: u32) -> Result<u32, BrokerError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(
            WorkerCommand::Advance {
                client_id,
                releases,
                reply,
            },
            receiver,
        )
    }

    pub fn set_pace(&self, client_id: u64, pace: PresentationPace) -> Result<(), BrokerError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(
            WorkerCommand::Pace {
                client_id,
                pace,
                reply,
            },
            receiver,
        )
    }

    pub fn set_lifecycle(
        &self,
        client_id: u64,
        requested: PresentationLifecycle,
    ) -> Result<(), BrokerError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(
            WorkerCommand::Lifecycle {
                client_id,
                requested,
                reply,
            },
            receiver,
        )
    }

    pub fn step_one_release(&self, client_id: u64) -> Result<u32, BrokerError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(WorkerCommand::Step { client_id, reply }, receiver)
    }

    pub fn submit_action(
        &self,
        client_id: u64,
        intent: PresentationActionIntent,
    ) -> Result<ActionReceiptView, BrokerError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(
            WorkerCommand::Action {
                client_id,
                intent,
                reply,
            },
            receiver,
        )
    }

    pub fn sealed_evidence(&self, client_id: u64) -> Result<Option<Vec<u8>>, BrokerError> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.request(WorkerCommand::Evidence { client_id, reply }, receiver)
    }

    fn request<T>(
        &self,
        command: WorkerCommand,
        receiver: Receiver<Result<T, BrokerError>>,
    ) -> Result<T, BrokerError> {
        self.sender.try_send(command).map_err(|error| match error {
            TrySendError::Full(_) => BrokerError::QueueFull,
            TrySendError::Disconnected(_) => BrokerError::WorkerGone,
        })?;
        receiver
            .recv_timeout(self.response_timeout)
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => BrokerError::Timeout,
                mpsc::RecvTimeoutError::Disconnected => BrokerError::WorkerGone,
            })?
    }
}

impl Drop for SessionBrokerHandle {
    fn drop(&mut self) {
        let _ = self.sender.send(WorkerCommand::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct WorkerState {
    attached_clients: Vec<u64>,
    controller_owner: Option<u64>,
}

impl WorkerState {
    fn attach(&mut self, client_id: u64, role: PresentationRole) -> Result<bool, BrokerError> {
        if client_id == 0 {
            return Err(BrokerError::Client);
        }
        if self.attached_clients.contains(&client_id) {
            return Err(BrokerError::ClientAlreadyAttached);
        }
        if self.attached_clients.len() >= MAX_ATTACHED_CLIENTS {
            return Err(BrokerError::ClientLimit);
        }
        self.attached_clients.push(client_id);
        if role.permits_operator_actions() && self.controller_owner.is_none() {
            self.controller_owner = Some(client_id);
        }
        Ok(self.controller_owner == Some(client_id))
    }

    fn disconnect(&mut self, client_id: u64) -> Result<(), BrokerError> {
        let Some(index) = self
            .attached_clients
            .iter()
            .position(|value| *value == client_id)
        else {
            return Err(BrokerError::Client);
        };
        self.attached_clients.swap_remove(index);
        // The control lease is intentionally retained. A disconnect cannot pause
        // authority or let another client silently take over.
        Ok(())
    }

    fn require_attached(&self, client_id: u64) -> Result<(), BrokerError> {
        if self.attached_clients.contains(&client_id) {
            Ok(())
        } else {
            Err(BrokerError::Client)
        }
    }

    fn require_controller(&self, client_id: u64) -> Result<(), BrokerError> {
        self.require_attached(client_id)?;
        if self.controller_owner == Some(client_id) {
            Ok(())
        } else {
            Err(BrokerError::Controller)
        }
    }
}

fn worker_loop<A: BrokerAuthority>(
    mut authority: A,
    receiver: Receiver<WorkerCommand>,
    config: WorkerConfig,
) {
    let mut state = WorkerState {
        attached_clients: Vec::with_capacity(MAX_ATTACHED_CLIENTS),
        controller_owner: None,
    };
    loop {
        let received = if config.autonomous_pacing {
            receive_with_pacing(&authority, &receiver)
        } else {
            receiver.recv().map_err(|_| RecvTimeoutError::Disconnected)
        };
        match received {
            Ok(WorkerCommand::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                let snapshot = authority.latest_snapshot();
                if snapshot.lifecycle == PresentationLifecycle::Running {
                    let releases = match snapshot.pace {
                        PresentationPace::Fast
                            if snapshot.validity_mask & SNAPSHOT_VALID_ACTION != 0 =>
                        {
                            0
                        }
                        PresentationPace::Fast => config.fast_release_batch,
                        PresentationPace::Realtime => 1,
                        PresentationPace::Paused | PresentationPace::SingleStep => 0,
                    };
                    if releases != 0 {
                        let _ = authority.advance_bounded(releases);
                    }
                }
            }
            Ok(command) => handle_command(&mut authority, &mut state, command),
        }
    }
}

fn receive_with_pacing<A: BrokerAuthority>(
    authority: &A,
    receiver: &Receiver<WorkerCommand>,
) -> Result<WorkerCommand, RecvTimeoutError> {
    let snapshot = authority.latest_snapshot();
    if snapshot.lifecycle != PresentationLifecycle::Running {
        return receiver.recv().map_err(|_| RecvTimeoutError::Disconnected);
    }
    match snapshot.pace {
        PresentationPace::Fast if snapshot.validity_mask & SNAPSHOT_VALID_ACTION != 0 => {
            receiver.recv().map_err(|_| RecvTimeoutError::Disconnected)
        }
        PresentationPace::Fast => receiver.recv_timeout(Duration::from_millis(1)),
        PresentationPace::Realtime => receiver.recv_timeout(Duration::from_micros(u64::from(
            snapshot.release_period_micros.max(1),
        ))),
        PresentationPace::Paused | PresentationPace::SingleStep => {
            receiver.recv().map_err(|_| RecvTimeoutError::Disconnected)
        }
    }
}

fn validate_snapshot_cursor(requested: u64, latest_available: u64) -> Result<(), BrokerError> {
    if requested > latest_available {
        return Err(BrokerError::Cursor(CursorError::Ahead {
            next_available: latest_available,
        }));
    }
    // Snapshots are deliberately coalesced. Missing intermediate publication
    // sequences is not a retention gap because every poll returns the latest
    // complete role-filtered snapshot. Only retained noncoalescing streams
    // require ResyncRequired handling.
    Ok(())
}

fn handle_command<A: BrokerAuthority>(
    authority: &mut A,
    state: &mut WorkerState,
    command: WorkerCommand,
) {
    match command {
        WorkerCommand::Attach {
            client_id,
            session_nonce,
            role,
            cursors,
            reply,
        } => {
            let result = (|| {
                if session_nonce != authority.session_nonce() {
                    return Err(BrokerError::SessionNonce);
                }
                if role != authority.role() {
                    return Err(BrokerError::Role);
                }
                cursors.validate().map_err(BrokerError::Cursor)?;
                Ok(AttachReply {
                    controlling: state.attach(client_id, role)?,
                    session_nonce,
                    cursors: authority.cursors(),
                })
            })();
            let _ = reply.try_send(result);
        }
        WorkerCommand::Disconnect { client_id, reply } => {
            let _ = reply.try_send(state.disconnect(client_id));
        }
        WorkerCommand::Poll {
            client_id,
            cursors,
            limit,
            reply,
        } => {
            let result = (|| {
                state.require_attached(client_id)?;
                if limit == 0 || limit > MAX_POLL_RECORDS {
                    return Err(BrokerError::InvalidLimit);
                }
                cursors.validate().map_err(BrokerError::Cursor)?;
                validate_snapshot_cursor(cursors.snapshots, authority.cursors().snapshots)?;
                Ok(BrokerPublication {
                    snapshot: authority.latest_snapshot(),
                    procedure: authority.current_procedure(),
                    disposition: authority.current_disposition(),
                    prediction_paths: authority.current_prediction_paths(),
                    transport: authority.transport_status(),
                    evidence: authority.finalization_evidence(),
                    cursors: authority.cursors(),
                    events: authority
                        .read_events(cursors.events, limit)
                        .map_err(BrokerError::Cursor)?,
                    timeline: authority
                        .read_timeline(cursors.timeline, limit)
                        .map_err(BrokerError::Cursor)?,
                    action_receipts: authority
                        .read_action_receipts(cursors.action_receipts, limit)
                        .map_err(BrokerError::Cursor)?,
                    release_samples: authority
                        .read_release_samples(cursors.release_samples, limit)
                        .map_err(BrokerError::Cursor)?,
                })
            })();
            let _ = reply.try_send(result);
        }
        WorkerCommand::Advance {
            client_id,
            releases,
            reply,
        } => {
            let result = state.require_controller(client_id).and_then(|()| {
                if releases == 0 {
                    Err(BrokerError::InvalidLimit)
                } else {
                    authority
                        .advance_bounded(releases)
                        .map_err(BrokerError::Authority)
                }
            });
            let _ = reply.try_send(result);
        }
        WorkerCommand::Pace {
            client_id,
            pace,
            reply,
        } => {
            let result = state
                .require_controller(client_id)
                .and_then(|()| authority.set_pace(pace).map_err(BrokerError::Authority));
            let _ = reply.try_send(result);
        }
        WorkerCommand::Lifecycle {
            client_id,
            requested,
            reply,
        } => {
            let result = state.require_controller(client_id).and_then(|()| {
                authority
                    .request_lifecycle(requested)
                    .map_err(BrokerError::Authority)
            });
            let _ = reply.try_send(result);
        }
        WorkerCommand::Step { client_id, reply } => {
            let result = state
                .require_controller(client_id)
                .and_then(|()| authority.step_one_release().map_err(BrokerError::Authority));
            let _ = reply.try_send(result);
        }
        WorkerCommand::Action {
            client_id,
            intent,
            reply,
        } => {
            let result = state.require_controller(client_id).and_then(|()| {
                authority
                    .submit_action(intent)
                    .map_err(BrokerError::Authority)
            });
            let _ = reply.try_send(result);
        }
        WorkerCommand::Evidence { client_id, reply } => {
            let result = state
                .require_attached(client_id)
                .map(|()| authority.sealed_evidence());
            let _ = reply.try_send(result);
        }
        WorkerCommand::Shutdown => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_presentation::{
        NavigationView, OverallDisposition, PredictionSummaryView, PresentationQueueStatus,
        PresentationStaleness, RetainedStream, PRESENTATION_MODEL_ID, SNAPSHOT_VALID_PUBLIC_MASK,
    };
    use std::sync::{Arc, Mutex};

    struct MockAuthority {
        nonce: u64,
        role: PresentationRole,
        snapshot: OperationalSnapshot,
        advances: Arc<Mutex<u32>>,
        events: RetainedStream<PresentationEventView>,
        timeline: RetainedStream<TimelineEventView>,
        receipts: RetainedStream<ActionReceiptView>,
        samples: RetainedStream<ReleaseSampleView>,
    }

    impl MockAuthority {
        fn new(role: PresentationRole, pace: PresentationPace) -> (Self, Arc<Mutex<u32>>) {
            let advances = Arc::new(Mutex::new(0));
            (
                Self {
                    nonce: 99,
                    role,
                    snapshot: OperationalSnapshot {
                        presentation_model_identity: PRESENTATION_MODEL_ID,
                        session_definition_identity: 1,
                        publication_sequence: 1,
                        validity_mask: SNAPSHOT_VALID_PUBLIC_MASK & !SNAPSHOT_VALID_ACTION,
                        role,
                        lifecycle: PresentationLifecycle::Running,
                        pace,
                        release_epoch: 0,
                        release_period_micros: 1_000,
                        frame_identity: 1,
                        mission_time_q16: 0,
                        onboard: NavigationView::default(),
                        ground: NavigationView::default(),
                        prediction: PredictionSummaryView::default(),
                        flight_checksum: 1,
                        command_checksum: 2,
                        procedure_chain: 3,
                        journal_chain: 4,
                        action_chain: 5,
                        staged_load_identity: 0,
                        action_count: 0,
                        rejected_loads: 0,
                        gnss_state: 1,
                        safe: false,
                        truth: None,
                    },
                    advances: advances.clone(),
                    events: RetainedStream::new(4).unwrap(),
                    timeline: RetainedStream::new(4).unwrap(),
                    receipts: RetainedStream::new(4).unwrap(),
                    samples: RetainedStream::new(4).unwrap(),
                },
                advances,
            )
        }
    }

    impl PresentationSession for MockAuthority {
        type Error = AuthorityError;

        fn role(&self) -> PresentationRole {
            self.role
        }
        fn lifecycle(&self) -> PresentationLifecycle {
            self.snapshot.lifecycle
        }
        fn latest_snapshot(&self) -> OperationalSnapshot {
            self.snapshot.clone()
        }
        fn current_procedure(&self) -> Option<ProcedureView> {
            None
        }
        fn current_disposition(&self) -> Option<DispositionView> {
            Some(DispositionView {
                overall: OverallDisposition::DegradedSuccess,
                axes: Default::default(),
                reason_identity: 1,
            })
        }
        fn current_prediction_paths(&self) -> Vec<PredictionPathView> {
            Vec::new()
        }
        fn transport_status(&self) -> TransportStatusView {
            TransportStatusView {
                staleness: PresentationStaleness::Current,
                worker_state: 1,
                finalization_state: 0,
                queue: PresentationQueueStatus::default(),
                last_command_result: 0,
            }
        }
        fn finalization_evidence(&self) -> Option<SealedEvidenceMetadata> {
            None
        }
        fn cursors(&self) -> PresentationCursors {
            PresentationCursors {
                snapshots: 1,
                events: self.events.next_cursor(),
                timeline: self.timeline.next_cursor(),
                action_receipts: self.receipts.next_cursor(),
                release_samples: self.samples.next_cursor(),
            }
        }
        fn read_events(
            &self,
            cursor: u64,
            limit: usize,
        ) -> Result<PresentationBatch<PresentationEventView>, CursorError> {
            self.events.read(cursor, limit)
        }
        fn read_timeline(
            &self,
            cursor: u64,
            limit: usize,
        ) -> Result<PresentationBatch<TimelineEventView>, CursorError> {
            self.timeline.read(cursor, limit)
        }
        fn read_action_receipts(
            &self,
            cursor: u64,
            limit: usize,
        ) -> Result<PresentationBatch<ActionReceiptView>, CursorError> {
            self.receipts.read(cursor, limit)
        }
        fn read_release_samples(
            &self,
            cursor: u64,
            limit: usize,
        ) -> Result<PresentationBatch<ReleaseSampleView>, CursorError> {
            self.samples.read(cursor, limit)
        }
        fn submit_action(
            &mut self,
            intent: PresentationActionIntent,
        ) -> Result<ActionReceiptView, Self::Error> {
            intent
                .validate(self.role)
                .map_err(|_| AuthorityError { code: 7 })?;
            Ok(ActionReceiptView {
                publication_sequence: 1,
                proposal_identity: intent.proposal_identity,
                load_identity: intent.expected_load_identity,
                control_identity: 1,
                receipt_epoch: self.snapshot.release_epoch,
                effective_epoch: intent.requested_activation_epoch,
                state: 1,
                reason: 0,
                accepted: true,
                operation: intent.operation,
                receipt_checksum: 8,
            })
        }
    }

    impl BrokerAuthority for MockAuthority {
        fn session_nonce(&self) -> u64 {
            self.nonce
        }
        fn advance_bounded(&mut self, max_releases: u32) -> Result<u32, AuthorityError> {
            self.snapshot.release_epoch = self.snapshot.release_epoch.saturating_add(max_releases);
            *self.advances.lock().unwrap() += max_releases;
            Ok(max_releases)
        }
        fn set_pace(&mut self, pace: PresentationPace) -> Result<(), AuthorityError> {
            self.snapshot.pace = pace;
            Ok(())
        }
        fn request_lifecycle(
            &mut self,
            requested: PresentationLifecycle,
        ) -> Result<(), AuthorityError> {
            self.snapshot.lifecycle = requested;
            Ok(())
        }
        fn step_one_release(&mut self) -> Result<u32, AuthorityError> {
            self.advance_bounded(1)?;
            self.snapshot.lifecycle = PresentationLifecycle::Paused;
            self.snapshot.pace = PresentationPace::Paused;
            Ok(1)
        }
    }

    #[test]
    fn one_worker_enforces_nonce_role_controller_and_bounded_polling() {
        let (authority, _) =
            MockAuthority::new(PresentationRole::GuidedOperator, PresentationPace::Paused);
        let broker = SessionBrokerHandle::spawn(
            authority,
            WorkerConfig {
                autonomous_pacing: false,
                ..WorkerConfig::default()
            },
        )
        .unwrap();
        assert_eq!(
            broker.attach(
                1,
                98,
                PresentationRole::GuidedOperator,
                PresentationCursors::default()
            ),
            Err(BrokerError::SessionNonce)
        );
        assert_eq!(
            broker.attach(
                1,
                99,
                PresentationRole::Observer,
                PresentationCursors::default()
            ),
            Err(BrokerError::Role)
        );
        assert!(
            broker
                .attach(
                    1,
                    99,
                    PresentationRole::GuidedOperator,
                    PresentationCursors::default()
                )
                .unwrap()
                .controlling
        );
        assert!(
            !broker
                .attach(
                    2,
                    99,
                    PresentationRole::GuidedOperator,
                    PresentationCursors::default()
                )
                .unwrap()
                .controlling
        );
        assert_eq!(
            broker.attach(
                1,
                99,
                PresentationRole::GuidedOperator,
                PresentationCursors::default()
            ),
            Err(BrokerError::ClientAlreadyAttached)
        );
        assert_eq!(broker.advance(2, 1), Err(BrokerError::Controller));
        assert_eq!(broker.advance(1, 3), Ok(3));
        assert_eq!(broker.step_one_release(1), Ok(1));
        assert_eq!(
            broker.set_lifecycle(1, PresentationLifecycle::Running),
            Ok(())
        );
        assert!(broker.poll(2, PresentationCursors::default(), 4).is_ok());
        assert_eq!(
            broker.poll(2, PresentationCursors::default(), MAX_POLL_RECORDS + 1),
            Err(BrokerError::InvalidLimit)
        );
    }

    #[test]
    fn disconnect_does_not_pause_or_stop_authority_and_lease_cannot_be_hijacked() {
        let (authority, advances) =
            MockAuthority::new(PresentationRole::GuidedOperator, PresentationPace::Fast);
        let broker = SessionBrokerHandle::spawn(authority, WorkerConfig::default()).unwrap();
        broker
            .attach(
                7,
                99,
                PresentationRole::GuidedOperator,
                PresentationCursors::default(),
            )
            .unwrap();
        std::thread::sleep(Duration::from_millis(10));
        broker.disconnect(7).unwrap();
        let before = *advances.lock().unwrap();
        let deadline = std::time::Instant::now() + Duration::from_millis(500);
        let after = loop {
            let observed = *advances.lock().unwrap();
            if observed > before || std::time::Instant::now() >= deadline {
                break observed;
            }
            std::thread::yield_now();
        };
        assert!(after > before);
        assert!(
            !broker
                .attach(
                    8,
                    99,
                    PresentationRole::GuidedOperator,
                    PresentationCursors::default()
                )
                .unwrap()
                .controlling
        );
        assert_eq!(broker.advance(8, 1), Err(BrokerError::Controller));
        assert!(
            broker
                .attach(
                    7,
                    99,
                    PresentationRole::GuidedOperator,
                    PresentationCursors::default()
                )
                .unwrap()
                .controlling
        );
    }

    #[test]
    fn coalesced_snapshot_cursors_never_require_a_resync() {
        assert_eq!(validate_snapshot_cursor(1, 42), Ok(()));
        assert_eq!(validate_snapshot_cursor(41, 42), Ok(()));
        assert_eq!(validate_snapshot_cursor(42, 42), Ok(()));
        assert_eq!(
            validate_snapshot_cursor(43, 42),
            Err(BrokerError::Cursor(CursorError::Ahead {
                next_available: 42
            }))
        );
    }

    #[test]
    fn autonomous_fast_pacing_waits_at_an_operator_action_gate() {
        let (mut authority, advances) =
            MockAuthority::new(PresentationRole::GuidedOperator, PresentationPace::Fast);
        authority.snapshot.validity_mask |= SNAPSHOT_VALID_ACTION;
        let broker = SessionBrokerHandle::spawn(authority, WorkerConfig::default()).unwrap();
        broker
            .attach(
                1,
                99,
                PresentationRole::GuidedOperator,
                PresentationCursors::default(),
            )
            .unwrap();

        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(*advances.lock().unwrap(), 0);

        broker.set_pace(1, PresentationPace::Realtime).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_millis(100);
        while *advances.lock().unwrap() == 0 && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(*advances.lock().unwrap() > 0);
    }

    #[test]
    fn retention_gap_is_explicitly_reported() {
        let (mut authority, _) =
            MockAuthority::new(PresentationRole::Observer, PresentationPace::Paused);
        for index in 0..8 {
            authority
                .events
                .push(PresentationEventView {
                    sequence: 0,
                    release_epoch: index,
                    kind: 1,
                    detail_identity: 1,
                })
                .unwrap();
        }
        let broker = SessionBrokerHandle::spawn(
            authority,
            WorkerConfig {
                autonomous_pacing: false,
                ..WorkerConfig::default()
            },
        )
        .unwrap();
        broker
            .attach(
                1,
                99,
                PresentationRole::Observer,
                PresentationCursors::default(),
            )
            .unwrap();
        assert_eq!(
            broker.poll(1, PresentationCursors::default(), 4),
            Err(BrokerError::Cursor(CursorError::ResyncRequired {
                oldest_available: 5
            }))
        );
    }
}
