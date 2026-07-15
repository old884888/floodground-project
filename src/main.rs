mod save;
mod app_types;
mod app;
mod components;
mod data;
mod desc;
mod entity_kind;
mod events;
mod items;
mod narrative;
mod systems;
mod ui;
mod village;
mod world;

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

use app::App;
use data::{init_item_registry, init_terrain_registry, load_actors, load_food, load_terrain, DataError};
use systems::{run_tick, ticks_this_frame};

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
        // 强制恢复终端，否则用户屏幕会卡在 raw mode + alternate screen
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
    let frame_min = Duration::from_millis(16); // ~60fps 封顶
    let mut last_tick = Instant::now();

    loop {
        let frame_start = Instant::now();
        terminal.draw(|f| ui::draw(f, app))?;

        // 一口气清空输入队列，别他妈一个一个啃
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    systems::input::handle_key(app, key);
                }
                Event::Mouse(_) => {}
                _ => {}
            }
        }

        // 延迟存档：渲染帧已画了"存档中..."，这里真写盘
        if app.saving {
            if let Err(e) = crate::save::save_game(app) {
                app.push_log(format!("存档失败: {}", e));
            } else {
                app.push_log("已存档。".into());
            }
            app.should_quit = true;
        }

        if app.should_quit {
            break;
        }

        // ── 加载界面：每帧推进 tick ──
        if app.screen == app::Screen::Loading {
            app.loading_tick = app.loading_tick.saturating_add(1);
            // 40 ticks @ ~60fps ≈ 0.67 秒后切到游戏
            if app.loading_tick >= 40 {
                app.screen = app::Screen::Gameplay;
            }
        }

        if app.screen == app::Screen::Gameplay {
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
