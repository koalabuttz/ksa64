use ksa64_host::phase6_runner::{
    run_native_host_mission_controlled, MissionControlSink, MissionControlUpdate, PaceController,
    RunnerEvidence, RunnerOptions, RunnerPace,
};
use ksa64_host::phase6_session::{RecordingSink, Session};
use ksa64_host::phase6_tui::{render_update_text, render_updates_text, PlotStyle, TrajectoryView};
use std::fs;
use std::path::PathBuf;
const PAGES_FOR_TEST: [&str; 7] = [
    "FLIGHT DIRECTOR",
    "TRAJECTORY",
    "GUIDANCE",
    "NAVIGATION",
    "VEHICLE",
    "NETWORK",
    "SIM DIRECTOR",
];
fn path(name: &str) -> PathBuf {
    PathBuf::from(format!("target/phase6/test-{}-{name}", std::process::id()))
}
#[test]
fn live_session_recovers_exports_and_renders_all_consoles() {
    let kmr = path("live.kmr6");
    let csv = path("live.csv");
    let json = path("live.json");
    let truncated = path("truncated.kmr6");
    let options = RunnerOptions {
        mission_control: true,
        pace: RunnerPace::Fast,
    };
    let control = PaceController::new(options.pace);
    let mut sink = RecordingSink::create(&kmr).unwrap();
    let evidence = run_native_host_mission_controlled(options, Some(&mut sink), &control).unwrap();
    sink.check().unwrap();
    assert!(evidence.complete);
    let session = Session::load(&kmr).unwrap();
    assert!(session.complete);
    assert!(!session.recovered);
    assert_eq!(session.updates.len(), evidence.fast_epochs as usize);
    assert_eq!(
        session.evidence.as_ref().unwrap().final_flight_checksum,
        evidence.final_flight_checksum
    );
    session.export_csv(&csv).unwrap();
    session.export_json(&json).unwrap();
    assert!(fs::metadata(&csv).unwrap().len() > 100_000);
    assert!(fs::metadata(&json).unwrap().len() > 100_000);
    let first = session.updates[0];
    for page in 0..7 {
        let text = render_update_text(first, 120, 40, page).unwrap();
        assert!(text.contains("KSA64 // MISSION CONTROL"));
        assert!(
            text.contains(match page {
                0 => "FLIGHT DIRECTOR",
                1 => "TRAJECTORY",
                2 => "GUIDANCE",
                3 => "NAVIGATION",
                4 => "VEHICLE",
                5 => "NETWORK",
                _ => "SIM DIRECTOR",
            }),
            "missing page {page}"
        );
    }
    let compact = render_update_text(first, 80, 24, 0).unwrap();
    assert!(compact.contains("FLIGHT"));

    let mut sampled = session
        .updates
        .iter()
        .step_by(32)
        .copied()
        .collect::<Vec<_>>();
    if sampled.last().map(|v| v.epoch) != session.updates.last().map(|v| v.epoch) {
        sampled.push(*session.updates.last().unwrap());
    }
    let sizes = [(80, 24), (100, 30), (120, 40), (160, 48), (200, 60)];
    for &(width, height) in &sizes {
        for view in [
            TrajectoryView::Ascent,
            TrajectoryView::Orbit,
            TrajectoryView::GroundTrack,
        ] {
            for style in [PlotStyle::Braille, PlotStyle::Ascii] {
                let text = render_updates_text(&sampled, width, height, 1, view, style).unwrap();
                assert!(text.contains("F2 TRAJECTORY"));
                assert!(text.contains(view.label()));
                assert!(!text.contains('\u{fffd}'));
                if style == PlotStyle::Ascii {
                    let non_ascii = text.chars().filter(|c| !c.is_ascii()).collect::<String>();
                    assert!(text.is_ascii(), "ASCII fallback emitted Unicode at {width}x{height} {view:?}: {non_ascii:?}");
                }
            }
        }
        for page in 0..7 {
            let text = render_updates_text(
                &sampled,
                width,
                height,
                page,
                TrajectoryView::Ascent,
                PlotStyle::Ascii,
            )
            .unwrap();
            assert!(text.contains(PAGES_FOR_TEST[page]));
            assert!(
                text.is_ascii(),
                "ASCII page {page} emitted Unicode at {width}x{height}"
            );
        }
    }

    let mut changed = *sampled.last().unwrap();
    changed.director.position_q12 = [i32::MAX, i32::MIN, 123];
    changed.director.velocity_q24 = [i32::MIN, 456, i32::MAX];
    changed.director.acceleration_q28 = [9, 8, 7];
    changed.director.total_mass_q12 = 1;
    changed.director.dynamic_pressure_q16 = i32::MAX;
    let mut changed_history = sampled.clone();
    *changed_history.last_mut().unwrap() = changed;
    for page in 0..6 {
        let baseline = render_updates_text(
            &sampled,
            120,
            40,
            page,
            TrajectoryView::Ascent,
            PlotStyle::Ascii,
        )
        .unwrap();
        let mutated = render_updates_text(
            &changed_history,
            120,
            40,
            page,
            TrajectoryView::Ascent,
            PlotStyle::Ascii,
        )
        .unwrap();
        assert_eq!(
            baseline, mutated,
            "operational page {page} leaked director truth"
        );
    }
    let truth_a = render_updates_text(
        &sampled,
        120,
        40,
        6,
        TrajectoryView::Ascent,
        PlotStyle::Ascii,
    )
    .unwrap();
    let truth_b = render_updates_text(
        &changed_history,
        120,
        40,
        6,
        TrajectoryView::Ascent,
        PlotStyle::Ascii,
    )
    .unwrap();
    assert_ne!(
        truth_a, truth_b,
        "SIM Director did not expose changed truth"
    );

    let bytes = fs::read(&kmr).unwrap();
    fs::write(&truncated, &bytes[..bytes.len() - 8]).unwrap();
    let recovered = Session::load(&truncated).unwrap();
    assert!(recovered.recovered);
    assert!(!recovered.complete);
    assert_eq!(recovered.updates.len(), session.updates.len());
    for p in [&kmr, &csv, &json, &truncated] {
        let _ = fs::remove_file(p);
    }
}

struct StoppingSink {
    inner: RecordingSink,
    control: PaceController,
}
impl MissionControlSink for StoppingSink {
    fn publish(&mut self, v: MissionControlUpdate) {
        self.inner.publish(v);
        if v.epoch == 7 {
            self.control.cancel()
        }
    }
    fn finish(&mut self, v: &RunnerEvidence) {
        self.inner.finish(v)
    }
}
#[test]
fn explicit_stop_records_a_valid_partial_outcome() {
    let kmr = path("stopped.kmr6");
    let options = RunnerOptions {
        mission_control: true,
        pace: RunnerPace::Fast,
    };
    let control = PaceController::new(options.pace);
    let inner = RecordingSink::create(&kmr).unwrap();
    let mut sink = StoppingSink {
        inner,
        control: control.clone(),
    };
    let evidence = run_native_host_mission_controlled(options, Some(&mut sink), &control).unwrap();
    assert!(!evidence.complete);
    assert!(evidence.operator_stopped);
    assert_eq!(evidence.fast_epochs, 8);
    sink.inner.check().unwrap();
    let session = Session::load(&kmr).unwrap();
    assert!(!session.complete);
    assert!(!session.recovered);
    assert_eq!(session.updates.len(), 8);
    let saved = session.evidence.unwrap();
    assert!(saved.operator_stopped);
    assert!(!saved.complete);
    let _ = fs::remove_file(kmr);
}
