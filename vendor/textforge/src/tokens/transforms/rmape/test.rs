use super::*;
#[test]
fn missing_context_and_missing_maps_return_errors() {
    let mut token = Rmape::default();
    assert!(
        token
            .transform(
                "".to_string().into(),
                Some(&mut GlobalExecutionContext::new())
            )
            .is_err()
    );
    token.set_params(&["missing".to_string().into()]).unwrap();
    assert!(token.transform("x".to_string().into(), None).is_err());
    assert!(
        token
            .transform(
                "x".to_string().into(),
                Some(&mut GlobalExecutionContext::new())
            )
            .is_err()
    );
    assert!(token.set_params(&[]).is_err());
    assert!(token.set_params(&[1usize.into()]).is_err());
}
