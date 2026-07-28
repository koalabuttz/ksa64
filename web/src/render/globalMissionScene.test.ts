import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GlobalSceneSnapshot } from "../model/globalScene";
import { startGlobalMissionRenderer } from "./globalMissionScene";

class FakeVector3 {
  constructor(public x = 0, public y = 0, public z = 0) {}
  static Zero(): FakeVector3 { return new FakeVector3(); }
  copyFrom(value: FakeVector3): void { this.x = value.x; this.y = value.y; this.z = value.z; }
}

function fakeMesh() {
  return { scaling: { y: 1, setAll: vi.fn() }, position: new FakeVector3(), rotation: { z: 0 },
    parent: undefined, material: undefined, color: undefined, alpha: 1, setEnabled: vi.fn(), dispose: vi.fn() };
}
class FakeQuaternion { constructor(public x = 0, public y = 0, public z = 0, public w = 1) {} }
class FakeTransformNode {
  position = new FakeVector3(); rotationQuaternion: FakeQuaternion | null = null; setEnabled = vi.fn();
}

vi.mock("@babylonjs/core/scene", () => ({ Scene: class {
  useRightHandedSystem = false; clearColor = { set: vi.fn() };
  render = vi.fn(); dispose = vi.fn(); whenReadyAsync = vi.fn(async () => undefined);
} }));
vi.mock("@babylonjs/core/Cameras/arcRotateCamera", () => ({ ArcRotateCamera: class {
  lowerRadiusLimit = 0; upperRadiusLimit = 0; wheelDeltaPercentage = 0; panningSensibility = 0;
  minZ = 0; alpha = 0; beta = 0; radius = 0;
  attachControl = vi.fn(); setTarget = vi.fn();
} }));
vi.mock("@babylonjs/core/Lights/hemisphericLight", () => ({ HemisphericLight: class { intensity = 0; } }));
vi.mock("@babylonjs/core/Lights/directionalLight", () => ({ DirectionalLight: class { intensity = 0; } }));
vi.mock("@babylonjs/core/Maths/math.vector", () => ({ Quaternion: FakeQuaternion, Vector3: FakeVector3 }));
vi.mock("@babylonjs/core/Meshes/transformNode", () => ({ TransformNode: FakeTransformNode }));
vi.mock("@babylonjs/core/Meshes/Builders/cylinderBuilder", () => ({ CreateCylinder: () => fakeMesh() }));
vi.mock("@babylonjs/core/Meshes/Builders/sphereBuilder", () => ({ CreateSphere: () => fakeMesh() }));
vi.mock("@babylonjs/core/Meshes/Builders/linesBuilder", () => ({
  CreateLines: (_name: string, options: { instance?: ReturnType<typeof fakeMesh> }) => options.instance ?? fakeMesh(),
  CreateDashedLines: (_name: string, options: { instance?: ReturnType<typeof fakeMesh> }) => options.instance ?? fakeMesh(),
}));
vi.mock("@babylonjs/core/Materials/standardMaterial", () => ({ StandardMaterial: class {
  diffuseColor: unknown; emissiveColor: unknown; specularColor: unknown; specularPower = 0;
  alpha = 1; backFaceCulling = true; dispose = vi.fn();
} }));
vi.mock("@babylonjs/core/Maths/math.color", () => ({ Color3: class {
  constructor(public red = 0, public green = 0, public blue = 0) {}
} }));
vi.mock("@babylonjs/core/Engines/engine", () => ({ Engine: class {
  webGLVersion = 2; resize = vi.fn(); runRenderLoop = vi.fn(); dispose = vi.fn();
  getRenderWidth = vi.fn(() => 16); getRenderHeight = vi.fn(() => 16);
  readPixels = vi.fn(async () => new Uint8Array([3, 7, 10, 255]));
} }));
vi.mock("@babylonjs/core/Engines/webgpuEngine", () => ({ WebGPUEngine: class {
  initAsync = vi.fn(async () => undefined); resize = vi.fn(); runRenderLoop = vi.fn(); dispose = vi.fn();
  getRenderWidth = vi.fn(() => 16); getRenderHeight = vi.fn(() => 16);
  readPixels = vi.fn(async () => new Uint8Array([3, 7, 10, 255]));
} }));
vi.mock("@babylonjs/core/ShadersWGSL/default.vertex", () => ({}));
vi.mock("@babylonjs/core/ShadersWGSL/default.fragment", () => ({}));
vi.mock("@babylonjs/core/ShadersWGSL/color.vertex", () => ({}));
vi.mock("@babylonjs/core/ShadersWGSL/color.fragment", () => ({}));

function snapshot(releaseEpoch: number): GlobalSceneSnapshot {
  return { releaseEpoch, missionTimeQ16: releaseEpoch * 2048, frame: "ecef", segment: "ecef-ascent",
    camera: "earth-fixed", eventMask: 0, discontinuityMask: 0, continuityIdentity: 1,
    originKm: [0, 0, 0], anchors: [], sources: [{ source: "onboard", positionKm: [6378.2, 0, 0],
      modelIdentity: 1, sourceEstimateIdentity: 2, sourceChecksum: 3, ageReleases: 0, locatorRequired: true }], paths: [], truthLabelVisible: false,
    exactSnapRequired: false, interpolated: false, quality: "global-display-v1" };
}

beforeEach(() => {
  const gradient = { addColorStop: vi.fn() };
  const context = { setTransform: vi.fn(), createRadialGradient: vi.fn(() => gradient), fillRect: vi.fn(),
    save: vi.fn(), translate: vi.fn(), beginPath: vi.fn(), ellipse: vi.fn(), fill: vi.fn(), stroke: vi.fn(),
    restore: vi.fn(), setLineDash: vi.fn(), moveTo: vi.fn(), lineTo: vi.fn(), arc: vi.fn(), fillText: vi.fn() };
  vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(context as unknown as CanvasRenderingContext2D);
});

describe("global renderer fallback", () => {
  it("registers the WGSL path and reports WebGPU only after a nonblack initial frame", async () => {
    Object.defineProperty(navigator, "gpu", { configurable: true, value: {} });
    const container = document.createElement("div");
    const backends: string[] = [];
    const renderer = await startGlobalMissionRenderer(container, "auto", snapshot(1), {
      onBackendChange: (backend) => backends.push(backend),
    });
    expect(renderer.backend).toBe("webgpu");
    expect(backends).toEqual(["webgpu"]);
    expect(container.querySelector("canvas")).not.toBeNull();
    renderer.dispose();
    Reflect.deleteProperty(navigator, "gpu");
  });

  it("falls back visibly to the latest 2-D state after a WebGL context loss", async () => {
    const container = document.createElement("div");
    const backends: string[] = [];
    const renderer = await startGlobalMissionRenderer(container, "webgl2", snapshot(1), {
      onBackendChange: (backend) => backends.push(backend),
    });
    expect(renderer.backend).toBe("webgl2");
    const webglCanvas = container.querySelector("canvas");
    expect(webglCanvas).not.toBeNull();
    renderer.update(snapshot(2));
    const loss = new Event("webglcontextlost", { cancelable: true });
    webglCanvas?.dispatchEvent(loss);
    expect(loss.defaultPrevented).toBe(true);
    expect(backends).toEqual(["webgl2", "2d"]);
    expect(container.querySelector("canvas")).not.toBe(webglCanvas);
    expect(() => renderer.update(snapshot(3))).not.toThrow();
    renderer.dispose();
    expect(container.childElementCount).toBe(0);
  });
});
