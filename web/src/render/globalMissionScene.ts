import {
  ksaToBabylon,
  type F64x3,
  type GlobalSceneSnapshot,
} from "../model/globalScene";

export type RendererPreference = "auto" | "webgl2" | "2d";
export type RendererBackend = "webgpu" | "webgl2" | "2d";

export interface GlobalRendererOriginProbe {
  readonly releaseEpoch: number;
  readonly camera: GlobalSceneSnapshot["camera"];
  readonly originKm: F64x3;
  readonly points: readonly { readonly identity: string; readonly absoluteKm: F64x3 }[];
}

export interface GlobalMissionRenderer {
  readonly backend: RendererBackend;
  update(snapshot: GlobalSceneSnapshot): void;
  originProbe(): GlobalRendererOriginProbe | undefined;
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
    if (source.source === "onboard") {
      context.arc(x, y, 5, 0, Math.PI * 2);
    } else if (source.source === "ground") {
      context.moveTo(x, y - 5); context.lineTo(x + 5, y);
      context.lineTo(x, y + 5); context.lineTo(x - 5, y); context.closePath();
    } else if (source.source === "truth") {
      context.moveTo(x, y - 5); context.lineTo(x + 5, y + 4);
      context.lineTo(x - 5, y + 4); context.closePath();
    } else {
      context.rect(x - 4, y - 4, 8, 8);
    }
    context.globalAlpha = source.source === "ground" ? 0.55 : source.source === "truth" ? 0.7 : 1;
    context.fill();
    context.stroke();
    context.globalAlpha = 1;
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
): Promise<{ update(snapshot: GlobalSceneSnapshot): void; originProbe(): GlobalRendererOriginProbe; dispose(): void }> {
  // Babylon's tree-shaken material and line builders do not register shader programs
  // by themselves. Register the exact backend's stock shaders before creating any
  // material so a packaged Vite build cannot mistake HTML fallbacks for shader code.
  if (backend === "webgpu") {
    await Promise.all([
      import("@babylonjs/core/ShadersWGSL/default.vertex"),
      import("@babylonjs/core/ShadersWGSL/default.fragment"),
      import("@babylonjs/core/ShadersWGSL/color.vertex"),
      import("@babylonjs/core/ShadersWGSL/color.fragment"),
    ]);
  }

  const [
    { Scene },
    { ArcRotateCamera },
    { HemisphericLight },
    { DirectionalLight },
    { Quaternion, Vector3 },
    { TransformNode },
    { CreateCylinder },
    { CreateSphere },
    { CreateDashedLines, CreateLines },
    { StandardMaterial },
    { Color3 },
  ] = await Promise.all([
    import("@babylonjs/core/scene"),
    import("@babylonjs/core/Cameras/arcRotateCamera"),
    import("@babylonjs/core/Lights/hemisphericLight"),
    import("@babylonjs/core/Lights/directionalLight"),
    import("@babylonjs/core/Maths/math.vector"),
    import("@babylonjs/core/Meshes/transformNode"),
    import("@babylonjs/core/Meshes/Builders/cylinderBuilder"),
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

  const earthAxes = [
    ["ecef-x", new Vector3(0, 0, 0), new Vector3(1.25, 0, 0), new Color3(0.82, 0.28, 0.25)],
    ["ecef-y", new Vector3(0, 0, 0), new Vector3(0, 1.25, 0), new Color3(0.34, 0.78, 0.50)],
    ["ecef-z", new Vector3(0, 0, 0), new Vector3(0, 0, 1.25), new Color3(0.31, 0.55, 0.92)],
  ].map(([name, start, finish, color]) => {
    const line = CreateLines(String(name), { points: [start as InstanceType<typeof Vector3>, finish as InstanceType<typeof Vector3>] }, scene);
    line.color = color as InstanceType<typeof Color3>;
    line.alpha = 0.52;
    return line;
  });
  const localGridMeshes: ReturnType<typeof CreateLines>[] = [];
  for (let step = -5; step <= 5; step += 1) {
    const offset = step * 0.0015;
    const eastWest = CreateLines(`local-grid-east-${step}`, {
      points: [new Vector3(-0.008, 0, offset), new Vector3(0.008, 0, offset)],
    }, scene);
    const northSouth = CreateLines(`local-grid-north-${step}`, {
      points: [new Vector3(offset, 0, -0.008), new Vector3(offset, 0, 0.008)],
    }, scene);
    eastWest.color = gridMaterialColors.grid; northSouth.color = gridMaterialColors.grid;
    eastWest.alpha = step === 0 ? 0.55 : 0.20; northSouth.alpha = step === 0 ? 0.55 : 0.20;
    localGridMeshes.push(eastWest, northSouth);
  }
  const anchorMeshes = new Map<string, ReturnType<typeof CreateSphere>>();

  const sourceMeshes = new Map<string, ReturnType<typeof CreateSphere>>();
  const sourceVehicles = new Map<string, InstanceType<typeof TransformNode>>();
  const sourceMaterials = new Map<string, InstanceType<typeof StandardMaterial>>();
  const pathMeshes = new Map<string, ReturnType<typeof CreateLines>>();
  const pathPointCounts = new Map<string, number>();
  const pathDashedStyles = new Map<string, boolean>();

  const quaternionProduct = (left: readonly [number, number, number, number], right: readonly [number, number, number, number]): readonly [number, number, number, number] => [
    left[0] * right[0] - left[1] * right[1] - left[2] * right[2] - left[3] * right[3],
    left[0] * right[1] + left[1] * right[0] + left[2] * right[3] - left[3] * right[2],
    left[0] * right[2] - left[1] * right[3] + left[2] * right[0] + left[3] * right[1],
    left[0] * right[3] + left[1] * right[2] - left[2] * right[1] + left[3] * right[0],
  ];
  const quaternion = (value: readonly [number, number, number, number]): InstanceType<typeof Quaternion> => {
    const half = Math.SQRT1_2;
    const basis: readonly [number, number, number, number] = [half, -half, 0, 0];
    const inverse: readonly [number, number, number, number] = [half, half, 0, 0];
    const mapped = quaternionProduct(quaternionProduct(basis, value), inverse);
    return new Quaternion(mapped[1], mapped[2], mapped[3], mapped[0]);
  };

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

  let latestSnapshot = initial;
  const update = (snapshot: GlobalSceneSnapshot): void => {
    latestSnapshot = snapshot;
    const earthPosition = vector([0, 0, 0], snapshot.originKm);
    const localDomain = snapshot.frame === "local-enu";
    earth.setEnabled(!localDomain);
    atmosphere.setEnabled(!localDomain);
    earth.position.copyFrom(earthPosition);
    atmosphere.position.copyFrom(earthPosition);
    for (const grid of gridMeshes) {
      grid.setEnabled(snapshot.frame === "ecef");
      grid.position.copyFrom(earthPosition);
    }
    for (const axis of earthAxes) { axis.setEnabled(!localDomain); axis.position.copyFrom(earthPosition); }
    for (const grid of localGridMeshes) { grid.setEnabled(localDomain); grid.position.copyFrom(earthPosition); }
    const activeAnchors = new Set<string>();
    for (const anchor of snapshot.anchors) {
      activeAnchors.add(anchor.kind);
      let marker = anchorMeshes.get(anchor.kind);
      if (marker === undefined) {
        marker = CreateSphere(`anchor-${anchor.kind}`, { diameter: 0.018, segments: 10 }, scene);
        const material = new StandardMaterial(`anchor-${anchor.kind}-material`, scene);
        material.diffuseColor = anchor.kind === "launch" ? new Color3(0.31, 0.86, 0.78) : new Color3(0.94, 0.65, 0.35);
        material.emissiveColor = anchor.kind === "launch" ? new Color3(0.08, 0.28, 0.24) : new Color3(0.28, 0.14, 0.04);
        marker.material = material;
        anchorMeshes.set(anchor.kind, marker);
      }
      marker.setEnabled(true);
      marker.position.copyFrom(vector(anchor.positionKm, snapshot.originKm));
    }
    for (const [kind, marker] of anchorMeshes) if (!activeAnchors.has(kind)) marker.setEnabled(false);
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

        material.alpha = source.source === "ground" ? 0.42 : source.source === "truth" ? 0.62 : 1;
        material.wireframe = source.source === "ground";
        if (source.source === "onboard") {
          const root = new TransformNode("vehicle-onboard", scene);
          const bodyLength = 0.0068 / EQUATORIAL_RADIUS_KM;
          const noseLength = 0.0012 / EQUATORIAL_RADIUS_KM;
          const diameter = 0.0004 / EQUATORIAL_RADIUS_KM;
          const body = CreateCylinder("vehicle-body-onboard", { height: bodyLength, diameter, tessellation: 12 }, scene);
          body.parent = root; body.rotation.z = Math.PI / 2; body.position.x = -noseLength / 2;
          const nose = CreateCylinder("vehicle-nose-onboard", { height: noseLength, diameterBottom: diameter, diameterTop: 0, tessellation: 12 }, scene);
          nose.parent = root; nose.rotation.z = -Math.PI / 2; nose.position.x = bodyLength / 2;
          const vehicleMaterial = new StandardMaterial("vehicle-onboard-material", scene);
          vehicleMaterial.diffuseColor = new Color3(red, green, blue);
          vehicleMaterial.emissiveColor = new Color3(red * 0.10, green * 0.10, blue * 0.10);
          body.material = vehicleMaterial; nose.material = vehicleMaterial;
          sourceMaterials.set("vehicle-onboard", vehicleMaterial);
          sourceVehicles.set(source.source, root);
        }
      }
      marker.setEnabled(true);
      marker.position.copyFrom(vector(source.positionKm, snapshot.originKm));
      const locatorScale = Math.max(0.000_02, camera.radius * (source.source === "onboard" ? 0.008 : 0.006));
      marker.scaling.setAll(locatorScale);
      const vehicle = sourceVehicles.get(source.source);
      if (vehicle !== undefined) {
        vehicle.setEnabled(true);
        vehicle.position.copyFrom(marker.position);
        vehicle.rotationQuaternion = source.bodyQuaternion === undefined ? null : quaternion(source.bodyQuaternion);
      }
    }
    for (const [source, marker] of sourceMeshes) if (!activeSources.has(source)) marker.setEnabled(false);
    for (const [source, vehicle] of sourceVehicles) if (!activeSources.has(source)) vehicle.setEnabled(false);

    const activePaths = new Set<string>();
    for (const path of snapshot.paths) {
      if (path.pointsKm.length < 2) continue;
      const pathKey = `${path.identity}:${path.anchorIdentity}:${path.stripIndex}`;
      activePaths.add(pathKey);
      const renderPoints = path.pointsKm.slice(0, MAX_RENDER_PATH_POINTS);
      let line = pathMeshes.get(pathKey);
      const createPath = path.dashed ? CreateDashedLines : CreateLines;
      if (line === undefined || pathPointCounts.get(pathKey) !== renderPoints.length ||
          pathDashedStyles.get(pathKey) !== path.dashed) {
        line?.dispose();
        line = createPath(`path-${path.identity}-${path.anchorIdentity}-${path.stripIndex}`, {
          points: renderPoints.map((point) => vector(point, snapshot.originKm)),
          updatable: true,
        }, scene);
        pathMeshes.set(pathKey, line);
        pathPointCounts.set(pathKey, renderPoints.length);
        pathDashedStyles.set(pathKey, path.dashed);
      } else {
        line = createPath(`path-${path.identity}-${path.anchorIdentity}-${path.stripIndex}`, {
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

  const originProbe = (): GlobalRendererOriginProbe => {
    const origin = latestSnapshot.originKm;
    const points: { identity: string; absoluteKm: F64x3 }[] = [];
    const reconstruct = (x: number, y: number, z: number): F64x3 => [
      x * EQUATORIAL_RADIUS_KM + origin[0],
      -z * EQUATORIAL_RADIUS_KM + origin[1],
      y * EQUATORIAL_RADIUS_KM + origin[2],
    ];
    const addPosition = (identity: string, position: { readonly x: number; readonly y: number; readonly z: number }) => {
      points.push({ identity, absoluteKm: reconstruct(position.x, position.y, position.z) });
    };
    if (earth.isEnabled()) addPosition("earth", earth.position);
    for (const [identity, marker] of [...anchorMeshes].sort(([left], [right]) => left.localeCompare(right))) {
      if (marker.isEnabled()) addPosition(`anchor:${identity}`, marker.position);
    }
    for (const [identity, marker] of [...sourceMeshes].sort(([left], [right]) => left.localeCompare(right))) {
      if (marker.isEnabled()) addPosition(`source:${identity}`, marker.position);
    }
    for (const [identity, line] of [...pathMeshes].sort(([left], [right]) => left.localeCompare(right))) {
      if (!line.isEnabled()) continue;
      const positions = line.getVerticesData("position");
      if (positions === null || positions.length < 6) continue;
      const last = positions.length - 3;
      points.push({ identity: `path:${identity}:first`, absoluteKm: reconstruct(
        Number(positions[0]) + line.position.x, Number(positions[1]) + line.position.y, Number(positions[2]) + line.position.z) });
      points.push({ identity: `path:${identity}:last`, absoluteKm: reconstruct(
        Number(positions[last]) + line.position.x, Number(positions[last + 1]) + line.position.y, Number(positions[last + 2]) + line.position.z) });
    }
    return { releaseEpoch: latestSnapshot.releaseEpoch, camera: latestSnapshot.camera, originKm: origin, points };
  };
  const resize = () => engine.resize();
  window.addEventListener("resize", resize);
  update(initial);

  // Do not announce a GPU backend until its shader set has compiled and one
  // observable frame has rendered. A failure here is safely handled by the
  // caller's WebGPU -> WebGL2 -> 2-D fallback chain.
  let readinessTimer: ReturnType<typeof setTimeout> | undefined;
  try {
    await Promise.race([
      scene.whenReadyAsync(true),
      new Promise<never>((_, reject) => {
        readinessTimer = setTimeout(() => reject(new Error("Babylon scene readiness timed out")), 8_000);
      }),
    ]);
    scene.render();
    const width = Math.max(1, Math.min(16, engine.getRenderWidth()));
    const height = Math.max(1, Math.min(16, engine.getRenderHeight()));
    const pixels = await engine.readPixels(0, 0, width, height, true, true);
    const bytes = new Uint8Array(pixels.buffer, pixels.byteOffset, pixels.byteLength);
    let nonBlack = false;
    for (let index = 0; index + 2 < bytes.length; index += 4) {
      if ((bytes[index] ?? 0) > 1 || (bytes[index + 1] ?? 0) > 1 || (bytes[index + 2] ?? 0) > 1) {
        nonBlack = true;
        break;
      }
    }
    if (!nonBlack) throw new Error("Babylon produced an empty initial frame");
  } finally {
    if (readinessTimer !== undefined) clearTimeout(readinessTimer);
  }
  engine.runRenderLoop(() => scene.render());
  return {
    update,
    originProbe,
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
      originProbe() { return undefined; },
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
        originProbe() { return disposed ? undefined : renderer.originProbe(); },
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
