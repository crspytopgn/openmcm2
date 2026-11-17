//! The builder's buttons, sliders and tooltips: drawn each frame from the
//! builder's state (immediate mode), on a screen 480 high and as wide as
//! the window's shape, which the sprites stretch over the 640×480 they
//! know.

use crate::ui::{Gfx, Ui};

/// How long the pointer rests on a control before its tip shows, s.
const TIP_DELAY: f32 = 0.45;

#[derive(Default)]
pub struct Input {
    pub pointer: (f32, f32),
    /// Where the left button went down over a panel, this frame.
    pub press: Option<(f32, f32)>,
    pub held: bool,
    pub released: bool,
    pub wheel: f32,
    /// How tall the tool's options were last frame.
    pub panel_height: f32,
    /// The panels drawn this frame and last.
    panels: Vec<[f32; 4]>,
    last_panels: Vec<[f32; 4]>,
    /// The control the button went down on.
    active: Option<String>,
    /// The control under the pointer, its tip and since when.
    hot: Option<(String, String)>,
    tip_for: Option<(String, f32)>,
    now: f32,
}

fn inside(r: [f32; 4], p: (f32, f32)) -> bool {
    p.0 >= r[0] && p.0 < r[0] + r[2] && p.1 >= r[1] && p.1 < r[1] + r[3]
}

impl Input {
    pub fn begin(&mut self) {
        self.last_panels = std::mem::take(&mut self.panels);
        self.hot = None;
    }

    /// Whether `p` is over a panel (as last drawn).
    /// The panels drawn last frame.
    pub fn panel_rects(&self) -> Vec<[f32; 4]> {
        self.last_panels.clone()
    }

    pub fn over(&self, p: (f32, f32)) -> bool {
        self.last_panels.iter().chain(&self.panels).any(|r| inside(*r, p))
    }

    /// Draws the tip of the control the pointer rests on, and forgets
    /// this frame's clicks.
    pub fn end(&mut self, p: &mut Painter, now: f32) {
        self.now = now;
        match (&self.hot, &self.tip_for) {
            (Some((id, _)), Some((tid, _))) if id == tid => {}
            (Some((id, _)), _) => self.tip_for = Some((id.clone(), now)),
            (None, _) => self.tip_for = None,
        }
        if let (Some((_, tip)), Some((_, since))) = (&self.hot, &self.tip_for) {
            if now - since >= TIP_DELAY && !tip.is_empty() && !self.held {
                p.tip(self.pointer, tip);
            }
        }
        self.press = None;
        if self.released {
            self.active = None;
        }
        self.released = false;
        self.wheel = 0.0;
    }

    fn hover(&mut self, id: &str, r: [f32; 4], tip: &str) -> bool {
        let over = inside(r, self.pointer) && (self.active.is_none() || self.active.as_deref() == Some(id));
        if over {
            self.hot = Some((id.to_string(), tip.to_string()));
        }
        over
    }
}

pub struct Painter<'a, 'b> {
    ui: &'a mut Ui,
    gfx: &'a mut Gfx<'b>,
    /// 640-wide pixels per builder pixel, across.
    kx: f32,
    pub width: f32,
    /// Panels that marks on the land are kept out from under.
    pub hide: Vec<[f32; 4]>,
}

impl<'a, 'b> Painter<'a, 'b> {
    pub fn new(ui: &'a mut Ui, gfx: &'a mut Gfx<'b>, width: f32) -> Self {
        Painter { ui, gfx, kx: 640.0 / width, width, hide: Vec::new() }
    }

    fn to640(&self, r: [f32; 4]) -> [f32; 4] {
        [r[0] * self.kx, r[1], r[2] * self.kx, r[3]]
    }

    pub fn fill(&mut self, r: [f32; 4], color: [f32; 4]) {
        if !self.hide.is_empty() && self.hidden([r[0] + r[2] / 2.0, r[1] + r[3] / 2.0]) {
            return;
        }
        let r = self.to640(r);
        self.ui.fill(self.gfx, r, color);
    }

    fn hidden(&self, p: [f32; 2]) -> bool {
        self.hide.iter().any(|r| inside(*r, (p[0], p[1])))
    }

    pub fn line(&mut self, a: [f32; 2], b: [f32; 2], width: f32, color: [f32; 4]) {
        if self.hidden(a) || self.hidden(b) {
            return;
        }
        // Keep the width even across a squeezed screen.
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let len = (dx * dx + dy * dy).sqrt().max(1e-3);
        let w = width * ((dy / len).abs() * self.kx + (dx / len).abs());
        self.ui.line(self.gfx, [a[0] * self.kx, a[1]], [b[0] * self.kx, b[1]], w, color);
    }

    /// Text with its top left at `at`; `size` in points.
    pub fn text(&mut self, at: [f32; 2], text: &str, size: f32, bold: bool, color: [f32; 4]) {
        if let Some(t) = self.ui.text(self.gfx, text, size, bold) {
            let (w, h) = self.gfx.sprites.size(t);
            let r = self.to640([at[0], at[1], w as f32 / self.kx_text(), h as f32]);
            self.gfx.sprites.quad(t, [0.0, 0.0, 1.0, 1.0], r, color);
        }
    }

    /// Text textures are made for 640-wide pixels as tall as the window's
    /// 480: a builder pixel is as wide as it is tall, so one builder pixel
    /// of text is one texture pixel.
    fn kx_text(&self) -> f32 {
        1.0
    }

    pub fn text_width(&mut self, text: &str, size: f32, bold: bool) -> f32 {
        self.ui.text_width(self.gfx, text, size, bold) / self.kx_text()
    }

    /// A short label centred over `at`, on a dark backing.
    pub fn label(&mut self, at: [f32; 2], text: &str, color: [f32; 4]) {
        let w = self.text_width(text, 10.5, true);
        if at[0] < -w || at[0] > self.width + w || at[1] < 0.0 || at[1] > 480.0 || self.hidden(at) {
            return;
        }
        self.fill([at[0] - w / 2.0 - 4.0, at[1] - 7.0, w + 8.0, 14.0], [0.0, 0.0, 0.0, 0.55]);
        self.text([at[0] - w / 2.0, at[1] - 6.0], text, 10.5, true, color);
    }

    /// A panel's backing; the pointer over it works the panel, not the
    /// land.
    pub fn panel(&mut self, input: &mut Input, r: [f32; 4]) {
        input.panels.push(r);
        self.fill(r, super::PANEL);
    }

    /// A button; true when clicked. `on` shows it chosen.
    pub fn button(&mut self, input: &mut Input, id: &str, r: [f32; 4], label: &str, tip: &str, on: bool) -> bool {
        let over = input.hover(id, r, tip);
        if over && input.press.is_some_and(|p| inside(r, p)) {
            input.active = Some(id.to_string());
        }
        let down = over && input.held && input.active.as_deref() == Some(id);
        let a = super::ACCENT;
        let bg = if on {
            [a[0] * 0.55, a[1] * 0.55, a[2] * 0.55, 0.95]
        } else if down {
            [0.30, 0.30, 0.27, 0.95]
        } else if over {
            [0.22, 0.22, 0.20, 0.95]
        } else {
            [0.14, 0.14, 0.13, 0.95]
        };
        self.fill(r, bg);
        if on {
            self.fill([r[0], r[1] + r[3] - 2.0, r[2], 2.0], [a[0], a[1], a[2], 1.0]);
        }
        let size = if r[3] < 19.0 { 9.5 } else { 10.5 };
        let mut text = label.to_string();
        while text.chars().count() > 3 && self.text_width(&text, size, on) > r[2] - 8.0 {
            text.pop();
            text = text.trim_end_matches('.').to_string();
            text.pop();
            text.push_str("..");
        }
        let w = self.text_width(&text, size, on);
        let color = if on { [1.0, 1.0, 1.0, 1.0] } else { [0.88, 0.88, 0.84, 1.0] };
        self.text([r[0] + (r[2] - w) / 2.0, r[1] + (r[3] - 12.0) / 2.0 + 0.5], &text, size, on, color);
        over && input.released && input.active.as_deref() == Some(id)
    }

    /// A slider with its label and value above the bar. Gives the new
    /// value while it is dragged (or the wheel turns over it), and whether
    /// a drag began this frame.
    #[allow(clippy::too_many_arguments)]
    pub fn slider(
        &mut self,
        input: &mut Input,
        id: &str,
        r: [f32; 4],
        label: &str,
        tip: &str,
        value: f32,
        (lo, hi): (f32, f32),
        shown: &str,
    ) -> (Option<f32>, bool) {
        let over = input.hover(id, r, tip);
        let mut started = false;
        if over && input.press.is_some_and(|p| inside(r, p)) {
            input.active = Some(id.to_string());
            started = true;
        }
        let bar = [r[0], r[1] + 16.0, r[2], 6.0];
        let t = ((value - lo) / (hi - lo)).clamp(0.0, 1.0);
        let mut out = None;
        if input.held && input.active.as_deref() == Some(id) {
            let nt = ((input.pointer.0 - bar[0]) / bar[2]).clamp(0.0, 1.0);
            if (nt - t).abs() > 1e-4 {
                out = Some(lo + (hi - lo) * nt);
            }
        } else if over && input.wheel != 0.0 {
            started = true;
            out = Some((value + input.wheel * (hi - lo) * 0.03).clamp(lo, hi));
        }
        let a = super::ACCENT;
        self.text([r[0], r[1]], label, 10.5, false, [0.88, 0.88, 0.84, 1.0]);
        let w = self.text_width(shown, 10.5, false);
        self.text([r[0] + r[2] - w, r[1]], shown, 10.5, false, [1.0, 1.0, 1.0, 1.0]);
        self.fill(bar, [0.18, 0.18, 0.16, 1.0]);
        let t = out.map_or(t, |v| ((v - lo) / (hi - lo)).clamp(0.0, 1.0));
        self.fill([bar[0], bar[1], bar[2] * t, bar[3]], [a[0] * 0.8, a[1] * 0.8, a[2] * 0.8, 1.0]);
        let knob = if over || input.active.as_deref() == Some(id) { 6.0 } else { 5.0 };
        self.fill([bar[0] + bar[2] * t - knob / 2.0, bar[1] - 2.0, knob, bar[3] + 4.0], [1.0, 1.0, 1.0, 1.0]);
        (out, started)
    }

    /// A tooltip by the pointer, kept on the screen.
    pub fn tip(&mut self, at: (f32, f32), text: &str) {
        let lines = wrap(self, text, 220.0, 10.0);
        let w = lines.iter().map(|l| self.text_width(l, 10.0, false)).fold(0.0, f32::max) + 12.0;
        let h = lines.len() as f32 * 12.0 + 8.0;
        let mut x = at.0 + 14.0;
        let mut y = at.1 + 18.0;
        if x + w > self.width - 4.0 {
            x = (at.0 - w - 6.0).max(4.0);
        }
        if y + h > 476.0 {
            y = at.1 - h - 6.0;
        }
        self.fill([x - 1.0, y - 1.0, w + 2.0, h + 2.0], [0.55, 0.55, 0.5, 0.9]);
        self.fill([x, y, w, h], [0.98, 0.97, 0.9, 0.97]);
        for (i, l) in lines.iter().enumerate() {
            self.text([x + 6.0, y + 4.0 + i as f32 * 12.0], l, 10.0, false, [0.08, 0.08, 0.08, 1.0]);
        }
    }
}

/// `text` broken into lines no wider than `width`.
pub fn wrap(p: &mut Painter, text: &str, width: f32, size: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
        if !line.is_empty() && p.text_width(&next, size, false) > width {
            lines.push(std::mem::replace(&mut line, word.to_string()));
        } else {
            line = next;
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}
