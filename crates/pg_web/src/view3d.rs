//! 3D scene: extruded involute sun, planets and ring, translucent carrier, rotating with
//! kinematically consistent mesh phases; orbit / pan / zoom camera and direct manipulation.
use crate::gl::{self, Mesh, Paint, Renderer};
use crate::math::{self, Orbit, OrbitCam, M4, V};
use pg_core::{self as core, Input, Report};
use std::f32::consts::{PI, TAU};
use web_sys::{HtmlCanvasElement, PointerEvent, WheelEvent};

/// World units per mm (100 mm gearbox ≈ 2 units).
const S: f32 = 0.02;

const SUN: [f32; 4] = [0.95, 0.62, 0.25, 1.0];
const PLANET: [f32; 4] = [0.35, 0.60, 0.95, 1.0];
const RING: [f32; 4] = [0.70, 0.72, 0.76, 1.0];

#[derive(Clone, Copy, PartialEq)]
pub enum Hit {
    None,
    Planet(u32),
    Ring,
}

/// Direct-manipulation result for the app to apply.
pub enum Edit {
    Teeth(u32, u32),
    Module(f64),
}

enum Drag {
    Orbit,
    Pan,
    Planet { dir: [f32; 2], start: f32, zs: u32, zp: u32 },
    Ring { start: f32, m: f64 },
}

struct Meshes {
    sun: Mesh,
    planet: Mesh,
    ring: Mesh,
    plate: Mesh,
    pin: Mesh,
    arrows: Mesh,
}

pub struct View {
    r: Renderer,
    canvas: HtmlCanvasElement,
    cam: OrbitCam,
    meshes: Option<Meshes>,
    inp: Input,
    rep: Option<Report>,
    /// Input member angle (rad) of the slowed-down animation.
    pub phase: f32,
    pub playing: bool,
    pub slow: f32,
    pub sf_color: bool,
    pub carrier: bool,
    pub exploded: bool,
    explode: f32,
    hover: Hit,
    drag: Option<Drag>,
    /// Active pointers (id, x, y) for two-finger pan / pinch.
    ptrs: Vec<(i32, f32, f32)>,
}

fn rot_y(a: f32, t: V) -> M4 {
    let (s, c) = a.sin_cos();
    math::basis([c, 0.0, s], [0.0, 1.0, 0.0], [-s, 0.0, c], [S; 3], t)
}

fn circle_at(pts: &[[f32; 2]], r: f32) -> Vec<[f32; 2]> {
    pts.iter().map(|p| { let a = p[1].atan2(p[0]); [r * a.cos(), r * a.sin()] }).collect()
}

/// Green (SF ≥ 2.5) → yellow (1.5) → red (≤ 1).
pub fn sf_ramp(sf: f64) -> [f32; 4] {
    let t = ((sf as f32 - 1.0) / 1.5).clamp(0.0, 1.0);
    if t < 0.5 { [0.9, 0.2 + 1.3 * t, 0.15, 1.0] } else { [0.9 - 1.4 * (t - 0.5), 0.85, 0.2, 1.0] }
}

impl View {
    pub fn new(canvas: HtmlCanvasElement, dark: bool) -> Option<Self> {
        let mut r = Renderer::new(&canvas)?;
        r.theme = if dark { gl::DARK } else { gl::LIGHT };
        r.cell = 10.0 * S;
        let o = Orbit { target: [0.0; 3], yaw: 0.6, pitch: 0.55, dist: 3.0 };
        Some(View {
            r, canvas, cam: OrbitCam::new(o), meshes: None, inp: Input::default(), rep: None,
            phase: 0.0, playing: true, slow: 0.3, sf_color: false, carrier: true, exploded: false,
            explode: 0.0, hover: Hit::None, drag: None, ptrs: vec![],
        })
    }

    pub fn set_design(&mut self, inp: &Input, rep: &Report) {
        let first = self.rep.is_none();
        self.inp = *inp;
        self.rep = Some(rep.clone());
        if let Some(m) = self.meshes.take() {
            for x in [m.sun, m.planet, m.ring, m.plate, m.pin, m.arrows] {
                self.r.free(x);
            }
        }
        self.meshes = self.build(inp, rep);
        if first {
            self.preset("persp");
        }
    }

    fn build(&self, i: &Input, rep: &Report) -> Option<Meshes> {
        let (m, a) = (i.m, i.alpha_deg.to_radians());
        let f32s = |v: Vec<[f64; 2]>| -> Vec<[f32; 2]> { v.iter().map(|p| [p[0] as f32, p[1] as f32]).collect() };
        let flank = (900 / rep.zr.max(1) as usize).clamp(2, 5);
        let gear = |z: u32, d: &core::Dia, bore: f32| {
            let o = f32s(core::outline(z, m, a, d.da / 2.0, d.df / 2.0, flank, false));
            gl::prism(&circle_at(&o, bore), &o, i.b as f32)
        };
        let [ds, dp, dr] = rep.dia;
        let pin_r = (dp.df / 2.0 * 0.3) as f32;
        let ring_in = f32s(core::outline(rep.zr, m, a, dr.df / 2.0, dr.da / 2.0, flank, true));
        let ring = gl::prism(&ring_in, &circle_at(&ring_in, rep.outer as f32 / 2.0), i.b as f32);
        // Direction arrows around the input and output members (0 sun, 1 ring, 2 carrier).
        let sp = core::speeds(i.zs, rep.zr, i.fixed, i.rpm.signum());
        let (ik, ok) = match i.fixed {
            core::Fixed::Sun => (1, 2),
            core::Fixed::Carrier => (0, 1),
            core::Fixed::Ring => (0, 2),
        };
        let rad = |k: usize| [ds.d * 0.35, rep.outer * 0.5 + 2.0 * m, rep.centre][k] as f32;
        let y = i.b as f32 * 1.6;
        let mut seg = vec![];
        for (r, s) in [(rad(ik), sp[ik]), (rad(ok), sp[ok])] {
            let dir = s.signum() as f32;
            let p = |t: f32, rr: f32| [rr * t.cos(), y, rr * t.sin()];
            for k in 0..16 {
                seg.extend([p(dir * k as f32 * 0.1, r), p(dir * (k + 1) as f32 * 0.1, r)]);
            }
            let tip = p(dir * 1.6, r);
            seg.extend([tip, p(dir * 1.45, r * 1.12), tip, p(dir * 1.45, r * 0.88)]);
        }
        // Input arrow yellow, output arrow cyan.
        let mut arrows = gl::lines(&seg);
        arrows.col = (0..seg.len()).flat_map(|k| if k < seg.len() / 2 { [1.0, 0.85, 0.2] } else { [0.3, 0.9, 1.0] }).collect();
        Some(Meshes {
            sun: self.r.mesh(&gear(i.zs, &ds, (ds.df / 2.0 * 0.3) as f32))?,
            planet: self.r.mesh(&gear(i.zp, &dp, pin_r))?,
            ring: self.r.mesh(&ring)?,
            plate: self.r.mesh(&gl::cylinder(48))?,
            pin: self.r.mesh(&gl::cylinder(16))?,
            arrows: self.r.mesh(&arrows)?,
        })
    }

    pub fn preset(&mut self, name: &str) {
        let od = self.rep.as_ref().map_or(60.0, |r| r.outer) as f32 * S;
        let mut o = Orbit { target: [0.0, self.inp.b as f32 * S * 0.5, 0.0], yaw: 0.6, pitch: 0.55, dist: od * 2.2 };
        match name {
            "front" => o.pitch = math::PITCH_MAX,
            "side" => o.pitch = 0.05,
            _ => {}
        }
        self.exploded = name == "exploded";
        if self.exploded {
            o.target[1] += 2.0 * self.inp.b as f32 * S;
        }
        self.cam.set_goal(o);
    }

    /// World-frame angles: sun, carrier, ring, planet k.
    fn angles(&self) -> (f32, f32, f32, impl Fn(u32) -> f32 + '_) {
        let rep = self.rep.as_ref().expect("design set");
        let i = &self.inp;
        let sp = core::speeds(i.zs, rep.zr, i.fixed, i.rpm.signum());
        let th = self.phase as f64;
        // Input is the sun unless the sun is fixed (then the ring drives).
        let (ts, tc) = (sp[0] * th, sp[2] * th);
        let ring = core::ring_phase(i.zs, i.zp, ts, tc) as f32;
        (ts as f32, tc as f32, ring, move |k| core::planet_phase(i.zs, i.zp, i.n, k, ts, tc) as f32)
    }

    fn planet_pos(&self, k: u32, tc: f32) -> [f32; 2] {
        let a = tc + TAU * k as f32 / self.inp.n.max(1) as f32;
        let c = self.rep.as_ref().map_or(0.0, |r| r.centre) as f32;
        [c * a.cos(), c * a.sin()]
    }

    pub fn frame(&mut self, dt: f32) {
        let (w, h) = (self.canvas.client_width().max(1), self.canvas.client_height().max(1));
        let dpr = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio()) as f32;
        let (pw, ph) = ((w as f32 * dpr) as u32, (h as f32 * dpr) as u32);
        if self.canvas.width() != pw || self.canvas.height() != ph {
            self.canvas.set_width(pw);
            self.canvas.set_height(ph);
        }
        if self.playing {
            self.phase += dt * self.slow * PI;
        }
        let goal = if self.exploded { 1.0 } else { 0.0 };
        self.explode += (goal - self.explode) * (1.0 - (-dt * 6.0).exp());
        self.cam.step(dt);
        let (Some(rep), Some(ms)) = (self.rep.as_ref(), self.meshes.as_ref()) else { return };
        let o = self.cam.cur;
        let eye = o.eye();
        let view = math::look_at(eye, o.target, [0.0, 1.0, 0.0]);
        let proj = math::perspective(0.8, pw as f32 / ph as f32, o.dist * 0.02, o.dist * 40.0);
        self.r.fog = [o.dist * 4.0, o.dist * 15.0];
        self.r.begin(&view, &proj, eye, [0, 0, pw as i32, ph as i32]);

        let (b, e) = (self.inp.b as f32, self.explode);
        let (ts, tc, tr, tp) = self.angles();
        let paint = |base: [f32; 4], sf: f64, hit: bool| {
            let mut p = Paint::lit(if self.sf_color { sf_ramp(sf) } else { base });
            p.glow = if hit { 0.3 } else { 0.0 };
            p
        };
        let mut solid: Vec<(&Mesh, M4, Paint)> = vec![
            (&ms.ring, rot_y(tr, [0.0; 3]), paint(RING, rep.sf_b[2], self.hover == Hit::Ring)),
            (&ms.sun, rot_y(ts, [0.0, e * 3.0 * b * S, 0.0]), paint(SUN, rep.sf_b[0], false)),
        ];
        for k in 0..self.inp.n {
            let p = self.planet_pos(k, tc);
            let m = rot_y(tp(k), [p[0] * S, e * 1.5 * b * S, p[1] * S]);
            solid.push((&ms.planet, m, paint(PLANET, rep.sf_b[1], self.hover == Hit::Planet(k))));
        }
        for (m, model, _) in &solid {
            self.r.shadow(m, model);
        }
        for (m, model, p) in &solid {
            self.r.draw(m, model, *p);
        }
        let lift = e * 4.5 * b * S;
        self.r.draw(&ms.arrows, &rot_y(0.0, [0.0, lift, 0.0]), Paint::flat([1.0; 4]));
        if self.carrier {
            // Translucent plate above the gears plus one pin per planet.
            let th = (b * 0.3).max(1.0);
            let pr = (rep.centre + rep.dia[1].d * 0.35) as f32 * 2.0;
            let plate = math::basis([1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [pr * S, th * S, pr * S], [0.0, (b + 1.0 + th / 2.0) * S + lift, 0.0]);
            let col = Paint::lit([0.92, 0.92, 0.96, 0.35]);
            let pin_r = (rep.dia[1].df * 0.3) as f32;
            for k in 0..self.inp.n {
                let p = self.planet_pos(k, tc);
                let y0 = e * 1.5 * b;
                let len = b + 1.0 + th + lift / S - y0;
                let m = math::basis([1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [pin_r * S, len * S, pin_r * S], [p[0] * S, (y0 + len / 2.0) * S, p[1] * S]);
                self.r.draw(&ms.pin, &m, col);
            }
            self.r.draw(&ms.plate, &plate, col);
        }
    }

    fn ray_hit(&self, x: f32, y: f32) -> Option<[f32; 2]> {
        let rect = self.canvas.get_bounding_client_rect();
        let nx = 2.0 * (x - rect.left() as f32) / rect.width() as f32 - 1.0;
        let ny = 1.0 - 2.0 * (y - rect.top() as f32) / rect.height() as f32;
        let o = self.cam.cur;
        let aspect = rect.width() as f32 / rect.height().max(1.0) as f32;
        let vp = math::mul(&math::perspective(0.8, aspect, o.dist * 0.02, o.dist * 40.0), &math::look_at(o.eye(), o.target, [0.0, 1.0, 0.0]));
        let (ro, rd) = math::ndc_ray(&math::invert(&vp)?, nx, ny);
        let p = math::ray_plane(ro, rd, [0.0, self.inp.b as f32 * S, 0.0], [0.0, 1.0, 0.0])?;
        Some([p[0] / S, p[2] / S])
    }

    fn pick(&self, x: f32, y: f32) -> Hit {
        let (Some(rep), Some(p)) = (self.rep.as_ref(), self.ray_hit(x, y)) else { return Hit::None };
        if self.explode > 0.05 {
            return Hit::None;
        }
        let (_, tc, _, _) = self.angles();
        let ra = rep.dia[1].da as f32 / 2.0;
        for k in 0..self.inp.n {
            let c = self.planet_pos(k, tc);
            if (p[0] - c[0]).hypot(p[1] - c[1]) < ra {
                return Hit::Planet(k);
            }
        }
        let r = p[0].hypot(p[1]);
        if r > rep.dia[2].da as f32 / 2.0 && r < rep.outer as f32 / 2.0 + rep.dia[2].d as f32 * 0.03 {
            return Hit::Ring;
        }
        Hit::None
    }

    pub fn pointer_down(&mut self, e: &PointerEvent) {
        let _ = self.canvas.set_pointer_capture(e.pointer_id());
        let (x, y) = (e.client_x() as f32, e.client_y() as f32);
        self.ptrs.push((e.pointer_id(), x, y));
        self.cam.held = true;
        self.drag = Some(if self.ptrs.len() > 1 || e.button() == 2 {
            Drag::Pan
        } else {
            match (self.pick(x, y), self.ray_hit(x, y)) {
                (Hit::Planet(k), Some(p)) => {
                    let (_, tc, _, _) = self.angles();
                    let c = self.planet_pos(k, tc);
                    let l = c[0].hypot(c[1]).max(1e-3);
                    let dir = [c[0] / l, c[1] / l];
                    Drag::Planet { dir, start: p[0] * dir[0] + p[1] * dir[1], zs: self.inp.zs, zp: self.inp.zp }
                }
                (Hit::Ring, Some(p)) => Drag::Ring { start: p[0].hypot(p[1]), m: self.inp.m },
                _ => Drag::Orbit,
            }
        });
    }

    pub fn pointer_up(&mut self, e: &PointerEvent) {
        self.ptrs.retain(|p| p.0 != e.pointer_id());
        if self.ptrs.is_empty() {
            self.drag = None;
            self.cam.held = false;
        }
    }

    pub fn pointer_move(&mut self, e: &PointerEvent) -> Option<Edit> {
        let (x, y) = (e.client_x() as f32, e.client_y() as f32);
        let Some(k) = self.ptrs.iter().position(|p| p.0 == e.pointer_id()) else {
            self.hover = self.pick(x, y);
            let cur = if self.hover == Hit::None { "default" } else { "grab" };
            let _ = self.canvas.style().set_property("cursor", cur);
            return None;
        };
        let old = self.ptrs.clone();
        self.ptrs[k] = (e.pointer_id(), x, y);
        let (dx, dy) = (x - old[k].1, y - old[k].2);
        let _ = self.canvas.style().set_property("cursor", if matches!(self.drag, Some(Drag::Orbit | Drag::Pan)) { "move" } else { "grabbing" });
        match self.drag.as_ref()? {
            Drag::Orbit => self.cam.rotate(-dx * 0.008, dy * 0.008),
            Drag::Pan => {
                if self.ptrs.len() == 2 {
                    let d = |p: &[(i32, f32, f32)]| (p[0].1 - p[1].1).hypot(p[0].2 - p[1].2).max(1.0);
                    self.cam.zoom(d(&old) / d(&self.ptrs));
                }
                let n = self.ptrs.len() as f32;
                let o = self.cam.cur;
                let f = math::norm(math::sub(o.target, o.eye()));
                let right = math::norm(math::cross(f, [0.0, 1.0, 0.0]));
                let up = math::cross(right, f);
                let s = o.dist * 0.0015 / n;
                self.cam.pan(math::add(math::scale(right, -dx * s), math::scale(up, dy * s)));
            }
            &Drag::Planet { dir, start, zs, zp } => {
                let p = self.ray_hit(x, y)?;
                let steps = ((p[0] * dir[0] + p[1] * dir[1] - start) / self.inp.m as f32).round() as i32;
                let zp_new = (zp as i32 + steps).clamp(8, 150) as u32;
                // Keep the ring if the sun can shrink / grow to fit, otherwise the ring follows.
                let zr = core::ring_teeth(zs, zp) as i32;
                let zs_new = zr - 2 * zp_new as i32;
                let zs_new = if zs_new >= 8 { zs_new as u32 } else { zs };
                if (zs_new, zp_new) != (self.inp.zs, self.inp.zp) {
                    return Some(Edit::Teeth(zs_new, zp_new));
                }
            }
            &Drag::Ring { start, m } => {
                let p = self.ray_hit(x, y)?;
                let m_new = ((m * (p[0].hypot(p[1]) / start.max(1e-3)) as f64) * 20.0).round() / 20.0;
                let m_new = m_new.clamp(0.3, 3.0);
                if (m_new - self.inp.m).abs() > 1e-9 {
                    return Some(Edit::Module(m_new));
                }
            }
        }
        None
    }

    pub fn wheel(&mut self, e: &WheelEvent) {
        self.cam.zoom((e.delta_y() as f32 * 0.0015).exp());
    }
}
