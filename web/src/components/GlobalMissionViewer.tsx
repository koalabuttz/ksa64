import { useEffect, useMemo, useRef, useState } from "react";
import {
  directorCameraForSample,
  displayDomainForSample,
  findSampleAtOrBefore,
  type GlobalDisplayCameraV1,
  type GlobalDisplayDomainV1,
  type GlobalDisplayLayoutV1,
  type GlobalDisplayModelV1,
  type GlobalDisplaySourceV1,
} from "../model/globalDisplay";
import { buildGlobalSceneSnapshot } from "../model/globalScene";
import {
  startGlobalMissionRenderer,
  type GlobalMissionRenderer,
  type RendererBackend,
  type RendererPreference,
} from "../render/globalMissionScene";

export interface GlobalMissionViewerProps {
  readonly model: GlobalDisplayModelV1;
  readonly replay: boolean;
  readonly layout: GlobalDisplayLayoutV1;
  readonly deskOpen: boolean;
  onLayoutChange(layout: GlobalDisplayLayoutV1): void;
  onDeskOpenChange(open: boolean): void;
}

type PlaybackRate = 0.25 | 0.5 | 1 | 2 | 4 | 8 | 16 | "unpaced";

const SOURCE_LABEL: Record<GlobalDisplaySourceV1, string> = {
  planned: "Planned reference",
  onboard: "Onboard estimate",
  ground: "Ground estimate",
  truth: "SIM truth",
};

const CAMERA_LABEL: Record<GlobalDisplayCameraV1, string> = {
  director: "Mission director",
  launch: "Launch site",
  chase: "Vehicle chase",
  "earth-fixed": "Earth-fixed globe",
  inertial: "Inertial GCRF",
  recovery: "Recovery site",
  free: "Free orbit",
  inspection: "Vehicle inspection",
};

function frameLabel(value: GlobalDisplayDomainV1): string {
  switch (value) {
    case "auto": return "Follow mission";
    case "local-enu": return "Local ENU";
    case "ecef": return "Earth-fixed ECEF";
    case "gcrf": return "Inertial GCRF";
  }
}

function segmentLabel(value: string): string {
  return value.split("-").map((part) => part[0]?.toUpperCase() + part.slice(1)).join(" ");
}

function formatMet(rawQ16: number): string {
  const seconds = rawQ16 / 65_536;
  const minutes = Math.floor(seconds / 60);
  return `${String(minutes).padStart(2, "0")}:${(seconds - minutes * 60).toFixed(3).padStart(6, "0")}`;
}

export function GlobalMissionViewer({
  model,
  replay,
  layout,
  deskOpen,
  onLayoutChange,
  onDeskOpenChange,
}: GlobalMissionViewerProps) {
  const container = useRef<HTMLDivElement>(null);
  const renderer = useRef<GlobalMissionRenderer | undefined>(undefined);
  const latestRelease = model.samples.at(-1)?.releaseEpoch ?? model.replay.selectedRelease;
  const [selectedRelease, setSelectedRelease] = useState(latestRelease);
  const [preference, setPreference] = useState<RendererPreference>("auto");
  const [backend, setBackend] = useState<RendererBackend | "initializing">("initializing");
  const [domain, setDomain] = useState<GlobalDisplayDomainV1>("auto");
  const [camera, setCamera] = useState<GlobalDisplayCameraV1>("director");
  const [directorEnabled, setDirectorEnabled] = useState(true);
  const [visibleSources, setVisibleSources] = useState<ReadonlySet<GlobalDisplaySourceV1>>(
    () => new Set(["planned", "onboard", "ground"]),
  );
  const [truthVisible, setTruthVisible] = useState(false);
  const [playing, setPlaying] = useState(false);
  const [playbackRate, setPlaybackRate] = useState<PlaybackRate>(1);

  useEffect(() => {
    if (!replay || playing || selectedRelease >= latestRelease) setSelectedRelease(latestRelease);
  }, [latestRelease, playing, replay, selectedRelease]);

  const sampleIndex = useMemo(() => {
    const index = model.samples.findIndex((value) => value.releaseEpoch >= selectedRelease);
    return index < 0 ? Math.max(0, model.samples.length - 1) : index;
  }, [model.samples, selectedRelease]);
  const sample = findSampleAtOrBefore(model.samples, selectedRelease) ??
    model.samples[Math.min(sampleIndex, Math.max(0, model.samples.length - 1))];
  const previous = sampleIndex > 0 ? model.samples[sampleIndex - 1] : undefined;
  const controls = useMemo(() => ({
    camera,
    domain,
    selectedRelease,
    visibleSources,
    truthVisible,
    directorEnabled,
  }), [camera, directorEnabled, domain, selectedRelease, truthVisible, visibleSources]);
  const sceneSnapshot = useMemo(
    () => buildGlobalSceneSnapshot(model, sample, controls, previous),
    [controls, model, previous, sample],
  );

  useEffect(() => {
    if (container.current === null) return;
    let active = true;
    setBackend("initializing");
    void startGlobalMissionRenderer(container.current, preference, sceneSnapshot, {
      onBackendChange(value) {
        if (active) setBackend(value);
      },
      onManualCamera() {
        if (!active) return;
        setDirectorEnabled(false);
        setCamera("free");
      },
    }).then((value) => {
      if (!active) {
        value.dispose();
        return;
      }
      renderer.current = value;
      setBackend(value.backend);
    });
    return () => {
      active = false;
      renderer.current?.dispose();
      renderer.current = undefined;
    };
    // The renderer owns mutable scene state; snapshots update through update().
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preference]);

  useEffect(() => {
    renderer.current?.update(sceneSnapshot);
  }, [sceneSnapshot]);

  useEffect(() => {
    if (!replay || !playing || model.samples.length < 2) return;
    const currentIndex = model.samples.findIndex((value) => value.releaseEpoch > selectedRelease);
    if (currentIndex < 0) {
      setPlaying(false);
      return;
    }
    const delay = playbackRate === "unpaced" ? 0 : Math.max(8, 31.25 / playbackRate);
    const timer = window.setTimeout(() => {
      const next = model.samples[currentIndex];
      if (next !== undefined) setSelectedRelease(next.releaseEpoch);
    }, delay);
    return () => window.clearTimeout(timer);
  }, [model.samples, playbackRate, playing, replay, selectedRelease]);

  const setSourceVisible = (source: GlobalDisplaySourceV1, checked: boolean) => {
    setVisibleSources((current) => {
      const next = new Set(current);
      if (checked) next.add(source);
      else next.delete(source);
      return next;
    });
  };

  const step = (direction: -1 | 1) => {
    if (!replay || model.samples.length === 0) return;
    const exact = model.samples.findIndex((value) => value.releaseEpoch === sample?.releaseEpoch);
    const base = exact >= 0 ? exact : sampleIndex;
    const next = model.samples[Math.max(0, Math.min(model.samples.length - 1, base + direction))];
    if (next !== undefined) {
      setPlaying(false);
      setSelectedRelease(next.releaseEpoch);
    }
  };

  const selectCamera = (value: GlobalDisplayCameraV1) => {
    setCamera(value);
    setDirectorEnabled(value === "director");
  };

  const actualDomain = displayDomainForSample(domain, sample);
  const actualCamera = directorEnabled ? directorCameraForSample(sample) : camera;
  const truthAvailable = model.definition.availableSources.includes("truth");
  const onboard = sceneSnapshot.sources.find((value) => value.source === "onboard");
  const markerOptions = model.replay.markers.filter((value) =>
    value.releaseEpoch >= model.replay.firstRelease && value.releaseEpoch <= Math.max(model.replay.lastRelease, latestRelease));

  return (
    <section
      className="panel global-viewer-panel"
      aria-labelledby="global-viewer-title"
      data-layout={layout}
      data-semantic-release={sceneSnapshot.releaseEpoch}
      data-semantic-frame={sceneSnapshot.frame}
      data-semantic-camera={sceneSnapshot.camera}
      data-quality={sceneSnapshot.quality}
    >
      <header className="global-viewer-header">
        <div>
          <p className="eyebrow">Passive renderer · Rust owns frames, roles, events, and evidence</p>
          <h2 id="global-viewer-title">Global mission director</h2>
        </div>
        <div className="global-viewer-status">
          <span data-state={backend}>{backend === "initializing" ? "Selecting renderer…" : `${backend.toUpperCase()} active`}</span>
          <span>{model.definition.quality === "global-display-v1" ? "GlobalDisplayV1 exact" :
            model.definition.quality === "demonstration" ? "Demonstration schematic" : "Legacy 2-D stream · schematic only"}</span>
        </div>
      </header>

      <div className="global-layout-switcher" role="group" aria-label="Mission viewer layout">
        {(["hybrid", "engineering", "cinematic"] as const).map((value) => (
          <button key={value} type="button" aria-pressed={layout === value}
            onClick={() => onLayoutChange(value)}>
            {value === "hybrid" ? "Mission director" : value === "engineering" ? "Engineering split" : "Cinematic"}
          </button>
        ))}
        <button type="button" aria-pressed={deskOpen} onClick={() => onDeskOpenChange(!deskOpen)}>
          {deskOpen ? "Hide operations desk" : "Show operations desk"}
        </button>
      </div>

      <div className="global-viewer-workspace">
        <div className="global-render-stage" ref={container}>
          <div className="global-render-loading">Preparing procedural Earth…</div>
        </div>
        <div className="global-scene-overlay" aria-live="polite">
          <span>{segmentLabel(sceneSnapshot.segment)}</span>
          <strong>{actualDomain.toUpperCase()} · R+{sceneSnapshot.releaseEpoch.toLocaleString()}</strong>
          <span>MET {formatMet(sceneSnapshot.missionTimeQ16)}</span>
        </div>
        {sceneSnapshot.truthLabelVisible ? <div className="sim-truth-watermark">SIM TRUTH</div> : null}
        <aside className="vehicle-locator-card" aria-label="True-scale vehicle inspection">
          <p className="eyebrow">True-scale local inset</p>
          <div className="vehicle-inset-diagram" aria-hidden="true">
            <span className="vehicle-inset-nose" />
            <span className="vehicle-inset-body" />
            <span className="vehicle-inset-plume" />
          </div>
          <dl>
            <div><dt>Vehicle</dt><dd>KSA-G10R</dd></div>
            <div><dt>Length</dt><dd>≈ 8 m</dd></div>
            <div><dt>Locator</dt><dd>{onboard?.locatorRequired ? "Screen-scale marker" : "Local scale"}</dd></div>
          </dl>
          <small>The Earth view never enlarges the physical trajectory or silently changes vehicle scale.</small>
        </aside>
      </div>

      <div className="global-viewer-controls">
        <label>Renderer<select aria-label="Global renderer backend" value={preference}
          onChange={(event) => setPreference(event.target.value as RendererPreference)}>
          <option value="auto">WebGPU → WebGL2 → 2-D</option>
          <option value="webgl2">Force WebGL2</option>
          <option value="2d">2-D only</option>
        </select></label>
        <label>Display frame<select aria-label="Global display frame" value={domain}
          onChange={(event) => setDomain(event.target.value as GlobalDisplayDomainV1)}>
          {(["auto", "local-enu", "ecef", "gcrf"] as const).map((value) =>
            <option value={value} key={value}>{frameLabel(value)}</option>)}
        </select></label>
        <label>Camera<select aria-label="Global camera" value={directorEnabled ? "director" : camera}
          onChange={(event) => selectCamera(event.target.value as GlobalDisplayCameraV1)}>
          {model.definition.availableCameras.map((value) =>
            <option value={value} key={value}>{CAMERA_LABEL[value]}</option>)}
        </select></label>
        {!directorEnabled ? <button type="button" onClick={() => selectCamera("director")}>Resume director</button> : null}
      </div>

      <div className="global-source-legend" aria-label="Visible trajectory sources">
        {model.definition.availableSources.map((source) => (
          <label key={source} data-source={source}>
            <input type="checkbox" checked={source === "truth" ? truthVisible : visibleSources.has(source)}
              onChange={(event) => {
                if (source === "truth") setTruthVisible(event.target.checked);
                else setSourceVisible(source, event.target.checked);
              }} />
            <span className="global-source-swatch" aria-hidden="true" />
            {SOURCE_LABEL[source]}
          </label>
        ))}
        {!truthAvailable ? <span className="truth-boundary-label">SIM truth absent from this role</span> : null}
      </div>

      <div className="global-replay-controls" aria-label={replay ? "Replay controls" : "Live mission controls"}>
        <div className="replay-buttons">
          <button type="button" disabled={!replay || sampleIndex <= 0} onClick={() => step(-1)}
            aria-label="Previous exact display sample">◀ Step</button>
          <button type="button" disabled={!replay || model.samples.length < 2}
            onClick={() => setPlaying((value) => !value)}>{playing ? "Pause replay" : "Play replay"}</button>
          <button type="button" disabled={!replay || sampleIndex >= model.samples.length - 1} onClick={() => step(1)}
            aria-label="Next exact display sample">Step ▶</button>
          <button type="button" disabled={!replay || selectedRelease === latestRelease}
            onClick={() => { setPlaying(false); setSelectedRelease(latestRelease); }}>Latest</button>
        </div>
        <label className="release-scrubber">
          <span>{replay ? "Exact release" : "Live release · rewind disabled"}</span>
          <input type="range" min={model.replay.firstRelease} max={Math.max(model.replay.lastRelease, latestRelease, 1)}
            value={Math.max(model.replay.firstRelease, Math.min(selectedRelease, Math.max(model.replay.lastRelease, latestRelease, 1)))}
            disabled={!replay || model.samples.length < 2}
            onChange={(event) => { setPlaying(false); setSelectedRelease(Number(event.target.value)); }} />
        </label>
        <label>Playback<select aria-label="Replay playback rate" value={String(playbackRate)} disabled={!replay}
          onChange={(event) => setPlaybackRate(event.target.value === "unpaced" ? "unpaced" : Number(event.target.value) as PlaybackRate)}>
          {[0.25, 0.5, 1, 2, 4, 8, 16].map((value) => <option value={value} key={value}>{value}×</option>)}
          <option value="unpaced">Unpaced</option>
        </select></label>
        <label>Event jump<select aria-label="Jump to mission event" value=""
          disabled={!replay || markerOptions.length === 0}
          onChange={(event) => {
            if (event.target.value !== "") {
              setPlaying(false);
              setSelectedRelease(Number(event.target.value));
            }
          }}>
          <option value="">Choose event…</option>
          {markerOptions.map((marker) =>
            <option value={marker.releaseEpoch} key={`${marker.releaseEpoch}-${marker.identity}`}>
              R+{marker.releaseEpoch.toLocaleString()} · {marker.label}
            </option>)}
        </select></label>
      </div>

      <footer className="global-viewer-footer">
        <span>Camera: {CAMERA_LABEL[actualCamera]}</span>
        <span>Frame: {frameLabel(actualDomain)}</span>
        <span>{sceneSnapshot.exactSnapRequired ? "Exact event/sample snap" : "Compatible sample smoothing permitted"}</span>
      </footer>
    </section>
  );
}
