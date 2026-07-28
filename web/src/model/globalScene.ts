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
  type I32x4,
} from "./globalDisplay";

export type F64x3 = readonly [number, number, number];
export type F64x4 = readonly [number, number, number, number];

export type GlobalDisplayMotionMode = "smooth" | "exact";
export type GlobalDisplayPathDetail = "auto" | 0 | 1 | 4;

export interface GlobalSceneControls {
  readonly camera: GlobalDisplayCameraV1;
  readonly domain: GlobalDisplayDomainV1;
  readonly selectedRelease: number;
  readonly motionMode: GlobalDisplayMotionMode;
  readonly pathDetail: GlobalDisplayPathDetail;
  readonly visibleSources: ReadonlySet<GlobalDisplaySourceV1>;
  readonly truthVisible: boolean;
  readonly directorEnabled: boolean;
  readonly interpolationAlpha?: number;
}

export interface GlobalSceneSourceState {
  readonly source: GlobalDisplaySourceV1;
  readonly positionKm: F64x3;
  readonly bodyQuaternion?: F64x4;
  readonly modelIdentity: number;
  readonly ageReleases: number;
  readonly locatorRequired: boolean;
}

export interface GlobalSceneAnchorState {
  readonly kind: "launch" | "recovery";
  readonly positionKm: F64x3;
}

export interface GlobalScenePathState {
  readonly identity: number;
  readonly source: GlobalDisplaySourceV1;
  readonly anchorIdentity: number;
  readonly stripIndex: number;
  readonly pointsKm: readonly F64x3[];
  readonly dashed: boolean;
  readonly stale: boolean;
  readonly incomplete: boolean;
  readonly lodSeconds: 0 | 1 | 4;
}

export interface GlobalSceneSemanticSourceV1 {
  readonly source: GlobalDisplaySourceV1;
  readonly modelIdentity: number;
  readonly ageReleases: number;
  readonly positionQ12Km: I32x3;
  readonly bodyQuaternionQ30?: I32x4;
}

export interface GlobalSceneSemanticPathV1 {
  readonly identity: number;
  readonly source: GlobalDisplaySourceV1;
  readonly anchorIdentity: number;
  readonly stripIndex: number;
  readonly lodSeconds: 0 | 1 | 4;
  readonly pointCount: number;
  readonly pointChecksum: number;
}

export interface GlobalSceneSemanticSnapshotV1 {
  readonly schema: "ksa64.global-scene-semantic.v1";
  readonly quality: GlobalDisplayModelV1["definition"]["quality"];
  readonly releaseEpoch: number;
  readonly missionTimeQ16: number;
  readonly frame: GlobalDisplayFrameV1;
  readonly segment: string;
  readonly camera: GlobalDisplayCameraV1;
  readonly exactSnapRequired: boolean;
  readonly truthLabelVisible: boolean;
  readonly anchors: readonly { readonly kind: "launch" | "recovery"; readonly positionQ12Km: I32x3 }[];
  readonly sources: readonly GlobalSceneSemanticSourceV1[];
  readonly paths: readonly GlobalSceneSemanticPathV1[];
}

export interface GlobalSceneSnapshot {
  readonly releaseEpoch: number;
  readonly missionTimeQ16: number;
  readonly frame: GlobalDisplayFrameV1;
  readonly segment: string;
  readonly camera: GlobalDisplayCameraV1;
  readonly originKm: F64x3;
  readonly anchors: readonly GlobalSceneAnchorState[];
  readonly sources: readonly GlobalSceneSourceState[];
  readonly paths: readonly GlobalScenePathState[];
  readonly truthLabelVisible: boolean;
  readonly exactSnapRequired: boolean;
  readonly interpolated: boolean;
  readonly quality: GlobalDisplayModelV1["definition"]["quality"];
}

function rawToKm(raw: I32x3): F64x3 {
  return [raw[0] / Q12_KILOMETRES, raw[1] / Q12_KILOMETRES, raw[2] / Q12_KILOMETRES];
}

function positionForDomain(
  pose: GlobalDisplaySourcePoseV1,
  domain: GlobalDisplayFrameV1,
  segment: string,
): I32x3 | undefined {
  switch (domain) {
    case "ecef": return pose.ecefPositionQ12Km;
    case "gcrf": return pose.gcrfPositionQ12Km;
    case "local-enu": return segment === "local-recovery"
      ? pose.recoveryEnuPositionQ12Km ?? pose.launchEnuPositionQ12Km
      : pose.launchEnuPositionQ12Km ?? pose.recoveryEnuPositionQ12Km;
  }
}

function attitudeForDomain(
  pose: GlobalDisplaySourcePoseV1,
  domain: GlobalDisplayFrameV1,
  segment: string,
): F64x4 | undefined {
  const raw = domain === "gcrf" ? pose.bodyToGcrfQ30
    : domain === "ecef" ? pose.bodyToEcefQ30
      : segment === "local-recovery"
        ? pose.bodyToRecoveryEnuQ30 ?? pose.bodyToLaunchEnuQ30
        : pose.bodyToLaunchEnuQ30 ?? pose.bodyToRecoveryEnuQ30;
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

function interpolatePosition(left: F64x3, right: F64x3, alpha: number): F64x3 {
  return [
    left[0] + (right[0] - left[0]) * alpha,
    left[1] + (right[1] - left[1]) * alpha,
    left[2] + (right[2] - left[2]) * alpha,
  ];
}

function normalizedQuaternion(value: F64x4): F64x4 {
  const magnitude = Math.hypot(value[0], value[1], value[2], value[3]);
  if (magnitude === 0) return [1, 0, 0, 0];
  return [value[0] / magnitude, value[1] / magnitude, value[2] / magnitude, value[3] / magnitude];
}

/** Shortest-arc unit-quaternion interpolation for passive display only. */
export function interpolateQuaternion(left: F64x4, right: F64x4, alpha: number): F64x4 {
  const start = normalizedQuaternion(left);
  let finish = normalizedQuaternion(right);
  let dot = start[0] * finish[0] + start[1] * finish[1] + start[2] * finish[2] + start[3] * finish[3];
  if (dot < 0) {
    finish = [-finish[0], -finish[1], -finish[2], -finish[3]];
    dot = -dot;
  }
  if (dot > 0.9995) {
    return normalizedQuaternion([
      start[0] + (finish[0] - start[0]) * alpha,
      start[1] + (finish[1] - start[1]) * alpha,
      start[2] + (finish[2] - start[2]) * alpha,
      start[3] + (finish[3] - start[3]) * alpha,
    ]);
  }
  const angle = Math.acos(Math.max(-1, Math.min(1, dot)));
  const denominator = Math.sin(angle);
  const leftWeight = Math.sin((1 - alpha) * angle) / denominator;
  const rightWeight = Math.sin(alpha * angle) / denominator;
  return [
    start[0] * leftWeight + finish[0] * rightWeight,
    start[1] * leftWeight + finish[1] * rightWeight,
    start[2] * leftWeight + finish[2] * rightWeight,
    start[3] * leftWeight + finish[3] * rightWeight,
  ];
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
    left.validityMask !== 0n && left.validityMask === right.validityMask;
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
  const exactSnapRequired = shouldSnapExactly(previous, sample);
  const requestedAlpha = Math.max(0, Math.min(1, controls.interpolationAlpha ?? 1));
  const interpolationAlpha = exactSnapRequired || controls.motionMode === "exact" ? 1 : requestedAlpha;
  const effectiveSources = new Set(controls.visibleSources);
  if (controls.truthVisible && allowedSources.includes("truth")) {
    effectiveSources.add("truth");
  }
  const sourceStates: GlobalSceneSourceState[] = [];
  for (const source of effectiveSources) {
    if (!allowedSources.includes(source) || (source === "truth" && !controls.truthVisible)) continue;
    const value = sourcePose(sample, source);
    if (value === undefined) continue;
    const raw = positionForDomain(value, domain, sample?.segment ?? "awaiting") ??
      value.ecefPositionQ12Km ??
      value.gcrfPositionQ12Km;
    if (raw === undefined) continue;
    let positionKm = rawToKm(raw);
    let bodyQuaternion = attitudeForDomain(value, domain, sample?.segment ?? "awaiting");
    if (interpolationAlpha < 1 && compatibleForInterpolation(previous, sample, source)) {
      const prior = sourcePose(previous, source);
      const priorRaw = prior === undefined ? undefined : positionForDomain(prior, domain, previous?.segment ?? "awaiting") ??
        prior.ecefPositionQ12Km ?? prior.gcrfPositionQ12Km;
      if (prior !== undefined && priorRaw !== undefined) {
        positionKm = interpolatePosition(rawToKm(priorRaw), positionKm, interpolationAlpha);
        const priorQuaternion = attitudeForDomain(prior, domain, previous?.segment ?? "awaiting");
        if (priorQuaternion !== undefined && bodyQuaternion !== undefined) {
          bodyQuaternion = interpolateQuaternion(priorQuaternion, bodyQuaternion, interpolationAlpha);
        }
      }
    }
    sourceStates.push({
      source,
      positionKm,
      bodyQuaternion,
      modelIdentity: value.modelIdentity,
      ageReleases: value.ageReleases,
      locatorRequired: positionMagnitude(positionKm) > 1000,
    });
  }
  const primary = sourceStates.find((value) => value.source === "onboard") ??
    sourceStates.find((value) => value.source === "ground") ??
    sourceStates[0];
  const eligiblePaths = model.paths.filter((chunk) =>
    effectiveSources.has(chunk.source) &&
    allowedSources.includes(chunk.source) &&
    (chunk.source !== "truth" || controls.truthVisible) &&
    chunk.frame === domain);
  const preferredDetail = controls.pathDetail === "auto"
    ? (["launch", "chase", "recovery", "inspection"].includes(camera) ? 0 : 4)
    : controls.pathDetail;
  const selectedPaths = new Map<string, GlobalDisplayModelV1["paths"][number]>();
  for (const chunk of eligiblePaths) {
    const key = `${chunk.source}:${chunk.frame}:${chunk.modelIdentity}`;
    const current = selectedPaths.get(key);
    const score = Math.abs(chunk.lodSeconds - preferredDetail);
    const currentScore = current === undefined ? Number.POSITIVE_INFINITY : Math.abs(current.lodSeconds - preferredDetail);
    if (current === undefined || score < currentScore || (score === currentScore && chunk.lodSeconds < current.lodSeconds)) {
      selectedPaths.set(key, chunk);
    }
  }
  const paths: GlobalScenePathState[] = [...selectedPaths.values()]
    .sort((left, right) => left.pathIdentity - right.pathIdentity || left.source.localeCompare(right.source))
    .flatMap((chunk) => {
      const strips: { anchorIdentity: number; points: typeof chunk.points }[] = [];
      for (const point of chunk.points) {
        const anchorIdentity = chunk.frame === "local-enu" ? point.anchorIdentity : 0;
        const current = strips.at(-1);
        if (current === undefined || current.anchorIdentity !== anchorIdentity) {
          strips.push({ anchorIdentity, points: [point] });
        } else {
          current.points = [...current.points, point];
        }
      }
      return strips.map((strip, stripIndex) => ({
        identity: chunk.pathIdentity,
        source: chunk.source,
        anchorIdentity: strip.anchorIdentity,
        stripIndex,
        pointsKm: strip.points.map((point) => rawToKm(point.positionQ12Km)),
        dashed: chunk.source === "planned",
        stale: chunk.stale,
        incomplete: chunk.incomplete,
        lodSeconds: chunk.lodSeconds,
      }));
    });
  const anchors: GlobalSceneAnchorState[] = [];
  if (domain === "ecef") {
    anchors.push({ kind: "launch", positionKm: rawToKm(model.definition.launchAnchor.ecefPositionQ12Km) });
    if (model.definition.recoveryAnchor !== undefined) {
      anchors.push({ kind: "recovery", positionKm: rawToKm(model.definition.recoveryAnchor.ecefPositionQ12Km) });
    }
  } else if (domain === "local-enu") {
    const kind = sample?.segment === "local-recovery" ? "recovery" : "launch";
    anchors.push({ kind, positionKm: [0, 0, 0] });
  }
  return {
    releaseEpoch: sample?.releaseEpoch ?? controls.selectedRelease,
    missionTimeQ16: sample?.missionTimeQ16 ?? 0,
    frame: domain,
    segment: sample?.segment ?? "awaiting",
    camera,
    originKm: displayOrigin(camera, primary?.positionKm),
    anchors,
    sources: sourceStates,
    paths,
    truthLabelVisible: controls.truthVisible && sourceStates.some((value) => value.source === "truth"),
    exactSnapRequired,
    interpolated: interpolationAlpha < 1,
    quality: model.definition.quality,
  };
}

function semanticI32(value: number): number {
  return Math.max(-0x8000_0000, Math.min(0x7fff_ffff, Math.round(value)));
}

function hashPath(path: GlobalScenePathState): number {
  let hash = 0x811c9dc5;
  const mix = (value: number): void => {
    const raw = value >>> 0;
    for (let shift = 0; shift < 32; shift += 8) {
      hash ^= (raw >>> shift) & 0xff;
      hash = Math.imul(hash, 0x01000193) >>> 0;
    }
  };
  for (const point of path.pointsKm) {
    mix(semanticI32(point[0] * Q12_KILOMETRES));
    mix(semanticI32(point[1] * Q12_KILOMETRES));
    mix(semanticI32(point[2] * Q12_KILOMETRES));
  }
  return hash >>> 0;
}

/** Small deterministic renderer-parity record; renderer-local origins are deliberately excluded. */
export function buildGlobalSceneSemanticSnapshot(snapshot: GlobalSceneSnapshot): GlobalSceneSemanticSnapshotV1 {
  const sourceOrder: Record<GlobalDisplaySourceV1, number> = { planned: 1, onboard: 2, ground: 3, truth: 4 };
  return {
    schema: "ksa64.global-scene-semantic.v1",
    quality: snapshot.quality,
    releaseEpoch: snapshot.releaseEpoch,
    missionTimeQ16: snapshot.missionTimeQ16,
    frame: snapshot.frame,
    segment: snapshot.segment,
    camera: snapshot.camera,
    exactSnapRequired: snapshot.exactSnapRequired,
    truthLabelVisible: snapshot.truthLabelVisible,
    anchors: snapshot.anchors.map((anchor) => ({
      kind: anchor.kind,
      positionQ12Km: anchor.positionKm.map((value) => semanticI32(value * Q12_KILOMETRES)) as unknown as I32x3,
    })),
    sources: [...snapshot.sources]
      .sort((left, right) => sourceOrder[left.source] - sourceOrder[right.source])
      .map((source) => ({
        source: source.source,
        modelIdentity: source.modelIdentity,
        ageReleases: source.ageReleases,
        positionQ12Km: source.positionKm.map((value) => semanticI32(value * Q12_KILOMETRES)) as unknown as I32x3,
        bodyQuaternionQ30: source.bodyQuaternion?.map((value) => semanticI32(value * (1 << 30))) as I32x4 | undefined,
      })),
    paths: [...snapshot.paths]
      .sort((left, right) => left.identity - right.identity || left.source.localeCompare(right.source))
      .map((path) => ({
        identity: path.identity,
        source: path.source,
        anchorIdentity: path.anchorIdentity,
        stripIndex: path.stripIndex,
        lodSeconds: path.lodSeconds,
        pointCount: path.pointsKm.length,
        pointChecksum: hashPath(path),
      })),
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
