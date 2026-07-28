import type { TrajectoryPoint } from "./trajectory";

// Presentation-only nominal reference. This is not authoritative world truth
// and does not participate in session execution or evidence generation.
export const plannedTrajectory: readonly TrajectoryPoint[] = [
  { downrange: 0, altitude: 0 },
  { downrange: 26, altitude: 42 },
  { downrange: 78, altitude: 118 },
  { downrange: 152, altitude: 191 },
  { downrange: 240, altitude: 238 },
  { downrange: 334, altitude: 252 },
  { downrange: 428, altitude: 231 },
  { downrange: 510, altitude: 174 },
  { downrange: 580, altitude: 88 },
  { downrange: 624, altitude: 18 },
];
