//! Single-stage planetary gearbox model: standard full-depth involute spur geometry (no profile
//! shift), Willis kinematics, assembly / neighbour conditions, Lewis bending and Hertz contact
//! estimates, and a small design search. Pure Rust, no web dependencies.
//!
//! Units: lengths mm, forces N, stresses MPa, torque N·m, speed rpm, E in GPa, density kg/m³.
use std::f64::consts::PI;

/// Which member is held still. Input / output: ring fixed → sun in, carrier out;
/// carrier fixed → sun in, ring out; sun fixed → ring in, carrier out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fixed {
    Ring,
    Carrier,
    Sun,
}

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub en: &'static str,
    pub ko: &'static str,
    /// Allowable bending stress (MPa).
    pub sb: f64,
    /// Allowable contact stress (MPa).
    pub sh: f64,
    pub e_gpa: f64,
    pub nu: f64,
    pub rho: f64,
    /// 3D-printed: allowables are multiplied by the print knock-down factor.
    pub printed: bool,
}

const fn mat(en: &'static str, ko: &'static str, sb: f64, sh: f64, e_gpa: f64, nu: f64, rho: f64, printed: bool) -> Material {
    Material { en, ko, sb, sh, e_gpa, nu, rho, printed }
}

/// Generic, rounded "typical" values for rough sizing only (not from any datasheet).
pub const MATERIALS: [Material; 6] = [
    mat("steel (typical, hardened)", "강철 (일반값, 열처리)", 400.0, 1200.0, 210.0, 0.30, 7850.0, false),
    mat("aluminium 7075 (typical)", "알루미늄 7075 (일반값)", 140.0, 450.0, 71.0, 0.33, 2810.0, false),
    mat("POM / acetal (typical)", "POM / 아세탈 (일반값)", 40.0, 60.0, 2.8, 0.35, 1410.0, false),
    mat("nylon PA12 printed (typical)", "나일론 PA12 프린팅 (일반값)", 30.0, 45.0, 1.7, 0.40, 1010.0, true),
    mat("PLA printed (typical)", "PLA 프린팅 (일반값)", 35.0, 50.0, 3.5, 0.36, 1240.0, true),
    mat("PETG printed (typical)", "PETG 프린팅 (일반값)", 30.0, 45.0, 2.1, 0.38, 1270.0, true),
];

#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub zs: u32,
    pub zp: u32,
    pub n: u32,
    /// Module (mm).
    pub m: f64,
    /// Face width (mm).
    pub b: f64,
    pub alpha_deg: f64,
    /// Motor torque (N·m) and speed (rpm) on the input member.
    pub torque: f64,
    pub rpm: f64,
    /// Material index into `MATERIALS` for sun, planets, ring.
    pub mat: [usize; 3],
    pub knock: f64,
    pub fixed: Fixed,
    /// Load-sharing factor on the per-planet force (1 = perfectly equal sharing).
    pub share: f64,
    /// Efficiency per mesh.
    pub eta: f64,
}

impl Default for Input {
    fn default() -> Self {
        Input {
            zs: 18, zp: 36, n: 3, m: 0.8, b: 8.0, alpha_deg: 20.0, torque: 0.3, rpm: 3000.0,
            mat: [2, 2, 2], knock: 0.6, fixed: Fixed::Ring, share: 1.15, eta: 0.98,
        }
    }
}

/// Pitch, tip and root diameters (mm). For the internal ring the tip is the inner diameter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dia {
    pub d: f64,
    pub da: f64,
    pub df: f64,
}

/// External gear (internal = false) or internal ring, addendum 1·m, dedendum 1.25·m.
pub fn diameters(z: u32, m: f64, internal: bool) -> Dia {
    let d = m * z as f64;
    let s = if internal { -1.0 } else { 1.0 };
    Dia { d, da: d + s * 2.0 * m, df: d - s * 2.5 * m }
}

pub fn ring_teeth(zs: u32, zp: u32) -> u32 {
    zs + 2 * zp
}

/// Input / output speed ratio (negative = output turns the other way).
pub fn ratio(zs: u32, zr: u32, fixed: Fixed) -> f64 {
    let (s, r) = (zs as f64, zr as f64);
    match fixed {
        Fixed::Ring => 1.0 + r / s,
        Fixed::Carrier => -r / s,
        Fixed::Sun => 1.0 + s / r,
    }
}

/// Absolute speeds [sun, ring, carrier] for input speed `n_in` from the Willis equation
/// (ωs − ωc)·Zs = −(ωr − ωc)·Zr, with the fixed member at zero.
pub fn speeds(zs: u32, zr: u32, fixed: Fixed, n_in: f64) -> [f64; 3] {
    let (s, r) = (zs as f64, zr as f64);
    match fixed {
        Fixed::Ring => [n_in, 0.0, n_in * s / (s + r)],
        Fixed::Carrier => [n_in, -n_in * s / r, 0.0],
        Fixed::Sun => [0.0, n_in, n_in * r / (s + r)],
    }
}

/// Equal spacing: (Zs + Zr) / N must be an integer.
pub fn assembly_ok(zs: u32, zp: u32, n: u32) -> bool {
    n > 0 && (zs + ring_teeth(zs, zp)) % n == 0
}

/// Neighbouring planet tip circles must not touch (standard addendum).
pub fn neighbour_ok(zs: u32, zp: u32, n: u32) -> bool {
    n >= 2 && (zs + zp) as f64 * (PI / n as f64).sin() > zp as f64 + 2.0
}

/// Planet counts 2…6 that pass both conditions.
pub fn valid_n(zs: u32, zp: u32) -> Vec<u32> {
    (2..=6).filter(|&n| assembly_ok(zs, zp, n) && neighbour_ok(zs, zp, n)).collect()
}

/// Transverse contact ratio. External mesh: gears 1, 2 with tip / base radii and centre distance a.
/// Internal mesh (gear 2 = ring, its tip radius is inside): the ring term changes sign.
pub fn contact_ratio(z1: u32, z2: u32, m: f64, alpha: f64, internal: bool) -> f64 {
    let (r1, r2) = (m * z1 as f64 / 2.0, m * z2 as f64 / 2.0);
    let (ra1, ra2) = (r1 + m, if internal { r2 - m } else { r2 + m });
    let (rb1, rb2) = (r1 * alpha.cos(), r2 * alpha.cos());
    let len = |ra: f64, rb: f64| (ra * ra - rb * rb).max(0.0).sqrt();
    let pb = PI * m * alpha.cos();
    if internal {
        (len(ra1, rb1) - len(ra2, rb2) + (r2 - r1) * alpha.sin()) / pb
    } else {
        (len(ra1, rb1) + len(ra2, rb2) - (r1 + r2) * alpha.sin()) / pb
    }
}

/// Fewest teeth without undercut for a standard rack cutter: 2 / sin²α (17 at 20°).
pub fn min_teeth(alpha: f64) -> u32 {
    (2.0 / alpha.sin().powi(2)).round() as u32
}

/// Lewis form factor, own smooth fit Y ≈ (0.473 − 2.74/z)·kα through the classic 20° full-depth
/// values at z = 12 (≈0.245) and z = 100 (≈0.446); kα = 1 + 0.035·(α° − 20) scales for the
/// 14.5° / 25° systems. Within ~5 % of tabulated values for z ≥ 12. `None` = rack (internal ring).
pub fn lewis_y(z: Option<u32>, alpha_deg: f64) -> f64 {
    let base = match z {
        Some(z) => 0.473 - 2.74 / z.max(6) as f64,
        None => 0.473,
    };
    base * (1.0 + 0.035 * (alpha_deg - 20.0))
}

/// Lewis bending stress σ = Ft / (b·m·Y).
pub fn lewis_stress(ft: f64, b: f64, m: f64, y: f64) -> f64 {
    ft / (b * m * y)
}

/// Hertz line-contact stress at the pitch point; ρ = r·sin α, internal contact subtracts curvature.
pub fn hertz(fn_: f64, b: f64, r1: f64, r2: f64, alpha: f64, m1: &Material, m2: &Material, internal: bool) -> f64 {
    let curv = 1.0 / (r1 * alpha.sin()) + if internal { -1.0 } else { 1.0 } / (r2 * alpha.sin());
    let compl = (1.0 - m1.nu * m1.nu) / (m1.e_gpa * 1e3) + (1.0 - m2.nu * m2.nu) / (m2.e_gpa * 1e3);
    (fn_ / b * curv / (PI * compl)).sqrt()
}

/// Involute tooth outline (closed, counter-clockwise, xy in mm) of an external gear with tip
/// radius `ra` and root radius `rf`; tooth k is centred on angle 2πk/z. `flank` samples per flank.
/// For an internal ring pass ra = ring root, rf = ring tip and `space_centred`, so material
/// (ring teeth) is centred on 2πk/z.
pub fn outline(z: u32, m: f64, alpha: f64, ra: f64, rf: f64, flank: usize, space_centred: bool) -> Vec<[f64; 2]> {
    let r = m * z as f64 / 2.0;
    let rb = r * alpha.cos();
    let inv = |a: f64| a.tan() - a;
    let half = PI / (2.0 * z as f64) + inv(alpha); // half tooth angle at the base circle
    let pitch = 2.0 * PI / z as f64;
    // Half-angle of the tooth at radius q (radial below the base circle).
    let at = |q: f64| if q <= rb { half } else { half - inv((rb / q).acos()) };
    let r0 = rf.max(0.0);
    let mut pts = vec![];
    for k in 0..z {
        let c = pitch * k as f64 + if space_centred { pitch / 2.0 } else { 0.0 };
        let mut push = |q: f64, a: f64| pts.push([q * a.cos(), q * a.sin()]);
        // Root, then rising flank (−side), tip, falling flank (+side).
        let lo = r0.max(rb.min(ra));
        push(r0, c - at(lo).min(pitch / 2.0));
        for i in 0..=flank {
            let q = lo + (ra - lo) * i as f64 / flank as f64;
            push(q, c - at(q).max(0.0));
        }
        for i in (0..=flank).rev() {
            let q = lo + (ra - lo) * i as f64 / flank as f64;
            push(q, c + at(q).max(0.0));
        }
        push(r0, c + at(lo).min(pitch / 2.0));
    }
    pts
}

/// Planet k's rotation (rad, world frame) for sun angle `ts` and carrier angle `tc`, so that it
/// meshes with a sun whose tooth 0 is at angle 0. Rolling: Zs·(ts − tc) = −Zp·(tp − tc). The
/// constant puts a planet gap (π/Zp) on the sun tooth at the contact direction. With the assembly
/// condition holding, the ring phase `ring_phase` is then consistent for every planet.
pub fn planet_phase(zs: u32, zp: u32, n: u32, k: u32, ts: f64, tc: f64) -> f64 {
    let (s, p) = (zs as f64, zp as f64);
    let phi = 2.0 * PI * k as f64 / n as f64; // planet position relative to the carrier
    tc + phi + PI - PI / p + s * (phi - (ts - tc)) / p
}

/// Ring rotation (world) meshing with the planets for sun angle `ts`, carrier angle `tc`
/// (ring tooth centred on angle 0 when the result is 0).
pub fn ring_phase(zs: u32, zp: u32, ts: f64, tc: f64) -> f64 {
    let (p, r) = (zp as f64, ring_teeth(zs, zp) as f64);
    // Planet 0 rotation ρ relative to the carrier; a ring tooth meets a planet gap at the contact
    // direction: Zr·(0 − τ) ≡ π + Zp·(0 − ρ).
    let rho = planet_phase(zs, zp, 1, 0, ts, tc) - tc;
    tc - (PI - p * rho) / r
}

#[derive(Clone, Debug)]
pub struct Report {
    pub zr: u32,
    pub ratio: f64,
    pub assembly: bool,
    pub neighbour: bool,
    pub valid_n: Vec<u32>,
    /// Sun, planet, ring.
    pub dia: [Dia; 3],
    pub centre: f64,
    pub outer: f64,
    /// Sun-planet and planet-ring contact ratios.
    pub eps: [f64; 2],
    pub undercut: bool,
    /// Tangential force per planet incl. load sharing (N).
    pub ft: f64,
    /// Bending stress sun, planet, ring; contact stress sun-planet, planet-ring (MPa).
    pub sig_b: [f64; 3],
    pub sig_h: [f64; 2],
    /// Safety factors per gear (sun, planet, ring): bending, contact.
    pub sf_b: [f64; 3],
    pub sf_h: [f64; 3],
    pub t_out: f64,
    pub t_out_eff: f64,
    pub n_out: f64,
    /// Absolute speeds [sun, ring, carrier] (rpm).
    pub speeds: [f64; 3],
    pub planet_rpm: f64,
    pub mesh_hz: f64,
    pub mass_g: f64,
}

impl Report {
    pub fn valid(&self) -> bool {
        self.assembly && self.neighbour
    }
    pub fn fails(&self) -> bool {
        self.sf_b.iter().chain(&self.sf_h).any(|&s| s < 1.0)
    }
    pub fn min_sf(&self) -> (f64, f64) {
        let min = |a: &[f64; 3]| a.iter().copied().fold(f64::INFINITY, f64::min);
        (min(&self.sf_b), min(&self.sf_h))
    }
}

pub fn analyze(i: &Input) -> Report {
    let (zs, zp, n, m) = (i.zs, i.zp, i.n.max(1), i.m);
    let zr = ring_teeth(zs, zp);
    let alpha = i.alpha_deg.to_radians();
    let dia = [diameters(zs, m, false), diameters(zp, m, false), diameters(zr, m, true)];
    let mats = i.mat.map(|k| MATERIALS[k.min(MATERIALS.len() - 1)]);
    let allow = |mt: &Material, v: f64| if mt.printed { v * i.knock } else { v };
    let ratio = ratio(zs, zr, i.fixed);
    let sp = speeds(zs, zr, i.fixed, i.rpm);
    // Torque on the sun: input torque, or the sun's reaction share Zs/Zr of the ring input.
    let t_sun = if i.fixed == Fixed::Sun { i.torque * zs as f64 / zr as f64 } else { i.torque };
    let ft = t_sun.abs() * 1e3 / (n as f64 * dia[0].d / 2.0) * i.share;
    let fn_ = ft / alpha.cos();
    let sig_b = [Some(zs), Some(zp), None].map(|z| lewis_stress(ft, i.b, m, lewis_y(z, i.alpha_deg)));
    let (rs, rp, rr) = (dia[0].d / 2.0, dia[1].d / 2.0, dia[2].d / 2.0);
    let sig_h = [
        hertz(fn_, i.b, rs, rp, alpha, &mats[0], &mats[1], false),
        hertz(fn_, i.b, rp, rr, alpha, &mats[1], &mats[2], true),
    ];
    let sf_b = std::array::from_fn(|k| allow(&mats[k], mats[k].sb) / sig_b[k]);
    let h = [sig_h[0], sig_h[0].max(sig_h[1]), sig_h[1]];
    let sf_h = std::array::from_fn(|k| allow(&mats[k], mats[k].sh) / h[k]);
    let outer = dia[2].df + 2.0 * 2.5 * m;
    // Solid discs at pitch diameter for sun / planets; ring as annulus pitch…outer.
    let disc = |d: f64| PI / 4.0 * d * d * i.b * 1e-9;
    let mass = mats[0].rho * disc(dia[0].d)
        + n as f64 * mats[1].rho * disc(dia[1].d)
        + mats[2].rho * (disc(outer) - disc(dia[2].d));
    let zmin = min_teeth(alpha);
    Report {
        zr,
        ratio,
        assembly: assembly_ok(zs, zp, n),
        neighbour: neighbour_ok(zs, zp, n),
        valid_n: valid_n(zs, zp),
        dia,
        centre: (zs + zp) as f64 * m / 2.0,
        outer,
        eps: [contact_ratio(zs, zp, m, alpha, false), contact_ratio(zp, zr, m, alpha, true)],
        undercut: zs < zmin || zp < zmin,
        ft,
        sig_b,
        sig_h,
        sf_b,
        sf_h,
        t_out: i.torque * ratio,
        t_out_eff: i.torque * ratio * i.eta * i.eta,
        n_out: i.rpm / ratio,
        speeds: sp,
        planet_rpm: sp[2] - (sp[0] - sp[2]) * zs as f64 / zp as f64,
        mesh_hz: zs as f64 * (sp[0] - sp[2]).abs() / 60.0,
        mass_g: mass * 1e3,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Design {
    pub zs: u32,
    pub zp: u32,
    pub n: u32,
    pub ratio: f64,
    pub err: f64,
    pub outer: f64,
}

/// Zs, Zp in 12…80 and N in 3…5 meeting assembly, neighbour, |ratio − target| ≤ tol and
/// outer diameter ≤ `max_od` (with the module / fixed member of `i`); best 12 by error then size.
pub fn search(i: &Input, target: f64, tol: f64, max_od: f64) -> Vec<Design> {
    let mut out = vec![];
    for zs in 12..=80 {
        for zp in 12..=80 {
            let zr = ring_teeth(zs, zp);
            let r = ratio(zs, zr, i.fixed);
            let outer = diameters(zr, i.m, true).df + 5.0 * i.m;
            if (r - target).abs() > tol || outer > max_od {
                continue;
            }
            for n in 3..=5 {
                if assembly_ok(zs, zp, n) && neighbour_ok(zs, zp, n) {
                    out.push(Design { zs, zp, n, ratio: r, err: (r - target).abs(), outer });
                }
            }
        }
    }
    out.sort_by(|a, b| a.err.total_cmp(&b.err).then(a.outer.total_cmp(&b.outer)).then(b.n.cmp(&a.n)));
    out.truncate(12);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) {
        assert!((a - b).abs() < tol, "{a} vs {b}");
    }

    #[test]
    fn ring_teeth_and_ratios() {
        assert_eq!(ring_teeth(12, 24), 60);
        close(ratio(12, 60, Fixed::Ring), 6.0, 1e-12);
        close(ratio(12, 60, Fixed::Carrier), -5.0, 1e-12);
        close(ratio(12, 60, Fixed::Sun), 1.2, 1e-12);
        let r = analyze(&Input::default());
        close(r.ratio, 6.0, 1e-12);
        close(r.n_out, 500.0, 1e-9);
    }

    #[test]
    fn willis_speeds() {
        for f in [Fixed::Ring, Fixed::Carrier, Fixed::Sun] {
            let [s, r, c] = speeds(12, 60, f, 600.0);
            close((s - c) * 12.0, -(r - c) * 60.0, 1e-9);
            let (inp, out) = match f {
                Fixed::Sun => (r, c),
                Fixed::Carrier => (s, r),
                Fixed::Ring => (s, c),
            };
            close(inp / out, ratio(12, 60, f), 1e-9);
        }
        assert_eq!(speeds(12, 60, Fixed::Ring, 600.0)[1], 0.0);
    }

    #[test]
    fn assembly_and_neighbour() {
        assert!(assembly_ok(12, 24, 3));
        assert!(!assembly_ok(13, 24, 3));
        assert!(neighbour_ok(12, 24, 3));
        assert!(!neighbour_ok(12, 60, 6));
        // N = 4: 36·sin 45° ≈ 25.5 < 26, tips collide.
        assert_eq!(valid_n(12, 24), vec![2, 3]);
    }

    #[test]
    fn diameters_and_contact_ratio() {
        let d = diameters(24, 1.5, false);
        close(d.d, 36.0, 1e-12);
        close(d.da, 39.0, 1e-12);
        close(d.df, 32.25, 1e-12);
        let ring = diameters(60, 1.0, true);
        assert!(ring.da < ring.d && ring.d < ring.df);
        // Classic 20° values: an external pair sits around 1.5…1.7, internal meshes higher.
        let e = contact_ratio(12, 24, 1.0, 20f64.to_radians(), false);
        let i = contact_ratio(24, 60, 1.0, 20f64.to_radians(), true);
        assert!(e > 1.3 && e < 1.7, "{e}");
        assert!(i > e && i < 2.2, "{i}");
        assert_eq!(min_teeth(20f64.to_radians()), 17);
    }

    #[test]
    fn involute_outline_radii() {
        let (z, m, a) = (20, 2.0, 20f64.to_radians());
        let d = diameters(z, m, false);
        let pts = outline(z, m, a, d.da / 2.0, d.df / 2.0, 6, false);
        let rad = |p: [f64; 2]| p[0].hypot(p[1]);
        let rb = d.d / 2.0 * a.cos();
        // Per tooth: root, base point, 5 flank points, tip, tip, … — check base and tip points.
        close(rad(pts[1]), rb, 1e-6);
        close(rad(pts[7]), d.da / 2.0, 1e-6);
        assert_eq!(pts.len(), z as usize * 16);
        // Tooth 0 is symmetric about angle 0.
        close(pts[7][1].atan2(pts[7][0]), -pts[8][1].atan2(pts[8][0]), 1e-12);
    }

    #[test]
    fn lewis_halves_with_double_width() {
        let y = lewis_y(Some(20), 20.0);
        assert!(y > 0.3 && y < 0.36, "{y}");
        close(lewis_stress(100.0, 10.0, 1.0, y), 2.0 * lewis_stress(100.0, 20.0, 1.0, y), 1e-12);
        let mut i = Input::default();
        let a = analyze(&i).sig_b[0];
        i.b *= 2.0;
        close(analyze(&i).sig_b[0], a / 2.0, 1e-9);
    }

    #[test]
    fn meshing_phase_rolls_without_slip() {
        let (zs, zp, n) = (12, 24, 3);
        for k in 0..n {
            let p0 = planet_phase(zs, zp, n, k, 0.0, 0.0);
            let p1 = planet_phase(zs, zp, n, k, 0.3, 0.0);
            // Carrier still: arc on the sun pitch circle equals arc on the planet pitch circle.
            close(0.3 * zs as f64, -(p1 - p0) * zp as f64, 1e-12);
        }
        // Ring held: with the ring-fixed speed relation the ring phase stays constant, and the
        // ring meshes with every planet (assembly condition): Zr·(φk − τ) − Zp·(φk − ρk) ≡ ½ turn.
        let zr = ring_teeth(zs, zp) as f64;
        let tc = 0.4;
        let ts = tc * (zs as f64 + zr) / zs as f64;
        close(ring_phase(zs, zp, ts, tc), ring_phase(zs, zp, 0.0, 0.0), 1e-9);
        let tau = ring_phase(zs, zp, ts, tc);
        for k in 0..n {
            let phi = tc + 2.0 * PI * k as f64 / n as f64;
            let rho = planet_phase(zs, zp, n, k, ts, tc);
            let x = (zr * (phi - tau) - zp as f64 * (phi - rho)) / (2.0 * PI);
            close((x - 0.5).rem_euclid(1.0).min(1.0 - (x - 0.5).rem_euclid(1.0)), 0.0, 1e-9);
        }
    }

    /// Radius of a star-shaped closed outline at polar angle `t` (linear between samples).
    fn polar(pts: &[[f64; 2]]) -> Vec<(f64, f64)> {
        pts.iter().map(|p| (p[1].atan2(p[0]).rem_euclid(2.0 * PI), p[0].hypot(p[1]))).collect()
    }
    fn radius_at(pol: &[(f64, f64)], t: f64) -> f64 {
        let t = t.rem_euclid(2.0 * PI);
        for k in 0..pol.len() {
            let (a, b) = (pol[k], pol[(k + 1) % pol.len()]);
            let span = (b.0 - a.0).rem_euclid(2.0 * PI);
            let off = (t - a.0).rem_euclid(2.0 * PI);
            if off <= span && span < PI {
                return a.1 + (b.1 - a.1) * if span > 0.0 { off / span } else { 0.0 };
            }
        }
        unreachable!()
    }

    #[test]
    fn rotating_teeth_do_not_interpenetrate() {
        // Undercut-free tooth counts only: below 17 teeth (20°) a standard planet tip reaches under
        // the sun's base circle, where a real cutter would undercut (not modelled; UI warns).
        for (zs, zp, n) in [(18, 36, 3), (18, 27, 3), (21, 30, 3)] {
            let (m, a) = (1.0, 20f64.to_radians());
            let zr = ring_teeth(zs, zp);
            assert!(assembly_ok(zs, zp, n));
            let [ds, dp, dr] = [diameters(zs, m, false), diameters(zp, m, false), diameters(zr, m, true)];
            let sun = polar(&outline(zs, m, a, ds.da / 2.0, ds.df / 2.0, 12, false));
            let planet = outline(zp, m, a, dp.da / 2.0, dp.df / 2.0, 12, false);
            let ring = polar(&outline(zr, m, a, dr.df / 2.0, dr.da / 2.0, 12, true));
            let c = (zs + zp) as f64 * m / 2.0;
            let tol = 0.02 * m; // polyline sampling error
            for step in 0..9 {
                let tc = 0.037 * step as f64; // covers more than one sun tooth pitch relative to the carrier
                let ts = tc * (zs + zr) as f64 / zs as f64; // ring fixed
                let tr = ring_phase(zs, zp, ts, tc);
                for k in 0..n {
                    let (tp, phi) = (planet_phase(zs, zp, n, k, ts, tc), tc + 2.0 * PI * k as f64 / n as f64);
                    for p in &planet {
                        let (s, co) = tp.sin_cos();
                        let w = [c * phi.cos() + co * p[0] - s * p[1], c * phi.sin() + s * p[0] + co * p[1]];
                        let (r, t) = (w[0].hypot(w[1]), w[1].atan2(w[0]));
                        assert!(r > radius_at(&sun, t - ts) - tol, "sun overlap {zs}/{zp} k{k} step{step}");
                        assert!(r < radius_at(&ring, t - tr) + tol, "ring overlap {zs}/{zp} k{k} step{step}");
                    }
                }
            }
        }
    }

    #[test]
    fn search_results_valid_and_sorted() {
        let i = Input::default();
        let res = search(&i, 5.0, 0.3, 90.0);
        assert!(!res.is_empty());
        for d in &res {
            assert!(assembly_ok(d.zs, d.zp, d.n) && neighbour_ok(d.zs, d.zp, d.n));
            assert!((12..=80).contains(&d.zs) && (12..=80).contains(&d.zp) && (3..=5).contains(&d.n));
            assert!(d.err <= 0.3 && d.outer <= 90.0);
            close(d.ratio, ratio(d.zs, ring_teeth(d.zs, d.zp), i.fixed), 1e-12);
        }
        assert!(res.windows(2).all(|w| w[0].err < w[1].err || (w[0].err == w[1].err && w[0].outer <= w[1].outer)));
    }

    #[test]
    fn strength_sanity() {
        let i = Input::default();
        let r = analyze(&i);
        // Ft = T / (N·rs) · K = 0.3e3 / (3·7.2) · 1.15.
        close(r.ft, 300.0 / 21.6 * 1.15, 1e-9);
        assert!(!r.undercut);
        assert!(r.sig_h[1] < r.sig_h[0], "internal contact is gentler");
        assert!(r.sf_b[2] > r.sf_b[0], "ring teeth (rack form) are strongest");
        close(r.t_out_eff, r.t_out * 0.98 * 0.98, 1e-12);
        close(r.mesh_hz, 18.0 * (3000.0 - 500.0) / 60.0, 1e-9);
        assert!(r.valid());
    }
}
