import {
  Q12_KILOMETRES,
  directorCameraForSample,
  displayDomainForSample,
  sourcePose,
  type GlobalDisplayCameraV1,
  type GlobalDisplayDomainV1,
  type GlobalDisplayFrameV1,
  type GlobalDisplayModelV1,
  type GlobalDisplaySampleV1,
  type GlobalDisplaySourcePoseV1,
  type GlobalDisplaySourceV1,
  type I32x3,
} from "./globalDisplay";

export type F64x3 = readonly [number, number, number];
export type F64x4 = readonly [number, number, number, number];

export interface GlobalSceneControls {
  readonly camera: GlobalDisplayCameraV1;
  readonly domain: GlobalDisplayDomainV1;
  readonly selectedRelease: number;
  readonly visibleSources: ReadonlySet<GlobalDisplaySourceV1>;
  readonly truthVisible: boolean;
  readonly directorEnabled: boolean;
}

export interface GlobalSceneSourceState {
  readonly source: GlobalDisplaySourceV1;
  readonly positionKm: F64x3;
  readonly bodyQuaternion?: F64x4;
  readonly modelIdentity: number;
  readonly ageReleases: number;
  readonly locatorRequired: boolean;
}

export interface GlobalScenePathState {
  readonly identity: number;
  readonly source: GlobalDisplaySourceV1;
  readonly pointsKm: readonly F64x3[];
  readonly dashed: boolean;
  readonly stale: boolean;
  readonly incomplete: boolean;
}

export interface GlobalSceneSnapshot {
  readonly releaseEpoch: number;
  readonly missionTimeQ16: number;
  readonly frame: GlobalDisplayFrameV1;
  readonly segment: string;
  readonly camera: GlobalDisplayCameraV1;
  readonly originKm: F64x3;
  readonly sources: readonly GlobalSceneSourceState[];
  readonly paths: readonly GlobalScenePathState[];
  readonly truthLabelVisible: boolean;
  readonly exactSnapRequired: boolean;
  readonly quality: GlobalDisplayModelV1["definition"]["quality"];
}

function rawToKm(raw: I32x3): F64x3 {
  return [raw[0] / Q12_KILOMETRES, raw[1] / Q12_KILOMETRES, raw[2] / Q12_KILOMETRES];
}

function positionForDomain(
  pose: GlobalDisplaySourcePoseV1,
  domain: GlobalDisplayFrameV1,
): I32x3 | undefined {
  switch (domain) {
    case "ecef": return pose.ecefPositionQ12Km;
    case "gcrf": return pose.gcrfPositionQ12Km;
    case "local-enu": return pose.launchEnuPositionQ12Km ?? pose.recoveryEnuPositionQ12Km;
  }
}

function attitudeForDomain(
  pose: GlobalDisplaySourcePoseV1,
  domain: GlobalDisplayFrameV1,
): F64x4 | undefined {
  const raw = domain === "gcrf" ? pose.bodyToGcrfQ30 : pose.bodyToEcefQ30;
  if (raw === undefined) return undefined;
  const scale = 1 << 30;
  return [raw[0] / scale, raw[1] / scale, raw[2] / scale, raw[3] / scale];
}

function quantize(value: number, quantum: number): number {
  return Math.round(value / quantum) * quantum;
}

function displayOrigin(
  camera: GlobalDisplayCameraV1,
  primary: F64x3 | undefined,
): F64x3 {
  if (primary === undefined || camera === "earth-fixed" || camera === "inertial" || camera === "free") {
    return [0, 0, 0];
  }
  const quantum = camera === "inspection" ? 0.001 : camera === "launch" || camera === "recovery" ? 0.1 : 10;
  return [
    quantize(primary[0], quantum),
    quantize(primary[1], quantum),
    quantize(primary[2], quantum),
  ];
}

function positionMagnitude(value: F64x3): number {
  return Math.hypot(value[0], value[1], value[2]);
}

export function compatibleForInterpolation(
  previous: GlobalDisplaySampleV1 | undefined,
  next: GlobalDisplaySampleV1 | undefined,
  source: GlobalDisplaySourceV1,
): boolean {
  if (previous === undefined || next === undefined ||
      previous.authoritativeFrame !== next.authoritativeFrame ||
      previous.segment !== next.segment ||
      previous.continuityIdentity !== next.continuityIdentity ||
      previous.discontinuityMask !== 0n || next.discontinuityMask !== 0n ||
      previous.eventMask !== 0n || next.eventMask !== 0n) {
    return false;
  }
  const left = sourcePose(previous, source);
  const right = sourcePose(next, source);
  return left !== undefined && right !== undefined &&
    left.modelIdentity === right.modelIdentity &&
    left.sourceEstimateIdentity === right.sourceEstimateIdentity &&
    left.validityMask !== 0n && right.validityMask !== 0n;
}

export function shouldSnapExactly(
  previous: GlobalDisplaySampleV1 | undefined,
  next: GlobalDisplaySampleV1 | undefined,
): boolean {
  if (previous === undefined || next === undefined) return true;
  return next.releaseEpoch <= previous.releaseEpoch ||
    previous.authoritativeFrame !== next.authoritativeFrame ||
    previous.segment !== next.segment ||
    previous.continuityIdentity !== next.continuityIdentity ||
    previous.discontinuityMask !== 0n ||
    next.discontinuityMask !== 0n ||
    next.eventMask !== 0n;
}

export function buildGlobalSceneSnapshot(
  model: GlobalDisplayModelV1,
  sample: GlobalDisplaySampleV1 | undefined,
  controls: GlobalSceneControls,
  previous?: GlobalDisplaySampleV1,
): GlobalSceneSnapshot {
  const domain = model.definition.quality === "global-display-v1"
    ? displayDomainForSample(controls.domain, sample)
    : "ecef";
  const camera = controls.directorEnabled || controls.camera === "director"
    ? directorCameraForSample(sample)
    : controls.camera;
  const allowedSources = model.definition.availableSources;
  const effectiveSources = new Set(controls.visibleSources);
  if (controls.truthVisible && allowedSources.includes("truth")) {
    effectiveSources.add("truth");
  }
  const sourceStates: GlobalSceneSourceState[] = [];
  for (const source of effectiveSources) {
    if (!allowedSources.includes(source) || (source === "truth" && !controls.truthVisible)) continue;
    const value = sourcePose(sample, source);
    if (value === undefined) continue;
    const raw = positionForDomain(value, domain) ??
      value.ecefPositionQ12Km ??
      value.gcrfPositionQ12Km;
    if (raw === undefined) continue;
    const positionKm = rawToKm(raw);
    sourceStates.push({
      source,
      positionKm,
      bodyQuaternion: attitudeForDomain(value, domain),
      modelIdentity: value.modelIdentity,
      ageReleases: value.ageReleases,
      locatorRequired: positionMagnitude(positionKm) > 1000,
    });
  }
  const primary = sourceStates.find((value) => value.source === "onboard") ??
    sourceStates.find((value) => value.source === "ground") ??
    sourceStates[0];
  const paths: GlobalScenePathState[] = [];
  for (const chunk of model.paths) {
    if (!effectiveSources.has(chunk.source) ||
        !allowedSources.includes(chunk.source) ||
        (chunk.source === "truth" && !controls.truthVisible)) continue;
    paths.push({
      identity: chunk.pathIdentity,
      source: chunk.source,
      pointsKm: chunk.points.map((point) => rawToKm(point.positionQ12Km)),
      dashed: chunk.source === "planned",
      stale: chunk.stale,
      incomplete: chunk.incomplete,
    });
  }
  return {
    releaseEpoch: sample?.releaseEpoch ?? controls.selectedRelease,
    missionTimeQ16: sample?.missionTimeQ16 ?? 0,
    frame: domain,
    segment: sample?.segment ?? "awaiting",
    camera,
    originKm: displayOrigin(camera, primary?.positionKm),
    sources: sourceStates,
    paths,
    truthLabelVisible: controls.truthVisible && sourceStates.some((value) => value.source === "truth"),
    exactSnapRequired: shouldSnapExactly(previous, sample),
    quality: model.definition.quality,
  };
}

/**
 * Rotates KSA64's right-handed +X/+Y/+Z basis into Babylon's right-handed
 * X/right, Y/up, Z/back basis without reflection.
 */
export function ksaToBabylon(value: F64x3, origin: F64x3 = [0, 0, 0]): F64x3 {
  const x = value[0] - origin[0];
  const y = value[1] - origin[1];
  const z = value[2] - origin[2];
  return [x, z, -y];
}
