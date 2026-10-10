use herdr_idle_inhibitor::{
    status::Status,
    ui::{View, draw},
};
#[test]
fn tiny_and_normal_render_and_escape_sanitization() {
    for (width, height) in [(8, 3), (52, 20), (100, 35)] {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        for view in [View::Main, View::Settings, View::Details] {
            terminal
                .draw(|f| {
                    draw(
                        f,
                        &Status::unavailable("monitor_unavailable", "Unknown"),
                        None,
                        view,
                        0,
                        "notice\x1b[2J",
                    )
                })
                .unwrap();
        }
    }
    assert_eq!(
        herdr_idle_inhibitor::diagnostics::sanitize("a\x1bb\nc\x07"),
        "abc"
    );
}

fn paused_unsaved() -> Status {
    use herdr_idle_inhibitor::status::{Control, Diagnostics, Monitor};
    let mut status = Status::unavailable("monitor_unavailable", "Monitor unavailable");
    status.available = true;
    status.error = None;
    status.monitor = Some(Monitor {
        instance_id: "test".into(),
        pid: 1,
        app_version: "0.1.0".into(),
        uptime_ms: 0,
        snapshot_seq: 1,
        evaluated_at_unix_ms: 0,
    });
    status.control = Some(Control {
        paused: true,
        pause_persisted: false,
    });
    status.observation = herdr_idle_inhibitor::model::aggregate([].iter(), true, 0);
    status.inhibition.desired = Some(false);
    status.inhibition.resource_owned = Some(false);
    status.inhibition.reason = "paused".into();
    status.inhibition.request_state = "none".into();
    status.diagnostics = Some(Diagnostics {
        issues: vec![],
        known_limitations: vec![],
        counters: Default::default(),
    });
    Status::from_wire(serde_json::to_value(status).unwrap()).unwrap()
}
#[test]
fn every_supported_focus_is_visible_and_unsaved_pause_warning_survives_reopen_and_toasts() {
    use herdr_idle_inhibitor::runtime::{config::Config, ipc::Details};
    let status = paused_unsaved();
    let config = Config {
        paused: true,
        ..Default::default()
    };
    let details = Details {
        config,
        config_path: "private-config".into(),
        servers: vec![],
        runtime: None,
    };
    let mut settings = vec!["Pause: true", "Release delay"];
    if cfg!(target_os = "linux") {
        settings.push("Linux backend");
    }
    settings.push("Back");
    for (width, height) in [(50, 10), (52, 12), (64, 22), (100, 35)] {
        for (view, labels) in [
            (
                View::Main,
                vec!["[Resume]", "[Settings]", "[Details]", "[Close]"],
            ),
            (View::Settings, settings.clone()),
        ] {
            for (focus, label) in labels.iter().enumerate() {
                for notice in ["", "Applied"] {
                    let mut terminal =
                        ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
                            .unwrap();
                    terminal
                        .draw(|f| draw(f, &status, Some(&details), view, focus, notice))
                        .unwrap();
                    let screen: String = terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .map(|c| c.symbol())
                        .collect();
                    assert!(
                        !screen.contains("Hypridle integration confirmed")
                            && !screen.contains("Reload"),
                        "obsolete manual setup/reload control still visible: {screen}"
                    );
                    assert!(
                        screen.contains(label),
                        "selected {label} hidden at {width}x{height}: {screen}"
                    );
                    assert!(
                        screen.contains("PAUSE NOT SAVED"),
                        "shared warning hidden by a reopen/toast: {screen}"
                    );
                }
            }
        }
    }
}
#[test]
fn resize_only_mode_accepts_dismissal_but_no_hidden_control_actions() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use herdr_idle_inhibitor::ui::can_handle_key;
    for (width, height) in [(8, 3), (24, 8), (49, 20), (50, 9)] {
        for code in [
            KeyCode::Enter,
            KeyCode::Char('p'),
            KeyCode::Right,
            KeyCode::Tab,
        ] {
            assert!(!can_handle_key(
                width,
                height,
                &KeyEvent::new(code, KeyModifiers::NONE)
            ));
        }
        for code in [KeyCode::Esc, KeyCode::Char('q')] {
            assert!(can_handle_key(
                width,
                height,
                &KeyEvent::new(code, KeyModifiers::NONE)
            ));
        }
        assert!(can_handle_key(
            width,
            height,
            &KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)
        ));
    }
    assert!(can_handle_key(
        50,
        10,
        &KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    ));
}

#[test]
fn unavailable_ui_labels_bootstrap_as_an_explicit_start_control() {
    let status = Status::unavailable("monitor_unavailable", "Monitor unavailable");
    for (width, height) in [(50, 10), (80, 22)] {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|f| draw(f, &status, None, View::Main, 0, ""))
            .unwrap();
        let screen: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(screen.contains("[Start monitoring]"), "{screen}");
        assert!(!screen.contains("activate"), "{screen}");
    }
}
