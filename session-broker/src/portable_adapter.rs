use ksa64_presentation::{
    ActionReceiptView, CursorError, DispositionView, GlobalDisplayCursorStateV1,
    GlobalDisplayFrameId, GlobalDisplayPathLod, GlobalDisplayRangeRequestV1, GlobalDisplaySourceId,
    OperationalSnapshot, PredictionPathView, PresentationActionIntent, PresentationBatch,
    PresentationCursors, PresentationEventView, PresentationLifecycle, PresentationPace,
    PresentationRole, PresentationSession, ProcedureView, ReleaseSampleView,
    SealedEvidenceMetadata, TimelineEventView, TransportStatusView,
    KPS1_EVIDENCE_OBJECT_MAX_LENGTH,
};
use ksa64_session::presentation_adapter::{
    FullMissionPresentationSession, PresentationSessionError,
};

use crate::{AuthorityError, BrokerAuthority, GlobalDisplayPublication};

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

    fn global_display(
        &self,
        request: GlobalDisplayRangeRequestV1,
    ) -> Option<GlobalDisplayPublication> {
        let authority = self.inner.authority();
        let definition = authority.global_display_definition();
        let samples = authority.global_display_samples_from_release(
            request.start_release,
            usize::from(request.max_count),
        );
        let transitions = authority
            .global_display_transitions_from_release(request.start_release)
            .to_vec();
        let replay_index = authority.global_display_replay_index();
        let mut paths = Vec::new();
        let completed = self.lifecycle() == PresentationLifecycle::Completed;
        let newest_sample_release = authority.global_display_newest_sample_release();
        let includes_terminal = completed
            && samples
                .last()
                .is_some_and(|sample| newest_sample_release == Some(sample.release_epoch));
        // Exact samples are the bounded live trail delta. Cumulative paths are
        // sent once for the planned/initial state and once with the terminal
        // sample, never rebuilt and resent every 32 releases.
        if request.start_release == 0 || includes_terminal {
            let sources = [
                GlobalDisplaySourceId::Planned,
                GlobalDisplaySourceId::OnboardEstimate,
                GlobalDisplaySourceId::GroundEstimate,
                GlobalDisplaySourceId::SimTruth,
            ];
            let frames = [
                GlobalDisplayFrameId::LocalEnu,
                GlobalDisplayFrameId::EarthFixedEcef,
                GlobalDisplayFrameId::EarthInertialGcrf,
            ];
            for source in sources {
                for frame in frames {
                    for lod in [
                        GlobalDisplayPathLod::OneSecond,
                        GlobalDisplayPathLod::FourSecond,
                    ] {
                        let mut chunk_index = 0_u16;
                        while let Ok(chunk) =
                            authority.global_display_path_chunk(source, frame, lod, chunk_index)
                        {
                            let chunk_count = chunk.chunk_count;
                            paths.push(chunk);
                            chunk_index = chunk_index.saturating_add(1);
                            if chunk_index >= chunk_count {
                                break;
                            }
                        }
                    }
                }
            }
        }
        let cursor = GlobalDisplayCursorStateV1 {
            sample_count: u32::try_from(authority.global_display_sample_count())
                .unwrap_or(u32::MAX),
            oldest_sample_release: authority
                .global_display_oldest_sample_release()
                .unwrap_or(0),
            newest_sample_release: newest_sample_release.unwrap_or(0),
            transition_count: u32::try_from(authority.global_display_transition_count())
                .unwrap_or(u32::MAX),
            path_generation: definition.display_identity,
            replay_generation: replay_index.index_identity,
            resync_mask: 0,
        };
        Some(GlobalDisplayPublication {
            definition,
            samples,
            paths,
            transitions,
            replay_index,
            cursor,
        })
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

    #[test]
    fn global_display_ranges_are_role_filtered_and_reconnectable() {
        let inner = FullMissionPresentationSession::new(OperationalRole::GuidedOperator).unwrap();
        let mut authority = PortableFullMissionAuthority::from_session(0x5566, inner).unwrap();
        authority.prepare().unwrap();
        authority.advance_bounded(1).unwrap();
        let first = authority
            .global_display(GlobalDisplayRangeRequestV1 {
                start_release: 0,
                max_count: 1,
            })
            .unwrap();
        assert_eq!(first.samples.len(), 1);
        assert!(first.samples[0]
            .sources
            .iter()
            .all(|pose| pose.source != GlobalDisplaySourceId::SimTruth));
        assert!(!first.paths.is_empty());
        assert_eq!(first.cursor.sample_count, 1);
        authority.advance_bounded(39).unwrap();
        let resumed = authority
            .global_display(GlobalDisplayRangeRequestV1 {
                start_release: 32,
                max_count: 4,
            })
            .unwrap();
        assert_eq!(resumed.samples.len(), 4);
        assert_eq!(resumed.samples[0].release_epoch, 32);
        assert!(resumed.paths.is_empty());
        assert!(resumed.cursor.newest_sample_release >= 39);
        assert_eq!(resumed.cursor.sample_count, 40);
        let tail = authority
            .global_display(GlobalDisplayRangeRequestV1 {
                start_release: resumed.cursor.newest_sample_release.saturating_add(1),
                max_count: 4,
            })
            .unwrap();
        assert!(tail.samples.is_empty());
        assert!(tail.paths.is_empty());
        assert_eq!(tail.cursor, resumed.cursor);
    }
}
