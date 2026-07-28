use core::fmt;

use ksa64_presentation::{parse_kps1_frame, DecodedKps1Frame, Kps1Error};

use crate::{BrowserServiceConfig, BROWSER_SUBPROTOCOL_PREFIX, BROWSER_TOKEN_BYTES};

const HEX: &[u8; 16] = b"0123456789abcdef";

#[derive(Clone, PartialEq, Eq)]
pub struct BrowserLaunchToken([u8; BROWSER_TOKEN_BYTES]);

impl BrowserLaunchToken {
    pub fn generate() -> Result<Self, BrowserAdmissionError> {
        let mut bytes = [0_u8; BROWSER_TOKEN_BYTES];
        getrandom::fill(&mut bytes).map_err(|_| BrowserAdmissionError::Entropy)?;
        Ok(Self(bytes))
    }

    pub const fn from_bytes(bytes: [u8; BROWSER_TOKEN_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn subprotocol(&self) -> String {
        let mut value = String::with_capacity(BROWSER_SUBPROTOCOL_PREFIX.len() + 64);
        value.push_str(BROWSER_SUBPROTOCOL_PREFIX);
        for byte in self.0 {
            value.push(char::from(HEX[usize::from(byte >> 4)]));
            value.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
        value
    }

    fn matches_subprotocol(&self, candidate: &str) -> bool {
        let expected = self.subprotocol();
        constant_time_equal(expected.as_bytes(), candidate.as_bytes())
    }
}

impl fmt::Debug for BrowserLaunchToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BrowserLaunchToken([REDACTED])")
    }
}

impl Drop for BrowserLaunchToken {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserAdmissionError {
    Entropy,
    OriginMissing,
    OriginRejected,
    QueryRejected,
    SubprotocolMissing,
    TokenRejected,
    ConnectionLimit,
    ConnectionUnknown,
    OutstandingLimit,
    RateLimited,
    TextMessage,
    Kps1(Kps1Error),
}

#[derive(Clone, Copy, Debug)]
pub struct BrowserHandshake<'a> {
    pub origin: Option<&'a str>,
    pub request_target_query: Option<&'a str>,
    pub offered_subprotocols: &'a [&'a str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BrowserAdmission {
    pub connection_id: u64,
    pub selected_subprotocol_index: usize,
}

#[derive(Clone, Debug)]
pub struct BrowserAdmissionController {
    config: BrowserServiceConfig,
    token: BrowserLaunchToken,
    next_connection_id: u64,
    active_connection_ids: Vec<u64>,
    outstanding_commands: u16,
    rate_window_started_millis: u64,
    messages_in_window: u16,
}

impl BrowserAdmissionController {
    pub fn new(
        config: BrowserServiceConfig,
        token: BrowserLaunchToken,
    ) -> Result<Self, crate::ConfigError> {
        config.validate()?;
        let connection_capacity = usize::from(config.max_connections);
        Ok(Self {
            config,
            token,
            next_connection_id: 1,
            active_connection_ids: Vec::with_capacity(connection_capacity),
            outstanding_commands: 0,
            rate_window_started_millis: 0,
            messages_in_window: 0,
        })
    }

    pub fn launch_subprotocol(&self) -> String {
        self.token.subprotocol()
    }

    pub fn admit(
        &mut self,
        request: BrowserHandshake<'_>,
    ) -> Result<BrowserAdmission, BrowserAdmissionError> {
        if self.active_connection_ids.len() >= usize::from(self.config.max_connections) {
            return Err(BrowserAdmissionError::ConnectionLimit);
        }
        let origin = request.origin.ok_or(BrowserAdmissionError::OriginMissing)?;
        if !self
            .config
            .allowed_origins
            .iter()
            .any(|allowed| allowed == origin)
        {
            return Err(BrowserAdmissionError::OriginRejected);
        }
        if request
            .request_target_query
            .is_some_and(|query| !query.is_empty())
        {
            return Err(BrowserAdmissionError::QueryRejected);
        }
        if request.offered_subprotocols.is_empty() {
            return Err(BrowserAdmissionError::SubprotocolMissing);
        }
        let selected_subprotocol_index = request
            .offered_subprotocols
            .iter()
            .position(|candidate| self.token.matches_subprotocol(candidate))
            .ok_or(BrowserAdmissionError::TokenRejected)?;
        let connection_id = self.next_connection_id;
        self.next_connection_id = self
            .next_connection_id
            .checked_add(1)
            .ok_or(BrowserAdmissionError::ConnectionLimit)?;
        self.active_connection_ids.push(connection_id);
        Ok(BrowserAdmission {
            connection_id,
            selected_subprotocol_index,
        })
    }

    pub fn disconnect(&mut self, connection_id: u64) -> Result<(), BrowserAdmissionError> {
        let Some(index) = self
            .active_connection_ids
            .iter()
            .position(|value| *value == connection_id)
        else {
            return Err(BrowserAdmissionError::ConnectionUnknown);
        };
        self.active_connection_ids.swap_remove(index);
        Ok(())
    }

    pub fn begin_command(&mut self, now_millis: u64) -> Result<(), BrowserAdmissionError> {
        self.account_message(now_millis)?;
        if self.outstanding_commands >= self.config.max_outstanding_commands {
            return Err(BrowserAdmissionError::OutstandingLimit);
        }
        self.outstanding_commands += 1;
        Ok(())
    }

    pub fn complete_command(&mut self) {
        self.outstanding_commands = self.outstanding_commands.saturating_sub(1);
    }

    pub fn accept_binary<'a>(
        &mut self,
        now_millis: u64,
        bytes: &'a [u8],
    ) -> Result<DecodedKps1Frame<'a>, BrowserAdmissionError> {
        self.account_message(now_millis)?;
        parse_kps1_frame(bytes).map_err(BrowserAdmissionError::Kps1)
    }

    pub const fn reject_text(&self) -> Result<(), BrowserAdmissionError> {
        Err(BrowserAdmissionError::TextMessage)
    }

    pub fn active_connections(&self) -> u16 {
        self.active_connection_ids.len() as u16
    }

    pub const fn outstanding_commands(&self) -> u16 {
        self.outstanding_commands
    }

    fn account_message(&mut self, now_millis: u64) -> Result<(), BrowserAdmissionError> {
        if now_millis < self.rate_window_started_millis {
            return Err(BrowserAdmissionError::RateLimited);
        }
        if now_millis.saturating_sub(self.rate_window_started_millis)
            >= self.config.rate_window_millis
        {
            self.rate_window_started_millis = now_millis;
            self.messages_in_window = 0;
        }
        if self.messages_in_window >= self.config.max_messages_per_window {
            return Err(BrowserAdmissionError::RateLimited);
        }
        self.messages_in_window += 1;
        Ok(())
    }
}

fn constant_time_equal(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let common = left.len().min(right.len());
    for index in 0..common {
        difference |= usize::from(left[index] ^ right[index]);
    }
    for byte in &left[common..] {
        difference |= usize::from(*byte);
    }
    for byte in &right[common..] {
        difference |= usize::from(*byte);
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ksa64_presentation::{
        write_kps1_frame, Kps1Header, PresentationMessageKind, KPS1_FLAG_COALESCED,
        KPS1_HEADER_LENGTH,
    };
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn controller() -> BrowserAdmissionController {
        let mut config = BrowserServiceConfig::loopback(
            8080,
            ["http://127.0.0.1:4173".to_owned()],
            ksa64_presentation::PresentationRole::GuidedOperator,
        )
        .unwrap();
        config.bind = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
        config.max_connections = 1;
        config.max_messages_per_window = 2;
        config.max_outstanding_commands = 1;
        BrowserAdmissionController::new(config, BrowserLaunchToken::from_bytes([0xabu8; 32]))
            .unwrap()
    }

    #[test]
    fn launch_token_never_appears_in_debug_and_must_arrive_as_subprotocol() {
        let mut controller = controller();
        let selected = controller.launch_subprotocol();
        assert!(!format!("{:?}", controller.token).contains("abab"));
        assert_eq!(
            controller.admit(BrowserHandshake {
                origin: Some("http://127.0.0.1:4173"),
                request_target_query: Some("token=secret"),
                offered_subprotocols: &[&selected],
            }),
            Err(BrowserAdmissionError::QueryRejected)
        );
        let admitted = controller
            .admit(BrowserHandshake {
                origin: Some("http://127.0.0.1:4173"),
                request_target_query: None,
                offered_subprotocols: &["other", &selected],
            })
            .unwrap();
        assert_eq!(admitted.selected_subprotocol_index, 1);
        assert_eq!(controller.active_connections(), 1);
        assert_eq!(
            controller.admit(BrowserHandshake {
                origin: Some("http://127.0.0.1:4173"),
                request_target_query: None,
                offered_subprotocols: &[&selected],
            }),
            Err(BrowserAdmissionError::ConnectionLimit)
        );
    }

    #[test]
    fn exact_origin_binary_framing_rate_and_outstanding_limits_fail_closed() {
        let mut controller = controller();
        let selected = controller.launch_subprotocol();
        assert_eq!(
            controller.admit(BrowserHandshake {
                origin: Some("http://localhost:4173"),
                request_target_query: None,
                offered_subprotocols: &[&selected],
            }),
            Err(BrowserAdmissionError::OriginRejected)
        );
        assert_eq!(
            controller.reject_text(),
            Err(BrowserAdmissionError::TextMessage)
        );

        let header = Kps1Header {
            kind: PresentationMessageKind::Snapshot,
            flags: KPS1_FLAG_COALESCED,
            session_nonce: 7,
            sequence: 1,
            correlation_id: 0,
            payload_length: 0,
        };
        let mut frame = vec![0u8; KPS1_HEADER_LENGTH];
        write_kps1_frame(header, &[], &mut frame).unwrap();
        assert!(controller.accept_binary(10, &frame).is_ok());
        assert_eq!(
            controller.accept_binary(11, b"not kps1"),
            Err(BrowserAdmissionError::Kps1(Kps1Error::Length))
        );
        assert_eq!(
            controller.accept_binary(12, &frame),
            Err(BrowserAdmissionError::RateLimited)
        );
        controller.begin_command(1_100).unwrap();
        assert_eq!(
            controller.begin_command(1_101),
            Err(BrowserAdmissionError::OutstandingLimit)
        );
        controller.complete_command();
        assert_eq!(controller.outstanding_commands(), 0);
    }
}
