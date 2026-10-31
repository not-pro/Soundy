use super::*;

#[test]
#[ignore = "run from a signed test app bundle with a unique bundle identifier"]
fn login_item_registers_and_unregisters() {
    assert!(
        matches!(status(), 0 | 3),
        "the test bundle must start unregistered"
    );
    let registration = set_enabled(true);
    let registered_status = status();
    // Unregister before asserting so a failed check leaves no enabled test item.
    set_enabled(false).expect("unregister the test login item");
    registration.expect("register the signed test bundle");
    assert!(matches!(registered_status, ENABLED | REQUIRES_APPROVAL));
    assert_eq!(status(), 0);
}
