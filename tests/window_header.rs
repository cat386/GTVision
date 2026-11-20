use gtvision::{
    platform::{Backend, HeadlessBackend, RawEvent},
    *,
};
use std::time::Duration;

struct MaterialBackend(HeadlessBackend);
impl Backend for MaterialBackend {
    fn material(&self) -> bool {
        true
    }
    fn size(&self) -> Point {
        self.0.size
    }
    fn read(&mut self, _: Duration) -> Option<RawEvent> {
        None
    }
    fn flush(&mut self, _: &ScreenBuffer, _: Option<Point>, _: bool) {}
}

fn mouse(pos: Point, buttons: u8) -> event::MouseEvent {
    event::MouseEvent { pos, buttons, double: false, mods: Mods::NONE, wheel: 0 }
}

#[test]
fn dialog_header_has_only_close_control() {
    let mut app = Program::new(Box::new(MaterialBackend(HeadlessBackend::new(80, 30))), None, None);
    app.insert_window(Dialog::centered(40, 15, "Dialog"));
    app.redraw();
    let flags = app.screen().scene.iter().find_map(|paint| match paint {
        material::Paint::Widget { kind: material::Kind::Window { flags, .. }, .. } => Some(*flags),
        _ => None,
    }).unwrap();
    assert_ne!(flags & wf::CLOSE, 0);
    assert_eq!(flags & (wf::ZOOM | wf::MINIMIZE), 0);
}

#[test]
fn minimize_restore_and_resize_preserve_contents_and_child_geometry() {
    let mut window = Window::new(Rect::new(2, 3, 52, 23), "Profile", 1);
    let mut input = InputLine::new(Rect::new(3, 4, 30, 6), 100);
    input.set_text("unsaved text");
    input.vs.grow_mode = gf::GROW_HI_X;
    let input = window.add(input);
    let mut header = Button::new(Rect::new(20, 0, 30, 1), "Action", 200, bf::NORMAL);
    header.vs.grow_mode = gf::GROW_LO_X | gf::GROW_HI_X;
    let header = window.add_header(header);
    let old_input = window.group.find::<InputLine>(input).unwrap().vs.bounds();
    window.minimize();
    window.minimize();
    assert!(window.is_minimized());
    assert_eq!(window.group.vs.size.y, 1);
    assert_eq!(window.group.find::<InputLine>(input).unwrap().vs.bounds(), old_input);
    View::change_bounds(&mut window, Rect::new(4, 5, 74, 6));
    assert_eq!(window.group.vs.size.y, 1);
    window.restore();
    assert_eq!(window.group.vs.bounds(), Rect::new(4, 5, 74, 25));
    let input = window.group.find::<InputLine>(input).unwrap();
    assert_eq!(input.text(), "unsaved text");
    assert_eq!(input.vs.bounds(), Rect::new(3, 4, 50, 6));
    assert_eq!(window.group.find::<Button>(header).unwrap().vs.origin.x, 40);
    let normal = window.group.vs.bounds();
    window.zoom(Point::new(100, 40));
    window.minimize();
    window.restore();
    assert!(window.is_zoomed());
    window.zoom(Point::new(100, 40));
    assert_eq!(window.group.vs.bounds(), normal);
}

#[test]
fn header_button_dispatches_command_without_dragging_window() {
    struct Handler(usize);
    impl AppHandler for Handler {
        fn handle_event(&mut self, _: &mut Program, event: &mut Event) {
            if event.command() == Some(200) {
                self.0 += 1;
                event.clear();
            }
        }
    }
    let mut app = Program::new(Box::new(MaterialBackend(HeadlessBackend::new(80, 30))), None, None);
    let mut window = Window::new(Rect::new(2, 2, 62, 24), "Profile", 1);
    let button = window.add_header(Button::new(Rect::new(20, 0, 32, 1), "Action", 200, bf::NORMAL));
    let id = app.insert_window(window);
    app.handle(Event::MouseDown(mouse(Point::new(27, 2), 1)), None);
    app.handle(Event::MouseUp(mouse(Point::new(27, 2), 0)), None);
    let mut handler = Handler(0);
    for _ in 0..4 {
        app.step(&mut handler);
    }
    assert_eq!(handler.0, 1);
    assert_eq!(app.find::<Window>(id).unwrap().group.vs.origin, Point::new(2, 2));
    assert!(app.find::<Button>(button).unwrap().vs.is_focused());
    app.redraw();
    let title_end = app
        .screen()
        .scene
        .iter()
        .find_map(|paint| match paint {
            material::Paint::Widget { kind: material::Kind::Window { title_end, .. }, .. } => Some(*title_end),
            _ => None,
        })
        .unwrap();
    assert_eq!(title_end, 22);
}

#[test]
fn tab_traverses_header_widgets_and_returns_to_body() {
    let mut window = Window::new(Rect::new(0, 0, 60, 20), "Profile", 1);
    let body = window.add(InputLine::new(Rect::new(3, 4, 30, 6), 100));
    let first = window.add_header(InputLine::new(Rect::new(10, 0, 22, 1), 30));
    let second = window.add_header(Button::new(Rect::new(24, 0, 34, 1), "Action", 200, bf::NORMAL));
    window.set_state(sf::FOCUSED | sf::SELECTED | sf::ACTIVE, true);
    window.select(body);
    let mut ctx = Ctx::new();
    ctx.material = true;
    for expected in [first, second, body] {
        window.handle_event(&mut Event::KeyDown(KeyEvent::new(Key::Tab, Mods::NONE)), &mut ctx);
        assert!(window.group.view(expected).unwrap().vs().is_focused());
    }
    window.select(first);
    window.minimize();
    window.handle_event(&mut Event::KeyDown(KeyEvent::new(Key::Char('x'), Mods::NONE)), &mut ctx);
    assert_eq!(window.group.find::<InputLine>(first).unwrap().text(), "x");
    window.handle_event(&mut Event::KeyDown(KeyEvent::new(Key::Tab, Mods::NONE)), &mut ctx);
    assert!(window.group.find::<Button>(second).unwrap().vs.is_focused());
}

#[test]
fn graphical_header_controls_close_zoom_minimize_and_restore() {
    let mut app = Program::new(Box::new(MaterialBackend(HeadlessBackend::new(80, 30))), None, None);
    let id = app.insert_window(Window::new(Rect::new(2, 2, 62, 24), "Profile", 1));
    struct Handler;
    impl AppHandler for Handler {}
    let mut handler = Handler;
    // При ширине 60 справа идут close 56..59, zoom 53..56, minimize 50..53.
    for (point, minimized) in [(Point::new(53, 2), true), (Point::new(53, 2), false)] {
        app.handle(Event::MouseDown(mouse(point, 1)), None);
        for _ in 0..3 {
            app.step(&mut handler);
        }
        assert_eq!(app.find::<Window>(id).unwrap().is_minimized(), minimized);
    }
    app.handle(Event::MouseDown(mouse(Point::new(56, 2), 1)), None);
    for _ in 0..3 {
        app.step(&mut handler);
    }
    assert!(app.find::<Window>(id).unwrap().is_zoomed());
    app.handle(Event::MouseDown(mouse(Point::new(77, 0), 1)), None);
    for _ in 0..3 {
        app.step(&mut handler);
    }
    assert!(app.find::<Window>(id).is_none());
}
