# planet-gear

## 한국어

로봇 관절 모터용 **1단 유성기어 감속기 설계 도구**입니다. 브라우저에서 100 % 로컬로 동작하는 정적 웹앱(Rust → WebAssembly)이며
서버, 분석 도구, 외부 요청이 전혀 없습니다.

"모터에 약 N:1 감속기를 달고 싶다 — 어떤 잇수 조합이 되는지, 얼마나 커지는지, 이가 토크를 견디는지?"에 답합니다.

### 기능
- 입력: 선/유성 잇수, 유성기어 수 N(2–6), 모듈, 치폭, 압력각(20° / 14.5° / 25°), 모터 토크·속도, 기어별 재료(일반값 프리셋),
  프린팅 재료 감쇠계수(기본 0.6), 고정 요소(링 / 캐리어 / 선), 하중분담계수(1.15), 맞물림당 효율(0.98).
- 결과: 링 잇수 Zr = Zs + 2·Zp, Willis 식 감속비, 조립 조건과 가능한 N 목록, 이웃 조건, 피치/이끝/이뿌리 지름, 중심거리,
  링 외경 추정, 물림률, 언더컷 경고, 유성기어당 접선력, Lewis 굽힘응력, Hertz 접촉응력, 안전계수, 출력 토크·속도, 맞물림 주파수, 질량.
- 설계 찾기: 목표 감속비 ± 허용오차와 최대 외경으로 Zs, Zp ∈ 12–80, N ∈ 3–5 를 탐색해 상위 12개를 표시(클릭하면 불러옴).
- 모듈에 따른 최소 안전계수 그래프(현재 모듈 표시).
- WebGL2 3D 뷰: 실제 인벌류트 치형을 압출한 선·유성·링기어와 반투명 캐리어, 운동학적으로 맞는 회전, 슬로모션, SF 색상,
  입력(노랑)/출력(하늘색) 방향 화살표, 그림자가 있는 체커 바닥. WebGL2가 없으면 수치와 2D 그래프만 표시됩니다.

### 조작
왼쪽 드래그 회전 · 오른쪽 드래그 또는 두 손가락 이동 · 휠/핀치 확대 · 더블클릭 초기화 · 원근/정면/측면/분해도 프리셋.
유성기어를 반경 방향으로 드래그하면 Zp ±1 (가능하면 선기어가 바뀌어 링을 유지, 아니면 링이 따라감), 링 테두리를 드래그하면 모듈 변경.

### 모델과 가정
표준 전치(full-depth) 인벌류트, 전위 없음(이끝높이 1·m, 이뿌리높이 1.25·m), 하중분담계수를 곱한 균등 분담, Lewis / Hertz 추정,
이상적 윤활. 링 외경 = 링 이뿌리원 + 림 2.5·m. 질량은 피치원 지름의 원판(링은 피치원–외경 환형)으로 근사합니다.
Lewis 치형계수는 직접 맞춘 식 Y ≈ (0.473 − 2.74/z)·(1 + 0.035·(α° − 20)) 이며 링(내치)은 랙 값(z → ∞)을 씁니다.
접촉 안전계수는 응력비(허용 / 계산)입니다. 유성기어의 양방향 굽힘(아이들러) 감쇠는 반영하지 않았습니다.
재료 값은 모두 대략적인 "일반값"이며 특정 제품 데이터가 아닙니다. 잇수가 17 미만(20°)이면 실제로는 언더컷이 생기며,
3D 뷰에서는 언더컷을 모델링하지 않아 이뿌리 근처에서 약간 겹쳐 보일 수 있습니다.

**교육용 추정치입니다 — 로봇에 맡기기 전에 실제 시험과 규격(ISO 6336, AGMA)으로 검증하세요.**

### 로컬 빌드
```sh
cargo test
trunk build --release   # 결과물: dist/
sh tools/privacy_check.sh
```

라이선스: MIT (`LICENSE`).

## English

A **single-stage planetary gearbox designer for robot joint motors**: a static web app (Rust → WebAssembly) that runs
100 % client-side — no server, no analytics, no external requests.

It answers: "I want a ~N:1 reducer for my motor — which tooth counts work, how big is it, and will the teeth survive the torque?"

### Features
- Inputs: sun / planet teeth, planet count N (2–6), module, face width, pressure angle (20° / 14.5° / 25°), motor torque and
  speed, material per gear (typical-value presets), print knock-down factor (default 0.6), fixed member (ring / carrier / sun),
  load-sharing factor (1.15), efficiency per mesh (0.98).
- Outputs: ring teeth Zr = Zs + 2·Zp, Willis ratio, assembly condition and valid N list, neighbour condition, pitch / tip / root
  diameters, centre distance, ring outer diameter estimate, contact ratios, undercut warning, tangential force per planet, Lewis
  bending stress, Hertz contact stress, safety factors, output torque and speed, mesh frequency, mass.
- Find designs: searches Zs, Zp in 12–80 and N in 3–5 for a target ratio ± tolerance under a maximum outer diameter; top 12,
  click a row to load it.
- Minimum safety factor vs module plot with the current module marked.
- WebGL2 3D view: extruded true-involute sun, planets and ring, translucent carrier, kinematically consistent rotation,
  slow motion, SF colouring, input (yellow) / output (cyan) direction arrows, checker ground with shadows. Without WebGL2 the
  numbers and the 2D plot keep working.

### Controls
Left drag orbit · right drag or two fingers pan · wheel / pinch zoom · double-click reset · Perspective / Front / Side / Exploded.
Drag a planet radially to change Zp by ±1 (the sun adapts to keep the ring when it can, otherwise the ring follows); drag the
ring rim to change the module.

### Model and assumptions
Standard full-depth involute, no profile shift (addendum 1·m, dedendum 1.25·m), equal load sharing times a factor, Lewis and
Hertz estimates, ideal lubrication. Ring outer diameter = ring root + 2.5·m rim. Mass uses solid discs at pitch diameter (ring:
annulus from pitch to outer diameter). Lewis form factor is an own fit Y ≈ (0.473 − 2.74/z)·(1 + 0.035·(α° − 20)); the internal
ring uses the rack value (z → ∞). Contact safety factor is a stress ratio (allowable / computed). Reversed bending of the planets
(idler) is not derated. All material values are rounded "typical" figures, not from any datasheet. Below 17 teeth (20°) a real
cutter undercuts the root; the 3D view does not model undercut, so teeth may look slightly overlapped near the root there.

**Educational estimates — validate with real tests / standards (ISO 6336, AGMA) before trusting a robot with it.**

### Local build
```sh
cargo test
trunk build --release   # output in dist/
sh tools/privacy_check.sh
```

License: MIT (`LICENSE`).
