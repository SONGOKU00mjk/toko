use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Wrap},
};
use ratatui_image::Image;

use crate::app::App;

pub fn ui(frame: &mut Frame, app: &mut App) {
    if app.fullscreen {
        let area = frame.area();
        app.preview_area = area;

        if let Some(state) = &app.animation
            && let Some(protocol) = &state.displayed
        {
            let image = Image::new(protocol);
            frame.render_widget(image, area);
        } else if app.loading {
            centered_line(frame, area, "Loading preview...", Color::Yellow);
        } else if let Some(key) = &app.preview_key {
            if let Some(protocol) = app.cache.get(key) {
                let image = Image::new(protocol);
                frame.render_widget(image, area);
            }
        } else if let Some(err) = &app.preview_error {
            centered_line(frame, area, err, Color::Red);
        } else {
            centered_line(frame, area, "No preview available", Color::Yellow);
        }
        return;
    }

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // header
            Constraint::Min(3),    // main area
            Constraint::Length(1), // status bar
        ])
        .split(frame.area());

    let header_area = vertical[0];
    let main_area = vertical[1];
    let status_area = vertical[2];

    let position_text = if app.wallpapers.is_empty() {
        String::from("0 / 0")
    } else {
        format!("{} / {}", app.selected + 1, app.wallpapers.len())
    };

    let header = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::DarkGray))
        .title_top(
            Line::from(" Toko ")
                .style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Left),
        )
        .title_top(
            Line::from(format!(" {position_text} "))
                .style(Style::default().fg(Color::White))
                .alignment(Alignment::Right),
        );
    frame.render_widget(header, header_area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(main_area);

    let list_area = horizontal[0];
    let preview_area = horizontal[1];

    if app.wallpapers.is_empty() {
        let msg = app.message.as_deref().unwrap_or("No wallpapers found");
        let paragraph = Paragraph::new(msg)
            .block(Block::default().borders(Borders::ALL).title("Wallpapers"))
            .style(Style::default().fg(Color::Yellow));
        frame.render_widget(paragraph, list_area);
    } else {
        let items: Vec<ListItem> = app
            .wallpapers
            .iter()
            .map(|w| ListItem::new(w.name.as_str()))
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Wallpapers"))
            .highlight_style(
                Style::default()
                    .bg(Color::Blue)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▶ ");

        let mut list_state = ListState::default();
        list_state.select(Some(app.selected));
        frame.render_stateful_widget(list, list_area, &mut list_state);
    }

    let entry_snapshot = app
        .wallpapers
        .get(app.selected)
        .map(|w| (w.name.clone(), w.path.clone()));
    let (filename, dims) = match entry_snapshot {
        Some((name, path)) => {
            let dims = app.get_dimensions(&path);
            (name, dims)
        }
        None => (String::new(), None),
    };

    let meta_text = match dims {
        Some((w, h)) => format!(" {filename} · {w}×{h} "),
        None if filename.is_empty() => String::from(" "),
        None => format!(" {filename} "),
    };

    let preview_block = Block::default()
        .borders(Borders::ALL)
        .title_top(Line::from(" Preview ").alignment(Alignment::Left))
        .title_top(Line::from(meta_text).alignment(Alignment::Right));
    let preview_inner = preview_block.inner(preview_area);
    app.preview_area = preview_inner;

    frame.render_widget(preview_block, preview_area);

    if let Some(state) = &app.animation
        && let Some(protocol) = &state.displayed
    {
        let image = Image::new(protocol);
        frame.render_widget(image, preview_inner);
    } else if app.loading {
        let paragraph =
            Paragraph::new("Loading preview...").style(Style::default().fg(Color::Yellow));
        frame.render_widget(paragraph, preview_inner);
    } else if let Some(key) = &app.preview_key {
        if let Some(protocol) = app.cache.get(key) {
            let image = Image::new(protocol);
            frame.render_widget(image, preview_inner);
        }
    } else if let Some(err) = &app.preview_error {
        let paragraph = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, preview_inner);
    } else {
        let placeholder = Paragraph::new("Image preview will appear here");
        frame.render_widget(placeholder, preview_inner);
    }

    let precaching = {
        let p = app.progress.lock().unwrap();
        if p.scheduler_done && p.total > 0 && p.completed < p.total {
            Some((p.completed, p.total))
        } else {
            None
        }
    };

    if let Some((completed, total)) = precaching {
        let ratio = completed as f64 / total as f64;
        let pct = (ratio * 100.0).round() as u16;
        let gauge = Gauge::default()
            .gauge_style(
                Style::default()
                    .fg(Color::Green)
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .ratio(ratio.clamp(0.0, 1.0))
            .label(format!(
                "Caching wallpapers... {}% ({}/{})",
                pct, completed, total
            ));
        frame.render_widget(gauge, status_area);
    } else {
        let hint = app.status_message.clone().unwrap_or_else(|| {
            String::from(
                "↑↓/jk Navigate   Home/End Jump   Enter Preview   s Set Wallpaper   q/Esc Quit",
            )
        });
        let status_bar =
            Paragraph::new(hint).style(Style::default().bg(Color::DarkGray).fg(Color::White));
        frame.render_widget(status_bar, status_area);
    }
}

fn centered_line(frame: &mut Frame, area: Rect, msg: &str, color: Color) {
    let line_area = Rect {
        x: area.x,
        y: area.y + area.height / 2,
        width: area.width,
        height: 1,
    };
    let text = Paragraph::new(msg)
        .style(Style::default().fg(color))
        .alignment(Alignment::Center);
    frame.render_widget(text, line_area);
}
