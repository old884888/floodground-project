use std::io::{self, stdout, Write};
use std::time::{Duration, Instant};

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use rand::thread_rng;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use bloodsoil::app::App;
use bloodsoil::data::{
    init_item_registry, init_terrain_registry, load_actors, load_food, load_terrain, DataError,
};
use bloodsoil::systems::{run_tick, ticks_this_frame};

fn main() -> io::Result<()> {
    // 任何初始化错误都走 install_panic_hook 之前：直接打到 stderr，正常退出
    let terrain = match load_terrain("assets/data/terrain.ron") {
        Ok(t) => t,
        Err(e) => return Err(data_error_to_io(e)),
    };
    if let Err(e) = init_terrain_registry(terrain.clone()) {
        return Err(data_error_to_io(e));
    }
    let actors = match load_actors("assets/data/actors.ron") {
        Ok(a) => a,
        Err(e) => return Err(data_error_to_io(e)),
    };
    let food_data = match load_food("assets/data/food.ron") {
        Ok(f) => f,
        Err(e) => return Err(data_error_to_io(e)),
    };
    if let Err(e) = init_item_registry("assets/data/items.ron") {
        return Err(data_error_to_io(e));
    }
    let mut rng = thread_rng();
    let mut app = match App::new(&terrain, &actors, food_data, &mut rng) {
        Ok(a) => a,
        Err(e) => return Err(data_error_to_io(e)),
    };

    install_panic_hook();
    let mut terminal_guard = TerminalGuard::enter()?;

    let result = run_loop(&mut terminal_guard.terminal, &mut app, &mut rng);
    if let Err(e) = &result {
        let _ = writeln!(io::stderr(), "游戏异常退出: {}", e);
    }
    drop(terminal_guard);
    result
}

fn data_error_to_io(e: DataError) -> io::Error {
    let kind = match &e {
        DataError::Io { source, .. } => source.kind(),
        _ => io::ErrorKind::InvalidData,
    };
    io::Error::new(kind, e.to_string())
}

/// 持有 terminal，在 Drop 时强制还原（即使 panic）。
/// 配合 install_panic_hook 双重保险：先恢复终端，再让默认 hook 打印栈。
struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}

fn install_panic_hook() {
    use std::panic;
    let prev = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        prev(info);
    }));
}

fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    rng: &mut impl rand::Rng,
) -> io::Result<()> {
    let tick_interval = Duration::from_millis(100);
    let frame_min = Duration::from_millis(16);
    let mut last_tick = Instant::now();

    loop {
        let frame_start = Instant::now();
        terminal.draw(|f| bloodsoil::ui::draw(f, app))?;

        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    bloodsoil::systems::input::handle_key(app, key);
                }
                Event::Mouse(_) => {}
                _ => {}
            }
        }

        // 延迟读档
        if app.pending_load {
            match bloodsoil::save::load_game() {
                Ok((data, world, uid_map)) => {
                    app.world = world;
                    app.player = uid_map.get(&data.player_uid).copied().unwrap_or(app.player);
                    app.selected = uid_map.get(&data.selected_uid).copied().or(Some(app.player));
                    app.tick = data.tick;
                    app.day = data.day;
                    app.weather = data.weather;
                    app.weather_timer = data.weather_timer;
                    app.reputation = data.reputation;
                    app.next_uid = data.next_uid;
                    app.map.apply_chunks(data.dirty_chunks);
                    app.rebuild_spatial_index();
                    app.loading_tick = 30;
                    app.push_log("已加载存档。".into());
                }
                Err(e) => {
                    app.screen = bloodsoil::app::Screen::MainMenu;
                    app.push_log(format!("读档失败: {}", e));
                }
            }
            app.pending_load = false;
        }

        // 延迟存档
        if app.saving {
            app.save_frame += 1;
            if app.save_frame >= 3 {
                if let Err(e) = bloodsoil::save::save_game(app) {
                    app.push_log(format!("存档失败: {}", e));
                } else {
                    app.push_log("已存档。".into());
                }
                app.should_quit = true;
            }
        }

        if app.should_quit {
            break;
        }

        // 加载界面
        if app.screen == bloodsoil::app::Screen::Loading {
            app.loading_tick = app.loading_tick.saturating_add(1);
            if app.loading_tick >= 40 {
                app.screen = bloodsoil::app::Screen::Gameplay;
            }
        }

        if app.screen == bloodsoil::app::Screen::Gameplay {
            let time_to_tick = last_tick.elapsed() >= tick_interval || app.force_step;
            if time_to_tick {
                last_tick = Instant::now();
                let mut steps = ticks_this_frame(app.speed);
                if app.force_step {
                    app.force_step = false;
                    steps = steps.max(1);
                }
                for _ in 0..steps {
                    run_tick(app, rng);
                }
            }
        }

        let elapsed = frame_start.elapsed();
        if elapsed < frame_min {
            std::thread::sleep(frame_min - elapsed);
        }
    }

    Ok(())
}
