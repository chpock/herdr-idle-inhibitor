use herdr_idle_inhibitor::backend::{APP_ID, REASON, kde_state};
#[test]
fn kde_cookie_is_not_activation_and_suppression_is_respected() {
    let mut confirmed = false;
    assert_eq!(kde_state(&[], &[], &mut confirmed).unwrap(), "pending");
    assert!(!confirmed);
    let row = |flags| {
        (
            "sleep".into(),
            APP_ID.into(),
            REASON.into(),
            "block".into(),
            flags,
        )
    };
    assert_eq!(
        kde_state(&[row(2)], &[], &mut confirmed).unwrap(),
        "pending"
    );
    assert_eq!(
        kde_state(&[row(3)], &[row(3)], &mut confirmed).unwrap(),
        "accepted"
    );
    assert_eq!(
        kde_state(&[row(0)], &[], &mut confirmed).unwrap(),
        "suppressed"
    );
    assert!(kde_state(&[], &[], &mut confirmed).is_err());
}
