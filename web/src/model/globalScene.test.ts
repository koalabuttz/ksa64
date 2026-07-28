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
  buildGlobalSceneSemanticSnapshot,
  buildGlobalSceneSnapshot,
  compatibleForInterpolation,
  displayFrameForCamera,
  interpolateQuaternion,
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
    launchAnchor: { identity: 5, geodeticQ28Q12: [0, 0, 0], ecefPositionQ12Km: [26_124_849, 0, 0] },
    recoveryAnchor: { identity: 6, geodeticQ28Q12: [0, 0, 0], ecefPositionQ12Km: [26_124_849, 1, 0] },
    availableFrames: ["local-enu", "ecef", "gcrf"], availableSources: sources,
    availableCameras: ["director", "launch", "chase", "earth-fixed", "inertial", "recovery", "free", "inspection"],
    quality: "global-display-v1" };
}

function model(sources: GlobalDisplayDefinitionV1["availableSources"], samples: readonly GlobalDisplaySampleV1[]): GlobalDisplayModelV1 {
  return { definition: definition(sources), samples,
    paths: sources.map((source, index) => ({ pathIdentity: index + 1, modelIdentity: 20 + index, sourceEstimateIdentity: 30 + index,
      sourceChecksum: 40 + index, continuityIdentity: 50 + index, flags: 0, source,
      frame: "ecef" as const, lodSeconds: 1 as const, chunkSequence: 1, validityMask: 1n,
      stale: false, incomplete: false, terminal: false,
      points: [{ releaseEpoch: 100, missionTimeQ16: 204_800, segment: "ecef-ascent" as const, eventMask: 0n, anchorIdentity: 0, positionQ12Km: [26_124_849 + index, 0, 0] }] })),
    transitions: [], replay: { firstRelease: 100, lastRelease: 100, selectedRelease: 100,
      terminalDispositionIdentity: 0, markers: [] } };
}

function controls(overrides: Partial<GlobalSceneControls> = {}): GlobalSceneControls {
  return { camera: "chase", domain: "auto", selectedRelease: 100, motionMode: "smooth", pathDetail: "auto",
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
    expect(shouldSnapExactly(previous, { ...next, releaseEpoch: previous.releaseEpoch + 65 })).toBe(true);

    const start = sample({ poses: [pose("onboard", [0, 0, 0])] });
    const finish = sample({ sequence: 2n, releaseEpoch: 101, poses: [pose("onboard", [4096, 8192, 0])] });
    const smoothed = buildGlobalSceneSnapshot(model(["onboard"], [start, finish]), finish,
      controls({ visibleSources: new Set(["onboard"]), interpolationAlpha: 0.5 }), start);
    expect(smoothed.sources[0]?.positionKm).toEqual([0.5, 1, 0]);
    expect(smoothed.interpolated).toBe(true);
    expect(interpolateQuaternion([1, 0, 0, 0], [-1, 0, 0, 0], 0.5)).toEqual([1, 0, 0, 0]);

    const noAttitudeStart = sample({ poses: [{ ...pose("onboard", [0, 0, 0]), bodyToEcefQ30: undefined }] });
    const noAttitudeFinish = sample({ sequence: 2n, releaseEpoch: 101,
      poses: [{ ...pose("onboard", [4096, 0, 0]), bodyToEcefQ30: undefined }] });
    const exactWithoutAttitude = buildGlobalSceneSnapshot(model(["onboard"], [noAttitudeStart, noAttitudeFinish]),
      noAttitudeFinish, controls({ visibleSources: new Set(["onboard"]), interpolationAlpha: 0.5 }), noAttitudeStart);
    expect(exactWithoutAttitude.sources[0]?.positionKm).toEqual([1, 0, 0]);
    expect(exactWithoutAttitude.interpolated).toBe(false);
  });

  it("rebases only renderer coordinates and preserves a right-handed Babylon mapping", () => {
    const value = sample({ eventMask: 3n, discontinuityMask: 5n });
    const chase = buildGlobalSceneSnapshot(model(["onboard"], [value]), value,
      controls({ visibleSources: new Set(["onboard"]) }));
    const globe = buildGlobalSceneSnapshot(model(["onboard"], [value]), value,
      controls({ camera: "earth-fixed", visibleSources: new Set(["onboard"]) }));
    expect(chase.sources[0]?.positionKm[0]).toBeCloseTo(6378.137, 3);
    expect(chase.originKm).not.toEqual([0, 0, 0]);
    expect(globe.originKm).toEqual([0, 0, 0]);
    expect(ksaToBabylon([1, 2, 3])).toEqual([1, 3, -2]);
    const semantic = buildGlobalSceneSemanticSnapshot(chase);
    expect(buildGlobalSceneSemanticSnapshot({ ...chase, originKm: [123, 456, 789] })).toEqual(semantic);
    expect(semantic.sources[0]?.positionQ12Km).toEqual([26_124_849, 0, 0]);
    expect(semantic).toMatchObject({
      eventMask: 3,
      discontinuityMask: 5,
      continuityIdentity: 42,
      visibleSourceMask: 2,
      sources: [{ source: "onboard", modelIdentity: 7, sourceEstimateIdentity: 8, sourceChecksum: 9 }],
      paths: [{ identity: 1, source: "onboard", modelIdentity: 20, sourceEstimateIdentity: 30,
        sourceChecksum: 40, continuityIdentity: 50, flags: 0 }],
    });
  });

  it("normalizes manual camera and frame combinations like Unreal", () => {
    const value = sample({ poses: [{ ...pose("onboard", [26_124_849, 0, 0]),
      launchEnuPositionQ12Km: [4_096, 0, 0], recoveryEnuPositionQ12Km: [8_192, 0, 0] }] });
    const recovery = buildGlobalSceneSnapshot(model(["onboard"], [value]), value,
      controls({ camera: "recovery", domain: "ecef", visibleSources: new Set(["onboard"]) }));
    expect(displayFrameForCamera("recovery", value)).toBe("local-enu");
    expect(recovery.frame).toBe("local-enu");
    expect(recovery.sources[0]?.positionKm).toEqual([2, 0, 0]);
    expect(displayFrameForCamera("earth-fixed", value)).toBe("ecef");
    expect(displayFrameForCamera("inertial", value)).toBe("gcrf");
  });

  it("does not substitute another frame when an exact Rust-resolved pose is absent", () => {
    const ecefOnly = sample({ authoritativeFrame: "ecef", segment: "local-launch",
      poses: [pose("onboard", [26_124_849, 0, 0])] });
    const local = buildGlobalSceneSnapshot(model(["onboard"], [ecefOnly]), ecefOnly,
      controls({ camera: "launch", visibleSources: new Set(["onboard"]) }));
    expect(local.frame).toBe("local-enu");
    expect(local.sources).toEqual([]);

    const launchOnly = sample({ authoritativeFrame: "ecef", segment: "local-launch",
      poses: [{ ...pose("onboard", [26_124_849, 0, 0]), launchEnuPositionQ12Km: [4_096, 0, 0] }] });
    const recovery = buildGlobalSceneSnapshot(model(["onboard"], [launchOnly]), launchOnly,
      controls({ camera: "recovery", visibleSources: new Set(["onboard"]) }));
    expect(recovery.frame).toBe("local-enu");
    expect(recovery.sources).toEqual([]);
  });

  it("selects deterministic path detail without changing authoritative samples", () => {
    const base = model(["onboard"], [sample()]);
    const path = base.paths[0]!;
    const withLods = { ...base, paths: [
      { ...path, pathIdentity: 101, lodSeconds: 0 as const },
      { ...path, pathIdentity: 102, lodSeconds: 1 as const },
      { ...path, pathIdentity: 103, lodSeconds: 4 as const },
    ] };
    const sharedDetail = buildGlobalSceneSnapshot(withLods, base.samples[0],
      controls({ camera: "chase", visibleSources: new Set(["onboard"]), pathDetail: "auto" }));
    const overview = buildGlobalSceneSnapshot(withLods, base.samples[0],
      controls({ camera: "earth-fixed", visibleSources: new Set(["onboard"]), pathDetail: "auto" }));
    const oneSecond = buildGlobalSceneSnapshot(withLods, base.samples[0],
      controls({ visibleSources: new Set(["onboard"]), pathDetail: 1 }));
    expect(sharedDetail.paths.map((value) => value.lodSeconds)).toEqual([1]);
    expect(overview.paths.map((value) => value.lodSeconds)).toEqual([4]);
    expect(oneSecond.paths.map((value) => value.lodSeconds)).toEqual([1]);
  });

  it("preserves and presents raw path resynchronization state", () => {
    const base = model(["onboard"], [sample()]);
    const flagged: GlobalDisplayModelV1 = { ...base, paths: [{ ...base.paths[0]!, flags: 1 << 3 }] };
    const scene = buildGlobalSceneSnapshot(flagged, flagged.samples[0],
      controls({ visibleSources: new Set(["onboard"]), pathDetail: 1 }));
    expect(scene.paths[0]?.resyncRequired).toBe(true);
    expect(buildGlobalSceneSemanticSnapshot(scene).paths[0]?.flags).toBe(1 << 3);
  });

  it("splits launch and recovery ENU trail strips at the Rust anchor identity", () => {
    const localSample = sample({ authoritativeFrame: "local-enu", segment: "local-launch",
      poses: [{ ...pose("onboard", [0, 0, 0]), launchEnuPositionQ12Km: [0, 0, 0] }] });
    const base = model(["onboard"], [localSample]);
    const localModel: GlobalDisplayModelV1 = { ...base, paths: [{ ...base.paths[0]!,
      frame: "local-enu", points: [
        { releaseEpoch: 1, missionTimeQ16: 2_048, segment: "local-launch", eventMask: 0n, anchorIdentity: 5, positionQ12Km: [0, 0, 0] },
        { releaseEpoch: 2, missionTimeQ16: 4_096, segment: "local-launch", eventMask: 0n, anchorIdentity: 5, positionQ12Km: [1, 0, 0] },
        { releaseEpoch: 3, missionTimeQ16: 6_144, segment: "local-recovery", eventMask: 0n, anchorIdentity: 6, positionQ12Km: [0, 0, 0] },
        { releaseEpoch: 4, missionTimeQ16: 8_192, segment: "local-recovery", eventMask: 0n, anchorIdentity: 6, positionQ12Km: [1, 0, 0] },
      ],
    }] };
    const scene = buildGlobalSceneSnapshot(localModel, localSample, controls({
      camera: "launch", visibleSources: new Set(["onboard"]), pathDetail: 1,
    }));
    expect(scene.paths).toHaveLength(2);
    expect(scene.paths.map((path) => [path.anchorIdentity, path.pointsKm.length])).toEqual([[5, 2], [6, 2]]);
    expect(buildGlobalSceneSemanticSnapshot(scene).paths.map((path) => path.anchorIdentity)).toEqual([5, 6]);
    expect(buildGlobalSceneSemanticSnapshot(scene).paths.map((path) => path.pointChecksum)).not.toEqual([0, 0]);
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
      launchAnchor: { identity: 5, geodeticQ28Q12: [6, 7, 8], ecefPositionQ12Km: [21, 22, 23] },
      recoveryAnchor: { identity: 9, geodeticQ28Q12: [10, 11, 12], ecefPositionQ12Km: [24, 25, 26] },
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
    expect(exact.definition.missionEpochTaiSeconds).toBe(19_723n * 86_400n + 37n);
    expect(exact.samples[0]?.missionTimeQ16).toBe(2048);
    expect(exact.samples[0]?.poses[0]?.ecefPositionQ12Km).toEqual([111, 222, 333]);
    expect(exact.samples[0]?.poses[0]?.ecefVelocityQ24KmS).toBeUndefined();
    expect(exact.samples[0]?.poses[0]?.bodyToEcefQ30).toBeUndefined();
    expect(exact.samples[0]?.poses[0]?.bodyAngularRateQ24RadS).toBeUndefined();

    const legacy = buildLegacySchematicDisplay({ samples: [], paths: new Map(), timeline: [], truthAllowed: false });
    expect(legacy.definition.quality).toBe("legacy-schematic");
    expect(legacy.transitions).toEqual([]);
  });
});
