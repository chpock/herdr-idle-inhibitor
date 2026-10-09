use herdr_idle_inhibitor::{model::*, policy::Policy};

fn server(status: &str, now: u64) -> ServerObservation {
    let mut s = ServerObservation::default();
    s.commit(
        vec![Agent {
            terminal_id: "t1".into(),
            agent_status: status.into(),
        }],
        now,
    )
    .unwrap();
    s
}
fn view(servers: &[ServerObservation], now: u64) -> Observation {
    aggregate(servers.iter(), true, now)
}
#[test]
fn cold_unknown_cannot_acquire_or_start_grace() {
    let mut p = Policy::default();
    let o = aggregate([].iter(), false, 0);
    assert_eq!(o.work, Work::Unknown);
    assert!(!p.evaluate(&o, &[], 0, false, true, false, 5).desired);
}
#[test]
fn release_deadline_does_not_slide_and_new_work_cancels_it() {
    let mut p = Policy::default();
    let mut s = server("working", 0);
    assert!(
        p.evaluate(
            &view(std::slice::from_ref(&s), 0),
            std::slice::from_ref(&s),
            0,
            false,
            true,
            false,
            5
        )
        .desired
    );
    s.commit(vec![], 1000).unwrap();
    assert_eq!(
        p.evaluate(
            &view(std::slice::from_ref(&s), 1000),
            std::slice::from_ref(&s),
            1000,
            false,
            true,
            true,
            5
        )
        .release_in_ms,
        Some(5000)
    );
    assert_eq!(
        p.evaluate(
            &view(std::slice::from_ref(&s), 4000),
            std::slice::from_ref(&s),
            4000,
            false,
            true,
            true,
            5
        )
        .release_in_ms,
        Some(2000)
    );
    assert!(
        !p.evaluate(
            &view(std::slice::from_ref(&s), 6000),
            std::slice::from_ref(&s),
            6000,
            false,
            true,
            true,
            5
        )
        .desired
    );
    s.commit(
        vec![Agent {
            terminal_id: "t1".into(),
            agent_status: "working".into(),
        }],
        7000,
    )
    .unwrap();
    assert!(
        p.evaluate(
            &view(std::slice::from_ref(&s), 7000),
            std::slice::from_ref(&s),
            7000,
            false,
            true,
            true,
            5
        )
        .desired
    );
}
#[test]
fn failure_retains_existing_resource_only_without_renewal() {
    let mut p = Policy::default();
    let mut s = server("working", 0);
    p.evaluate(
        &view(std::slice::from_ref(&s), 0),
        std::slice::from_ref(&s),
        0,
        false,
        true,
        true,
        5,
    );
    s.fail();
    for now in [1000, 10000, 29999] {
        let d = p.evaluate(
            &view(std::slice::from_ref(&s), now),
            std::slice::from_ref(&s),
            now,
            false,
            true,
            true,
            5,
        );
        assert_eq!(d.reason, "observation_grace");
        assert_eq!(d.retention_remaining_ms, Some(30000 - now));
    }
    assert!(
        !p.evaluate(
            &view(std::slice::from_ref(&s), 30000),
            std::slice::from_ref(&s),
            30000,
            false,
            true,
            true,
            5
        )
        .desired
    );
    assert!(
        !p.evaluate(
            &view(std::slice::from_ref(&s), 1000),
            std::slice::from_ref(&s),
            1000,
            false,
            true,
            false,
            5
        )
        .desired
    );
}
#[test]
fn completed_work_cannot_be_resurrected_by_an_error() {
    let mut p = Policy::default();
    let mut s = server("working", 0);
    p.evaluate(
        &view(std::slice::from_ref(&s), 0),
        std::slice::from_ref(&s),
        0,
        false,
        true,
        true,
        5,
    );
    s.commit(vec![], 1000).unwrap();
    p.evaluate(
        &view(std::slice::from_ref(&s), 1000),
        std::slice::from_ref(&s),
        1000,
        false,
        true,
        true,
        5,
    );
    s.fail();
    assert!(
        !p.evaluate(
            &view(std::slice::from_ref(&s), 6000),
            std::slice::from_ref(&s),
            6000,
            false,
            true,
            true,
            5
        )
        .desired
    );
}
#[test]
fn aggregate_is_server_scoped_and_unknown_is_not_idle() {
    let a = server("working", 0);
    let b = server("working", 0);
    assert_eq!(view(&[a.clone(), b], 0).working_agents_observed, Some(2));
    let c = server("future_status", 0);
    let o = view(&[a, c], 0);
    assert_eq!(o.work, Work::Working);
    assert!(!o.complete);
    assert_eq!(view(&[server("blocked", 0)], 0).work, Work::None);
    assert_eq!(view(&[server("unknown", 0)], 0).work, Work::Unknown);
    assert_eq!(view(&[server("working", 0)], 6001).work, Work::Unknown);
}
#[test]
fn conflicting_projection_is_atomic_and_valid_duplicate_counts_once() {
    let mut s = server("idle", 0);
    let a = Agent {
        terminal_id: "t".into(),
        agent_status: "working".into(),
    };
    s.commit(vec![a.clone(), a.clone()], 1).unwrap();
    assert_eq!(
        view(std::slice::from_ref(&s), 1).working_agents_observed,
        Some(1)
    );
    let mut b = a.clone();
    b.agent_status = "idle".into();
    assert!(s.commit(vec![a, b], 2).is_err());
    assert_eq!(s.snapshot_at, Some(1));
}
#[test]
fn pause_and_retained_eligibility_cannot_authorize_acquisition() {
    let mut p = Policy::default();
    let s = server("working", 0);
    let o = view(std::slice::from_ref(&s), 0);
    assert_eq!(
        p.evaluate(&o, std::slice::from_ref(&s), 0, true, true, true, 5)
            .reason,
        "paused"
    );
    assert!(!p.evaluate(&o, &[s], 0, false, false, false, 5).desired);
}
proptest::proptest! {
    #[test]
    fn unknown_without_ownership_never_acquires(now in 0u64..1_000_000) {
        let mut p=Policy::default(); let mut s=server("working",0); s.fail();
        let d=p.evaluate(&view(std::slice::from_ref(&s),now), &[s],now,false,true,false,5);
        proptest::prop_assert!(!d.desired);
    }
}

#[test]
fn unrelated_completion_cannot_extend_an_unknown_servers_retention() {
    for (resume_at, release_secs) in [(30_000, 5), (120_000, 5), (30_000, 60), (120_000, 60)] {
        let mut p = Policy::default();
        let mut a = server("working", 0);
        let mut b = server("working", 0);
        p.evaluate(
            &view(&[a.clone(), b.clone()], 0),
            &[a.clone(), b.clone()],
            0,
            false,
            true,
            true,
            5,
        );
        a.commit(vec![], 1000).unwrap();
        b.fail();
        assert!(
            p.evaluate(
                &view(&[a.clone(), b.clone()], 1000),
                &[a.clone(), b.clone()],
                1000,
                false,
                true,
                true,
                release_secs
            )
            .desired
        );
        let d = p.evaluate(
            &view(&[a.clone(), b.clone()], resume_at),
            &[a, b],
            resume_at,
            false,
            true,
            true,
            release_secs,
        );
        assert!(!d.desired);
        assert_eq!(d.release_in_ms, None);
    }
}

#[test]
fn expired_unknown_in_same_server_cannot_hide_confirmed_last_work_end() {
    let agents = |status: &str| {
        vec![
            Agent {
                terminal_id: "a".into(),
                agent_status: status.into(),
            },
            Agent {
                terminal_id: "b".into(),
                agent_status: "unknown".into(),
            },
        ]
    };
    let mut s = ServerObservation::default();
    s.commit(
        vec![
            Agent {
                terminal_id: "a".into(),
                agent_status: "working".into(),
            },
            Agent {
                terminal_id: "b".into(),
                agent_status: "working".into(),
            },
        ],
        0,
    )
    .unwrap();
    let mut p = Policy::default();
    p.evaluate(
        &view(std::slice::from_ref(&s), 0),
        std::slice::from_ref(&s),
        0,
        false,
        true,
        true,
        5,
    );
    for now in (2000..=40000).step_by(2000) {
        s.commit(agents("working"), now).unwrap();
        p.evaluate(
            &view(std::slice::from_ref(&s), now),
            std::slice::from_ref(&s),
            now,
            false,
            true,
            true,
            5,
        );
    }
    s.commit(agents("idle"), 41000).unwrap();
    let d = p.evaluate(
        &view(std::slice::from_ref(&s), 41000),
        &[s],
        41000,
        false,
        true,
        true,
        5,
    );
    assert_eq!(d.reason, "release_grace");
    assert_eq!(d.release_in_ms, Some(5000));
}
