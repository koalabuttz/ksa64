# Phase 12 asset, provenance, and binary-source policy

Status: accepted policy. Phase 12A creates only the minimal project/plugin
content needed for the bridge smoke test; NASA and production visual assets are
deferred to Phase 12E.

## Source and generated assets

Open, portable files are visual source masters:

- Blender `.blend`, glTF/GLB, and documented procedural source;
- EXR/TIFF/PNG source textures;
- SVG source interface art;
- source-controlled import, transformation, and validation scripts;
- provenance and generated-asset manifests.

Unreal `.uasset` and `.umap` files are engine-target assets, not universal or
engineering masters. A visual mesh may attach to stable KSA64 component IDs,
but it cannot define physical geometry, mass, inertia, aerodynamics, joints,
events, or authority.

## Git and LFS

Configure Git LFS before the first governed binary is committed. At minimum,
track and lock `.uasset` and `.umap`; also govern large `.blend`, `.fbx`,
`.glb`, `.exr`, `.tif`, `.tiff`, and original texture masters according to the
repository's reviewed size policy.

Ignore generated output including Unreal `Binaries`, `Intermediate`, `Saved`,
Derived Data Cache, IDE output, cooked/staged packages, local automation
reports, local performance captures, and crash dumps. Do not ignore project
`Config`, intentional `Content`, plugin source/content, `Source`, `.uproject`,
import scripts, or provenance.

The completion audit must fail if:

- a governed binary is tracked outside LFS;
- a generated directory is tracked;
- an external asset lacks provenance;
- an imported asset has no reproducible source/transformation record.

## Provenance sidecar

Each external asset has a reviewable sidecar containing at least:

```yaml
schema: ksa64-visual-provenance-v1
id: provider.collection.asset
source_url: https://example.invalid/source
retrieved_utc: 2026-07-26T00:00:00Z
provider: Provider name
creator: Creator when known
credit: Required display credit
rights_url: https://example.invalid/rights
license_or_basis: Review required
third_party_content_reviewed: false
sha256: "<original-download-sha256>"
source_units: "<declared or unknown>"
source_axis: "<declared or unknown>"
modifications:
  - "<ordered transformation>"
importer_identity: "<script and commit>"
generated_targets:
  - "<Unreal object path and hash>"
engineering_authority: false
notes: "<limitations and intended visual use>"
```

Unknown units, axes, creators, or rights are explicit blockers, not values to
guess. Preserve the untouched download when redistribution rights permit;
otherwise preserve its hash, source locator, and transformation recipe.

## Import and validation

1. Verify the original hash and rights record.
2. Normalize units, basis, names, pivots, topology, and material slots in an
   open master or reproducible script.
3. Import idempotently through source-controlled Unreal Python only during
   development.
4. Validate bounds, scale, texture size/color space, collision, material
   complexity, naming, and stable component attachment.
5. Record importer/engine versions and generated targets.
6. Capture a fixed reference/turntable screenshot.
7. Review Git/LFS changes before accepting content.

Python is not required to load or use the packaged product.

## NASA material

NASA resources are encouraged later for Earth, Moon, terrain, historical
references, and comparison exhibits. They are not KSA-G10R or Firestorm
engineering authority. Every NASA-derived record sets
`engineering_authority: false`.

Before use:

- review the specific asset page and NASA Images and Media Usage Guidelines;
- identify third-party material and its separate rights;
- preserve required creator/provider credits;
- avoid NASA insignia/logotype use and any implication of endorsement;
- verify scale, topology, units, materials, and suitability for real-time use;
- never infer KSA64 mass properties, dimensions, aerodynamics, or attachment
  state from a NASA visual model.

NASA 3D Resources states that its hosted resources are free to download and
use, but points users to the usage guidelines. That general statement does not
replace per-asset third-party review.

## Licensing and release checklist

Before any external content enters an accepted or packaged build, record:

- Unreal Engine EULA and redistribution review for the intended distribution;
- engine, Starter Content, Fab, Marketplace, and plugin source/license;
- asset provider, creator, license/public-domain basis, credit, and
  redistribution constraints;
- trademarks, protected identifiers, likenesses, and endorsement risk;
- whether source redistribution and generated/cooked redistribution differ;
- transformation/import tools and their licenses;
- reviewer, review date, and unresolved restrictions.

References:

- [NASA 3D Resources](https://science.nasa.gov/3d-resources/)
- [NASA Images and Media Usage Guidelines](https://www.nasa.gov/nasa-brand-center/images-and-media/)