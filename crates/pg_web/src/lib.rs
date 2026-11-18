//! planet-gear web UI: form ↔ pg_core model, results table, HUD, design search, SF-vs-module
//! plot (canvas 2D) and the WebGL2 view. Everything runs locally in the browser.
pub mod gl;
pub mod math;
mod view3d;

use pg_core::{self as core, Fixed, Input, Report, MATERIALS};
use std::{cell::RefCell, fmt::Write, rc::Rc};
use wasm_bindgen::{prelude::*, JsCast};
use web_sys::{CanvasRenderingContext2d, Document, Element, Event, HtmlCanvasElement, HtmlElement, HtmlInputElement, HtmlSelectElement, PointerEvent, WheelEvent};

struct App {
    inp: Input,
    rep: Report,
    ko: bool,
    view: Option<view3d::View>,
    found: Vec<core::Design>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.borrow_mut().as_mut().map(f))
}

fn doc() -> Document {
    web_sys::window().expect("window").document().expect("document")
}
fn el<T: JsCast>(id: &str) -> T {
    doc().get_element_by_id(id).expect(id).dyn_into().expect(id)
}
fn on(t: &web_sys::EventTarget, ev: &str, f: impl FnMut(Event) + 'static) {
    let c = Closure::<dyn FnMut(Event)>::new(f);
    let _ = t.add_event_listener_with_callback(ev, c.as_ref().unchecked_ref());
    c.forget();
}
fn num(id: &str, lo: f64, hi: f64, def: f64) -> f64 {
    let v = el::<HtmlInputElement>(id).value_as_number();
    if v.is_finite() { v.clamp(lo, hi) } else { def }
}
fn sel(id: &str) -> usize {
    el::<HtmlSelectElement>(id).selected_index().max(0) as usize
}
fn set_val(id: &str, v: impl ToString) {
    el::<HtmlInputElement>(id).set_value(&v.to_string());
}
fn tr(ko: bool, k: &'static str, e: &'static str) -> &'static str {
    if ko { k } else { e }
}

fn read() -> Input {
    Input {
        zs: num("zs", 6.0, 150.0, 12.0) as u32,
        zp: num("zp", 6.0, 150.0, 24.0) as u32,
        n: num("n", 2.0, 6.0, 3.0) as u32,
        m: num("m", 0.3, 3.0, 1.0),
        b: num("b", 1.0, 100.0, 8.0),
        alpha_deg: [20.0, 14.5, 25.0][sel("alpha").min(2)],
        torque: num("torque", 0.0, 1e4, 0.3),
        rpm: num("rpm", -1e5, 1e5, 3000.0),
        mat: [sel("mat0"), sel("mat1"), sel("mat2")],
        knock: num("knock", 0.1, 1.0, 0.6),
        fixed: [Fixed::Ring, Fixed::Carrier, Fixed::Sun][sel("fixed").min(2)],
        share: num("share", 1.0, 2.0, 1.15),
        eta: num("eta", 0.5, 1.0, 0.98),
    }
}

/// Status text and CSS class: invalid (with reason) > fails > OK.
fn status(r: &Report, ko: bool) -> (String, &'static str) {
    let mut why = vec![];
    if !r.assembly {
        why.push(tr(ko, "(Zs+Zr)/N 비정수 → 등간격 조립 불가", "(Zs+Zr)/N not an integer → no equal spacing"));
    }
    if !r.neighbour {
        why.push(tr(ko, "유성기어 이끝 간섭", "planet tips collide"));
    }
    if !why.is_empty() {
        return (format!("{}: {}", tr(ko, "무효", "invalid"), why.join(", ")), "bad");
    }
    if r.fails() {
        return (tr(ko, "실패: 안전계수 < 1", "fails: safety factor < 1").into(), "bad");
    }
    ("OK".into(), "ok")
}

fn sf_cell(sf: f64) -> String {
    let c = if sf < 1.0 { "bad" } else if sf < 1.5 { "warn" } else { "ok" };
    format!("<td class={c}>{sf:.2}</td>")
}

fn render(a: &mut App) {
    let (r, i, ko) = (&a.rep, &a.inp, a.ko);
    let mut h = String::new();
    let g = [tr(ko, "선", "sun"), tr(ko, "유성", "planet"), tr(ko, "링", "ring")];
    let _ = write!(h, "<table><tr><th></th><th>{}</th><th>{}</th><th>{}</th></tr>", g[0], g[1], g[2]);
    let _ = write!(h, "<tr><td>Z</td><td>{}</td><td>{}</td><td>{}</td></tr>", i.zs, i.zp, r.zr);
    h += &format!("<tr><td>d / da / df (mm)</td>");
    for d in &r.dia {
        let _ = write!(h, "<td>{:.1} / {:.1} / {:.1}</td>", d.d, d.da, d.df);
    }
    let _ = write!(h, "</tr><tr><td>{} (MPa)</td>", tr(ko, "굽힘응력 σb", "bending σb"));
    for s in &r.sig_b {
        let _ = write!(h, "<td>{s:.1}</td>");
    }
    let _ = write!(h, "</tr><tr><td>{}</td>{}{}{}</tr>", tr(ko, "굽힘 SF", "bending SF"), sf_cell(r.sf_b[0]), sf_cell(r.sf_b[1]), sf_cell(r.sf_b[2]));
    let _ = write!(h, "<tr><td>{}</td>{}{}{}</tr></table><table>", tr(ko, "접촉 SF", "contact SF"), sf_cell(r.sf_h[0]), sf_cell(r.sf_h[1]), sf_cell(r.sf_h[2]));
    let ok = |b: bool| if b { "✓" } else { "✗" };
    let vn: Vec<String> = r.valid_n.iter().map(|n| n.to_string()).collect();
    let rows = [
        (tr(ko, "감속비", "ratio"), format!("{:.3} : 1", r.ratio)),
        (tr(ko, "조립 조건 / 이웃 조건", "assembly / neighbour"), format!("{} / {}", ok(r.assembly), ok(r.neighbour))),
        (tr(ko, "가능한 N (2–6)", "valid N (2–6)"), if vn.is_empty() { "–".into() } else { vn.join(", ") }),
        (tr(ko, "중심거리", "centre distance"), format!("{:.2} mm", r.centre)),
        (tr(ko, "링 외경 (추정)", "ring outer dia (est.)"), format!("{:.1} mm", r.outer)),
        (tr(ko, "물림률 선-유성 / 유성-링", "contact ratio sun-planet / planet-ring"), format!("{:.2} / {:.2}", r.eps[0], r.eps[1])),
        (tr(ko, "유성기어당 접선력", "tangential force per planet"), format!("{:.1} N", r.ft)),
        (tr(ko, "접촉응력 선-유성 / 유성-링", "contact stress sun-planet / planet-ring"), format!("{:.0} / {:.0} MPa", r.sig_h[0], r.sig_h[1])),
        (tr(ko, "출력 토크 (이상 / 효율)", "output torque (ideal / with η)"), format!("{:.2} / {:.2} N·m", r.t_out, r.t_out_eff)),
        (tr(ko, "출력 속도", "output speed"), format!("{:.0} rpm", r.n_out)),
        (tr(ko, "유성기어 자전 속도", "planet spin speed"), format!("{:.0} rpm", r.planet_rpm)),
        (tr(ko, "맞물림 주파수", "mesh frequency"), format!("{:.0} Hz", r.mesh_hz)),
        (tr(ko, "기어 질량 (근사)", "gear mass (approx.)"), format!("{:.1} g", r.mass_g)),
    ];
    for (k, v) in rows {
        let _ = write!(h, "<tr><td>{k}</td><td>{v}</td></tr>");
    }
    h += "</table>";
    if r.undercut {
        let _ = write!(h, "<p class=warn>{} ({} {})</p>", tr(ko, "언더컷 주의: 잇수가 너무 적음 (전위 필요)", "undercut warning: too few teeth (profile shift needed)"), tr(ko, "최소", "min"), core::min_teeth(i.alpha_deg.to_radians()));
    }
    let (st, cls) = status(r, ko);
    let _ = write!(h, "<p class={cls}><b>{st}</b></p>");
    el::<Element>("out").set_inner_html(&h);

    let (b, c) = r.min_sf();
    let hud = format!(
        "{} {:.2}:1\n{} {:.2} N·m\n{} {:.0} rpm\nSF min b {:.2} / c {:.2}\nOD {:.1} mm\n{} {:.0} g\n{}",
        tr(ko, "감속비", "ratio"), r.ratio, tr(ko, "출력 토크", "T out"), r.t_out_eff, tr(ko, "출력", "n out"), r.n_out,
        b, c, r.outer, tr(ko, "질량", "mass"), r.mass_g, st
    );
    el::<HtmlElement>("hud").set_text_content(Some(&hud));
    plot(i);
    render_found(a);
}

fn render_found(a: &App) {
    let ko = a.ko;
    let mut h = format!("<tr><th>Zs</th><th>Zp</th><th>N</th><th>Zr</th><th>{}</th><th>{}</th><th>OD</th></tr>", tr(ko, "감속비", "ratio"), tr(ko, "오차", "error"));
    for (k, d) in a.found.iter().enumerate() {
        let zr = core::ring_teeth(d.zs, d.zp);
        let _ = write!(h, "<tr data-i={k}><td>{}</td><td>{}</td><td>{}</td><td>{zr}</td><td>{:.3}</td><td>{:.3}</td><td>{:.1}</td></tr>", d.zs, d.zp, d.n, d.ratio, d.err, d.outer);
    }
    if a.found.is_empty() {
        let _ = write!(h, "<tr><td colspan=7>{}</td></tr>", tr(ko, "결과 없음 — 찾기를 누르거나 조건을 완화하세요", "no results — press Search or relax the limits"));
    }
    el::<Element>("found").set_inner_html(&h);
}

/// Minimum bending / contact SF over module 0.3…3 mm, with the current module marked.
fn plot(i: &Input) {
    let cv: HtmlCanvasElement = el("plot");
    let dpr = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio());
    let (w, h) = (cv.client_width().max(1) as f64, cv.client_height().max(1) as f64);
    cv.set_width((w * dpr) as u32);
    cv.set_height((h * dpr) as u32);
    let Some(Ok(cx)) = cv.get_context("2d").ok().flatten().map(|c| c.dyn_into::<CanvasRenderingContext2d>()) else { return };
    let _ = cx.scale(dpr, dpr);
    let (l, rgt, t, bot) = (34.0, w - 8.0, 8.0, h - 22.0);
    let ms: Vec<f64> = (0..=54).map(|k| 0.3 + 0.05 * k as f64).collect();
    let sf: Vec<(f64, f64)> = ms.iter().map(|&m| core::analyze(&Input { m, ..*i }).min_sf()).collect();
    let ymax = 4.0;
    let x = |m: f64| l + (m - 0.3) / 2.7 * (rgt - l);
    let y = |s: f64| bot - (s.min(ymax) / ymax) * (bot - t);
    cx.set_font("11px sans-serif");
    cx.set_fill_style_str("#888");
    cx.set_stroke_style_str("#888");
    cx.set_line_width(1.0);
    cx.stroke_rect(l, t, rgt - l, bot - t);
    for s in [1.0, 2.0, 3.0, 4.0] {
        let _ = cx.fill_text(&format!("{s}"), 18.0, y(s) + 4.0);
    }
    for m in [0.5, 1.0, 1.5, 2.0, 2.5, 3.0] {
        let _ = cx.fill_text(&format!("{m}"), x(m) - 8.0, h - 6.0);
    }
    let _ = cx.fill_text("m (mm)", rgt - 44.0, h - 6.0);
    let line = |col: &str, pick: &dyn Fn(&(f64, f64)) -> f64| {
        cx.set_stroke_style_str(col);
        cx.set_line_width(2.0);
        cx.begin_path();
        for (k, s) in sf.iter().enumerate() {
            let (px, py) = (x(ms[k]), y(pick(s)));
            if k == 0 { cx.move_to(px, py) } else { cx.line_to(px, py) }
        }
        cx.stroke();
    };
    line("#e8912d", &|s| s.0);
    line("#3d8bf0", &|s| s.1);
    cx.set_line_width(1.0);
    let _ = cx.set_line_dash(&js_sys::Array::of2(&4.into(), &4.into()));
    cx.set_stroke_style_str("#e0464e");
    cx.begin_path();
    cx.move_to(l, y(1.0));
    cx.line_to(rgt, y(1.0));
    cx.stroke();
    cx.set_stroke_style_str("#888");
    cx.begin_path();
    cx.move_to(x(i.m), t);
    cx.line_to(x(i.m), bot);
    cx.stroke();
    let _ = cx.set_line_dash(&js_sys::Array::new());
    cx.set_fill_style_str("#e8912d");
    let _ = cx.fill_text("SF bending", l + 6.0, t + 12.0);
    cx.set_fill_style_str("#3d8bf0");
    let _ = cx.fill_text("SF contact", l + 80.0, t + 12.0);
}

fn refresh(a: &mut App) {
    a.inp = read();
    a.rep = core::analyze(&a.inp);
    if let Some(v) = a.view.as_mut() {
        v.set_design(&a.inp, &a.rep);
    }
    render(a);
}

fn apply_lang(a: &mut App) {
    let ko = a.ko;
    let nodes = doc().query_selector_all("[data-ko]").expect("selector");
    for k in 0..nodes.length() {
        let Some(e) = nodes.item(k).and_then(|n| n.dyn_into::<Element>().ok()) else { continue };
        if e.get_attribute("data-en").is_none() {
            let _ = e.set_attribute("data-en", &e.text_content().unwrap_or_default());
        }
        e.set_text_content(e.get_attribute(if ko { "data-ko" } else { "data-en" }).as_deref());
    }
    for id in ["mat0", "mat1", "mat2"] {
        let s: HtmlSelectElement = el(id);
        // First fill: default to POM.
        let keep = if s.selected_index() < 0 { 2 } else { s.selected_index() };
        let opts: String = MATERIALS.iter().map(|m| format!("<option>{}</option>", if ko { m.ko } else { m.en })).collect();
        s.set_inner_html(&opts);
        s.set_selected_index(keep);
    }
    let _ = doc().document_element().map(|d| d.set_attribute("lang", if ko { "ko" } else { "en" }));
    el::<HtmlElement>("lang").set_text_content(Some(if ko { "English" } else { "한국어" }));
}

fn sync_tools(v: &view3d::View) {
    for (id, on) in [("t-sf", v.sf_color), ("t-carrier", v.carrier), ("t-play", v.playing), ("t-explode", v.exploded)] {
        let _ = el::<Element>(id).class_list().toggle_with_force("on", on);
    }
}

fn apply_edit(a: &mut App, e: view3d::Edit) {
    match e {
        view3d::Edit::Teeth(zs, zp) => {
            set_val("zs", zs);
            set_val("zp", zp);
        }
        view3d::Edit::Module(m) => set_val("m", m),
    }
    refresh(a);
}

#[wasm_bindgen(start)]
pub fn start() {
    let win = web_sys::window().expect("window");
    let ko = win.navigator().language().is_some_and(|l| l.starts_with("ko"));
    let dark = win.match_media("(prefers-color-scheme: dark)").ok().flatten().is_some_and(|q| q.matches());
    let canvas: HtmlCanvasElement = el("gl");
    let view = view3d::View::new(canvas.clone(), dark);
    if view.is_none() {
        let _ = canvas.style().set_property("display", "none");
        let _ = el::<HtmlElement>("nogl").style().set_property("display", "block");
        let _ = el::<HtmlElement>("tools").style().set_property("display", "none");
    }
    let inp = Input::default();
    let app = App { rep: core::analyze(&inp), inp, ko, view, found: vec![] };
    APP.with(|a| *a.borrow_mut() = Some(app));
    with(|a| {
        apply_lang(a);
        refresh(a);
    });

    on(&el::<Element>("form"), "input", |_| { with(refresh); });
    on(&el::<Element>("lang"), "click", |_| {
        with(|a| {
            a.ko = !a.ko;
            apply_lang(a);
            render(a);
        });
    });
    on(&el::<Element>("go"), "click", |_| {
        with(|a| {
            a.found = core::search(&a.inp, num("target", -1e3, 1e3, 6.0), num("tol", 0.0, 1e3, 0.3), num("maxod", 1.0, 1e5, 90.0));
            render_found(a);
        });
    });
    on(&el::<Element>("found"), "click", |e| {
        let Some(row) = e.target().and_then(|t| t.dyn_into::<Element>().ok()).and_then(|t| t.closest("tr[data-i]").ok().flatten()) else { return };
        let k: usize = row.get_attribute("data-i").and_then(|s| s.parse().ok()).unwrap_or(0);
        with(|a| {
            if let Some(d) = a.found.get(k).copied() {
                set_val("zs", d.zs);
                set_val("zp", d.zp);
                set_val("n", d.n);
                refresh(a);
            }
        });
    });

    // View toggles, presets and the slow-motion slider.
    for (id, f) in [
        ("t-sf", (|v| v.sf_color = !v.sf_color) as fn(&mut view3d::View)),
        ("t-carrier", |v| v.carrier = !v.carrier),
        ("t-play", |v| v.playing = !v.playing),
        ("t-explode", |v| v.exploded = !v.exploded),
    ] {
        on(&el::<Element>(id), "click", move |_| {
            with(|a| a.view.as_mut().map(|v| { f(v); sync_tools(v); }));
        });
    }
    let presets = doc().query_selector_all("[data-view]").expect("selector");
    for k in 0..presets.length() {
        let Some(b) = presets.item(k).and_then(|n| n.dyn_into::<Element>().ok()) else { continue };
        let name = b.get_attribute("data-view").unwrap_or_default();
        on(&b, "click", move |_| {
            with(|a| a.view.as_mut().map(|v| { v.preset(&name); sync_tools(v); }));
        });
    }
    on(&el::<Element>("slow"), "input", |_| {
        let s = num("slow", 0.0, 1.0, 0.3) as f32;
        with(|a| a.view.as_mut().map(|v| v.slow = s));
    });

    // Canvas pointer / wheel input.
    on(&canvas, "pointerdown", |e| { with(|a| a.view.as_mut().map(|v| v.pointer_down(e.unchecked_ref::<PointerEvent>()))); });
    on(&canvas, "pointermove", |e| {
        with(|a| {
            if let Some(edit) = a.view.as_mut().and_then(|v| v.pointer_move(e.unchecked_ref::<PointerEvent>())) {
                apply_edit(a, edit);
            }
        });
    });
    for ev in ["pointerup", "pointercancel"] {
        on(&canvas, ev, |e| { with(|a| a.view.as_mut().map(|v| v.pointer_up(e.unchecked_ref::<PointerEvent>()))); });
    }
    on(&canvas, "wheel", |e| {
        e.prevent_default();
        with(|a| a.view.as_mut().map(|v| v.wheel(e.unchecked_ref::<WheelEvent>())));
    });
    on(&canvas, "contextmenu", |e| e.prevent_default());
    on(&canvas, "dblclick", |_| { with(|a| a.view.as_mut().map(|v| { v.preset("persp"); sync_tools(v); })); });
    on(&win, "resize", |_| { with(|a| plot(&a.inp)); });

    // Animation loop.
    let tick: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> = Rc::new(RefCell::new(None));
    let again = tick.clone();
    let mut last = 0.0;
    *tick.borrow_mut() = Some(Closure::new(move |t: f64| {
        let dt = if last == 0.0 { 1.0 / 60.0 } else { ((t - last) / 1000.0).min(0.1) as f32 };
        last = t;
        with(|a| a.view.as_mut().map(|v| v.frame(dt)));
        if let Some(c) = again.borrow().as_ref() {
            let _ = web_sys::window().expect("window").request_animation_frame(c.as_ref().unchecked_ref());
        }
    }));
    let first = tick.borrow();
    if let Some(c) = first.as_ref() {
        let _ = win.request_animation_frame(c.as_ref().unchecked_ref());
    }
}
