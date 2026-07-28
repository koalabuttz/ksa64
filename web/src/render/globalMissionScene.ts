import {
  ksaToBabylon,
  type F64x3,
  type GlobalSceneSnapshot,
} from "../model/globalScene";

export type RendererPreference = "auto" | "webgl2" | "2d";
export type RendererBackend = "webgpu" | "webgl2" | "2d";

export interface GlobalMissionRenderer {
  readonly backend: RendererBackend;
  update(snapshot: GlobalSceneSnapshot): void;
  dispose(): void;
}

export interface GlobalMissionRendererOptions {
  readonly onBackendChange?: (backend: RendererBackend) => void;
  readonly onManualCamera?: () => void;
}

const SOURCE_COLOR = {
  planned: "#839e9b",
  onboard: "#f2a65a",
  ground: "#75a7ff",
  truth: "#d46dff",
} as const;
const EQUATORIAL_RADIUS_KM = 6378.137;
const POLAR_RATIO = 6356.752314245 / EQUATORIAL_RADIUS_KM;
const MAX_RENDER_PATH_POINTS = 4096;

function makeCanvas(container: HTMLElement): HTMLCanvasElement {
  container.replaceChildren();
  const canvas = document.createElement("canvas");
  canvas.className = "global-renderer-canvas";
  canvas.setAttribute("aria-label", "KSA64 global mission presentation");
  container.append(canvas);
  return canvas;
}

function hexToRgb(hex: string): readonly [number, number, number] {
  return [
    Number.parseInt(hex.slice(1, 3), 16) / 255,
    Number.parseInt(hex.slice(3, 5), 16) / 255,
    Number.parseInt(hex.slice(5, 7), 16) / 255,
  ];
}

function project2d(value: F64x3, width: number, height: number): readonly [number, number] {
  const scale = Math.min(width, height) * 0.36 / EQUATORIAL_RADIUS_KM;
  return [width * 0.5 + value[0] * scale, height * 0.52 - value[2] * scale];
}

function paint2d(canvas: HTMLCanvasElement, snapshot: GlobalSceneSnapshot): void {
  const context = canvas.getContext("2d");
  if (context === null) throw new Error("2-D canvas is unavailable");
  const width = Math.max(canvas.clientWidth, 640);
  const height = Math.max(canvas.clientHeight, 360);
  const density = Math.min(window.devicePixelRatio || 1, 2);
  canvas.width = Math.round(width * density);
  canvas.height = Math.round(height * density);
  context.setTransform(density, 0, 0, density, 0, 0);
  const gradient = context.createRadialGradient(
    width * 0.54,
    height * 0.46,
    8,
    width * 0.54,
    height * 0.46,
    Math.min(width, height) * 0.62,
  );
  gradient.addColorStop(0, "#123b45");
  gradient.addColorStop(0.65, "#081a20");
  gradient.addColorStop(1, "#03080b");
  context.fillStyle = gradient;
  context.fillRect(0, 0, width, height);

  const earthRadius = Math.min(width, height) * 0.31;
  context.save();
  context.translate(width * 0.5, height * 0.52);
  context.strokeStyle = "#31545c";
  context.fillStyle = "#09262f";
  context.lineWidth = 1.4;
  context.beginPath();
  context.ellipse(0, 0, earthRadius, earthRadius * POLAR_RATIO, 0, 0, Math.PI * 2);
  context.fill();
  context.stroke();
  context.strokeStyle = "rgba(77, 217, 200, 0.22)";
  for (let latitude = -60; latitude <= 60; latitude += 30) {
    const radius = Math.cos(latitude * Math.PI / 180);
    const y = -Math.sin(latitude * Math.PI / 180) * earthRadius * POLAR_RATIO;
    context.beginPath();
    context.ellipse(0, y, earthRadius * radius, Math.max(1, earthRadius * 0.08 * radius), 0, 0, Math.PI * 2);
    context.stroke();
  }
  for (let longitude = 0; longitude < 180; longitude += 30) {
    context.beginPath();
    context.ellipse(0, 0, Math.max(1, earthRadius * Math.cos(longitude * Math.PI / 180)),
      earthRadius * POLAR_RATIO, 0, 0, Math.PI * 2);
    context.stroke();
  }
  context.restore();

  for (const path of snapshot.paths) {
    const points = path.pointsKm.slice(0, MAX_RENDER_PATH_POINTS);
    if (points.length < 2) continue;
    context.strokeStyle = SOURCE_COLOR[path.source];
    context.lineWidth = path.source === "onboard" ? 2.5 : 1.6;
    context.setLineDash(path.dashed ? [7, 7] : []);
    context.beginPath();
    points.forEach((point, index) => {
      const [x, y] = project2d(point, width, height);
      if (index === 0) context.moveTo(x, y);
      else context.lineTo(x, y);
    });
    context.stroke();
  }
  context.setLineDash([]);

  for (const source of snapshot.sources) {
    const [x, y] = project2d(source.positionKm, width, height);
    context.fillStyle = SOURCE_COLOR[source.source];
    context.strokeStyle = "#050b0d";
    context.lineWidth = 2;
    context.beginPath();
    context.arc(x, y, source.source === "onboard" ? 5 : 4, 0, Math.PI * 2);
    context.fill();
    context.stroke();
  }

  context.fillStyle = "#dce9e6";
  context.font = "700 11px ui-monospace, monospace";
  context.fillText(`2-D GLOBAL FALLBACK · ${snapshot.frame.toUpperCase()} · R+${snapshot.releaseEpoch}`, 16, 24);
}

async function makeBabylonRenderer(
  canvas: HTMLCanvasElement,
  backend: "webgpu" | "webgl2",
  initial: GlobalSceneSnapshot,
  options: GlobalMissionRendererOptions,
): Promise<{ update(snapshot: GlobalSceneSnapshot): void; dispose(): void }> {
  const [
    { Scene },
    { ArcRotateCamera },
    { HemisphericLight },
    { DirectionalLight },
    { Vector3 },
    { CreateSphere },
    { CreateLines },
    { StandardMaterial },
    { Color3 },
  ] = await Promise.all([
    import("@babylonjs/core/scene"),
    import("@babylonjs/core/Cameras/arcRotateCamera"),
    import("@babylonjs/core/Lights/hemisphericLight"),
    import("@babylonjs/core/Lights/directionalLight"),
    import("@babylonjs/core/Maths/math.vector"),
    import("@babylonjs/core/Meshes/Builders/sphereBuilder"),
    import("@babylonjs/core/Meshes/Builders/linesBuilder"),
    import("@babylonjs/core/Materials/standardMaterial"),
    import("@babylonjs/core/Maths/math.color"),
  ]);

  const engine = backend === "webgpu"
    ? await (async () => {
      const { WebGPUEngine } = await import("@babylonjs/core/Engines/webgpuEngine");
      const value = new WebGPUEngine(canvas, { antialias: true, adaptToDeviceRatio: true });
      await value.initAsync();
      return value;
    })()
    : await (async () => {
      const { Engine } = await import("@babylonjs/core/Engines/engine");
      const value = new Engine(canvas, true, {
        disableWebGL2Support: false,
        preserveDrawingBuffer: false,
        stencil: true,
      }, true);
      if (value.webGLVersion !== 2) {
        value.dispose();
        throw new Error("WebGL2 is unavailable");
      }
      return value;
    })();

  const scene = new Scene(engine);
  scene.useRightHandedSystem = true;
  scene.clearColor.set(0.012, 0.028, 0.038, 1);
  const camera = new ArcRotateCamera("global-camera", -1.35, 1.08, 3.1, Vector3.Zero(), scene);
  camera.lowerRadiusLimit = 0.025;
  camera.upperRadiusLimit = 24;
  camera.wheelDeltaPercentage = 0.02;
  camera.panningSensibility = 0;
  camera.minZ = 0.00001;
  camera.attachControl(canvas, true);
  const manualCameraHandler = options.onManualCamera ?? (() => undefined);
  canvas.addEventListener("pointerdown", manualCameraHandler);

  new HemisphericLight("earth-fill", new Vector3(0.2, 0.85, -0.4), scene).intensity = 0.52;
  const sun = new DirectionalLight("sun", new Vector3(-0.65, -0.2, 0.45), scene);
  sun.intensity = 1.2;

  const earth = CreateSphere("wgs84-ellipsoid", {
    diameter: 2,
    segments: 64,
  }, scene);
  earth.scaling.y = POLAR_RATIO;
  const earthMaterial = new StandardMaterial("wgs84-material", scene);
  earthMaterial.diffuseColor = new Color3(0.025, 0.19, 0.24);
  earthMaterial.emissiveColor = new Color3(0.005, 0.025, 0.034);
  earthMaterial.specularColor = new Color3(0.08, 0.42, 0.48);
  earthMaterial.specularPower = 48;
  earth.material = earthMaterial;

  const atmosphere = CreateSphere("atmosphere-shell", { diameter: 2.035, segments: 48 }, scene);
  atmosphere.scaling.y = POLAR_RATIO;
  const atmosphereMaterial = new StandardMaterial("atmosphere-material", scene);
  atmosphereMaterial.diffuseColor = new Color3(0.12, 0.56, 0.66);
  atmosphereMaterial.emissiveColor = new Color3(0.02, 0.12, 0.18);
  atmosphereMaterial.alpha = 0.08;
  atmosphereMaterial.backFaceCulling = false;
  atmosphere.material = atmosphereMaterial;

  const gridMaterialColors = {
    grid: new Color3(0.20, 0.48, 0.51),
    equator: new Color3(0.30, 0.85, 0.78),
  };
  const gridMeshes: ReturnType<typeof CreateLines>[] = [];
  for (let latitude = -60; latitude <= 60; latitude += 30) {
    const phi = latitude * Math.PI / 180;
    const points = Array.from({ length: 97 }, (_, index) => {
      const theta = index / 96 * Math.PI * 2;
      return new Vector3(
        Math.cos(phi) * Math.cos(theta) * 1.002,
        Math.sin(phi) * POLAR_RATIO * 1.002,
        -Math.cos(phi) * Math.sin(theta) * 1.002,
      );
    });
    const line = CreateLines(`latitude-${latitude}`, { points }, scene);
    line.color = latitude === 0 ? gridMaterialColors.equator : gridMaterialColors.grid;
    line.alpha = latitude === 0 ? 0.55 : 0.24;
    gridMeshes.push(line);
  }
  for (let longitude = 0; longitude < 180; longitude += 30) {
    const theta = longitude * Math.PI / 180;
    const points = Array.from({ length: 97 }, (_, index) => {
      const phi = -Math.PI / 2 + index / 96 * Math.PI;
      return new Vector3(
        Math.cos(phi) * Math.cos(theta) * 1.002,
        Math.sin(phi) * POLAR_RATIO * 1.002,
        -Math.cos(phi) * Math.sin(theta) * 1.002,
      );
    });
    const line = CreateLines(`longitude-${longitude}`, { points }, scene);
    line.color = gridMaterialColors.grid;
    line.alpha = 0.22;
    gridMeshes.push(line);
  }

  const sourceMeshes = new Map<string, ReturnType<typeof CreateSphere>>();
  const sourceMaterials = new Map<string, InstanceType<typeof StandardMaterial>>();
  const pathMeshes = new Map<number, ReturnType<typeof CreateLines>>();
  const pathPointCounts = new Map<number, number>();

  const vector = (value: F64x3, origin: F64x3): InstanceType<typeof Vector3> => {
    const mapped = ksaToBabylon(value, origin);
    return new Vector3(
      mapped[0] / EQUATORIAL_RADIUS_KM,
      mapped[1] / EQUATORIAL_RADIUS_KM,
      mapped[2] / EQUATORIAL_RADIUS_KM,
    );
  };

  const configureCamera = (snapshot: GlobalSceneSnapshot): void => {
    const primary = snapshot.sources.find((value) => value.source === "onboard") ??
      snapshot.sources.find((value) => value.source === "ground");
    const target = primary === undefined ? Vector3.Zero() : vector(primary.positionKm, snapshot.originKm);
    camera.setTarget(target);
    switch (snapshot.camera) {
      case "launch":
      case "recovery":
        camera.alpha = -1.05; camera.beta = 1.18; camera.radius = 0.12;
        break;
      case "chase":
        camera.alpha = -1.3; camera.beta = 1.02; camera.radius = 0.18;
        break;
      case "inspection":
        camera.alpha = -1.1; camera.beta = 1.1; camera.radius = 0.025;
        break;
      case "inertial":
        camera.alpha = -1.7; camera.beta = 0.92; camera.radius = 3.4;
        break;
      case "earth-fixed":
      case "director":
        camera.alpha = -1.35; camera.beta = 1.08; camera.radius = 3.1;
        break;
      case "free":
        break;
    }
  };

  const update = (snapshot: GlobalSceneSnapshot): void => {
    const earthPosition = vector([0, 0, 0], snapshot.originKm);
    earth.position.copyFrom(earthPosition);
    atmosphere.position.copyFrom(earthPosition);
    for (const grid of gridMeshes) grid.position.copyFrom(earthPosition);
    const activeSources = new Set<string>();
    for (const source of snapshot.sources) {
      activeSources.add(source.source);
      let marker = sourceMeshes.get(source.source);
      if (marker === undefined) {
        marker = CreateSphere(`locator-${source.source}`, { diameter: 0.026, segments: 12 }, scene);
        const material = new StandardMaterial(`locator-${source.source}-material`, scene);
        const [red, green, blue] = hexToRgb(SOURCE_COLOR[source.source]);
        material.diffuseColor = new Color3(red, green, blue);
        material.emissiveColor = new Color3(red * 0.48, green * 0.48, blue * 0.48);
        material.alpha = source.source === "ground" || source.source === "truth" ? 0.7 : 1;
        marker.material = material;
        sourceMeshes.set(source.source, marker);
        sourceMaterials.set(source.source, material);
      }
      marker.setEnabled(true);
      marker.position.copyFrom(vector(source.positionKm, snapshot.originKm));
      marker.scaling.setAll(source.source === "onboard" ? 1 : 0.78);
    }
    for (const [source, marker] of sourceMeshes) if (!activeSources.has(source)) marker.setEnabled(false);

    const activePaths = new Set<number>();
    for (const path of snapshot.paths) {
      if (path.pointsKm.length < 2) continue;
      activePaths.add(path.identity);
      const renderPoints = path.pointsKm.slice(0, MAX_RENDER_PATH_POINTS);
      let line = pathMeshes.get(path.identity);
      if (line === undefined || pathPointCounts.get(path.identity) !== renderPoints.length) {
        line?.dispose();
        line = CreateLines(`path-${path.identity}`, {
          points: renderPoints.map((point) => vector(point, snapshot.originKm)),
          updatable: true,
        }, scene);
        pathMeshes.set(path.identity, line);
        pathPointCounts.set(path.identity, renderPoints.length);
      } else {
        line = CreateLines(`path-${path.identity}`, {
          points: renderPoints.map((point) => vector(point, snapshot.originKm)),
          instance: line,
        }, scene);
      }
      line.setEnabled(true);
      const [red, green, blue] = hexToRgb(SOURCE_COLOR[path.source]);
      line.color = new Color3(red, green, blue);
      line.alpha = path.stale ? 0.28 : path.incomplete ? 0.48 : path.source === "planned" ? 0.55 : 0.88;
    }
    for (const [identity, line] of pathMeshes) if (!activePaths.has(identity)) line.setEnabled(false);
    configureCamera(snapshot);
  };

  const resize = () => engine.resize();
  window.addEventListener("resize", resize);
  engine.runRenderLoop(() => scene.render());
  update(initial);
  return {
    update,
    dispose() {
      canvas.removeEventListener("pointerdown", manualCameraHandler);
      window.removeEventListener("resize", resize);
      for (const material of sourceMaterials.values()) material.dispose();
      scene.dispose();
      engine.dispose();
    },
  };
}

export async function startGlobalMissionRenderer(
  container: HTMLElement,
  preference: RendererPreference,
  initial: GlobalSceneSnapshot,
  options: GlobalMissionRendererOptions = {},
): Promise<GlobalMissionRenderer> {
  if (preference === "2d") {
    const canvas = makeCanvas(container);
    let snapshot = initial;
    const paint = () => paint2d(canvas, snapshot);
    const resize = () => paint();
    window.addEventListener("resize", resize);
    paint();
    options.onBackendChange?.("2d");
    return {
      backend: "2d",
      update(next) { snapshot = next; paint(); },
      dispose() {
        window.removeEventListener("resize", resize);
        container.replaceChildren();
      },
    };
  }

  const attempts: readonly ("webgpu" | "webgl2")[] =
    preference === "webgl2" ? ["webgl2"] : ["webgpu", "webgl2"];
  for (const backend of attempts) {
    if (backend === "webgpu" && !("gpu" in navigator)) continue;
    const canvas = makeCanvas(container);
    try {
      const renderer = await makeBabylonRenderer(canvas, backend, initial, options);
      options.onBackendChange?.(backend);
      let disposed = false;
      let latest = initial;
      const contextLost = (event: Event) => {
        event.preventDefault();
        if (disposed) return;
        disposed = true;
        renderer.dispose();
        const fallback = makeCanvas(container);
        paint2d(fallback, latest);
        options.onBackendChange?.("2d");
      };
      canvas.addEventListener("webglcontextlost", contextLost, { once: true });
      return {
        backend,
        update(snapshot) {
          latest = snapshot;
          if (disposed) {
            const fallback = container.querySelector("canvas");
            if (fallback instanceof HTMLCanvasElement) paint2d(fallback, snapshot);
          } else {
            renderer.update(snapshot);
          }
        },
        dispose() {
          canvas.removeEventListener("webglcontextlost", contextLost);
          if (!disposed) renderer.dispose();
          disposed = true;
          container.replaceChildren();
        },
      };
    } catch {
      container.replaceChildren();
    }
  }

  return startGlobalMissionRenderer(container, "2d", initial, options);
}
