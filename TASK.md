# Task: planetary gearbox designer for robot actuators (Rust → WebAssembly, static web app)

Original open-source tool. Write all code yourself; do not copy anyone's code, tables or assets.
Use textbook methods only (involute spur gear geometry, Willis equation, planetary assembly and
neighbour conditions, Lewis bending formula with a simple form-factor approximation, Hertz contact
stress estimate). Material values are generic and clearly labelled "typical" (no vendor names, no
copied datasheet tables).

## Product (SMALL scope — core + UI about 800-1200 lines of new Rust; renderer/math are reused)
A single-page web app (GitHub Pages, sub-path `/planet-gear/`): "I want a ~N:1 single-stage planetary
reducer for my robot joint motor — which tooth counts work, how big is it, and will the teeth survive
the torque?"

1. **Inputs**: sun teeth Zs, planet teeth Zp, number of planets N (2-6), module m (mm, 0.3-3),
   face width b (mm), pressure angle (20° default, 14.5°/25° options), motor torque (N·m) and speed
   (rpm), material preset per gear set: "steel (typical, hardened)", "aluminium 7075 (typical)",
   "POM / acetal (typical)", "nylon PA12 printed (typical)", "PLA printed (typical)",
   "PETG printed (typical)", each with allowable bending stress (MPa), allowable contact stress (MPa),
   E (GPa), Poisson, density; an editable "print knock-down" factor (default 0.6) for printed ones.
   Fixed member selector: ring fixed (sun in, carrier out — default), carrier fixed, sun fixed.
2. **Model (core)**: ring teeth Zr = Zs + 2·Zp; ratio by Willis equation for the chosen fixed member;
   assembly condition (Zs + Zr) / N integer (equal spacing) and a list of the nearest valid N;
   neighbour condition (planet tip circles do not collide: (Zs+Zp)·sin(π/N) > Zp + 2 for standard
   addendum), pitch / tip / root diameters, centre distance, ring outer diameter estimate (ring
   root + rim 2.5·m), contact ratio of sun-planet and planet-ring meshes (external/internal formula),
   undercut warning (Zs or Zp < 17 at 20°), tangential force per planet with a load-sharing factor
   (default 1.15), Lewis bending stress σ = Ft / (b·m·Y) with Y approximated from tooth count (own
   smooth fit, document it), Hertz contact stress for sun-planet, safety factors (bending + contact)
   per gear, output torque and speed (ideal and with a per-mesh efficiency, default 0.98 per mesh),
   mesh frequency (Hz) at the input speed, approximate mass of all gears (solid discs at pitch dia).
   Mark "fails" when any SF < 1 and "invalid" when assembly/neighbour fails.
3. **Extras (small)**: (a) "find designs" search: for a target ratio ± tolerance and max ring outer
   diameter, enumerate Zs, Zp in 12-80 and N in 3-5 that satisfy all conditions, list top 12 sorted by
   |ratio error| then outer diameter, click a row to load it; (b) safety-factor vs module sweep plot
   with the current module marked.
4. **3D view (WebGL2, game-grade — see below)**: real 3D extruded involute gears (sun, N planets,
   internal ring with outer rim, carrier plate with pins drawn translucent), each gear a distinct
   tint, animated rotating at correct relative speeds (kinematically consistent meshing — teeth must
   not visibly interpenetrate at standard geometry), slow-motion speed control, tooth colouring by
   bending SF on a green→yellow→red ramp (toggle); a small arrow showing the input/output direction;
   checker ground with shadow, gearbox standing on its face.
5. **2D plots** (canvas 2D, own small module): SF vs module sweep; and a 2D face view of one
   sun-planet mesh zoom showing the line of action (optional, skip if over budget).
6. Korean + English toggle (browser default). Mobile friendly. CSS prefers-color-scheme only.
Out of scope (do NOT build): helical/herringbone, profile shift optimisation beyond a note, compound
or multi-stage planetary, backlash/tolerance analysis, bearing life, FEM, URL share, JSON/CSV
import/export, STL export.

## 3D view requirements (game-like, reuse the existing renderer)
- `crates/pg_web/src/gl.rs` and `crates/pg_web/src/math.rs` already exist: a tool-independent WebGL2
  renderer (lit meshes, checker/grid ground with fog, sky gradient, stencil planar shadows, translucent
  volumes, lines) and pure math (mat4, damped orbit camera with inertia, ray-plane/ray-sphere picking)
  with unit tests. Reuse them; extend only if needed and keep them tool-independent and tested.
- Scene code in `view3d.rs`. Orbit (left drag) / pan (right drag, two fingers) / zoom (wheel, pinch)
  with inertia, double-click reset, `touch-action:none`. View presets: Perspective / Front (face-on) /
  Side / Exploded (gears separated along the axis with the carrier).
- Direct manipulation: dragging a planet radially in/out changes Zp by ±1 (sun adjusts to keep the
  ring if possible, otherwise ring follows) and re-runs live; dragging the ring rim changes module;
  hover highlight + cursor change.
- Toggle buttons over the view: SF colouring, carrier, play/pause rotation, exploded.
- HUD over the canvas: ratio, output torque N·m, output rpm, min SF (bending/contact), outer
  diameter mm, mass g, status (OK / fails / invalid with reason).
- If WebGL2 is unavailable, show a message and keep the numeric readouts and 2D plots working.
- Release wasm ≲ 350 KB. No crates besides wasm-bindgen, web-sys, js-sys.

## Core crate (pure Rust, no web deps, unit tested) — `crates/pg_core`
- Tooth geometry, involute point generation (for the mesh), kinematics, conditions, strength, search.
- Tests (hand-checkable, 6-10): Zs=12,Zp=24 → Zr=60 and ring-fixed ratio 6.0; carrier-fixed ratio
  = −Zr/Zs; assembly condition true for (12,24,N=3) and false for (13,24,N=3); neighbour condition
  fails for Zs=12,Zp=60,N=6; pitch diameter = m·Z; involute point at the base circle has radius
  r_b and the tip point has radius r_a within 1e-6; Willis speeds give zero ring speed when ring is
  fixed; Lewis stress halves when face width doubles; search results all satisfy every condition
  and are sorted by ratio error.
- `crates/pg_web`: UI, plain web-sys. The existing math.rs/gl.rs tests must keep passing.

## Repo hygiene
- README (Korean first, then English): what it is, model and assumptions (standard full-depth
  involute, no profile shift, equal load sharing with factor, Lewis/Hertz estimates, ideal lubrication),
  controls, local build (`cargo test`, `trunk build --release`), MIT license, "educational
  estimates — validate with real tests / standards (ISO 6336, AGMA) before trusting a robot with it".
- LICENSE: MIT, copyright "planet-gear contributors" (exists). `.gitignore` for target/ dist/ (exists).
- `index.html` with a CSP meta that blocks external requests:
  `default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; object-src 'none'; base-uri 'self'`.
  `Trunk.toml` exists (`filehash = false`, `public_url = "/planet-gear/"`, external `web/boot.js`
  copied via `<link data-trunk rel="copy-file" href="web/boot.js">`, which imports `./pg_web.js`).
- Root `Cargo.toml` workspace with members `crates/pg_core`, `crates/pg_web`, release profile
  opt-level "z", lto, codegen-units 1, panic abort.
- `.github/workflows/pages.yml` exists (trunk 0.22.1, privacy check, cargo test, deploy-pages). Keep it.
- `tools/privacy_check.sh` already exists; keep it and keep it clean.

## Done when
- `cargo test` green (workspace); `trunk build --release` succeeds with no warnings;
  `sh tools/privacy_check.sh` clean.
- Commit author `planet-gear contributors <59640857+roscore@users.noreply.github.com>`.
- Commit everything with clear commits; open a PR. Do NOT create new GitHub repos or change repo settings.
