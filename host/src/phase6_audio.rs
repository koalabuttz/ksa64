//! Optional procedural Mission Control audio. No sampled or copyrighted assets.
use crate::phase6_tui::SoundProfile;
#[cfg(feature = "mission-control-audio")]
use rodio::source::SineWave;
#[cfg(feature = "mission-control-audio")]
use rodio::{DeviceSinkBuilder, MixerDeviceSink, Source};
use std::io::{self, Write};
#[cfg(feature = "mission-control-audio")]
use std::time::Duration;

pub struct AudioEngine {
    profile: SoundProfile,
    #[cfg(feature = "mission-control-audio")]
    stream: Option<MixerDeviceSink>,
}
impl AudioEngine {
    pub fn new(profile: SoundProfile) -> Self {
        #[cfg(feature = "mission-control-audio")]
        {
            if let Ok(mut stream) = DeviceSinkBuilder::open_default_sink() {
                stream.log_on_drop(false);
                return Self {
                    profile,
                    stream: Some(stream),
                };
            }
            Self {
                profile,
                stream: None,
            }
        }
        #[cfg(not(feature = "mission-control-audio"))]
        {
            Self { profile }
        }
    }
    pub fn set_profile(&mut self, profile: SoundProfile) {
        self.profile = profile
    }
    pub fn cue(&self, events: u16) {
        if self.profile == SoundProfile::Off || events == 0 {
            return;
        }
        #[cfg(feature = "mission-control-audio")]
        if let Some(stream) = self.stream.as_ref() {
            let (hz, ms) = if events & 4 != 0 {
                (880.0, 180)
            } else if events & 2 != 0 {
                (520.0, 260)
            } else {
                (720.0, 120)
            };
            stream.mixer().add(
                SineWave::new(hz)
                    .take_duration(Duration::from_millis(ms))
                    .amplify(0.16),
            );
            if self.profile == SoundProfile::Cinematic {
                stream.mixer().add(
                    SineWave::new(hz / 2.0)
                        .take_duration(Duration::from_millis(ms * 2))
                        .amplify(0.08),
                );
            }
            return;
        }
        let _ = io::stderr().write_all(b"\x07");
        let _ = io::stderr().flush();
    }
    pub fn completion(&self, _success: bool) {
        if self.profile == SoundProfile::Off {
            return;
        }
        #[cfg(feature = "mission-control-audio")]
        if let Some(stream) = self.stream.as_ref() {
            let notes: &[f32] = if _success {
                &[523.25, 659.25, 783.99]
            } else {
                &[330.0, 277.18, 220.0]
            };
            for hz in notes {
                stream.mixer().add(
                    SineWave::new(*hz)
                        .take_duration(Duration::from_millis(150))
                        .amplify(0.15),
                );
            }
            return;
        }
        let _ = io::stderr().write_all(b"\x07\x07");
        let _ = io::stderr().flush();
    }
}
