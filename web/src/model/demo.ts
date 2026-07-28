import type { TrajectoryPoint } from "./trajectory";

export type { TrajectoryPoint } from "./trajectory";

export interface ProcedureStep {
  readonly id: string;
  readonly title: string;
  readonly detail: string;
  readonly status: "complete" | "active" | "pending";
}

export interface TimelineEvent {
  readonly time: string;
  readonly title: string;
  readonly detail: string;
  readonly severity: "nominal" | "advisory" | "caution";
}

export const onboardTrajectory: readonly TrajectoryPoint[] = [
  { downrange: 0, altitude: 0 },
  { downrange: 27, altitude: 41 },
  { downrange: 81, altitude: 116 },
  { downrange: 156, altitude: 188 },
  { downrange: 246, altitude: 234 },
  { downrange: 340, altitude: 247 },
  { downrange: 434, altitude: 226 },
  { downrange: 515, altitude: 169 },
  { downrange: 584, altitude: 84 },
  { downrange: 629, altitude: 16 },
];

export const groundTrajectory: readonly TrajectoryPoint[] = [
  { downrange: 0, altitude: 0 },
  { downrange: 26, altitude: 42 },
  { downrange: 79, altitude: 117 },
  { downrange: 153, altitude: 190 },
  { downrange: 242, altitude: 237 },
  { downrange: 337, altitude: 250 },
  { downrange: 431, altitude: 229 },
  { downrange: 513, altitude: 172 },
  { downrange: 582, altitude: 86 },
  { downrange: 627, altitude: 17 },
];

export const procedures: readonly ProcedureStep[] = [
  {
    id: "nav-1",
    title: "Confirm GNSS invalid",
    detail: "GPS aiding is invalid; inertial propagation remains healthy.",
    status: "complete",
  },
  {
    id: "nav-2",
    title: "Compare velocity residual",
    detail: "Ground and onboard residual is inside the bounded update gate.",
    status: "complete",
  },
  {
    id: "nav-3",
    title: "Review ground-state update",
    detail: "Proposal GSU-04 is ready for operator review.",
    status: "active",
  },
  {
    id: "nav-4",
    title: "Select continuation branch",
    detail: "Continue inertial coast after the committed correction.",
    status: "pending",
  },
];

export const timeline: readonly TimelineEvent[] = [
  {
    time: "T+06:14",
    title: "GNSS aiding lost",
    detail: "Onboard inertial navigation continued without reset.",
    severity: "caution",
  },
  {
    time: "T+06:16",
    title: "Procedure entered",
    detail: "ASCENT / LOSS OF GPS AIDING",
    severity: "advisory",
  },
  {
    time: "T+06:19",
    title: "Ground track solution",
    detail: "Independent estimate converged inside the correction gate.",
    severity: "nominal",
  },
  {
    time: "T+06:22",
    title: "Proposal GSU-04",
    detail: "Awaiting guided-operator review.",
    severity: "advisory",
  },
];
