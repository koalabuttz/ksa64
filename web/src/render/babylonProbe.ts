export type RendererPreference = "auto" | "webgl2" | "2d";
export type RendererBackend = "webgpu" | "webgl2" | "2d";

export interface RendererHandle {
  readonly backend: RendererBackend;
  dispose(): void;
}

function makeCanvas(container: HTMLElement): HTMLCanvasElement {
  container.replaceChildren();
  const canvas = document.createElement("canvas");
  canvas.className = "renderer-canvas";
  canvas.setAttribute("aria-label", "Presentation renderer feasibility view");
  container.append(canvas);
  return canvas;
}

function paint2d(canvas: HTMLCanvasElement): void {
  const context = canvas.getContext("2d");
  if (context === null) throw new Error("2-D canvas is unavailable");
  const width = Math.max(canvas.clientWidth, 640);
  const height = Math.max(canvas.clientHeight, 260);
  const density = Math.min(window.devicePixelRatio || 1, 2);
  canvas.width = width * density;
  canvas.height = height * density;
  context.scale(density, density);
  const gradient = context.createRadialGradient(
    width * 0.52,
    height * 0.54,
    6,
    width * 0.52,
    height * 0.54,
    Math.min(width, height) * 0.42,
  );
  gradient.addColorStop(0, "#184b55");
  gradient.addColorStop(0.65, "#0b252c");
  gradient.addColorStop(1, "#071013");
  context.fillStyle = gradient;
  context.fillRect(0, 0, width, height);
  context.strokeStyle = "#4dd9c8";
  context.lineWidth = 2;
  context.beginPath();
  context.ellipse(width * 0.52, height * 0.55, height * 0.27, height * 0.27, 0, 0, Math.PI * 2);
  context.stroke();
  context.strokeStyle = "#f2a65a";
  context.beginPath();
  context.ellipse(width * 0.52, height * 0.55, height * 0.4, height * 0.16, -0.18, 0, Math.PI * 2);
  context.stroke();
  context.fillStyle = "#dce9e6";
  context.font = "600 12px ui-monospace, monospace";
  context.fillText("2-D PRESENTATION FALLBACK", 18, 28);
}

async function makeBabylonScene(
  canvas: HTMLCanvasElement,
  backend: "webgpu" | "webgl2",
): Promise<{ dispose(): void }> {
  const [
    { Scene },
    { ArcRotateCamera },
    { HemisphericLight },
    { Vector3 },
    { CreateSphere },
    { CreateLines },
    { StandardMaterial },
    { Color3 },
  ] = await Promise.all([
    import("@babylonjs/core/scene"),
    import("@babylonjs/core/Cameras/arcRotateCamera"),
    import("@babylonjs/core/Lights/hemisphericLight"),
    import("@babylonjs/core/Maths/math.vector"),
    import("@babylonjs/core/Meshes/Builders/sphereBuilder"),
    import("@babylonjs/core/Meshes/Builders/linesBuilder"),
    import("@babylonjs/core/Materials/standardMaterial"),
    import("@babylonjs/core/Maths/math.color"),
  ]);

  const engine =
    backend === "webgpu"
      ? await (async () => {
          const { WebGPUEngine } = await import("@babylonjs/core/Engines/webgpuEngine");
          const webgpu = new WebGPUEngine(canvas, {
            antialias: true,
            adaptToDeviceRatio: true,
          });
          await webgpu.initAsync();
          return webgpu;
        })()
      : await (async () => {
          const { Engine } = await import("@babylonjs/core/Engines/engine");
          const webgl = new Engine(
            canvas,
            true,
            { disableWebGL2Support: false, preserveDrawingBuffer: false, stencil: true },
            true,
          );
          if (webgl.webGLVersion !== 2) {
            webgl.dispose();
            throw new Error("WebGL2 is unavailable");
          }
          return webgl;
        })();

  const scene = new Scene(engine);
  scene.clearColor.set(0.027, 0.063, 0.075, 1);
  const camera = new ArcRotateCamera("camera", -1.25, 1.08, 6.5, Vector3.Zero(), scene);
  camera.minZ = 0.1;
  new HemisphericLight("key", new Vector3(0.25, 0.8, 0.4), scene);
  const globe = CreateSphere("presentation-globe", { diameter: 2.7, segments: 24 }, scene);
  const globeMaterial = new StandardMaterial("presentation-globe-material", scene);
  globeMaterial.diffuseColor = new Color3(0.045, 0.29, 0.34);
  globeMaterial.specularColor = new Color3(0.08, 0.5, 0.5);
  globe.material = globeMaterial;

  const orbitPoints = Array.from({ length: 97 }, (_, index) => {
    const angle = (index / 96) * Math.PI * 2;
    return new Vector3(Math.cos(angle) * 2.05, Math.sin(angle) * 0.72, Math.sin(angle) * 1.3);
  });
  const orbit = CreateLines("reference-path", { points: orbitPoints }, scene);
  orbit.color = new Color3(0.95, 0.55, 0.25);

  const resize = () => engine.resize();
  window.addEventListener("resize", resize);
  engine.runRenderLoop(() => scene.render());

  return {
    dispose() {
      window.removeEventListener("resize", resize);
      scene.dispose();
      engine.dispose();
    },
  };
}

export async function startRendererProbe(
  container: HTMLElement,
  preference: RendererPreference,
): Promise<RendererHandle> {
  if (preference === "2d") {
    const canvas = makeCanvas(container);
    paint2d(canvas);
    return { backend: "2d", dispose: () => container.replaceChildren() };
  }

  const attempts: readonly ("webgpu" | "webgl2")[] =
    preference === "webgl2" ? ["webgl2"] : ["webgpu", "webgl2"];
  for (const backend of attempts) {
    if (backend === "webgpu" && !("gpu" in navigator)) continue;
    const canvas = makeCanvas(container);
    try {
      const scene = await makeBabylonScene(canvas, backend);
      return {
        backend,
        dispose() {
          scene.dispose();
          container.replaceChildren();
        },
      };
    } catch {
      container.replaceChildren();
    }
  }

  const canvas = makeCanvas(container);
  paint2d(canvas);
  return { backend: "2d", dispose: () => container.replaceChildren() };
}
