use super::{blend, Frame, Renderer};
use crate::{
    material::{self as m, Kind, Paint},
    Cell, Point, Rect,
};

const HEADER_BUTTON_SHIFT: f32 = 6.0;

#[derive(Clone, Copy)]
struct BoxPx {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
}

impl BoxPx {
    fn inset(self, amount: f32) -> Self {
        Self { x: self.x + amount, y: self.y + amount, w: (self.w - amount * 2.0).max(0.0), h: (self.h - amount * 2.0).max(0.0) }
    }

    fn contains(self, p: (f32, f32)) -> bool {
        p.0 >= self.x && p.1 >= self.y && p.0 < self.x + self.w && p.1 < self.y + self.h
    }
}

impl Renderer {
    fn header_widget_offset(&self, scene: &[Paint], bounds: Rect) -> i32 {
        scene.iter().rev().find_map(|paint| match paint {
            Paint::Widget { bounds: window, kind: Kind::Window { .. }, .. }
                if window.a.y == bounds.a.y && bounds.a.x >= window.a.x && bounds.b.x <= window.b.x => {
                let band = self.header_band(self.px_box(*window));
                Some((band.y + band.h / 2.0 - (bounds.a.y as f32 + 0.5) * self.cell_h() as f32).round() as i32)
            }
            _ => None,
        }).unwrap_or(0)
    }

    /// Переводит точное попадание в сдвинутую кнопку в её логическую область.
    pub fn header_control_at(&self, frame: Option<&Frame>, point: (f32, f32)) -> Option<Point> {
        let frame = frame.filter(|frame| frame.screen.material)?;
        for (index, paint) in frame.screen.scene.iter().enumerate().rev() {
            let Paint::Widget { bounds, clip, kind, .. } = paint else { continue };
            if !matches!(kind, Kind::Window { .. }) {
                let offset = self.header_widget_offset(&frame.screen.scene[..index], *bounds);
                let logical = (point.0, point.1 - offset as f32);
                if offset != 0 && self.px_box(*clip).contains(logical) {
                    return Some(Point::new((logical.0 / self.cell_w() as f32) as i32, bounds.a.y));
                }
            }
            if !self.px_box(*clip).contains(point) {
                continue;
            }
            let Kind::Window { flags, .. } = kind else { return None };
            let band = self.header_band(self.px_box(*bounds));
            let y = bounds.a.y as f32 * self.cell_h() as f32;
            if point.1 >= (band.y + band.h).max(y + self.cell_h() as f32) {
                return None;
            }
            let buttons = crate::widgets::window::header_buttons(bounds.width(), *flags);
            let shift = HEADER_BUTTON_SHIFT * (self.font.px / 16.0).max(0.25);
            let mut old_button = false;
            for rect in [buttons.close, buttons.zoom, buttons.minimize].into_iter().flatten() {
                let mut pixels = self.px_box(rect.moved(bounds.a.x, bounds.a.y));
                old_button |= pixels.contains(point);
                pixels.x -= shift;
                pixels.y = band.y;
                pixels.h = band.h;
                if pixels.contains(point) {
                    return Some(Point::new(bounds.a.x + (rect.a.x + rect.b.x) / 2, bounds.a.y));
                }
            }
            return old_button.then(|| Point::new(bounds.b.x - 1, bounds.a.y));
        }
        None
    }
    pub fn set_hover(&mut self, hover: Option<(f32, f32)>) {
        self.hover = hover;
    }

    fn px_box(&self, rect: Rect) -> BoxPx {
        BoxPx {
            x: rect.a.x as f32 * self.cell_w() as f32,
            y: rect.a.y as f32 * self.cell_h() as f32,
            w: rect.width().max(0) as f32 * self.cell_w() as f32,
            h: rect.height().max(0) as f32 * self.cell_h() as f32,
        }
    }

    fn header_band(&self, r: BoxPx) -> BoxPx {
        let scale = (self.font.px / 16.0).max(0.25);
        BoxPx {
            x: r.x + 12.0 * scale,
            y: r.y + 4.0 * scale,
            w: (r.w - 24.0 * scale).max(0.0),
            h: (self.cell_h() as f32 + 20.0 * scale).min(r.h - 5.0 * scale).max(0.0),
        }
    }

    pub(super) fn update_material(&mut self, frame: &Frame, cursor_on: bool) {
        self.pixels.fill(m::SURFACE);
        self.material_clip = (0, 0, self.width as i32, self.height as i32);
        for (index, paint) in frame.screen.scene.iter().enumerate() {
            match paint {
                Paint::Cell { pos, cell } => self.material_cell(*pos, *cell, None, false),
                Paint::Widget { bounds, clip, kind, cells } => {
                    self.material_offset_y = if matches!(kind, Kind::Window { .. }) { 0 } else {
                        self.header_widget_offset(&frame.screen.scene[..index], *bounds)
                    };
                    let hover = self.hover;
                    self.hover = hover.map(|(x, y)| (x, y - self.material_offset_y as f32));
                    let c = self.px_box(*clip);
                    self.material_clip = (c.x as i32, c.y as i32, (c.x + c.w) as i32, (c.y + c.h) as i32);
                    self.material_widget(*bounds, *clip, kind, cells);
                    self.hover = hover;
                    self.material_offset_y = 0;
                    self.material_clip = (0, 0, self.width as i32, self.height as i32);
                }
            }
        }
        if let (Some(pos), true) = (frame.cursor, cursor_on) {
            // Переключателям фокус задаёт state layer, текстовым полям — caret.
            let input = frame.screen.scene.iter().rev().find_map(|paint| match paint {
                Paint::Widget { bounds, kind: Kind::Input { .. }, .. } if bounds.contains(pos) => Some(*bounds),
                _ => None,
            });
            let in_choice = frame.screen.scene.iter().any(|paint| {
                matches!(paint,
                Paint::Widget { bounds, kind: Kind::Choices { .. }, .. } if bounds.contains(pos))
            });
            if !in_choice {
                let mut y = pos.y as f32 * self.cell_h() as f32;
                if let Some(bounds) = input {
                    let r = self.px_box(bounds);
                    y = r.y + (r.h - self.cell_h() as f32) / 2.0
                        + self.header_widget_offset(&frame.screen.scene, bounds) as f32;
                }
                let x = pos.x as f32 * self.cell_w() as f32;
                self.round_box(BoxPx { x, y: y + 4.0, w: 2.0, h: (self.cell_h() as f32 - 8.0).max(2.0) }, 1.0, m::PRIMARY, 255);
            }
        }
    }

    fn material_widget(&mut self, bounds: Rect, clip: Rect, kind: &Kind, cells: &[Cell]) {
        let r = self.px_box(bounds);
        let scale = (self.font.px / 16.0).max(0.25);
        let hover = self.hover.map(|p| r.contains(p)).unwrap_or(false);
        match kind {
            Kind::Background => self.round_box(r, 0.0, m::SURFACE, 255),
            Kind::Scrim => self.round_box(r, 0.0, 0x000000, 82),
            Kind::Window { title, active, flags, title_end, zoomed, minimized } => {
                let radius = (28.0 * scale).min(r.h / 4.0).min(r.w / 4.0);
                self.round_box(r, radius, m::OUTLINE_VARIANT, 255);
                self.round_box(r.inset(scale), radius, m::CONTAINER, 255);
                let band = self.header_band(r);
                self.round_box(band, 16.0 * scale, m::CONTAINER_HIGH, 255);
                self.text_size(
                    title,
                    BoxPx {
                        x: r.x + self.cell_w() as f32 * 2.0,
                        y: band.y,
                        w: ((*title_end - 2).max(0) as f32 * self.cell_w() as f32).min(r.w),
                        h: band.h,
                    },
                    if *active { m::ON_SURFACE } else { m::ON_SURFACE_VARIANT },
                    false,
                    self.font.px * 1.1,
                );
                let buttons = crate::widgets::window::header_buttons(bounds.width(), *flags);
                for (rect, icon) in [
                    (buttons.close, "close"),
                    (buttons.zoom, if *zoomed { "restore" } else { "expand" }),
                    (buttons.minimize, if *minimized { "restore" } else { "minimize" }),
                ] {
                    if let Some(rect) = rect {
                        let mut icon_rect = self.px_box(rect.moved(bounds.a.x, bounds.a.y));
                        icon_rect.x -= HEADER_BUTTON_SHIFT * scale;
                        icon_rect.y = band.y;
                        icon_rect.h = band.h;
                        self.icon_button(icon_rect, icon, hover);
                    }
                }
                if !minimized && flags & crate::wf::GROW != 0 {
                    for i in 0..3 {
                        let d = (4 + i * 4) as f32 * scale;
                        self.line((r.x + r.w - d - 4.0, r.y + r.h - 5.0), (r.x + r.w - 5.0, r.y + r.h - d - 4.0), scale, m::OUTLINE_VARIANT);
                    }
                }
            }
            Kind::Button { label, primary, pressed, enabled, focused } => {
                let height = (40.0 * scale).min(r.h - 4.0 * scale).max(0.0);
                let shape = BoxPx { x: r.x + 2.0 * scale, y: r.y + (r.h - height) / 2.0, w: (r.w - 4.0 * scale).max(0.0), h: height };
                let fill = if !enabled {
                    m::CONTAINER_HIGH
                } else if *primary {
                    m::PRIMARY
                } else {
                    m::PRIMARY_CONTAINER
                };
                let ink = if !enabled {
                    m::OUTLINE
                } else if *primary {
                    m::ON_PRIMARY
                } else {
                    m::PRIMARY
                };
                self.round_box(shape, shape.h / 2.0, fill, 255);
                if *enabled && (hover || *pressed || *focused) {
                    self.round_box(shape, shape.h / 2.0, ink, if *pressed { 31 } else { 20 });
                }
                let label_bounds = if self.material_offset_y != 0 {
                    BoxPx { x: shape.x + 8.0 * scale, w: (shape.w - 16.0 * scale).max(0.0), ..shape }
                } else {
                    shape.inset(8.0 * scale)
                };
                self.text_size(label, label_bounds, ink, true, self.font.px * 0.875);
            }
            Kind::Input { focused, enabled } => {
                self.round_box(r.inset(scale), 4.0 * scale, m::CONTAINER_HIGH, 255);
                let ink = if *focused && *enabled { m::PRIMARY } else { m::OUTLINE };
                let stroke = if *focused { 2.0 } else { 1.0 } * scale;
                self.round_box(BoxPx { x: r.x, y: r.y + r.h - stroke, w: r.w, h: stroke }, 0.0, ink, 255);
                // Ширина символов совпадает с caret, выделением и hit testing редактора.
                let top = r.y + (r.h - self.cell_h() as f32) / 2.0;
                for x in clip.a.x..clip.b.x {
                    let index = (x - clip.a.x) as usize;
                    if let Some(cell) = cells.get(index) {
                        self.material_glyph_at(x as f32 * self.cell_w() as f32, top, *cell, cells.first().map(|c| crate::color::bg256(c.attr)), true);
                    }
                }
            }
            Kind::Choices { items, radio } => {
                for item in items {
                    let x = r.x + (item.pos.x as f32 + 1.0) * self.cell_w() as f32;
                    let y = r.y + item.pos.y as f32 * self.cell_h() as f32;
                    let side = (18.0 * scale).min(self.cell_h() as f32 - 4.0);
                    let icon = BoxPx { x, y: y + (self.cell_h() as f32 - side) / 2.0, w: side, h: side };
                    let ink = if !item.enabled {
                        m::OUTLINE_VARIANT
                    } else if item.checked || item.focused {
                        m::PRIMARY
                    } else {
                        m::ON_SURFACE_VARIANT
                    };
                    if item.focused || (item.enabled && self.hover.map(|p| icon.inset(-8.0 * scale).contains(p)).unwrap_or(false)) {
                        self.round_box(icon.inset(-8.0 * scale), 24.0 * scale, m::PRIMARY, 20);
                    }
                    let radius = if *radio { side / 2.0 } else { 2.0 * scale };
                    self.round_box(icon, radius, ink, 255);
                    if *radio || (!item.checked && !item.mixed) {
                        self.round_box(icon.inset(2.0 * scale), radius, m::CONTAINER, 255);
                    }
                    if *radio && item.checked {
                        self.round_box(icon.inset(5.0 * scale), radius, ink, 255);
                    } else if item.mixed {
                        self.line((x + side * 0.25, icon.y + side / 2.0), (x + side * 0.75, icon.y + side / 2.0), 2.0 * scale, m::ON_PRIMARY);
                    } else if item.checked && !radio {
                        self.line((x + side * 0.2, icon.y + side * 0.5), (x + side * 0.42, icon.y + side * 0.72), 2.0 * scale, m::ON_PRIMARY);
                        self.line((x + side * 0.42, icon.y + side * 0.72), (x + side * 0.8, icon.y + side * 0.28), 2.0 * scale, m::ON_PRIMARY);
                    }
                    self.text(
                        &item.label,
                        BoxPx {
                            x: r.x + (item.pos.x as f32 + 5.0) * self.cell_w() as f32,
                            y,
                            w: (r.w - (item.pos.x as f32 + 5.0) * self.cell_w() as f32).max(0.0),
                            h: self.cell_h() as f32,
                        },
                        if item.enabled { m::ON_SURFACE } else { m::OUTLINE },
                        false,
                    );
                }
            }
            Kind::Progress { percent, label, indeterminate } => {
                let track = BoxPx { x: r.x + 2.0 * scale, y: r.y + r.h / 2.0 - 2.0 * scale, w: (r.w - 4.0 * scale).max(0.0), h: 4.0 * scale };
                self.round_box(track, 2.0 * scale, m::PRIMARY_CONTAINER, 255);
                let fraction = if *indeterminate { 0.35 } else { *percent as f32 / 100.0 };
                let offset = if *indeterminate { (track.w * (1.0 - fraction)) * (*percent as f32 / 100.0) } else { 0.0 };
                self.round_box(BoxPx { x: track.x + offset, w: track.w * fraction, ..track }, 2.0 * scale, m::PRIMARY, 255);
                if r.h >= self.cell_h() as f32 * 2.0 {
                    self.text(label, BoxPx { y: r.y, h: self.cell_h() as f32, ..r }, m::ON_SURFACE_VARIANT, true);
                }
            }
            Kind::Scroll { value, min, max, page } => {
                let vertical = bounds.width() == 1;
                let length = if vertical { r.h } else { r.w };
                let unit = if vertical { self.cell_h() as f32 } else { self.cell_w() as f32 };
                let (offset, thumb) = m::scroll_thumb(length / unit, *value, *min, *max, *page);
                let (offset, thumb) = (offset * unit, thumb * unit);
                let bar = if vertical {
                    BoxPx { x: r.x + r.w / 2.0 - 2.0 * scale, y: r.y + offset, w: 4.0 * scale, h: thumb }
                } else {
                    BoxPx { x: r.x + offset, y: r.y + r.h / 2.0 - 2.0 * scale, w: thumb, h: 4.0 * scale }
                };
                self.round_box(bar, 2.0 * scale, if hover { m::PRIMARY } else { m::OUTLINE }, 255);
            }
            Kind::Text => {
                let width = clip.width().max(0) as usize;
                if width > 0 {
                    for (row, cells) in cells.chunks(width).enumerate() {
                        let start = cells.iter().position(|c| c.ch != ' ' && c.ch != '\0').unwrap_or(cells.len());
                        let line: String = cells[start..].iter().filter(|c| !c.is_wide_tail()).map(|c| c.ch).collect();
                        self.text(
                            line.trim_end(),
                            BoxPx {
                                x: (clip.a.x as f32 + start as f32) * self.cell_w() as f32,
                                y: (clip.a.y as f32 + row as f32) * self.cell_h() as f32,
                                w: (cells.len() - start) as f32 * self.cell_w() as f32,
                                h: self.cell_h() as f32,
                            },
                            m::ON_SURFACE,
                            false,
                        );
                    }
                }
            }
            Kind::List { entries, column_width } => {
                for (pos, label, selected) in entries {
                    let row = BoxPx {
                        x: r.x + pos.x as f32 * self.cell_w() as f32,
                        y: r.y + pos.y as f32 * self.cell_h() as f32,
                        w: (*column_width as f32 * self.cell_w() as f32).min(r.w),
                        h: self.cell_h() as f32,
                    };
                    if *selected {
                        self.round_box(row.inset(scale), 8.0 * scale, m::PRIMARY_CONTAINER, 255);
                    } else if self.hover.map(|p| row.contains(p)).unwrap_or(false) {
                        self.round_box(row.inset(scale), 8.0 * scale, m::ON_SURFACE, 20);
                    }
                    self.text(
                        label,
                        BoxPx { x: row.x + self.cell_w() as f32, w: (row.w - self.cell_w() as f32 * 2.0).max(0.0), ..row },
                        if *selected { m::PRIMARY } else { m::ON_SURFACE },
                        false,
                    );
                }
            }
            Kind::Menu | Kind::Bar | Kind::Content => {
                if matches!(kind, Kind::Menu | Kind::Bar) {
                    self.round_box(r, if matches!(kind, Kind::Menu) { 8.0 * scale } else { 0.0 }, m::CONTAINER_HIGH, 255);
                }
                self.content(bounds, clip, cells, matches!(kind, Kind::Menu), matches!(kind, Kind::Menu | Kind::Bar));
            }
        }
    }

    fn content(&mut self, bounds: Rect, clip: Rect, cells: &[Cell], menu: bool, proportional: bool) {
        let width = clip.width().max(0) as usize;
        if width == 0 {
            return;
        }
        let mut backgrounds = [0usize; 256];
        for cell in cells {
            backgrounds[crate::color::bg256(cell.attr) as usize] += 1;
        }
        let base = backgrounds.iter().enumerate().max_by_key(|(_, count)| **count).map(|(color, _)| color as u8);
        for (row, line) in cells.chunks(width).enumerate() {
            let y = clip.a.y + row as i32;
            if menu && (y == bounds.a.y || y == bounds.b.y - 1) {
                continue;
            }
            if menu && line.iter().filter(|c| c.ch == '─').count() > line.len() / 2 {
                let r = self.px_box(Rect::new(bounds.a.x + 2, y, bounds.b.x - 2, y + 1));
                self.line((r.x, r.y + r.h / 2.0), (r.x + r.w, r.y + r.h / 2.0), 1.0, m::OUTLINE_VARIANT);
                continue;
            }
            for (column, cell) in line.iter().enumerate() {
                let x = clip.a.x + column as i32;
                if menu && (x <= bounds.a.x + 1 || x >= bounds.b.x - 2 || y == bounds.a.y || y == bounds.b.y - 1) {
                    continue;
                }
                let background = if proportional { Cell::new(' ', cell.attr) } else { *cell };
                self.material_cell(Point::new(x, y), background, base, true);
            }
            if proportional {
                let mut column = 0;
                while column < line.len() {
                    if line[column].ch.is_whitespace() || line[column].ch == '\0' || is_frame(line[column].ch) {
                        column += 1;
                        continue;
                    }
                    let start = column;
                    column += 1;
                    while column < line.len()
                        && !is_frame(line[column].ch)
                        && !(line[column].ch == ' ' && line.get(column + 1).map(|c| c.ch == ' ').unwrap_or(true))
                    {
                        column += 1;
                    }
                    let label: String = line[start..column].iter().filter(|c| !c.is_wide_tail()).map(|c| c.ch).collect();
                    self.text(
                        label.trim_end(),
                        BoxPx {
                            x: (clip.a.x as f32 + start as f32) * self.cell_w() as f32,
                            y: y as f32 * self.cell_h() as f32,
                            w: (column - start + 1) as f32 * self.cell_w() as f32,
                            h: self.cell_h() as f32,
                        },
                        m::ON_SURFACE,
                        false,
                    );
                }
            }
        }
    }

    fn material_cell(&mut self, pos: Point, cell: Cell, base: Option<u8>, transparent: bool) {
        let x = pos.x as f32 * self.cell_w() as f32;
        let y = pos.y as f32 * self.cell_h() as f32;
        self.material_glyph_at(x, y, cell, base, transparent);
    }

    fn material_glyph_at(&mut self, x: f32, y: f32, cell: Cell, base: Option<u8>, transparent: bool) {
        let r = BoxPx { x, y, w: self.cell_w() as f32, h: self.cell_h() as f32 };
        let bg = crate::color::bg256(cell.attr);
        let fg = crate::color::fg256(cell.attr);
        let selected = base.map(|color| color != bg).unwrap_or(false) && fg != bg;
        if selected {
            self.round_box(r, 0.0, m::PRIMARY_CONTAINER, 255);
        } else if !transparent {
            self.round_box(r, 0.0, m::SURFACE, 255);
        }
        let ink = if selected {
            m::PRIMARY
        } else if fg == 8 {
            m::OUTLINE
        } else {
            m::ON_SURFACE
        };
        let mid = (x + r.w / 2.0, y + r.h / 2.0);
        match cell.ch {
            '\0' | ' ' | '░' | '▒' | '▓' => {}
            '─' | '═' | '┄' | '┈' => self.line((x, mid.1), (x + r.w, mid.1), 1.0, m::OUTLINE_VARIANT),
            '│' | '║' => self.line((mid.0, y), (mid.0, y + r.h), 1.0, m::OUTLINE_VARIANT),
            '┌' | '╔' | '┐' | '╗' | '└' | '╚' | '┘' | '╝' | '├' | '╠' | '┤' | '╣' | '┬' | '╦' | '┴' | '╩' | '┼' | '╬' =>
                {}
            '█' | '▄' | '▀' => self.round_box(r.inset(2.0), 2.0, m::PRIMARY, 255),
            c => {
                let key = self.ensure_mask(super::display_char(c), 1);
                let mask = self.cache[&key].clone();
                for row in 0..self.cell_h() {
                    for col in 0..self.cell_w() {
                        self.pixel((x as i32) + col as i32, (y as i32) + row as i32, ink, mask[row * self.cell_w() + col]);
                    }
                }
            }
        }
    }

    fn icon_button(&mut self, r: BoxPx, icon: &str, _hover: bool) {
        if self.hover.map(|p| r.contains(p)).unwrap_or(false) {
            self.round_box(r.inset(2.0), r.h / 2.0, m::PRIMARY, 20);
        }
        let size = (self.font.px * 0.7).min(r.h * 0.5);
        let (x, y) = (r.x + r.w / 2.0, r.y + r.h / 2.0);
        if icon == "close" {
            self.line((x - size / 2.0, y - size / 2.0), (x + size / 2.0, y + size / 2.0), 1.5, m::ON_SURFACE_VARIANT);
            self.line((x + size / 2.0, y - size / 2.0), (x - size / 2.0, y + size / 2.0), 1.5, m::ON_SURFACE_VARIANT);
        } else if icon == "minimize" {
            self.line((x - size / 2.0, y), (x + size / 2.0, y), 1.5, m::ON_SURFACE_VARIANT);
        } else {
            let square = BoxPx { x: x - size / 2.0, y: y - size / 2.0, w: size, h: size };
            if icon == "restore" {
                self.round_box(BoxPx { x: square.x + 3.0, y: square.y - 3.0, ..square }, 1.0, m::ON_SURFACE_VARIANT, 255);
                self.round_box(BoxPx { x: square.x + 4.5, y: square.y - 1.5, w: square.w - 3.0, h: square.h - 3.0 }, 0.0, m::CONTAINER_HIGH, 255);
            }
            self.round_box(square, 1.0, m::ON_SURFACE_VARIANT, 255);
            self.round_box(square.inset(1.5), 0.0, m::CONTAINER_HIGH, 255);
        }
    }

    fn text(&mut self, text: &str, r: BoxPx, color: u32, centered: bool) {
        self.text_size(text, r, color, centered, self.font.px);
    }

    fn text_size(&mut self, text: &str, r: BoxPx, color: u32, centered: bool, px: f32) {
        let px = if self.material_offset_y != 0 { px * 0.85 } else { px };
        if r.w <= 0.0 || r.h <= 0.0 {
            return;
        }
        let text_width: f32 = text.chars().map(|c| self.ui_font.advance(c, px)).sum();
        let mut x = r.x + if centered { ((r.w - text_width) / 2.0).max(0.0) } else { 0.0 };
        let baseline = r.y + (r.h - px) / 2.0 + px * 0.8;
        for c in text.chars() {
            if c.is_control() {
                continue;
            }
            if let Some((metrics, bitmap)) = self.ui_font.glyph(c, px) {
                if x + metrics.advance_width > r.x + r.w {
                    break;
                }
                let x0 = x + metrics.xmin as f32;
                let y0 = baseline - metrics.ymin as f32 - metrics.height as f32;
                for row in 0..metrics.height {
                    for col in 0..metrics.width {
                        let gx = x0 + col as f32;
                        let gy = y0 + row as f32;
                        if gx >= r.x && gx < r.x + r.w && gy >= r.y && gy < r.y + r.h {
                            self.pixel(gx as i32, gy as i32, color, bitmap[row * metrics.width + col]);
                        }
                    }
                }
                x += metrics.advance_width;
            }
        }
    }

    fn pixel(&mut self, x: i32, y: i32, color: u32, alpha: u8) {
        let (left, top, right, bottom) = self.material_clip;
        if alpha == 0 || x < left.max(0) || y < top.max(0) || x >= right.min(self.width as i32) || y >= bottom.min(self.height as i32) {
            return;
        }
        let y = y + self.material_offset_y;
        if y < 0 || y >= self.height as i32 {
            return;
        }
        let index = y as usize * self.width + x as usize;
        self.pixels[index] = blend(self.pixels[index], color, alpha);
    }

    fn round_box(&mut self, r: BoxPx, radius: f32, color: u32, alpha: u8) {
        if r.w <= 0.0 || r.h <= 0.0 {
            return;
        }
        let radius = radius.max(0.0).min(r.w / 2.0).min(r.h / 2.0);
        let (left, top, right, bottom) = self.material_clip;
        for y in (r.y.floor() as i32).max(top).max(0)..((r.y + r.h).ceil() as i32).min(bottom).min(self.height as i32) {
            for x in (r.x.floor() as i32).max(left).max(0)..((r.x + r.w).ceil() as i32).min(right).min(self.width as i32) {
                let dx = ((x as f32 + 0.5) - (r.x + r.w / 2.0)).abs() - (r.w / 2.0 - radius);
                let dy = ((y as f32 + 0.5) - (r.y + r.h / 2.0)).abs() - (r.h / 2.0 - radius);
                let distance = dx.max(0.0).hypot(dy.max(0.0)) + dx.max(dy).min(0.0) - radius;
                let coverage = (0.5 - distance).clamp(0.0, 1.0);
                self.pixel(x, y, color, (coverage * alpha as f32).round() as u8);
            }
        }
    }

    fn line(&mut self, a: (f32, f32), b: (f32, f32), width: f32, color: u32) {
        let distance = (b.0 - a.0).hypot(b.1 - a.1);
        let steps = distance.ceil().max(1.0) as usize;
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            self.round_box(
                BoxPx { x: a.0 + (b.0 - a.0) * t - width / 2.0, y: a.1 + (b.1 - a.1) * t - width / 2.0, w: width, h: width },
                width / 2.0,
                color,
                255,
            );
        }
    }
}

fn is_frame(c: char) -> bool {
    matches!(c, '─' | '═' | '│' | '║' | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_widget_render_and_hit_area_share_the_header_center() {
        for font_size in [16.0, 32.0] {
            let font = crate::font::FontSet::load(None, font_size);
            let mut renderer = Renderer::new(font, crate::GRUVBOX_PALETTE);
            renderer.resize(renderer.cell_w() * 40, renderer.cell_h() * 10);
            let bounds = Rect::new(1, 1, 39, 9);
            let widget = Rect::new(15, 1, 25, 2);
            let mut screen = crate::ScreenBuffer::new(40, 10);
            screen.material = true;
            screen.scene.push(Paint::Widget { bounds, clip: bounds, cells: Vec::new(), kind: Kind::Window {
                title: "Title".into(), active: true, flags: crate::wf::CLOSE,
                title_end: 14, zoomed: false, minimized: false,
            }});
            screen.scene.push(Paint::Widget { bounds: widget, clip: widget, cells: Vec::new(), kind: Kind::Button {
                label: String::new(), primary: true, pressed: false, enabled: true, focused: false,
            }});
            let offset = renderer.header_widget_offset(&screen.scene[..1], widget);
            let band = renderer.header_band(renderer.px_box(bounds));
            let pixels = renderer.px_box(widget);
            assert!((pixels.y + pixels.h / 2.0 + offset as f32 - band.y - band.h / 2.0).abs() <= 0.5);
            let frame = Frame { screen, cursor: None, insert: false };
            renderer.update(&frame, false);
            let point = (pixels.x + pixels.w / 2.0, pixels.y + pixels.h - 1.0 + offset as f32);
            assert_eq!(renderer.header_control_at(Some(&frame), point), Some(Point::new(20, 1)));
            let center = (band.y + band.h / 2.0) as usize * renderer.width + (pixels.x + pixels.w / 2.0) as usize;
            assert_eq!(renderer.pixels()[center], m::PRIMARY);
        }
    }

    #[test]
    fn shifted_header_buttons_hit_their_visual_bounds_at_each_scale() {
        for font_size in [16.0, 32.0] {
            let font = crate::font::FontSet::load(None, font_size);
            let renderer = Renderer::new(font, crate::GRUVBOX_PALETTE);
            let bounds = Rect::new(1, 1, 31, 8);
            let flags = crate::wf::CLOSE | crate::wf::ZOOM | crate::wf::MINIMIZE;
            let mut screen = crate::ScreenBuffer::new(36, 10);
            screen.material = true;
            screen.scene.push(Paint::Widget {
                bounds,
                clip: bounds,
                cells: Vec::new(),
                kind: Kind::Window { title: String::new(), active: true, flags, title_end: 19, zoomed: false, minimized: false },
            });
            let frame = Frame { screen, cursor: None, insert: false };
            let buttons = crate::widgets::window::header_buttons(bounds.width(), flags);
            let shift = HEADER_BUTTON_SHIFT * font_size / 16.0;
            let band = renderer.header_band(renderer.px_box(bounds));
            for rect in [buttons.close, buttons.zoom, buttons.minimize].into_iter().flatten() {
                let pixels = renderer.px_box(rect.moved(bounds.a.x, bounds.a.y));
                let point = (pixels.x - shift + 0.5, pixels.y + pixels.h / 2.0);
                let expected = Point::new(bounds.a.x + (rect.a.x + rect.b.x) / 2, bounds.a.y);
                assert_eq!(renderer.header_control_at(Some(&frame), point), Some(expected));
                let bottom = (point.0, band.y + band.h - 0.5);
                assert_eq!(renderer.header_control_at(Some(&frame), bottom), Some(expected));
            }
            let close = renderer.px_box(buttons.close.unwrap().moved(bounds.a.x, bounds.a.y));
            let empty_edge = (close.x + close.w - 0.5, close.y + close.h / 2.0);
            assert_eq!(renderer.header_control_at(Some(&frame), empty_edge), Some(Point::new(bounds.b.x - 1, bounds.a.y)));
        }
    }

    #[test]
    fn material_button_has_rounded_shape_state_and_clipping() {
        let font = crate::font::FontSet::load(None, 16.0);
        let mut renderer = Renderer::new(font, crate::VGA_PALETTE);
        renderer.resize(renderer.cell_w() * 10, renderer.cell_h() * 3);
        let bounds = Rect::new(1, 0, 9, 2);
        let clip = Rect::new(1, 0, 7, 2);
        let mut screen = crate::ScreenBuffer::new(10, 3);
        screen.material = true;
        let button = Kind::Button { label: "Save".into(), primary: true, pressed: false, enabled: true, focused: false };
        screen.scene.push(Paint::Widget { bounds, clip, kind: button, cells: Vec::new() });
        renderer.update(&Frame { screen: screen.clone(), cursor: None, insert: false }, false);
        let before = renderer.pixels().to_vec();
        assert!(before.contains(&m::PRIMARY));
        for y in 0..renderer.height {
            for x in 7 * renderer.cell_w()..renderer.width {
                assert_eq!(before[y * renderer.width + x], m::SURFACE);
            }
        }
        renderer.set_hover(Some((renderer.cell_w() as f32 * 4.0, renderer.cell_h() as f32)));
        renderer.update(&Frame { screen, cursor: None, insert: false }, false);
        assert_ne!(before, renderer.pixels());
    }

    #[test]
    fn button_focus_and_press_do_not_paint_outside_the_button() {
        let font = crate::font::FontSet::load(None, 16.0);
        let mut renderer = Renderer::new(font, crate::GRUVBOX_PALETTE);
        renderer.resize(renderer.cell_w() * 12, renderer.cell_h() * 4);
        let bounds = Rect::new(1, 1, 11, 3);
        let mut screen = crate::ScreenBuffer::new(12, 4);
        screen.material = true;
        screen.scene.push(Paint::Widget {
            bounds,
            clip: bounds,
            cells: Vec::new(),
            kind: Kind::Button { label: "OK".into(), primary: true, pressed: false, enabled: true, focused: false },
        });
        renderer.update(&Frame { screen: screen.clone(), cursor: None, insert: false }, false);
        let before = renderer.pixels().to_vec();
        if let Paint::Widget { kind: Kind::Button { pressed, focused, .. }, .. } = &mut screen.scene[0] {
            *pressed = true;
            *focused = true;
        }
        renderer.update(&Frame { screen, cursor: None, insert: false }, false);
        let top = renderer.cell_h() + (renderer.cell_h() * 2 - 40) / 2;
        assert_ne!(before, renderer.pixels());
        for y in 0..renderer.height {
            if y < top || y >= top + 40 {
                let range = y * renderer.width..(y + 1) * renderer.width;
                assert_eq!(before[range.clone()], renderer.pixels()[range], "external halo at row {y}");
            }
        }
    }
}
