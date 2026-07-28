import type { TrajectoryPoint } from "../model/trajectory";

interface TrajectoryPlotProps {
  readonly planned: readonly TrajectoryPoint[];
  readonly onboard: readonly TrajectoryPoint[];
  readonly ground: readonly TrajectoryPoint[];
  readonly apogeeKm?: number;
}

const VIEW_WIDTH = 720;
const VIEW_HEIGHT = 310;
const PLOT_LEFT = 54;
const PLOT_TOP = 20;
const PLOT_WIDTH = 636;
const PLOT_HEIGHT = 246;
const MAX_DOWNRANGE = 650;
const MAX_ALTITUDE = 275;

function path(points: readonly TrajectoryPoint[]): string {
  return points
    .map(({ downrange, altitude }, index) => {
      const x = PLOT_LEFT + (downrange / MAX_DOWNRANGE) * PLOT_WIDTH;
      const y = PLOT_TOP + PLOT_HEIGHT - (altitude / MAX_ALTITUDE) * PLOT_HEIGHT;
      return `${index === 0 ? "M" : "L"} ${x.toFixed(1)} ${y.toFixed(1)}`;
    })
    .join(" ");
}

export function TrajectoryPlot({ planned, onboard, ground, apogeeKm }: TrajectoryPlotProps) {
  const current = onboard.at(-1) ?? { downrange: 0, altitude: 0 };
  const currentX = PLOT_LEFT + (current.downrange / MAX_DOWNRANGE) * PLOT_WIDTH;
  const currentY = PLOT_TOP + PLOT_HEIGHT - (current.altitude / MAX_ALTITUDE) * PLOT_HEIGHT;

  return (
    <figure className="trajectory-figure">
      <figcaption className="panel-heading">
        <div>
          <p className="eyebrow">Estimate comparison</p>
          <h2>Trajectory</h2>
        </div>
        <p className="panel-stat">
          <span>Apogee estimate</span>
          <strong>{apogeeKm === undefined ? "Awaiting" : `${apogeeKm.toFixed(1)} km`}</strong>
        </p>
      </figcaption>
      <svg
        className="trajectory-svg"
        viewBox={`0 0 ${VIEW_WIDTH} ${VIEW_HEIGHT}`}
        role="img"
        aria-labelledby="trajectory-title trajectory-description"
      >
        <title id="trajectory-title">Planned and estimated mission trajectories</title>
        <desc id="trajectory-description">
          Planned, onboard-estimated, and ground-estimated altitude versus downrange paths.
        </desc>
        <g className="plot-grid" aria-hidden="true">
          {[0, 55, 110, 165, 220, 275].map((altitude) => {
            const y = PLOT_TOP + PLOT_HEIGHT - (altitude / MAX_ALTITUDE) * PLOT_HEIGHT;
            return (
              <g key={altitude}>
                <line x1={PLOT_LEFT} y1={y} x2={PLOT_LEFT + PLOT_WIDTH} y2={y} />
                <text x={PLOT_LEFT - 10} y={y + 4} textAnchor="end">
                  {altitude}
                </text>
              </g>
            );
          })}
          {[0, 130, 260, 390, 520, 650].map((downrange) => {
            const x = PLOT_LEFT + (downrange / MAX_DOWNRANGE) * PLOT_WIDTH;
            return (
              <g key={downrange}>
                <line x1={x} y1={PLOT_TOP} x2={x} y2={PLOT_TOP + PLOT_HEIGHT} />
                <text x={x} y={PLOT_TOP + PLOT_HEIGHT + 23} textAnchor="middle">
                  {downrange}
                </text>
              </g>
            );
          })}
        </g>
        <text className="axis-label" x={16} y={PLOT_TOP + PLOT_HEIGHT / 2} textAnchor="middle">
          ALT KM
        </text>
        <text
          className="axis-label"
          x={PLOT_LEFT + PLOT_WIDTH / 2}
          y={VIEW_HEIGHT - 4}
          textAnchor="middle"
        >
          DOWNRANGE KM
        </text>
        <path className="trajectory-path planned" d={path(planned)} />
        <path className="trajectory-path ground" d={path(ground)} />
        <path className="trajectory-path onboard" d={path(onboard)} />
        <g className="vehicle-marker" transform={`translate(${currentX} ${currentY})`}>
          <circle r="9" />
          <path d="M0 -15 L5 -4 L0 0 L-5 -4 Z" />
        </g>
      </svg>
      <ul className="legend" aria-label="Trajectory sources">
        <li><span className="legend-line planned" />Planned reference</li>
        <li><span className="legend-line onboard" />Onboard estimate</li>
        <li><span className="legend-line ground" />Ground estimate</li>
      </ul>
    </figure>
  );
}
