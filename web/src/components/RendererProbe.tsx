import { useEffect, useRef, useState } from "react";
import {
  startRendererProbe,
  type RendererBackend,
  type RendererPreference,
} from "../render/babylonProbe";

export function RendererProbe() {
  const container = useRef<HTMLDivElement>(null);
  const [preference, setPreference] = useState<RendererPreference>("auto");
  const [backend, setBackend] = useState<RendererBackend | "initializing">("initializing");

  useEffect(() => {
    if (container.current === null) return;
    let active = true;
    let dispose: (() => void) | undefined;
    setBackend("initializing");
    void startRendererProbe(container.current, preference).then((handle) => {
      if (!active) {
        handle.dispose();
        return;
      }
      dispose = () => handle.dispose();
      setBackend(handle.backend);
    });
    return () => {
      active = false;
      dispose?.();
    };
  }, [preference]);

  return (
    <section className="panel renderer-panel" aria-labelledby="renderer-title">
      <div className="panel-heading">
        <div>
          <p className="eyebrow">Presentation only · physics disabled</p>
          <h2 id="renderer-title">Renderer probe</h2>
        </div>
        <label className="select-label">
          Backend
          <select
            value={preference}
            onChange={(event) => setPreference(event.target.value as RendererPreference)}
          >
            <option value="auto">Auto</option>
            <option value="webgl2">Force WebGL2</option>
            <option value="2d">2-D only</option>
          </select>
        </label>
      </div>
      <div className="renderer-stage" ref={container} />
      <p className="renderer-status" aria-live="polite">
        {backend === "initializing" ? "Selecting renderer…" : `${backend.toUpperCase()} active`}
      </p>
    </section>
  );
}
