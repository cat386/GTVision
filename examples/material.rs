//! Material 3 — форма, списки, переключатели и действия.
//! cargo run --example material

use gtvision::*;

const SAVE: u16 = 200;
const RESET: u16 = 201;
const HEADER_HELP: u16 = 202;

struct App;

impl AppHandler for App {
    fn handle_event(&mut self, app: &mut Program, event: &mut Event) {
        match event.command() {
            Some(HEADER_HELP) => {
                message_box(
                    app,
                    "В заголовок можно добавлять кнопки, поля ввода и другие виджеты.\nСвёрнутое окно восстанавливается нажатием на заголовок.",
                    mf::INFORMATION | mf::OK_BUTTON,
                );
                event.clear();
            }
            Some(SAVE) => {
                message_box(app, "Изменения приняты", mf::INFORMATION | mf::OK_BUTTON);
                event.clear();
            }
            Some(RESET) => {
                message_box(app, "Настройки можно изменить в полях формы.", mf::INFORMATION | mf::OK_BUTTON);
                event.clear();
            }
            _ => {}
        }
    }
}

fn app_main() -> std::io::Result<()> {
    locale::set_language(locale::Language::Ru);
    let menu = Menu::new().submenu("~G~TVision", Menu::new().item("~П~рименить", SAVE).separator().item_key("~В~ыход", cm::QUIT, kb::ALT_X, "Alt+X"));
    let status = vec![StatusDef::all(vec![
        StatusItem { text: "~Tab~ Следующее поле".into(), key: None, command: 0 },
        StatusItem::new("~Alt+X~ Выход", kb::ALT_X, cm::QUIT),
    ])];
    let mut app = Program::with_window(Some(menu), Some(status))?;
    let mut window = Window::new(Rect::new(3, 2, 93, 26), "GTVision · Material 3", 1);
    window.flags = wf::MOVE | wf::CLOSE | wf::GROW | wf::ZOOM | wf::MINIMIZE;
    let header_width = window.header_bounds().width();
    let mut help = Button::new(Rect::new(header_width - 14, 0, header_width - 1, 1), "Помощь", HEADER_HELP, bf::NORMAL);
    help.vs.grow_mode = gf::GROW_LO_X | gf::GROW_HI_X;
    window.add_header(help);
    window.add(StaticText::new(Rect::new(3, 2, 50, 3), "Профиль и уведомления"));
    window.add(StaticText::new(Rect::new(3, 3, 68, 4), "Настройте рабочее пространство под себя."));
    window.add(StaticText::new(Rect::new(3, 5, 39, 6), "Имя"));
    let mut name = InputLine::new(Rect::new(3, 6, 42, 8), 80);
    name.set_text("Александр");
    let name_id = window.add(name);
    window.select(name_id);
    window.add(StaticText::new(Rect::new(3, 9, 39, 10), "Электронная почта"));
    let mut email = InputLine::new(Rect::new(3, 10, 42, 12), 120);
    email.set_text("alex@example.com");
    window.add(email);
    window.add(StaticText::new(Rect::new(49, 5, 83, 6), "Рабочее пространство"));
    let mut list = ListBox::new(Rect::new(49, 7, 84, 12), 1, None);
    list.set_items(vec!["Личное".into(), "Команда".into(), "Проекты".into(), "Архив".into()]);
    window.add(list);
    window.add(StaticText::new(Rect::new(3, 13, 39, 14), "Уведомления"));
    let mut checks = CheckBoxes::check_boxes(Rect::new(3, 14, 42, 17), &["Письма о событиях", "Напоминания", "Новости продукта"]);
    checks.value = 3;
    window.add(checks);
    window.add(StaticText::new(Rect::new(49, 13, 83, 14), "Частота писем"));
    window.add(RadioButtons::radio_buttons(Rect::new(49, 14, 84, 17), &["Сразу", "Раз в день", "Раз в неделю"]));
    let mut progress = ProgressBar::new(Rect::new(3, 18, 42, 19), 100);
    progress.set_value(65);
    window.add(progress);
    window.add(StaticText::new(Rect::new(49, 18, 83, 19), "Профиль заполнен на 65%"));
    window.add(Button::new(Rect::new(49, 20, 65, 22), "Отмена", RESET, bf::NORMAL));
    window.add(Button::new(Rect::new(67, 20, 84, 22), "Применить", SAVE, bf::DEFAULT));
    app.insert_window(window);
    app.run(&mut App);
    Ok(())
}

fn main() -> std::io::Result<()> {
    run(GuiConfig::new("GTVision — Material 3").size(96, 29).font_size(16.0).remember_settings(false), app_main)
}
