import { describe, expect, it } from "vitest";
import {
  buildExactGlobalDisplay,
  buildLegacySchematicDisplay,
  type GlobalDisplayDefinitionV1,
  type GlobalDisplayModelV1,
  type GlobalDisplaySampleV1,
  type GlobalDisplaySourcePoseV1,
} from "./globalDisplay";
import {
  buildGlobalSceneSnapshot,
  compatibleForInterpolation,
  ksaToBabylon,
  shouldSnapExactly,
  type GlobalSceneControls,
} from "./globalScene";
import {
  GLOBAL_POSE_VALID_ACTIVE_POSITION,
  GLOBAL_POSE_VALID_ECEF_POSITION,
  type GlobalDisplayDefinitionV1 as WireDefinition,
  type GlobalDisplayResolvedPoseV1 as WireResolvedPose,
  type GlobalDisplaySampleV1 as WireSample,
  type GlobalDisplaySourcePoseV1 as WireSourcePose,
} from "../protocol/globalDisplay";

const identityQuaternion = [1 << 30, 0, 0, 0] as const;

function pose(source: GlobalDisplaySourcePoseV1["source"], position: readonly [number, number, number], modelIdentity = 7): GlobalDisplaySourcePoseV1 {
  return { source, modelIdentity, sourceEstimateIdentity: 8, sourceChecksum: 9, ageReleases: 0,
    validityMask: 1n, ecefPositionQ12Km: position, gcrfPositionQ12Km: position,
    bodyToEcefQ30: identityQuaternion, bodyToGcrfQ30: identityQuaternion };
}

function sample(overrides: Partial<GlobalDisplaySampleV1> = {}): GlobalDisplaySampleV1 {
  return { sequence: 1n, releaseEpoch: 100, missionTimeQ16: 204_800, segment: "ecef-ascent",
    authoritativeFrame: "ecef", eventMask: 0n, discontinuityMask: 0n, continuityIdentity: 42,
    geodeticQ28Q12: [0, 0, 0], altitudeQ12Km: 1, machQ24: 2, dynamicPressureQ14Pa: 3,
    totalMassQ21Kg: 4, mainPropellantQ21Kg: 5, rcsPropellantQ21Kg: 6,
    componentStatusMask: 0n, poses: [pose("onboard", [26_124_849, 0, 0])], ...overrides };
}

function definition(sources: GlobalDisplayDefinitionV1["availableSources"]): GlobalDisplayDefinitionV1 {
  return { modelIdentity: 1, definitionIdentity: 2, earthIdentity: 3, transformIdentity: 4,
    missionEpochTaiSeconds: 0n, equatorialRadiusQ12Km: 26_124_849, polarRadiusQ12Km: 26_036_734,
    launchAnchor: { identity: 5, geodeticQ28Q12: [0, 0, 0] },
    recoveryAnchor: { identity: 6, geodeticQ28Q12: [0, 0, 0] },
    availableFrames: ["local-enu", "ecef", "gcrf"], availableSources: sources,
    availableCameras: ["director", "launch", "chase", "earth-fixed", "inertial", "recovery", "free", "inspection"],
    quality: "global-display-v1" };
}

function model(sources: GlobalDisplayDefinitionV1["availableSources"], samples: readonly GlobalDisplaySampleV1[]): GlobalDisplayModelV1 {
  return { definition: definition(sources), samples,
    paths: sources.map((source, index) => ({ pathIdentity: index + 1, modelIdentity: 20 + index, source,
      frame: "ecef" as const, lodSeconds: 1 as const, chunkSequence: 1, validityMask: 1n,
      stale: false, incomplete: false, terminal: false,
      points: [{ releaseEpoch: 100, eventMask: 0n, positionQ12Km: [26_124_849 + index, 0, 0] }] })),
    transitions: [], replay: { firstRelease: 100, lastRelease: 100, selectedRelease: 100,
      terminalDispositionIdentity: 0, markers: [] } };
}

function controls(overrides: Partial<GlobalSceneControls> = {}): GlobalSceneControls {
  return { camera: "chase", domain: "auto", selectedRelease: 100,
    visibleSources: new Set(["planned", "onboard", "ground"]), truthVisible: false,
    directorEnabled: false, ...overrides };
}

describe("global semantic scene model", () => {
  it("permits interpolation only across the same continuous source model", () => {
    const previous = sample();
    const next = sample({ sequence: 2n, releaseEpoch: 101 });
    expect(compatibleForInterpolation(previous, next, "onboard")).toBe(true);
    expect(shouldSnapExactly(previous, next)).toBe(false);
    expect(compatibleForInterpolation(previous, { ...next, eventMask: 1n }, "onboard")).toBe(false);
    expect(shouldSnapExactly(previous, { ...next, authoritativeFrame: "gcrf" })).toBe(true);
    expect(shouldSnapExactly(previous, { ...next, discontinuityMask: 1n })).toBe(true);
  });

  it("rebases only renderer coordinates and preserves a right-handed Babylon mapping", () => {
    const value = sample();
    const chase = buildGlobalSceneSnapshot(model(["onboard"], [value]), value,
      controls({ visibleSources: new Set(["onboard"]) }));
    const globe = buildGlobalSceneSnapshot(model(["onboard"], [value]), value,
      controls({ camera: "earth-fixed", visibleSources: new Set(["onboard"]) }));
    expect(chase.sources[0]?.positionKm[0]).toBeCloseTo(6378.137, 3);
    expect(chase.originKm).not.toEqual([0, 0, 0]);
    expect(globe.originKm).toEqual([0, 0, 0]);
    expect(ksaToBabylon([1, 2, 3])).toEqual([1, 3, -2]);
  });

  it("renders truth poses and paths only for a director-derived model when explicitly enabled", () => {
    const directorSample = sample({ poses: [pose("onboard", [1, 2, 3]), pose("truth", [4, 5, 6])] });
    const directorModel = model(["onboard", "truth"], [directorSample]);
    const hidden = buildGlobalSceneSnapshot(directorModel, directorSample,
      controls({ visibleSources: new Set(["onboard"]), truthVisible: false }));
    const visible = buildGlobalSceneSnapshot(directorModel, directorSample,
      controls({ visibleSources: new Set(["onboard"]), truthVisible: true }));
    expect(hidden.sources.map((value) => value.source)).toEqual(["onboard"]);
    expect(hidden.paths.map((value) => value.source)).toEqual(["onboard"]);
    expect(hidden.truthLabelVisible).toBe(false);
    expect(visible.sources.map((value) => value.source)).toContain("truth");
    expect(visible.paths.map((value) => value.source)).toContain("truth");
    expect(visible.truthLabelVisible).toBe(true);

    const operationalModel = model(["onboard"], [directorSample]);
    const rejected = buildGlobalSceneSnapshot(operationalModel, directorSample,
      controls({ visibleSources: new Set(["onboard"]), truthVisible: true }));
    expect(rejected.sources.map((value) => value.source)).not.toContain("truth");
    expect(rejected.paths.map((value) => value.source)).not.toContain("truth");
    expect(rejected.truthLabelVisible).toBe(false);
  });

  it("copies Rust-resolved absolute poses on the exact path and labels the legacy path non-exact", () => {
    const wireDefinition: WireDefinition = { displayIdentity: 1, earthIdentity: 2, transformIdentity: 3,
      missionIdentity: 4, epochUnixDay: 19_723, epochTaiMinusUtc: 37,
      semiMajorQ12Km: 26_124_849, semiMinorQ12Km: 26_036_734, inverseFlatteningQ20: 313_883_719,
      launchAnchor: { identity: 5, geodeticQ28Q12: [6, 7, 8] }, recoveryAnchor: { identity: 9, geodeticQ28Q12: [10, 11, 12] },
      availableSourceMask: 2, availableFrameMask: 7, cameraDomainMask: 0xff };
    const resolved: WireResolvedPose = { positionQ12Km: [111, 222, 333], velocityQ24KmS: [4, 5, 6], attitudeQ30: identityQuaternion };
    const wirePose: WireSourcePose = { source: 2, activeFrame: 2,
      validityMask: GLOBAL_POSE_VALID_ACTIVE_POSITION | GLOBAL_POSE_VALID_ECEF_POSITION,
      modelIdentity: 13, estimateIdentity: 14, checksum: 15, ageReleases: 0,
      active: resolved, ecef: resolved, gcrf: resolved, launchEnu: resolved, recoveryEnu: resolved,
      angularRateQ24: [0, 0, 0] };
    const wireSample: WireSample = { sequence: 1n, releaseEpoch: 1, missionTimeQ16: 2048,
      activeFrame: 2, segment: 2, flightMode: 1, transitionCount: 0, eventMask: 0,
      discontinuityMask: 0, continuityIdentity: 16, geodeticQ28Q12: [0, 0, 0], altitudeQ12Km: 0,
      machQ24: 0, dynamicPressureQ14Pa: 0, totalMassQ21Kg: 1, mainPropellantQ21Kg: 1,
      rcsPropellantQ21Kg: 1, gimbalQ15: [0, 0], rcsPulses: new Uint8Array(12),
      commandFlags: 0, commandDiscrete: 0, alarms: 0, sources: [wirePose] };
    const exact = buildExactGlobalDisplay({ definition: wireDefinition, samples: [wireSample], paths: new Map(), transitions: [] });
    expect(exact.definition.quality).toBe("global-display-v1");
    expect(exact.samples[0]?.poses[0]?.ecefPositionQ12Km).toEqual([111, 222, 333]);

    const legacy = buildLegacySchematicDisplay({ samples: [], paths: new Map(), timeline: [], truthAllowed: false });
    expect(legacy.definition.quality).toBe("legacy-schematic");
    expect(legacy.transitions).toEqual([]);
  });
});
