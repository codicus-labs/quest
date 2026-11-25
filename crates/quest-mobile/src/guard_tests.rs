use super::*;

#[test]
fn a_panic_becomes_an_internal_error_with_its_message() {
    let formatted: Result<(), _> = guard(|| panic!("boom {}", 42));
    assert_eq!(formatted, Err(MobileError::Internal { detail: "boom 42".into() }));
    let literal: Result<(), _> = guard(|| panic!("static"));
    assert_eq!(literal, Err(MobileError::Internal { detail: "static".into() }));
}

#[test]
fn results_and_errors_pass_through() {
    assert_eq!(guard(|| Ok(7)), Ok(7));
    let refused = MobileError::InvalidId { value: "x".into() };
    assert_eq!(guard::<()>(|| Err(refused.clone())), Err(refused));
}
