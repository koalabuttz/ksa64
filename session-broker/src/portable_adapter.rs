use ksa64_presentation::{
    ActionReceiptView, CursorError, DispositionView, OperationalSnapshot, PredictionPathView,
    PresentationActionIntent, PresentationBatch, PresentationCursors, PresentationEventView,
    PresentationLifecycle, PresentationPace, PresentationRole, PresentationSession, ProcedureView,
    ReleaseSampleView, SealedEvidenceMetadata, TimelineEventView, TransportStatusView,
    KPS1_EVIDENCE_OBJECT_MAX_LENGTH,
};
use ksa64_session::presentation_adapter::{
    FullMissionPresentationSession, PresentationSessionError,
};

use crate::{AuthorityError, BrokerAuthority};

/// Concrete broker adapter around the accepted portable full-mission session.
///
/// The broker nonce is per-launch transport identity and never changes the
/// underlying scenario, release order, action transcript, or KSB11 bytes.
pub struct PortableFullMissionAuthority {
    session_nonce: u64,
    inner: FullMissionPresentationSession,
}

impl PortableFullMissionAuthority {
    pub fn from_session(
        session_nonce: u64,
        inner: FullMissionPresentationSession,
    ) -> Result<Self, AuthorityError> {
        if session_nonce == 0 {
            return Err(AuthorityError { code: 0x1000_0001 });
        }
        Ok(Self {
            session_nonce,
            inner,
        })
    }

    pub fn prepare(&mut self) -> Result<(), AuthorityError> {
        self.inner.prepare().map_err(map_session_error)
    }

    pub const fn session_nonce(&self) -> u64 {
        self.session_nonce
    }

    pub const fn inner(&self) -> &FullMissionPresentationSession {
        &self.inner
    }

    pub fn into_inner(self) -> FullMissionPresentationSession {
        self.inner
    }
}

impl PresentationSession for PortableFullMissionAuthority {
    type Error = AuthorityError;

    fn role(&self) -> PresentationRole {
        self.inner.role()
    }

    fn lifecycle(&self) -> PresentationLifecycle {
        self.inner.lifecycle()
    }

    fn latest_snapshot(&self) -> OperationalSnapshot {
        self.inner.latest_snapshot()
    }

    fn current_procedure(&self) -> Option<ProcedureView> {
        self.inner.current_procedure()
    }

    fn current_disposition(&self) -> Option<DispositionView> {
        self.inner.current_disposition()
    }

    fn current_prediction_paths(&self) -> Vec<PredictionPathView> {
        self.inner.current_prediction_paths()
    }

    fn transport_status(&self) -> TransportStatusView {
        self.inner.transport_status()
    }

    fn finalization_evidence(&self) -> Option<SealedEvidenceMetadata> {
        self.inner.finalization_evidence()
    }

    fn cursors(&self) -> PresentationCursors {
        self.inner.cursors()
    }

    fn read_events(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<PresentationBatch<PresentationEventView>, CursorError> {
        self.inner.read_events(cursor, limit)
    }

    fn read_timeline(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<PresentationBatch<TimelineEventView>, CursorError> {
        self.inner.read_timeline(cursor, limit)
    }

    fn read_action_receipts(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<PresentationBatch<ActionReceiptView>, CursorError> {
        self.inner.read_action_receipts(cursor, limit)
    }

    fn read_release_samples(
        &self,
        cursor: u64,
        limit: usize,
    ) -> Result<PresentationBatch<ReleaseSampleView>, CursorError> {
        self.inner.read_release_samples(cursor, limit)
    }

    fn submit_action(
        &mut self,
        intent: PresentationActionIntent,
    ) -> Result<ActionReceiptView, Self::Error> {
        self.inner.submit_action(intent).map_err(map_session_error)
    }
}

impl BrokerAuthority for PortableFullMissionAuthority {
    fn session_nonce(&self) -> u64 {
        self.session_nonce
    }

    fn advance_bounded(&mut self, max_releases: u32) -> Result<u32, AuthorityError> {
        self.inner
            .advance_bounded(max_releases)
            .map_err(map_session_error)
    }

    fn set_pace(&mut self, pace: PresentationPace) -> Result<(), AuthorityError> {
        self.inner.set_pace(pace).map_err(map_session_error)
    }

    fn request_lifecycle(
        &mut self,
        requested: PresentationLifecycle,
    ) -> Result<(), AuthorityError> {
        let current = self.inner.lifecycle();
        match (current, requested) {
            (value, target) if value == target => Ok(()),
            (PresentationLifecycle::Compiled, PresentationLifecycle::Ready) => {
                self.inner.prepare().map_err(map_session_error)
            }
            (PresentationLifecycle::Ready, PresentationLifecycle::Running) => {
                self.inner.advance_one_release().map_err(map_session_error)
            }
            (PresentationLifecycle::Paused, PresentationLifecycle::Running) => {
                self.inner.resume().map_err(map_session_error)
            }
            (
                PresentationLifecycle::Ready | PresentationLifecycle::Running,
                PresentationLifecycle::Paused,
            ) => self.inner.pause().map_err(map_session_error),
            _ => Err(AuthorityError { code: 0x1205_0006 }),
        }
    }

    fn step_one_release(&mut self) -> Result<u32, AuthorityError> {
        let before = self.inner.latest_snapshot().release_epoch;
        self.inner.step_one_release().map_err(map_session_error)?;
        Ok(self
            .inner
            .latest_snapshot()
            .release_epoch
            .saturating_sub(before))
    }

    fn sealed_evidence(&self) -> Option<Vec<u8>> {
        self.inner
            .sealed_evidence_bytes()
            .filter(|bytes| bytes.len() as u64 <= KPS1_EVIDENCE_OBJECT_MAX_LENGTH)
            .map(<[u8]>::to_vec)
    }
}

fn map_session_error(error: PresentationSessionError) -> AuthorityError {
    let detail = match error {
        PresentationSessionError::Authority(_) => 1,
        PresentationSessionError::Intent(_) => 2,
        PresentationSessionError::ActionSequence => 3,
        PresentationSessionError::Proposal => 4,
        PresentationSessionError::Retention(_) => 5,
    };
    AuthorityError {
        code: 0x1205_0000 | detail,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_interface::phase11::OperationalRole;

    #[test]
    fn broker_nonce_is_transport_only_and_prepare_uses_the_portable_authority() {
        let inner = FullMissionPresentationSession::new(OperationalRole::GuidedOperator).unwrap();
        let definition_before = inner.latest_snapshot().session_definition_identity;
        let mut broker = PortableFullMissionAuthority::from_session(0x1122_3344, inner).unwrap();
        broker.prepare().unwrap();
        assert_eq!(broker.session_nonce(), 0x1122_3344);
        assert_eq!(
            broker.latest_snapshot().session_definition_identity,
            definition_before
        );
        assert_eq!(broker.role(), PresentationRole::GuidedOperator);
        broker
            .request_lifecycle(PresentationLifecycle::Running)
            .unwrap();
        assert_eq!(broker.latest_snapshot().release_epoch, 1);
        broker
            .request_lifecycle(PresentationLifecycle::Paused)
            .unwrap();
        assert_eq!(broker.step_one_release().unwrap(), 1);
        assert_eq!(broker.lifecycle(), PresentationLifecycle::Paused);
    }
}
