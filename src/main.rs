mod animation;
mod app;
mod backend;
mod cache;
mod config;
mod discover;
mod preview;
mod ui;

use std::{io, sync::Arc, sync::Mutex, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend, layout::Size};
use ratatui_image::picker::Picker;

use crate::{
    app::App,
    backend::detect_backend,
    config::Config,
    preview::{PrecacheProgress, SharedProgress, spawn_precache_scheduler, spawn_worker_pool},
    ui::ui,
};

fn main() -> io::Result<()> {
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("toko: {e}");
            std::process::exit(1);
        }
    };

    let picker = Arc::new(Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks()));

    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        .clamp(1, 4);

    let progress: SharedProgress = Arc::new(Mutex::new(PrecacheProgress::new()));

    let (queue, response_rx) = spawn_worker_pool(
        Arc::clone(&picker),
        num_workers,
        Arc::clone(&progress),
        config.preview_max_dim,
    );
    let precache_tx = spawn_precache_scheduler(
        Arc::clone(&queue),
        Arc::clone(&progress),
        config.wallpaper_dir.clone(),
        config.preview_max_dim,
    );

    let backend = detect_backend(&config.backend);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend_term = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend_term)?;

    let mut app = App::new(
        queue,
        response_rx,
        precache_tx,
        Arc::clone(&progress),
        backend,
        Arc::clone(&picker),
        &config.wallpaper_dir,
    );
    let result = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {err:?}");
    }
    Ok(())
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> io::Result<()> {
    const POLL_INTERVAL: Duration = Duration::from_millis(50);

    while !app.should_quit {
        app.tick_status();
        app.tick_animation();
        app.poll_preview();
        terminal.draw(|frame| ui(frame, app))?;

        if !app.precache_triggered && app.preview_area.width > 0 && app.preview_area.height > 0 {
            let size = Size::new(app.preview_area.width, app.preview_area.height);
            let _ = app.precache_tx.send(size);
            app.precache_triggered = true;
        }

        if app.needs_reload {
            app.load_preview();
            app.needs_reload = false;
        }

        if event::poll(POLL_INTERVAL)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    let selection_changed = if app.fullscreen {
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                                app.toggle_fullscreen();
                                false
                            }
                            _ => false,
                        }
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => {
                                app.should_quit = true;
                                false
                            }
                            KeyCode::Enter => {
                                if !app.wallpapers.is_empty() {
                                    app.select();
                                    app.toggle_fullscreen();
                                }
                                false
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.next();
                                true
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.previous();
                                true
                            }
                            KeyCode::Home => {
                                app.first();
                                true
                            }
                            KeyCode::End => {
                                app.last();
                                true
                            }
                            KeyCode::Char('s') => {
                                app.set_wallpaper();
                                false
                            }
                            _ => false,
                        }
                    };
                    if selection_changed {
                        app.load_preview();
                    }
                }
                Event::Resize(_, _) => {
                    app.needs_reload = true;
                }
                _ => {}
            }
        }
    }
    Ok(())
}
