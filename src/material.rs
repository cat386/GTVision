//! Графическая сцена и семантические роли Material 3.
//!
//! Геометрия сцены задаётся в логических единицах layout и переводится
//! в пиксели backend. Формы и текст рисуются независимо от символьных масок.

use crate::widgets::*;
use crate::{
    drawbuf::Cell,
    geometry::{Point, Rect},
    view::{sf, CommandSet, View},
};

/// Команды сцены идут в порядке рисования, включая перекрытия и clipping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Paint {
    Cell { pos: Point, cell: Cell },
    Widget { bounds: Rect, clip: Rect, kind: Kind, cells: Vec<Cell> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub pos: Point,
    pub label: String,
    pub checked: bool,
    pub mixed: bool,
    pub enabled: bool,
    pub focused: bool,
}

/// Состояние виджета передаётся renderer явно, без распознавания символов.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Background,
    Scrim,
    Window { title: String, active: bool, flags: u8, title_end: i32, zoomed: bool, minimized: bool },
    Button { label: String, primary: bool, pressed: bool, enabled: bool, focused: bool },
    Input { focused: bool, enabled: bool },
    Choices { items: Vec<Choice>, radio: bool },
    Progress { percent: u8, label: String, indeterminate: bool },
    Scroll { value: i32, min: i32, max: i32, page: i32 },
    Menu,
    Bar,
    Text,
    List { entries: Vec<(Point, String, bool)>, column_width: i32 },
    Content,
}

pub(crate) fn kind(view: &dyn View, commands: &CommandSet) -> Option<Kind> {
    let any = view.as_any();
    let state = view.vs();
    let focused = state.is_focused();
    let enabled = state.state & sf::DISABLED == 0;
    if any.is::<Background>() {
        return Some(Kind::Background);
    }
    if let Some(button) = any.downcast_ref::<Button>() {
        return Some(Kind::Button {
            label: label(&button.title),
            primary: button.am_default,
            pressed: button.pressed(),
            enabled: enabled && commands.is_enabled(button.command),
            focused,
        });
    }
    if any.is::<InputLine>() || any.is::<InputLong>() {
        return Some(Kind::Input { focused, enabled });
    }
    if let Some(cluster) = any.downcast_ref::<Cluster>() {
        let items = cluster
            .strings
            .iter()
            .enumerate()
            .map(|(i, text)| Choice {
                pos: cluster.item_position(i),
                label: label(text),
                checked: cluster.mark(i),
                mixed: cluster.kind == ClusterKind::MultiCheckBoxes && cluster.multi_mark(i) > 1,
                enabled: enabled && i < 32 && cluster.enable_mask & (1u32 << i) != 0,
                focused: focused && cluster.sel == i,
            })
            .collect();
        return Some(Kind::Choices { items, radio: cluster.kind == ClusterKind::RadioButtons });
    }
    if let Some(progress) = any.downcast_ref::<ProgressBar>() {
        return Some(Kind::Progress {
            percent: if progress.indeterminate { progress.animation_phase() } else { progress.percent() },
            label: if progress.show_text { progress.text() } else { String::new() },
            indeterminate: progress.indeterminate,
        });
    }
    if let Some(scroll) = any.downcast_ref::<ScrollBar>() {
        return Some(Kind::Scroll { value: scroll.value, min: scroll.min, max: scroll.max, page: scroll.pg_step });
    }
    if any.is::<MenuBar>() || any.is::<StatusLine>() {
        return Some(Kind::Bar);
    }
    if any.is::<StaticText>() || any.is::<Label>() || any.is::<ColoredText>() {
        return Some(Kind::Text);
    }
    if let Some(list) = any.downcast_ref::<ListBox>() {
        let rows = state.size.y.max(0) as usize;
        let columns = list.num_cols.max(1);
        let column_width = state.size.x.max(0) / columns as i32 + 1;
        let mut entries = Vec::new();
        for column in 0..columns {
            for row in 0..rows {
                let index = list.top_item + column * rows + row;
                if let Some(text) = list.items.get(index) {
                    entries.push((
                        Point::new(column as i32 * column_width, row as i32),
                        text.chars().skip(list.indent).collect(),
                        index == list.focused,
                    ));
                }
            }
        }
        return Some(Kind::List { entries, column_width });
    }
    if view.as_group().is_none() {
        return Some(Kind::Content);
    }
    None
}

pub(crate) fn label(text: &str) -> String {
    text.chars().filter(|&c| c != '~').collect()
}

/// Тёмная Gruvbox — схема графических элементов по умолчанию.
pub const SURFACE: u32 = 0x1D2021;
pub const CONTAINER: u32 = 0x282828;
pub const CONTAINER_HIGH: u32 = 0x3C3836;
pub const PRIMARY: u32 = 0xFABD2F;
pub const ON_PRIMARY: u32 = 0x282828;
pub const PRIMARY_CONTAINER: u32 = 0x504945;
pub const ON_SURFACE: u32 = 0xEBDBB2;
pub const ON_SURFACE_VARIANT: u32 = 0xD5C4A1;
pub const OUTLINE: u32 = 0xA89984;
pub const OUTLINE_VARIANT: u32 = 0x665C54;

pub(crate) fn scroll_thumb(length: f32, value: i32, min: i32, max: i32, page: i32) -> (f32, f32) {
    let track = (length - 2.0).max(0.0);
    let range = (max as i64 - min as i64).max(0) as f32;
    let page = page.max(1) as f32;
    let thumb = (track * page / (range + page)).max(1.0).min(track);
    let fraction = if range > 0.0 { ((value as i64 - min as i64) as f32 / range).clamp(0.0, 1.0) } else { 0.0 };
    (1.0 + (track - thumb) * fraction, thumb)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{platform::HeadlessBackend, AppHandler, Button, Dialog, Program};

    struct MaterialBackend(HeadlessBackend);
    impl crate::platform::Backend for MaterialBackend {
        fn material(&self) -> bool {
            true
        }
        fn size(&self) -> Point {
            self.0.size
        }
        fn read(&mut self, _: std::time::Duration) -> Option<crate::platform::RawEvent> {
            None
        }
        fn flush(&mut self, _: &crate::ScreenBuffer, _: Option<Point>, _: bool) {}
    }

    #[test]
    fn scene_preserves_semantics_clipping_and_frame_order() {
        let mut app = Program::new(Box::new(MaterialBackend(HeadlessBackend::new(60, 24))), None, None);
        let mut dialog = Dialog::new(Rect::new(3, 2, 40, 18), "Настройки");
        dialog.add(Button::new(Rect::new(4, 5, 20, 7), "~O~K", crate::cm::OK, bf::DEFAULT));
        app.insert_window(dialog);
        app.redraw();
        let scene = &app.screen().scene;
        let window = scene.iter().position(|p| matches!(p, Paint::Widget { kind: Kind::Window { .. }, .. })).unwrap();
        let button = scene.iter().position(|p| matches!(p, Paint::Widget { kind: Kind::Button { label, .. }, .. } if label == "OK")).unwrap();
        assert!(window < button);
        for paint in scene {
            if let Paint::Widget { bounds, clip, cells, .. } = paint {
                assert_eq!(*clip, bounds.intersect(clip));
                assert_eq!(cells.len(), (clip.width().max(0) * clip.height().max(0)) as usize);
            }
        }
        let previous = scene.clone();
        app.redraw();
        assert_eq!(previous, app.screen().scene, "redraw must replace the previous scene");
        struct App;
        impl AppHandler for App {}
        app.step(&mut App);
    }

    #[test]
    fn bottom_of_graphical_button_is_clickable_and_takes_focus() {
        use crate::{event::MouseEvent, Ctx, Event, Mods, View};
        let mut button = Button::new(Rect::new(0, 0, 12, 2), "OK", 200, bf::NORMAL);
        let mut ctx = Ctx::new();
        ctx.material = true;
        let mouse = MouseEvent { pos: Point::new(6, 1), buttons: 1, double: false, mods: Mods::NONE, wheel: 0 };
        button.handle_event(&mut Event::MouseDown(mouse), &mut ctx);
        assert!(button.pressed());
        button.handle_event(&mut Event::MouseUp(MouseEvent { buttons: 0, ..mouse }), &mut ctx);
        assert!(!button.pressed());
        assert!(ctx.queue.iter().any(|event| event.command() == Some(200)));
    }

    #[test]
    fn scrollbar_geometry_matches_page_and_range() {
        assert_eq!(scroll_thumb(12.0, 0, 0, 100, 100), (1.0, 5.0));
        assert_eq!(scroll_thumb(12.0, 100, 0, 100, 100), (6.0, 5.0));
        assert_eq!(scroll_thumb(12.0, 0, 0, 0, 100), (1.0, 10.0));
        assert_eq!(scroll_thumb(1.0, 0, 0, 0, 1), (1.0, 0.0));
    }

    #[test]
    fn input_select_all_and_unicode_replacement() {
        use crate::{Ctx, Event, InputLine, Key, KeyEvent, Mods, View};
        let mut input = InputLine::new(Rect::new(0, 0, 20, 2), 100);
        input.set_text("Александр");
        input.vs.state |= sf::FOCUSED;
        let mut ctx = Ctx::new();
        input.handle_event(&mut Event::KeyDown(KeyEvent::new(Key::Char('a'), Mods::CTRL)), &mut ctx);
        for c in "Мария".chars() {
            input.handle_event(&mut Event::KeyDown(KeyEvent::new(Key::Char(c), Mods::NONE)), &mut ctx);
        }
        assert_eq!(input.text(), "Мария");
    }
}
