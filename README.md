# slop-game

A procedurally generated, low-poly "gaming landscape" built entirely with Python (`bpy`) in Blender — no manual modeling.

## Contents

- `blender_landscape/build_scene.py` — the generation script. Builds a fractal-noise mountain terrain (grass → rock → snow shading by height), scatters low-poly pine trees and rocks, adds a lake in the lowest valley, sets up a stylized sunset sky with a glowing sun, and renders the final image.
- `blender_landscape/gaming_landscape.blend` — the resulting Blender project file.
- `blender_landscape/gaming_landscape.png` — the rendered output.

## Usage

Requires [Blender](https://www.blender.org/) (tested on 5.2 LTS):

```bash
blender -b --python blender_landscape/build_scene.py
```

This regenerates the terrain (seeded, so it's reproducible) and re-renders `gaming_landscape.png`.

---
This repository's initial commit was created by an AI agent (OpenHands) on behalf of the repo owner.
