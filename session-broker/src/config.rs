use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use ksa64_presentation::PresentationRole;

pub const BROWSER_TOKEN_BYTES: usize = 32;
pub const BROWSER_SUBPROTOCOL_PREFIX: &str = "ksa64.presentation.v1.token.";
pub const DEFAULT_BROWSER_MAX_CONNECTIONS: u16 = 4;
pub const DEFAULT_MAX_OUTSTANDING_COMMANDS: u16 = 32;
pub const DEFAULT_MAX_MESSAGES_PER_WINDOW: u16 = 128;
pub const DEFAULT_RATE_WINDOW_MILLIS: u64 = 1_000;
pub const DEFAULT_PAIRING_TIMEOUT_MILLIS: u64 = 120_000;
pub const DEFAULT_PAIRING_ATTEMPTS: u8 = 5;
pub const MAX_ALLOWED_ORIGINS: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    UnspecifiedBind,
    NonLoopbackBind,
    LoopbackLanBind,
    MulticastBind,
    NonLanBind,
    EmptyOrigins,
    TooManyOrigins,
    InvalidOrigin,
    DuplicateOrigin,
    InvalidBound,
    InvalidRole,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrowserServiceConfig {
    pub bind: SocketAddr,
    pub allowed_origins: Vec<String>,
    pub role: PresentationRole,
    pub max_connections: u16,
    pub max_outstanding_commands: u16,
    pub max_messages_per_window: u16,
    pub rate_window_millis: u64,
}

impl BrowserServiceConfig {
    pub fn loopback(
        port: u16,
        allowed_origins: impl IntoIterator<Item = String>,
        role: PresentationRole,
    ) -> Result<Self, ConfigError> {
        let value = Self {
            bind: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port),
            allowed_origins: allowed_origins.into_iter().collect(),
            role,
            max_connections: DEFAULT_BROWSER_MAX_CONNECTIONS,
            max_outstanding_commands: DEFAULT_MAX_OUTSTANDING_COMMANDS,
            max_messages_per_window: DEFAULT_MAX_MESSAGES_PER_WINDOW,
            rate_window_millis: DEFAULT_RATE_WINDOW_MILLIS,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.bind.ip().is_unspecified() {
            return Err(ConfigError::UnspecifiedBind);
        }
        if !self.bind.ip().is_loopback() {
            return Err(ConfigError::NonLoopbackBind);
        }
        validate_origins(&self.allowed_origins)?;
        if self.max_connections == 0
            || self.max_outstanding_commands == 0
            || self.max_messages_per_window == 0
            || self.rate_window_millis == 0
        {
            return Err(ConfigError::InvalidBound);
        }
        if matches!(self.role, PresentationRole::ScriptedOperator) {
            return Err(ConfigError::InvalidRole);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PairedLanConfig {
    pub bind: SocketAddr,
    pub assigned_role: PresentationRole,
    pub pairing_timeout_millis: u64,
    pub max_pairing_attempts: u8,
    pub max_connections: u8,
}

impl PairedLanConfig {
    pub fn selected_interface(
        bind: SocketAddr,
        assigned_role: PresentationRole,
    ) -> Result<Self, ConfigError> {
        let value = Self {
            bind,
            assigned_role,
            pairing_timeout_millis: DEFAULT_PAIRING_TIMEOUT_MILLIS,
            max_pairing_attempts: DEFAULT_PAIRING_ATTEMPTS,
            max_connections: 1,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(self) -> Result<(), ConfigError> {
        if self.bind.ip().is_unspecified() {
            return Err(ConfigError::UnspecifiedBind);
        }
        if self.bind.ip().is_loopback() {
            return Err(ConfigError::LoopbackLanBind);
        }
        if is_multicast(self.bind.ip()) {
            return Err(ConfigError::MulticastBind);
        }
        if !is_private_lan(self.bind.ip()) {
            return Err(ConfigError::NonLanBind);
        }
        if self.pairing_timeout_millis == 0
            || self.max_pairing_attempts == 0
            || self.max_connections != 1
        {
            return Err(ConfigError::InvalidBound);
        }
        if matches!(self.assigned_role, PresentationRole::ScriptedOperator) {
            return Err(ConfigError::InvalidRole);
        }
        Ok(())
    }
}

fn validate_origins(origins: &[String]) -> Result<(), ConfigError> {
    if origins.is_empty() {
        return Err(ConfigError::EmptyOrigins);
    }
    if origins.len() > MAX_ALLOWED_ORIGINS {
        return Err(ConfigError::TooManyOrigins);
    }
    for (index, origin) in origins.iter().enumerate() {
        if !valid_exact_origin(origin) {
            return Err(ConfigError::InvalidOrigin);
        }
        if origins[..index].iter().any(|prior| prior == origin) {
            return Err(ConfigError::DuplicateOrigin);
        }
    }
    Ok(())
}

fn valid_exact_origin(origin: &str) -> bool {
    if origin.is_empty()
        || origin.contains('*')
        || origin.contains('?')
        || origin.contains('#')
        || origin.ends_with('/')
        || origin.chars().any(char::is_whitespace)
    {
        return false;
    }
    let Some(rest) = origin
        .strip_prefix("http://")
        .or_else(|| origin.strip_prefix("https://"))
    else {
        return false;
    };
    !rest.is_empty() && !rest.contains('/') && !rest.contains('@')
}

fn is_private_lan(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(value) => value.is_private() || value.is_link_local(),
        IpAddr::V6(value) => value.is_unique_local() || value.is_unicast_link_local(),
    }
}

fn is_multicast(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(value) => value.is_multicast(),
        IpAddr::V6(value) => value.is_multicast(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_service_is_loopback_only_and_exact_origin_only() {
        let valid = BrowserServiceConfig::loopback(
            8080,
            ["http://127.0.0.1:8080".to_owned()],
            PresentationRole::GuidedOperator,
        )
        .unwrap();
        assert_eq!(valid.validate(), Ok(()));

        let mut invalid = valid.clone();
        invalid.bind = "0.0.0.0:8080".parse().unwrap();
        assert_eq!(invalid.validate(), Err(ConfigError::UnspecifiedBind));
        invalid.bind = "192.168.1.2:8080".parse().unwrap();
        assert_eq!(invalid.validate(), Err(ConfigError::NonLoopbackBind));
        invalid.bind = valid.bind;
        invalid.allowed_origins = vec!["http://*".to_owned()];
        assert_eq!(invalid.validate(), Err(ConfigError::InvalidOrigin));
    }

    #[test]
    fn paired_lan_requires_one_explicit_unicast_interface() {
        assert!(PairedLanConfig::selected_interface(
            "192.168.1.4:27864".parse().unwrap(),
            PresentationRole::FlightController,
        )
        .is_ok());
        assert_eq!(
            PairedLanConfig::selected_interface(
                "0.0.0.0:27864".parse().unwrap(),
                PresentationRole::FlightController,
            ),
            Err(ConfigError::UnspecifiedBind)
        );
        assert_eq!(
            PairedLanConfig::selected_interface(
                "8.8.8.8:27864".parse().unwrap(),
                PresentationRole::FlightController,
            ),
            Err(ConfigError::NonLanBind)
        );
        assert_eq!(
            PairedLanConfig::selected_interface(
                SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::LOCALHOST), 27864),
                PresentationRole::FlightController,
            ),
            Err(ConfigError::LoopbackLanBind)
        );
    }
}
