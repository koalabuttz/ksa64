import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GlobalDisplayModelV1, GlobalDisplaySampleV1, GlobalDisplaySourceV1 } from "../model/globalDisplay";
import type { GlobalMissionRendererOptions } from "../render/globalMissionScene";
import { GlobalMissionViewer } from "./GlobalMissionViewer";

const rendererMock = vi.hoisted(() => ({ start: vi.fn() }));
vi.mock("../render/globalMissionScene", () => ({ startGlobalMissionRenderer: rendererMock.start }));

function sample(releaseEpoch: number, sources: readonly GlobalDisplaySourceV1[]): GlobalDisplaySampleV1 {
  return { sequence: BigInt(releaseEpoch + 1), releaseEpoch, missionTimeQ16: releaseEpoch * 2048,
    segment: "ecef-ascent", authoritativeFrame: "ecef", eventMask: 0n, discontinuityMask: 0n,
    continuityIdentity: 1, geodeticQ28Q12: [0, 0, 0], altitudeQ12Km: 0, machQ24: 0,
    dynamicPressureQ14Pa: 0, totalMassQ21Kg: 0, mainPropellantQ21Kg: 0,
    rcsPropellantQ21Kg: 0, componentStatusMask: 0n,
    poses: sources.map((source, index) => ({ source, modelIdentity: index + 1,
      sourceEstimateIdentity: index + 10, sourceChecksum: index + 20, ageReleases: 0,
      validityMask: 1n, ecefPositionQ12Km: [26_124_165 + releaseEpoch + index, 0, 0],
      gcrfPositionQ12Km: [26_124_165 + releaseEpoch + index, 0, 0],
      bodyToEcefQ30: [1 << 30, 0, 0, 0], bodyToGcrfQ30: [1 << 30, 0, 0, 0] })) };
}

function model(sources: readonly GlobalDisplaySourceV1[] = ["planned", "onboard", "ground"]): GlobalDisplayModelV1 {
  const samples = [sample(100, sources.filter((source) => source !== "planned")), sample(101, sources.filter((source) => source !== "planned"))];
  return { definition: { modelIdentity: 1, definitionIdentity: 2, earthIdentity: 3,
      transformIdentity: 4, missionEpochTaiSeconds: 0n, equatorialRadiusQ12Km: 26_124_165,
      polarRadiusQ12Km: 26_036_734,
      launchAnchor: { identity: 5, geodeticQ28Q12: [0, 0, 0], ecefPositionQ12Km: [26_124_165, 0, 0] },
      recoveryAnchor: { identity: 6, geodeticQ28Q12: [0, 0, 0], ecefPositionQ12Km: [26_124_165, 1, 0] },
      availableFrames: ["local-enu", "ecef", "gcrf"], availableSources: sources,
      availableCameras: ["director", "launch", "chase", "earth-fixed", "inertial", "recovery", "free", "inspection"],
      quality: "global-display-v1" }, samples, paths: [], transitions: [],
    replay: { firstRelease: 100, lastRelease: 101, selectedRelease: 101, terminalDispositionIdentity: 1,
      markers: [{ identity: 7, releaseEpoch: 100, kind: "event", label: "Test event" }] } };
}

function lastOptions(): GlobalMissionRendererOptions {
  const call = rendererMock.start.mock.calls.at(-1);
  if (call === undefined) throw new Error("renderer was not started");
  return call[3] as GlobalMissionRendererOptions;
}

beforeEach(() => {
  rendererMock.start.mockReset();
  rendererMock.start.mockResolvedValue({ backend: "webgl2", update: vi.fn(), dispose: vi.fn() });
});

describe("global mission viewer", () => {
  it("switches layouts, camera direction, backend state, and exact replay releases", async () => {
    const onLayoutChange = vi.fn();
    const onSemanticSnapshot = vi.fn();
    render(<GlobalMissionViewer model={model()} replay layout="hybrid" deskOpen
      onLayoutChange={onLayoutChange} onDeskOpenChange={vi.fn()} onSemanticSnapshot={onSemanticSnapshot} />);
    await waitFor(() => expect(rendererMock.start).toHaveBeenCalled());
    await waitFor(() => expect(onSemanticSnapshot).toHaveBeenCalled());
    const viewer = screen.getByRole("heading", { name: "Global mission director" }).closest("section");
    expect(JSON.parse(viewer?.getAttribute("data-semantic-scene") ?? "{}")).toMatchObject({
      schema: "ksa64.global-scene-semantic.v1", releaseEpoch: 101, frame: "ecef",
    });
    fireEvent.change(screen.getByLabelText("Global display motion"), { target: { value: "exact" } });
    expect(screen.getByText("Exact-release display")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Global path detail"), { target: { value: "1" } });
    expect(screen.getByLabelText("Global path detail")).toHaveValue("1");
    fireEvent.click(screen.getByRole("button", { name: "Engineering split" }));
    expect(onLayoutChange).toHaveBeenCalledWith("engineering");
    fireEvent.change(screen.getByLabelText("Global camera"), { target: { value: "free" } });
    expect(screen.getByRole("button", { name: "Resume director" })).toBeInTheDocument();
    lastOptions().onBackendChange?.("2d");
    expect(await screen.findByText("2D active")).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Exact release"), { target: { value: "100" } });
    await waitFor(() => expect(screen.getByLabelText("Exact release")).toHaveValue("100"));
    fireEvent.click(screen.getByRole("button", { name: "Next exact display sample" }));
    expect(screen.getByLabelText("Exact release")).toHaveValue("101");
  });

  it("keeps truth hidden by default and reveals it only from a permitted director stream", async () => {
    render(<GlobalMissionViewer model={model(["planned", "onboard", "ground", "truth"])} replay
      layout="hybrid" deskOpen onLayoutChange={vi.fn()} onDeskOpenChange={vi.fn()} />);
    await waitFor(() => expect(rendererMock.start).toHaveBeenCalled());
    const toggle = screen.getByRole("checkbox", { name: "SIM truth" });
    expect(toggle).not.toBeChecked();
    expect(screen.queryByText("SIM TRUTH")).not.toBeInTheDocument();
    fireEvent.click(toggle);
    expect(await screen.findByText("SIM TRUTH")).toBeInTheDocument();
  });

  it("makes an operational role's structural truth absence explicit", async () => {
    render(<GlobalMissionViewer model={model()} replay={false}
      layout="hybrid" deskOpen onLayoutChange={vi.fn()} onDeskOpenChange={vi.fn()} />);
    expect(await screen.findByText("SIM truth absent from this role")).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: "SIM truth" })).not.toBeInTheDocument();
    expect(screen.getByText("Live release · rewind disabled")).toBeInTheDocument();
    expect(screen.getByLabelText("Live release · rewind disabled")).toBeDisabled();
  });
});
