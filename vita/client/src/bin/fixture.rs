//! Desktop fixture harness for the Vita view model.
//!
//! This intentionally has no SDL2 or VitaSDK dependency. It validates the
//! platform-independent state machine before the Vita platform shell exists.

use ksa64_presentation::{
    encode_typed_payload, write_kps1_frame, ActionProposalView, Kps1Header, PresentationCursors,
    PresentationHandshake, PresentationMessageKind, PresentationPayload, PresentationRole,
    ACTION_PERMIT_CANCEL, ACTION_PERMIT_COMMIT, ACTION_PERMIT_REVIEW, ACTION_PERMIT_STAGE,
    KPS1_FLAG_RESPONSE, KPS1_HEADER_LENGTH,
};
use ksa64_vita_client::{VitaInput, VitaMissionControl};

fn main() {
    let role = PresentationRole::GuidedOperator;
    let mut client = VitaMissionControl::new(role).expect("client");
    let handshake = PresentationHandshake {
        role,
        client_instance: 0x5649_5441,
        capability_mask: 0,
        cursors: PresentationCursors::default(),
    };
    send(
        &mut client,
        PresentationMessageKind::HandshakeResponse,
        PresentationPayload::HandshakeResponse(handshake),
        1,
    );
    send(
        &mut client,
        PresentationMessageKind::ActionProposal,
        PresentationPayload::ActionProposal(ActionProposalView {
            proposal_identity: 0xA11C_E001,
            load_identity: 0xA11C_E002,
            load_type: 1,
            permitted_operations: ACTION_PERMIT_REVIEW
                | ACTION_PERMIT_STAGE
                | ACTION_PERMIT_COMMIT
                | ACTION_PERMIT_CANCEL,
            stage_epoch: 4,
            earliest_commit_epoch: 6,
            activation_epoch: 8,
            expires_epoch: 32,
            payload_checksum: 0xC0DE_0001,
            completed_event_mask: 0,
            label: String::from("GROUND NAVIGATION UPDATE"),
        }),
        2,
    );

    let review = client
        .handle_input(VitaInput::Cross)
        .expect("review")
        .expect("intent");
    let stage = client
        .handle_input(VitaInput::Cross)
        .expect("stage")
        .expect("intent");
    let commit = client
        .handle_input(VitaInput::Cross)
        .expect("commit")
        .expect("intent");
    let mut bytes = [0_u8; 256];
    let encoded = client
        .encode_action_intent(commit, &mut bytes)
        .expect("encoded intent");

    println!("KSA64 Vita feasibility fixture");
    println!(
        "  display: {}x{} at {} fps target",
        ksa64_vita_client::VITA_WIDTH,
        ksa64_vita_client::VITA_HEIGHT,
        ksa64_vita_client::VITA_FRAME_RATE_TARGET
    );
    println!("  connection: {:?}", client.connection());
    println!(
        "  actions: {:?} -> {:?} -> {:?}",
        review.operation, stage.operation, commit.operation
    );
    println!("  KPS1 action bytes: {encoded}");
    println!(
        "  budget: {} / {} bytes",
        client.memory_budget().total_bytes,
        ksa64_vita_client::VITA_WORKING_SET_LIMIT_BYTES
    );
    println!("  physical Vita and Vita3K acceptance remain pending");
}

fn send(
    client: &mut VitaMissionControl,
    kind: PresentationMessageKind,
    message: PresentationPayload,
    sequence: u64,
) {
    let role = client.role();
    let payload = encode_typed_payload(&message, role).expect("payload");
    let header = Kps1Header {
        kind,
        flags: KPS1_FLAG_RESPONSE,
        session_nonce: 0xAABB_CCDD_EEFF_0011,
        sequence,
        correlation_id: if matches!(kind, PresentationMessageKind::HandshakeResponse) {
            1
        } else {
            0
        },
        payload_length: payload.len() as u32,
    };
    let mut bytes = vec![0_u8; KPS1_HEADER_LENGTH + payload.len()];
    write_kps1_frame(header, &payload, &mut bytes).expect("frame");
    client.receive_kps1(&bytes).expect("receive");
}
