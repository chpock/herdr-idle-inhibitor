use crate::{
    diagnostics::sanitize,
    model::Work,
    runtime::{
        bootstrap,
        ipc::{self, Details, Operation, SettingsPatch},
        paths::Paths,
    },
    status::Status,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};
use std::{io, time::Duration};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum View {
    Main,
    Settings,
    Details,
}
fn usable(width: u16, height: u16) -> bool {
    width >= 50 && height >= 10
}
fn closes(key: &KeyEvent) -> bool {
    key.code == KeyCode::Esc
        || key.code == KeyCode::Char('q')
        || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
}
pub fn can_handle_key(width: u16, height: u16, key: &KeyEvent) -> bool {
    closes(key) || usable(width, height)
}
fn rows(view: View) -> usize {
    match view {
        View::Main => 4,
        View::Settings => {
            if cfg!(target_os = "linux") {
                6
            } else {
                4
            }
        }
        View::Details => 1,
    }
}
pub fn draw(
    f: &mut Frame,
    status: &Status,
    details: Option<&Details>,
    view: View,
    focus: usize,
    notice: &str,
) {
    let area = f.area();
    let pause_not_saved = status.control.as_ref().is_some_and(|c| !c.pause_persisted);
    if !usable(area.width, area.height) {
        let title = if pause_not_saved {
            "PAUSE NOT SAVED"
        } else {
            "Idle Inhibitor"
        };
        f.render_widget(
            Paragraph::new(format!("{title}\nResize to 50x10\nq/Esc: close")),
            area,
        );
        return;
    }
    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(1),
        Constraint::Length(3),
    ])
    .split(area);
    f.render_widget(
        Paragraph::new(
            "Idle Inhibitor — idle sleep only\nDisplay-off, lock and manual sleep allowed",
        ),
        chunks[0],
    );
    let mut lines = vec![];
    let mut focused_line = None;
    let mut action = |index: usize, text: String| {
        if index == focus {
            focused_line = Some(lines.len());
        }
        lines.push(Line::from(Span::styled(
            format!("{} {text}", if index == focus { ">" } else { " " }),
            if index == focus {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default()
            },
        )));
    };
    match view {
        View::Main => {
            let work = match status.observation.work {
                Work::Working => "Working",
                Work::None => "No work",
                Work::Unknown => "Unknown",
            };
            lines.push(Line::from(format!(
                "Work: {work} — {} agents / {} servers",
                status
                    .observation
                    .working_agents_observed
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".into()),
                status
                    .observation
                    .servers_tracked
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".into())
            )));
            lines.push(Line::from(format!(
                "Observation: {}",
                if status.observation.complete {
                    "Complete"
                } else {
                    "Incomplete"
                }
            )));
            lines.push(Line::from(format!(
                "Request: {}",
                if status.available {
                    status.inhibition.request_state.as_str()
                } else {
                    "Monitor unavailable"
                }
            )));
            lines.push(Line::from(format!("Reason: {}", status.inhibition.reason)));
            lines.push(Line::from(format!(
                "Desired: {:?} / Owned: {:?}",
                status.inhibition.desired, status.inhibition.resource_owned
            )));
            if let Some(error) = status
                .error
                .as_ref()
                .or(status.inhibition.last_error.as_ref())
            {
                lines.push(Line::from(format!(
                    "{}: {}",
                    sanitize(&error.code),
                    sanitize(&error.message)
                )));
            }
            if let Some(t) = status.inhibition.release_in_ms {
                lines.push(Line::from(format!("Releasing in {}s", t.div_ceil(1000))));
            }
            if let Some(t) = status.inhibition.retention_remaining_ms {
                lines.push(Line::from(format!(
                    "Retained after observation loss: {}s",
                    t.div_ceil(1000)
                )));
            }
            let paused = status.control.as_ref().is_some_and(|c| c.paused);
            for (i, name) in [
                if !status.available {
                    "Retry / activate"
                } else if paused {
                    "Resume"
                } else {
                    "Pause"
                },
                "Settings",
                "Details",
                "Close",
            ]
            .iter()
            .enumerate()
            {
                if i == focus {
                    focused_line = Some(lines.len());
                }
                lines.push(Line::from(Span::styled(
                    format!("{} [{name}]", if i == focus { ">" } else { " " }),
                    if i == focus {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    },
                )));
            }
            lines.push(Line::from(
                "Pause applies to all local servers until Resume.",
            ));
        }
        View::Settings => {
            if let Some(d) = details {
                let c = &d.config;
                action(0, format!("Pause: {}", c.paused));
                action(
                    1,
                    format!("Release delay: {}s (Left/Right)", c.release_delay_secs),
                );
                if cfg!(target_os = "linux") {
                    action(
                        2,
                        format!("Linux backend: {} (Enter cycles)", c.linux.backend),
                    );
                    action(
                        3,
                        format!(
                            "Hypridle integration confirmed: {}",
                            c.linux.hypridle_integration_confirmed
                        ),
                    );
                    action(4, "Reload TOML settings".into());
                    action(5, "Back".into());
                } else {
                    action(2, "Reload TOML settings".into());
                    action(3, "Back".into());
                }
                if cfg!(target_os = "linux") {
                    lines.push(Line::from("Hypridle: ignore_systemd_inhibit=false; set ignore_inhibit=true only for dim/off/lock listeners, not idle-suspend. This override ignores all inhibitors. Confirm only after applying docs/hypridle-setup.md."));
                }
                lines.push(Line::from(
                    "Advanced local endpoints: edit TOML, then Reload.",
                ));
            } else {
                lines.push(Line::from("Monitor unavailable. Return and choose Retry."));
            }
        }
        View::Details => {
            lines.push(Line::from(format!(
                "Backend: {}",
                status
                    .inhibition
                    .backend
                    .as_deref()
                    .unwrap_or("not selected")
            )));
            lines.push(Line::from(format!(
                "Native resource owned: {:?}",
                status.inhibition.resource_owned
            )));
            if let Some(d) = details {
                lines.push(Line::from(format!("Config: {}", sanitize(&d.config_path))));
                for s in &d.servers {
                    lines.push(Line::from(format!(
                        "{} — {} / age {:?}ms / Herdr {}",
                        sanitize(&s.endpoint),
                        if s.current { "current" } else { "unknown" },
                        s.age_ms,
                        s.version.as_deref().unwrap_or("unqualified")
                    )));
                }
            }
            if let Some(d) = &status.diagnostics {
                for issue in &d.issues {
                    lines.push(Line::from(format!(
                        "{}: {}",
                        sanitize(&issue.code),
                        sanitize(&issue.message)
                    )));
                }
                if d.known_limitations
                    .iter()
                    .any(|s| s == "windows_modern_standby_battery")
                {
                    lines.push(Line::from("Windows Modern Standby on battery may still sleep. A request is not an unlimited-awake guarantee; policy can also ignore it."));
                }
            }
            if status.inhibition.backend.as_deref() == Some("kde-powerdevil") {
                lines.push(Line::from("PowerDevil 6.7.5 limitation: a suppressed request can outlive its owner and reactivate when allowed. See README."));
            }
            lines.push(Line::from("Up/Down: scroll. Enter: Back. Escape: Close"));
        }
    }
    let content_height = chunks[1].height.saturating_sub(2) as usize;
    let scroll = if view == View::Details {
        focus
    } else {
        focused_line
            .unwrap_or(0)
            .saturating_sub(content_height.saturating_sub(1))
    };
    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(match view {
            View::Main => "Status",
            View::Settings => "Settings",
            View::Details => "Details",
        }))
        .scroll((scroll.min(u16::MAX as usize) as u16, 0));
    // Control rows stay single-line and in view; long diagnostics wrap in Details.
    let paragraph = if view == View::Details {
        paragraph.wrap(Wrap { trim: false })
    } else {
        paragraph
    };
    f.render_widget(paragraph, chunks[1]);
    let mut footer = if pause_not_saved {
        vec![
            Line::from(Span::styled(
                "PAUSE NOT SAVED",
                Style::default().fg(Color::Yellow),
            )),
            Line::from("Restart may restore the previous setting."),
        ]
    } else {
        vec![Line::from(sanitize(notice))]
    };
    footer.push(Line::from("Tab/arrows Enter: select  p/s/d  q/Esc: close"));
    f.render_widget(Paragraph::new(footer), chunks[2]);
}
struct TerminalGuard;
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
    }
}
fn panic_restore() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = terminal::disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen);
            previous(info);
        }));
    });
}
pub async fn run() -> anyhow::Result<()> {
    let paths = Paths::get()?;
    panic_restore();
    terminal::enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(io::stdout(), EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let mut terminal = ratatui::Terminal::new(backend)?;
    let (mut status, mut details) = (
        Status::unavailable("monitor_unavailable", "Monitor unavailable"),
        None,
    );
    let (mut view, mut focus, mut notice) = (View::Main, 0, String::new());
    let mut refresh = tokio::time::Instant::now();
    loop {
        if tokio::time::Instant::now() >= refresh {
            match ipc::query(&paths.endpoint, Operation::GetStatus { details: true }).await {
                Ok(r) => {
                    status = Status::from_wire(r.status)?;
                    details = r.details;
                }
                Err(e) => {
                    status = Status::unavailable(e.code, "Monitor unavailable");
                    details = None;
                }
            }
            refresh = tokio::time::Instant::now() + Duration::from_secs(1);
        }
        terminal.draw(|f| draw(f, &status, details.as_ref(), view, focus, &notice))?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        let size = terminal.size()?;
        if !can_handle_key(size.width, size.height, &key) {
            continue;
        }
        if closes(&key) {
            break;
        }
        if view == View::Details
            && matches!(
                key.code,
                KeyCode::Up | KeyCode::Down | KeyCode::PageUp | KeyCode::PageDown
            )
        {
            focus = match key.code {
                KeyCode::Up => focus.saturating_sub(1),
                KeyCode::Down => (focus + 1).min(4096),
                KeyCode::PageUp => focus.saturating_sub(10),
                _ => (focus + 10).min(4096),
            };
            continue;
        }
        let mut operation = None;
        match key.code {
            KeyCode::Tab | KeyCode::Down => focus = (focus + 1) % rows(view),
            KeyCode::BackTab | KeyCode::Up => focus = (focus + rows(view) - 1) % rows(view),
            KeyCode::Char('s') => {
                view = View::Settings;
                focus = 0;
            }
            KeyCode::Char('d') => {
                view = View::Details;
                focus = 0;
            }
            KeyCode::Char('p') if status.available => {
                operation = Some(Operation::SetPaused {
                    paused: !status.control.as_ref().unwrap().paused,
                })
            }
            KeyCode::Left | KeyCode::Right if view == View::Settings && focus == 1 => {
                if let Some(d) = &details {
                    let n = d.config.release_delay_secs;
                    let n = if key.code == KeyCode::Left {
                        n.saturating_sub(1)
                    } else {
                        (n + 1).min(60)
                    };
                    operation = Some(Operation::ApplySettings {
                        patch: SettingsPatch {
                            release_delay_secs: Some(n),
                            ..Default::default()
                        },
                    });
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => match view {
                View::Main => match focus {
                    0 if !status.available => {
                        match crate::herdr::discovery::Registration::from_env() {
                            Ok(r) => match bootstrap::ensure(r).await {
                                Ok(_) => notice = "Activated".into(),
                                Err(_) => {
                                    notice = "Activation failed; inspect Herdr plugin logs".into()
                                }
                            },
                            Err(_) => {
                                notice =
                                    "No Herdr invocation context; reopen through its plugin action"
                                        .into()
                            }
                        }
                    }
                    0 => {
                        operation = Some(Operation::SetPaused {
                            paused: !status.control.as_ref().unwrap().paused,
                        })
                    }
                    1 => {
                        view = View::Settings;
                        focus = 0;
                    }
                    2 => {
                        view = View::Details;
                        focus = 0;
                    }
                    _ => break,
                },
                View::Details => {
                    view = View::Main;
                    focus = 0;
                }
                View::Settings => {
                    if let Some(d) = &details {
                        match focus {
                            0 => {
                                operation = Some(Operation::SetPaused {
                                    paused: !d.config.paused,
                                })
                            }
                            2 if cfg!(target_os = "linux") => {
                                let options = ["auto", "hypridle", "gnome", "kde"];
                                let index = options
                                    .iter()
                                    .position(|s| *s == d.config.linux.backend)
                                    .unwrap_or(0);
                                operation = Some(Operation::ApplySettings {
                                    patch: SettingsPatch {
                                        linux_backend: Some(options[(index + 1) % 4].into()),
                                        ..Default::default()
                                    },
                                });
                            }
                            3 if cfg!(target_os = "linux") => {
                                operation = Some(Operation::ApplySettings {
                                    patch: SettingsPatch {
                                        hypridle_integration_confirmed: Some(
                                            !d.config.linux.hypridle_integration_confirmed,
                                        ),
                                        ..Default::default()
                                    },
                                })
                            }
                            4 if cfg!(target_os = "linux") => {
                                operation = Some(Operation::ReloadSettings)
                            }
                            2 if !cfg!(target_os = "linux") => {
                                operation = Some(Operation::ReloadSettings)
                            }
                            5 | 3 if !cfg!(target_os = "linux") || focus == 5 => {
                                view = View::Main;
                                focus = 0;
                            }
                            _ => (),
                        }
                    }
                }
            },
            _ => (),
        }
        if let Some(op) = operation {
            match ipc::query(&paths.endpoint, op).await {
                Ok(r) => {
                    notice = r
                        .error
                        .map(|i| i.message)
                        .unwrap_or_else(|| "Applied".into());
                    status = Status::from_wire(r.status)?;
                }
                Err(_) => notice = "Monitor unavailable; operation not acknowledged".into(),
            }
            refresh = tokio::time::Instant::now();
        }
    }
    Ok(())
}
