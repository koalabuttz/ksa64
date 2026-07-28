import {
  GLOBAL_PATH_FLAG_INCOMPLETE, GLOBAL_PATH_FLAG_STALE, GLOBAL_PATH_FLAG_TERMINAL,
  GLOBAL_POSE_VALID_ACTIVE_ATTITUDE, GLOBAL_POSE_VALID_ACTIVE_POSITION,
  GLOBAL_POSE_VALID_ACTIVE_VELOCITY, GLOBAL_POSE_VALID_ANGULAR_RATE,
  GLOBAL_POSE_VALID_ECEF_ATTITUDE, GLOBAL_POSE_VALID_ECEF_POSITION, GLOBAL_POSE_VALID_ECEF_VELOCITY,
  GLOBAL_POSE_VALID_GCRF_ATTITUDE, GLOBAL_POSE_VALID_GCRF_POSITION, GLOBAL_POSE_VALID_GCRF_VELOCITY,
  GLOBAL_POSE_VALID_LAUNCH_ENU_ATTITUDE, GLOBAL_POSE_VALID_LAUNCH_ENU_POSITION,
  GLOBAL_POSE_VALID_LAUNCH_ENU_VELOCITY, GLOBAL_POSE_VALID_RECOVERY_ENU_ATTITUDE,
  GLOBAL_POSE_VALID_RECOVERY_ENU_POSITION, GLOBAL_POSE_VALID_RECOVERY_ENU_VELOCITY,
  type GlobalDisplayDefinitionV1 as WireGlobalDisplayDefinitionV1,
  type GlobalDisplayPathChunkV1 as WireGlobalDisplayPathChunkV1,
  type GlobalDisplaySampleV1 as WireGlobalDisplaySampleV1,
  type GlobalDisplaySourcePoseV1 as WireGlobalDisplaySourcePoseV1,
  type GlobalDisplayTransitionV1 as WireGlobalDisplayTransitionV1,
  type GlobalReplayIndexV1 as WireGlobalReplayIndexV1,
} from "../protocol/globalDisplay";
import type {
  OperationalSnapshot,
  PredictionPathView,
  ReleaseSampleView,
  TimelineEventView,
} from "../protocol/presentation";
import { plannedTrajectory } from "./missionReference";

export const GLOBAL_DISPLAY_V1_CAPABILITY = 1n << 8n;
export const GLOBAL_DISPLAY_MODEL_IDENTITY = 0x12c0_0001;
export const Q12_KILOMETRES = 4096;

export type GlobalDisplayFrameV1 = "local-enu" | "ecef" | "gcrf";
export type GlobalDisplaySegmentV1 =
  | "local-launch"
  | "ecef-ascent"
  | "eci-coast"
  | "ecef-entry"
  | "local-recovery";
export type GlobalDisplaySourceV1 = "planned" | "onboard" | "ground" | "truth";
export type GlobalDisplayDomainV1 = "auto" | GlobalDisplayFrameV1;
export type GlobalDisplayCameraV1 =
  | "director"
  | "launch"
  | "chase"
  | "earth-fixed"
  | "inertial"
  | "recovery"
  | "free"
  | "inspection";
export type GlobalDisplayLayoutV1 = "hybrid" | "engineering" | "cinematic";
export type GlobalDisplayQualityV1 = "global-display-v1" | "legacy-schematic" | "demonstration";

export type I32x3 = readonly [number, number, number];
export type I32x4 = readonly [number, number, number, number];

export interface GlobalDisplayAnchorV1 {
  readonly identity: number;
  readonly geodeticQ28Q12: I32x3;
  readonly ecefPositionQ12Km: I32x3;
}

export interface GlobalDisplayDefinitionV1 {
  readonly modelIdentity: number;
  readonly definitionIdentity: number;
  readonly earthIdentity: number;
  readonly transformIdentity: number;
  readonly missionEpochTaiSeconds: bigint;
  readonly equatorialRadiusQ12Km: number;
  readonly polarRadiusQ12Km: number;
  readonly launchAnchor: GlobalDisplayAnchorV1;
  readonly recoveryAnchor?: GlobalDisplayAnchorV1;
  readonly availableFrames: readonly GlobalDisplayFrameV1[];
  readonly availableSources: readonly GlobalDisplaySourceV1[];
  readonly availableCameras: readonly GlobalDisplayCameraV1[];
  readonly quality: GlobalDisplayQualityV1;
}

export interface GlobalDisplaySourcePoseV1 {
  readonly source: GlobalDisplaySourceV1;
  readonly modelIdentity: number;
  readonly sourceEstimateIdentity: number;
  readonly sourceChecksum: number;
  readonly ageReleases: number;
  readonly validityMask: bigint;
  readonly ecefPositionQ12Km?: I32x3;
  readonly ecefVelocityQ24KmS?: I32x3;
  readonly gcrfPositionQ12Km?: I32x3;
  readonly gcrfVelocityQ24KmS?: I32x3;
  readonly launchEnuPositionQ12Km?: I32x3;
  readonly launchEnuVelocityQ24KmS?: I32x3;
  readonly recoveryEnuPositionQ12Km?: I32x3;
  readonly recoveryEnuVelocityQ24KmS?: I32x3;
  readonly bodyToLaunchEnuQ30?: I32x4;
  readonly bodyToRecoveryEnuQ30?: I32x4;
  readonly bodyToEcefQ30?: I32x4;
  readonly bodyToGcrfQ30?: I32x4;
  readonly bodyAngularRateQ24RadS?: I32x3;
}

export interface GlobalDisplaySampleV1 {
  readonly sequence: bigint;
  readonly releaseEpoch: number;
  readonly missionTimeQ16: number;
  readonly segment: GlobalDisplaySegmentV1;
  readonly authoritativeFrame: GlobalDisplayFrameV1;
  readonly eventMask: bigint;
  readonly discontinuityMask: bigint;
  readonly continuityIdentity: number;
  readonly geodeticQ28Q12: I32x3;
  readonly altitudeQ12Km: number;
  readonly machQ24: number;
  readonly dynamicPressureQ14Pa: number;
  readonly totalMassQ21Kg: number;
  readonly mainPropellantQ21Kg: number;
  readonly rcsPropellantQ21Kg: number;
  readonly componentStatusMask: bigint;
  readonly poses: readonly GlobalDisplaySourcePoseV1[];
}

export interface GlobalDisplayPathPointV1 {
  readonly releaseEpoch: number;
  readonly missionTimeQ16: number;
  readonly segment: GlobalDisplaySegmentV1;
  readonly eventMask: bigint;
  readonly anchorIdentity: number;
  readonly positionQ12Km: I32x3;
}

export interface GlobalDisplayPathChunkV1 {
  readonly pathIdentity: number;
  readonly modelIdentity: number;
  readonly sourceEstimateIdentity: number;
  readonly sourceChecksum: number;
  readonly continuityIdentity: number;
  readonly flags: number;
  readonly source: GlobalDisplaySourceV1;
  readonly frame: GlobalDisplayFrameV1;
  readonly lodSeconds: 0 | 1 | 4;
  readonly chunkSequence: number;
  readonly validityMask: bigint;
  readonly stale: boolean;
  readonly incomplete: boolean;
  readonly terminal: boolean;
  readonly points: readonly GlobalDisplayPathPointV1[];
}

export interface GlobalDisplayTransitionV1 {
  readonly identity: number;
  readonly releaseEpoch: number;
  readonly missionTimeQ16: number;
  readonly oldFrame: GlobalDisplayFrameV1;
  readonly newFrame: GlobalDisplayFrameV1;
  readonly oldSegment: GlobalDisplaySegmentV1;
  readonly newSegment: GlobalDisplaySegmentV1;
  readonly transformIdentity: number;
  readonly anchorIdentity: number;
  readonly continuityPositionQ12Km: number;
  readonly continuityVelocityQ24KmS: number;
  readonly continuityAttitudeQ30: number;
  readonly continuityAngularRateQ24: number;
  readonly reasonIdentity: number;
}

export interface GlobalReplayMarkerV1 {
  readonly identity: number;
  readonly releaseEpoch: number;
  readonly kind: "event" | "transition" | "action" | "fault" | "terminal";
  readonly label: string;
}

export interface GlobalReplayIndexV1 {
  readonly firstRelease: number;
  readonly lastRelease: number;
  readonly selectedRelease: number;
  readonly markers: readonly GlobalReplayMarkerV1[];
  readonly terminalDispositionIdentity: number;
}

export interface GlobalDisplayModelV1 {
  readonly definition: GlobalDisplayDefinitionV1;
  readonly samples: readonly GlobalDisplaySampleV1[];
  readonly paths: readonly GlobalDisplayPathChunkV1[];
  readonly transitions: readonly GlobalDisplayTransitionV1[];
  readonly replay: GlobalReplayIndexV1;
}

export interface OperationalDisplayInput {
  readonly snapshot?: OperationalSnapshot;
  readonly samples: readonly ReleaseSampleView[];
  readonly paths: ReadonlyMap<number, PredictionPathView>;
  readonly timeline: readonly TimelineEventView[];
  readonly truthAllowed: boolean;
  readonly demonstration?: boolean;
}

const WGS84_A_KM = 6378.137;
const WGS84_B_KM = 6356.752314245;
const LAUNCH_LATITUDE_DEGREES = 28.5;
const LAUNCH_LONGITUDE_DEGREES = -80.6;
const REFERENCE_EARTH_IDENTITY = 0x1000_0001;
const REFERENCE_TRANSFORM_IDENTITY = 0x1000_0002;
const PLANNED_MODEL_IDENTITY = 0x120c_1001;
const ONBOARD_MODEL_IDENTITY = 0x120c_1002;
const GROUND_MODEL_IDENTITY = 0x120c_1003;

function i32(value: number): number {
  return Math.max(-0x8000_0000, Math.min(0x7fff_ffff, Math.round(value)));
}

function q12Km(value: number): number {
  return i32(value * Q12_KILOMETRES);
}

function degrees(value: number): number {
  return value * Math.PI / 180;
}

function geodeticToEcefQ12(
  latitudeDegrees: number,
  longitudeDegrees: number,
  altitudeKm: number,
): I32x3 {
  const latitude = degrees(latitudeDegrees);
  const longitude = degrees(longitudeDegrees);
  const flattening = (WGS84_A_KM - WGS84_B_KM) / WGS84_A_KM;
  const eccentricitySquared = flattening * (2 - flattening);
  const sinLatitude = Math.sin(latitude);
  const primeVertical = WGS84_A_KM / Math.sqrt(1 - eccentricitySquared * sinLatitude * sinLatitude);
  return [
    q12Km((primeVertical + altitudeKm) * Math.cos(latitude) * Math.cos(longitude)),
    q12Km((primeVertical + altitudeKm) * Math.cos(latitude) * Math.sin(longitude)),
    q12Km((primeVertical * (1 - eccentricitySquared) + altitudeKm) * sinLatitude),
  ];
}

function schematicGroundPointQ12(downrangeKm: number, crossrangeKm: number, altitudeKm: number): I32x3 {
  const meanRadius = (2 * WGS84_A_KM + WGS84_B_KM) / 3;
  const latitude = LAUNCH_LATITUDE_DEGREES + (crossrangeKm / meanRadius) * 180 / Math.PI;
  const cosine = Math.max(0.15, Math.cos(degrees(latitude)));
  const longitude = LAUNCH_LONGITUDE_DEGREES + (downrangeKm / (meanRadius * cosine)) * 180 / Math.PI;
  return geodeticToEcefQ12(latitude, longitude, altitudeKm);
}

function frameFromIdentity(identity: number | undefined): GlobalDisplayFrameV1 {
  switch (identity) {
    case 1: return "local-enu";
    case 3: return "gcrf";
    default: return "ecef";
  }
}

export function segmentAtRelease(releaseEpoch: number): GlobalDisplaySegmentV1 {
  if (releaseEpoch < 29) return "local-launch";
  if (releaseEpoch < 3579) return "ecef-ascent";
  if (releaseEpoch < 12_669) return "eci-coast";
  if (releaseEpoch < 15_255) return "ecef-entry";
  return "local-recovery";
}

function pose(
  source: GlobalDisplaySourceV1,
  modelIdentity: number,
  position: I32x3,
  velocity?: I32x3,
  checksum = 0,
): GlobalDisplaySourcePoseV1 {
  return {
    source,
    modelIdentity,
    sourceEstimateIdentity: modelIdentity,
    sourceChecksum: checksum,
    ageReleases: 0,
    validityMask: 1n,
    ecefPositionQ12Km: position,
    ecefVelocityQ24KmS: velocity,
  };
}

function sampleFromOperational(value: ReleaseSampleView): GlobalDisplaySampleV1 {
  const altitudeKm = value.altitudeQ12Km / Q12_KILOMETRES;
  const downrangeKm = value.downrangeQ12Km / Q12_KILOMETRES;
  const crossrangeKm = value.crossrangeQ12Km / Q12_KILOMETRES;
  const ecef = schematicGroundPointQ12(downrangeKm, crossrangeKm, altitudeKm);
  const onboard = pose("onboard", ONBOARD_MODEL_IDENTITY, ecef, value.onboardVelocity);
  const ground = pose("ground", GROUND_MODEL_IDENTITY, ecef, value.groundVelocity);
  return {
    sequence: value.sequence,
    releaseEpoch: value.releaseEpoch,
    missionTimeQ16: value.missionTimeQ16,
    segment: segmentAtRelease(value.releaseEpoch),
    authoritativeFrame: frameFromIdentity(value.frameIdentity),
    eventMask: 0n,
    discontinuityMask: 0n,
    continuityIdentity: value.frameIdentity || 1,
    geodeticQ28Q12: [0, 0, 0],
    altitudeQ12Km: value.altitudeQ12Km,
    machQ24: 0,
    dynamicPressureQ14Pa: 0,
    totalMassQ21Kg: 0,
    mainPropellantQ21Kg: 0,
    rcsPropellantQ21Kg: 0,
    componentStatusMask: 0n,
    poses: [onboard, ground],
  };
}

function currentSample(input: OperationalDisplayInput): GlobalDisplaySampleV1 | undefined {
  const latest = input.samples.at(-1);
  if (latest !== undefined) return sampleFromOperational(latest);
  const snapshot = input.snapshot;
  if (snapshot === undefined) return undefined;
  const position = schematicGroundPointQ12(
    0,
    0,
    snapshot.onboard.position[2] / Q12_KILOMETRES,
  );
  return {
    sequence: snapshot.publicationSequence,
    releaseEpoch: snapshot.releaseEpoch,
    missionTimeQ16: snapshot.missionTimeQ16,
    segment: segmentAtRelease(snapshot.releaseEpoch),
    authoritativeFrame: frameFromIdentity(snapshot.frameIdentity),
    eventMask: 0n,
    discontinuityMask: 0n,
    continuityIdentity: snapshot.frameIdentity || 1,
    geodeticQ28Q12: [0, 0, 0],
    altitudeQ12Km: snapshot.onboard.position[2],
    machQ24: 0,
    dynamicPressureQ14Pa: 0,
    totalMassQ21Kg: 0,
    mainPropellantQ21Kg: 0,
    rcsPropellantQ21Kg: 0,
    componentStatusMask: 0n,
    poses: [
      pose("onboard", ONBOARD_MODEL_IDENTITY, position, snapshot.onboard.velocity, snapshot.onboard.checksum),
      pose("ground", GROUND_MODEL_IDENTITY, position, snapshot.ground.velocity, snapshot.ground.checksum),
    ],
  };
}

function pathPoint(
  releaseEpoch: number,
  downrangeQ12Km: number,
  crossrangeQ12Km: number,
  altitudeQ12Km: number,
): GlobalDisplayPathPointV1 {
  return {
    releaseEpoch,
    missionTimeQ16: releaseEpoch * 2048,
    segment: segmentAtRelease(releaseEpoch),
    eventMask: 0n,
    anchorIdentity: 0,
    positionQ12Km: schematicGroundPointQ12(
      downrangeQ12Km / Q12_KILOMETRES,
      crossrangeQ12Km / Q12_KILOMETRES,
      altitudeQ12Km / Q12_KILOMETRES,
    ),
  };
}

function operationalPath(
  source: GlobalDisplaySourceV1,
  identity: number,
  path: PredictionPathView | undefined,
  fallbackSamples: readonly ReleaseSampleView[],
): GlobalDisplayPathChunkV1 | undefined {
  const points = path?.points.map((point) => pathPoint(
    point.releaseEpoch,
    point.downrangeQ12Km,
    point.crossrangeQ12Km,
    point.altitudeQ12Km,
  )) ?? fallbackSamples.map((value) => pathPoint(
    value.releaseEpoch,
    value.downrangeQ12Km,
    value.crossrangeQ12Km,
    value.altitudeQ12Km,
  ));
  if (points.length === 0) return undefined;
  return {
    pathIdentity: path?.pathIdentity ?? identity,
    modelIdentity: path?.modelIdentity ?? identity,
    sourceEstimateIdentity: path?.sourceEstimateIdentity ?? identity,
    sourceChecksum: path?.sourceEstimateChecksum ?? 0,
    continuityIdentity: 1,
    flags: path?.terminalReason === 1 ? GLOBAL_PATH_FLAG_TERMINAL : 0,
    source,
    frame: "ecef",
    lodSeconds: 1,
    chunkSequence: 1,
    validityMask: 1n,
    stale: false,
    incomplete: false,
    terminal: path?.terminalReason === 1,
    points,
  };
}

function plannedPath(): GlobalDisplayPathChunkV1 {
  const lastRelease = 22_014;
  return {
    pathIdentity: PLANNED_MODEL_IDENTITY,
    modelIdentity: PLANNED_MODEL_IDENTITY,
    sourceEstimateIdentity: PLANNED_MODEL_IDENTITY,
    sourceChecksum: 0,
    continuityIdentity: 1,
    flags: GLOBAL_PATH_FLAG_TERMINAL,
    source: "planned",
    frame: "ecef",
    lodSeconds: 4,
    chunkSequence: 1,
    validityMask: 1n,
    stale: false,
    incomplete: false,
    terminal: true,
    points: plannedTrajectory.map((point, index) => pathPoint(
      Math.round(index / Math.max(1, plannedTrajectory.length - 1) * lastRelease),
      q12Km(point.downrange),
      0,
      q12Km(point.altitude),
    )),
  };
}

function replayMarkers(input: OperationalDisplayInput): readonly GlobalReplayMarkerV1[] {
  const fixed: GlobalReplayMarkerV1[] = [
    { identity: 0x120c_3001, releaseEpoch: 29, kind: "transition", label: "ENU → ECEF" },
    { identity: 0x120c_3002, releaseEpoch: 1920, kind: "event", label: "Burnout" },
    { identity: 0x120c_3003, releaseEpoch: 3579, kind: "transition", label: "ECEF → GCRF" },
    { identity: 0x120c_3004, releaseEpoch: 8124, kind: "event", label: "Apogee" },
    { identity: 0x120c_3005, releaseEpoch: 12_669, kind: "transition", label: "GCRF → ECEF" },
    { identity: 0x120c_3006, releaseEpoch: 15_255, kind: "transition", label: "ECEF → recovery ENU" },
    { identity: 0x120c_3007, releaseEpoch: 15_257, kind: "event", label: "Drogue deployment" },
    { identity: 0x120c_3008, releaseEpoch: 20_929, kind: "event", label: "Main deployment" },
    { identity: 0x120c_3009, releaseEpoch: 22_014, kind: "terminal", label: "Landing" },
  ];
  const timeline = input.timeline.map((value) => ({
    identity: value.eventIdentity,
    releaseEpoch: value.releaseEpoch,
    kind: "event" as const,
    label: value.label,
  }));
  const byReleaseAndIdentity = new Map<string, GlobalReplayMarkerV1>();
  for (const marker of [...fixed, ...timeline]) {
    byReleaseAndIdentity.set(`${marker.releaseEpoch}:${marker.identity}`, marker);
  }
  return [...byReleaseAndIdentity.values()].sort((left, right) =>
    left.releaseEpoch - right.releaseEpoch || left.identity - right.identity);
}

/**
 * Temporary compatibility view for the Phase 12B two-dimensional stream.
 * It is visibly schematic, never carries GlobalDisplayV1 quality, and is not
 * accepted coordinate or renderer-parity evidence. Only Rust-produced absolute
 * poses may enter the GlobalDisplayV1 acceptance path.
 */
export function buildLegacySchematicDisplay(input: OperationalDisplayInput): GlobalDisplayModelV1 {
  const samples = input.samples.map(sampleFromOperational);
  const latest = currentSample(input);
  if (samples.length === 0 && latest !== undefined) samples.push(latest);
  const availableSources: GlobalDisplaySourceV1[] = ["planned", "onboard", "ground"];
  if (input.truthAllowed) availableSources.push("truth");
  const paths = [
    plannedPath(),
    operationalPath("onboard", ONBOARD_MODEL_IDENTITY, input.paths.get(2), input.samples),
    operationalPath("ground", GROUND_MODEL_IDENTITY, input.paths.get(3), input.samples),
  ].filter((value): value is GlobalDisplayPathChunkV1 => value !== undefined);
  const selectedRelease = latest?.releaseEpoch ?? 0;
  return {
    definition: {
      modelIdentity: GLOBAL_DISPLAY_MODEL_IDENTITY,
      definitionIdentity: input.snapshot?.definitionIdentity ?? 0x12c0_4001,
      earthIdentity: REFERENCE_EARTH_IDENTITY,
      transformIdentity: REFERENCE_TRANSFORM_IDENTITY,
      missionEpochTaiSeconds: 0n,
      equatorialRadiusQ12Km: q12Km(WGS84_A_KM),
      polarRadiusQ12Km: q12Km(WGS84_B_KM),
      launchAnchor: {
        identity: 1,
        geodeticQ28Q12: [
          i32(degrees(LAUNCH_LATITUDE_DEGREES) * (1 << 28)),
          i32(degrees(LAUNCH_LONGITUDE_DEGREES) * (1 << 28)),
          q12Km(0.003),
        ],
        ecefPositionQ12Km: geodeticToEcefQ12(
          LAUNCH_LATITUDE_DEGREES,
          LAUNCH_LONGITUDE_DEGREES,
          0.003,
        ),
      },
      availableFrames: ["local-enu", "ecef", "gcrf"],
      availableSources,
      availableCameras: ["director", "launch", "chase", "earth-fixed", "inertial", "recovery", "free", "inspection"],
      quality: input.demonstration ? "demonstration" : "legacy-schematic",
    },
    samples,
    paths,
    transitions: [],
    replay: {
      firstRelease: samples[0]?.releaseEpoch ?? 0,
      lastRelease: samples.at(-1)?.releaseEpoch ?? selectedRelease,
      selectedRelease,
      markers: replayMarkers(input),
      terminalDispositionIdentity: 0,
    },
  };
}

export function findSampleAtOrBefore(
  samples: readonly GlobalDisplaySampleV1[],
  releaseEpoch: number,
): GlobalDisplaySampleV1 | undefined {
  let low = 0;
  let high = samples.length - 1;
  let result: GlobalDisplaySampleV1 | undefined;
  while (low <= high) {
    const middle = Math.floor((low + high) / 2);
    const value = samples[middle];
    if (value === undefined) break;
    if (value.releaseEpoch <= releaseEpoch) {
      result = value;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  return result;
}

export function directorCameraForSample(sample: GlobalDisplaySampleV1 | undefined): GlobalDisplayCameraV1 {
  switch (sample?.segment) {
    case "local-launch": return "launch";
    case "ecef-ascent": return sample.releaseEpoch < 1920 ? "chase" : "earth-fixed";
    case "eci-coast": return "inertial";
    case "ecef-entry": return "earth-fixed";
    case "local-recovery": return "recovery";
    default: return "earth-fixed";
  }
}

export function displayDomainForSample(
  requested: GlobalDisplayDomainV1,
  sample: GlobalDisplaySampleV1 | undefined,
): GlobalDisplayFrameV1 {
  return requested === "auto" ? sample?.authoritativeFrame ?? "ecef" : requested;
}

export function sourcePose(
  sample: GlobalDisplaySampleV1 | undefined,
  source: GlobalDisplaySourceV1,
): GlobalDisplaySourcePoseV1 | undefined {
  return sample?.poses.find((value) => value.source === source);
}


export interface ExactGlobalDisplayInput {
  readonly definition: WireGlobalDisplayDefinitionV1;
  readonly samples: readonly WireGlobalDisplaySampleV1[];
  readonly paths: ReadonlyMap<string, WireGlobalDisplayPathChunkV1>;
  readonly transitions: readonly WireGlobalDisplayTransitionV1[];
  readonly replay?: WireGlobalReplayIndexV1;
}
function wireFrame(value: number): GlobalDisplayFrameV1 { switch (value) { case 1: return "local-enu"; case 2: return "ecef"; case 3: return "gcrf"; default: throw new Error("unsupported global display frame"); } }
function wireSegment(value: number): GlobalDisplaySegmentV1 { switch (value) { case 1: return "local-launch"; case 2: return "ecef-ascent"; case 3: return "eci-coast"; case 4: return "ecef-entry"; case 5: return "local-recovery"; default: throw new Error("unsupported global display segment"); } }
function wireSource(value: number): GlobalDisplaySourceV1 { switch (value) { case 1: return "planned"; case 2: return "onboard"; case 3: return "ground"; case 4: return "truth"; default: throw new Error("unsupported global display source"); } }
function valid(mask: number, bit: number): boolean { return (mask & bit) !== 0; }
function wirePose(value: WireGlobalDisplaySourcePoseV1, segment: GlobalDisplaySegmentV1): GlobalDisplaySourcePoseV1 {
  const activeFrame = wireFrame(value.activeFrame);
  const localIsRecovery = segment === "local-recovery";
  const activePositionValid = valid(value.validityMask, GLOBAL_POSE_VALID_ACTIVE_POSITION);
  const activeVelocityValid = valid(value.validityMask, GLOBAL_POSE_VALID_ACTIVE_VELOCITY);
  const ecefPosition = valid(value.validityMask, GLOBAL_POSE_VALID_ECEF_POSITION) ? value.ecef.positionQ12Km
    : activeFrame === "ecef" && activePositionValid ? value.active.positionQ12Km : undefined;
  const ecefVelocity = valid(value.validityMask, GLOBAL_POSE_VALID_ECEF_VELOCITY) ? value.ecef.velocityQ24KmS
    : activeFrame === "ecef" && activeVelocityValid ? value.active.velocityQ24KmS : undefined;
  const gcrfPosition = valid(value.validityMask, GLOBAL_POSE_VALID_GCRF_POSITION) ? value.gcrf.positionQ12Km
    : activeFrame === "gcrf" && activePositionValid ? value.active.positionQ12Km : undefined;
  const gcrfVelocity = valid(value.validityMask, GLOBAL_POSE_VALID_GCRF_VELOCITY) ? value.gcrf.velocityQ24KmS
    : activeFrame === "gcrf" && activeVelocityValid ? value.active.velocityQ24KmS : undefined;
  const launchActive = activeFrame === "local-enu" && !localIsRecovery;
  const recoveryActive = activeFrame === "local-enu" && localIsRecovery;
  return { source: wireSource(value.source), modelIdentity: value.modelIdentity, sourceEstimateIdentity: value.estimateIdentity,
    sourceChecksum: value.checksum, ageReleases: value.ageReleases, validityMask: BigInt(value.validityMask),
    ecefPositionQ12Km: ecefPosition, ecefVelocityQ24KmS: ecefVelocity, gcrfPositionQ12Km: gcrfPosition,
    gcrfVelocityQ24KmS: gcrfVelocity,
    launchEnuPositionQ12Km: valid(value.validityMask, GLOBAL_POSE_VALID_LAUNCH_ENU_POSITION) ? value.launchEnu.positionQ12Km : launchActive && activePositionValid ? value.active.positionQ12Km : undefined,
    launchEnuVelocityQ24KmS: valid(value.validityMask, GLOBAL_POSE_VALID_LAUNCH_ENU_VELOCITY) ? value.launchEnu.velocityQ24KmS : launchActive && activeVelocityValid ? value.active.velocityQ24KmS : undefined,
    recoveryEnuPositionQ12Km: valid(value.validityMask, GLOBAL_POSE_VALID_RECOVERY_ENU_POSITION) ? value.recoveryEnu.positionQ12Km : recoveryActive && activePositionValid ? value.active.positionQ12Km : undefined,
    recoveryEnuVelocityQ24KmS: valid(value.validityMask, GLOBAL_POSE_VALID_RECOVERY_ENU_VELOCITY) ? value.recoveryEnu.velocityQ24KmS : recoveryActive && activeVelocityValid ? value.active.velocityQ24KmS : undefined,
    bodyToLaunchEnuQ30: valid(value.validityMask, GLOBAL_POSE_VALID_LAUNCH_ENU_ATTITUDE) ? value.launchEnu.attitudeQ30 : launchActive && valid(value.validityMask, GLOBAL_POSE_VALID_ACTIVE_ATTITUDE) ? value.active.attitudeQ30 : undefined,
    bodyToRecoveryEnuQ30: valid(value.validityMask, GLOBAL_POSE_VALID_RECOVERY_ENU_ATTITUDE) ? value.recoveryEnu.attitudeQ30 : recoveryActive && valid(value.validityMask, GLOBAL_POSE_VALID_ACTIVE_ATTITUDE) ? value.active.attitudeQ30 : undefined,
    bodyToEcefQ30: valid(value.validityMask, GLOBAL_POSE_VALID_ECEF_ATTITUDE) ? value.ecef.attitudeQ30 : activeFrame === "ecef" && valid(value.validityMask, GLOBAL_POSE_VALID_ACTIVE_ATTITUDE) ? value.active.attitudeQ30 : undefined,
    bodyToGcrfQ30: valid(value.validityMask, GLOBAL_POSE_VALID_GCRF_ATTITUDE) ? value.gcrf.attitudeQ30 : activeFrame === "gcrf" && valid(value.validityMask, GLOBAL_POSE_VALID_ACTIVE_ATTITUDE) ? value.active.attitudeQ30 : undefined,
    bodyAngularRateQ24RadS: valid(value.validityMask, GLOBAL_POSE_VALID_ANGULAR_RATE) ? value.angularRateQ24 : undefined };
}
function replayKind(value: number): GlobalReplayMarkerV1["kind"] { switch (value) { case 1: return "event"; case 2: return "transition"; case 3: return "action"; case 4: return "fault"; case 5: return "terminal"; default: throw new Error("unsupported global replay entry kind"); } }
function replayLabel(release: number, identity: number): string {
  const known = new Map<number, string>([[29, "ENU → ECEF"], [1920, "Burnout"], [3579, "ECEF → GCRF"], [8124, "Apogee"], [12_669, "GCRF → ECEF"], [15_255, "ECEF → recovery ENU"], [15_257, "Drogue deployment"], [20_929, "Main deployment"], [22_014, "Landing"]]);
  return known.get(release) ?? ("Event 0x" + identity.toString(16).toUpperCase());
}
/** Builds the accepted renderer model exclusively from Rust-resolved absolute poses. */
export function buildExactGlobalDisplay(input: ExactGlobalDisplayInput): GlobalDisplayModelV1 {
  const sourceValues: GlobalDisplaySourceV1[] = []; for (let source = 1; source <= 4; source += 1) if ((input.definition.availableSourceMask & (1 << (source - 1))) !== 0) sourceValues.push(wireSource(source));
  const frameValues: GlobalDisplayFrameV1[] = []; for (let frame = 1; frame <= 3; frame += 1) if ((input.definition.availableFrameMask & (1 << (frame - 1))) !== 0) frameValues.push(wireFrame(frame));
  const samples: GlobalDisplaySampleV1[] = input.samples.map((value) => ({ sequence: value.sequence, releaseEpoch: value.releaseEpoch,
    missionTimeQ16: value.missionTimeQ16, segment: wireSegment(value.segment), authoritativeFrame: wireFrame(value.activeFrame),
    eventMask: BigInt(value.eventMask), discontinuityMask: BigInt(value.discontinuityMask), continuityIdentity: value.continuityIdentity,
    geodeticQ28Q12: value.geodeticQ28Q12, altitudeQ12Km: value.altitudeQ12Km, machQ24: value.machQ24,
    dynamicPressureQ14Pa: value.dynamicPressureQ14Pa, totalMassQ21Kg: value.totalMassQ21Kg,
    mainPropellantQ21Kg: value.mainPropellantQ21Kg, rcsPropellantQ21Kg: value.rcsPropellantQ21Kg,
    componentStatusMask: BigInt(value.commandFlags) | (BigInt(value.commandDiscrete) << 8n) | (BigInt(value.alarms) << 16n), poses: value.sources.map((pose) => wirePose(pose, wireSegment(value.segment))) }));
  const grouped = new Map<string, WireGlobalDisplayPathChunkV1[]>();
  for (const value of input.paths.values()) {
    const key = [value.pathIdentity, value.source, value.displayFrame, value.lod].join(":");
    const chunks = grouped.get(key) ?? [];
    chunks.push(value);
    grouped.set(key, chunks);
  }
  const paths: GlobalDisplayPathChunkV1[] = [];
  for (const chunks of grouped.values()) {
    const first = chunks[0];
    if (first === undefined) continue;
    if (!Number.isSafeInteger(first.chunkCount) || first.chunkCount <= 0 || chunks.length !== first.chunkCount) {
      throw new Error("exact global display path has a missing or extra chunk");
    }
    const seen = new Set<number>();
    for (const chunk of chunks) {
      const metadataMatches = chunk.pathIdentity === first.pathIdentity && chunk.source === first.source &&
        chunk.displayFrame === first.displayFrame && chunk.lod === first.lod && chunk.flags === first.flags &&
        chunk.modelIdentity === first.modelIdentity && chunk.estimateIdentity === first.estimateIdentity &&
        chunk.sourceChecksum === first.sourceChecksum && chunk.continuityIdentity === first.continuityIdentity &&
        chunk.chunkCount === first.chunkCount;
      if (!metadataMatches) throw new Error("exact global display path chunk metadata changed within one path");
      if (!Number.isSafeInteger(chunk.chunkIndex) || chunk.chunkIndex < 0 || chunk.chunkIndex >= first.chunkCount ||
        seen.has(chunk.chunkIndex)) throw new Error("exact global display path contains a duplicate or invalid chunk index");
      seen.add(chunk.chunkIndex);
    }
    for (let index = 0; index < first.chunkCount; index += 1) {
      if (!seen.has(index)) throw new Error("exact global display path is missing a chunk index");
    }
    chunks.sort((left, right) => left.chunkIndex - right.chunkIndex);
    for (let index = 1; index < chunks.length; index += 1) {
      const previous = chunks[index - 1]?.points.at(-1);
      const current = chunks[index]?.points[0];
      if (previous !== undefined && current !== undefined &&
        (current.releaseEpoch <= previous.releaseEpoch || current.missionTimeQ16 <= previous.missionTimeQ16)) {
        throw new Error("exact global display path point order changed between chunks");
      }
    }
    paths.push({
      pathIdentity: first.pathIdentity,
      modelIdentity: first.modelIdentity,
      sourceEstimateIdentity: first.estimateIdentity,
      sourceChecksum: first.sourceChecksum,
      continuityIdentity: first.continuityIdentity,
      flags: first.flags,
      source: wireSource(first.source),
      frame: wireFrame(first.displayFrame),
      lodSeconds: first.lod === 1 ? 0 : first.lod === 2 ? 1 : 4,
      chunkSequence: first.sourceChecksum,
      validityMask: BigInt(first.continuityIdentity),
      stale: (first.flags & GLOBAL_PATH_FLAG_STALE) !== 0,
      incomplete: (first.flags & GLOBAL_PATH_FLAG_INCOMPLETE) !== 0,
      terminal: (first.flags & GLOBAL_PATH_FLAG_TERMINAL) !== 0,
      points: chunks.flatMap((chunk) => chunk.points.map((point) => ({
        releaseEpoch: point.releaseEpoch,
        missionTimeQ16: point.missionTimeQ16,
        segment: wireSegment(point.segment),
        eventMask: BigInt(point.eventMask),
        anchorIdentity: point.anchorIdentity,
        positionQ12Km: point.positionQ12Km,
      }))),
    });
  }
  paths.sort((left, right) => left.pathIdentity - right.pathIdentity || left.source.localeCompare(right.source) || left.lodSeconds - right.lodSeconds);
  const transitions: GlobalDisplayTransitionV1[] = input.transitions.map((value) => ({ identity: value.transitionIdentity,
    releaseEpoch: value.releaseEpoch, missionTimeQ16: value.missionTimeQ16, oldFrame: wireFrame(value.fromFrame), newFrame: wireFrame(value.toFrame),
    oldSegment: wireSegment(value.fromSegment), newSegment: wireSegment(value.toSegment), transformIdentity: value.transformIdentity,
    anchorIdentity: value.anchorIdentity, continuityPositionQ12Km: Math.abs(value.positionMaxDeltaRaw),
    continuityVelocityQ24KmS: Math.abs(value.velocityMaxDeltaRaw), continuityAttitudeQ30: Math.abs(value.attitudeMaxDeltaRaw),
    continuityAngularRateQ24: Math.abs(value.angularRateMaxDeltaRaw), reasonIdentity: value.reason }));
  const replayEntries = input.replay?.entries.map((value) => ({ identity: value.eventIdentity, releaseEpoch: value.releaseEpoch,
    kind: replayKind(value.kind), label: replayLabel(value.releaseEpoch, value.eventIdentity) })) ?? [];
  const firstRelease = input.replay?.firstRelease ?? samples[0]?.releaseEpoch ?? 0;
  const lastRelease = input.replay?.lastRelease ?? samples.at(-1)?.releaseEpoch ?? firstRelease;
  return { definition: { modelIdentity: input.definition.displayIdentity, definitionIdentity: input.definition.missionIdentity,
      earthIdentity: input.definition.earthIdentity, transformIdentity: input.definition.transformIdentity,
      missionEpochTaiSeconds: BigInt(input.definition.epochUnixDay) * 86_400n + BigInt(input.definition.epochTaiMinusUtc), equatorialRadiusQ12Km: input.definition.semiMajorQ12Km,
      polarRadiusQ12Km: input.definition.semiMinorQ12Km,
      launchAnchor: { identity: input.definition.launchAnchor.identity, geodeticQ28Q12: input.definition.launchAnchor.geodeticQ28Q12,
        ecefPositionQ12Km: input.definition.launchAnchor.ecefPositionQ12Km },
      recoveryAnchor: { identity: input.definition.recoveryAnchor.identity, geodeticQ28Q12: input.definition.recoveryAnchor.geodeticQ28Q12,
        ecefPositionQ12Km: input.definition.recoveryAnchor.ecefPositionQ12Km },
      availableFrames: frameValues, availableSources: sourceValues,
      availableCameras: ["director", "launch", "chase", "earth-fixed", "inertial", "recovery", "free", "inspection"], quality: "global-display-v1" },
    samples, paths, transitions, replay: { firstRelease, lastRelease, selectedRelease: samples.at(-1)?.releaseEpoch ?? firstRelease,
      markers: replayEntries, terminalDispositionIdentity: input.replay?.terminalDisposition ?? 0 } };
}
