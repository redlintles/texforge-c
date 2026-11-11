use super::*;
#[test]
fn cache_is_shared_and_updates_are_atomic() {
    let mut token = Rmapd::default();
    let params = vec![
        "m".to_string().into(),
        "cat".to_string().into(),
        "dog".to_string().into(),
    ];
    token.set_params(&params).unwrap();
    let first = token.compiled.clone().unwrap();
    let mut clone = token.clone();
    clone.set_params(&params).unwrap();
    assert!(std::sync::Arc::ptr_eq(
        &first,
        clone.compiled.as_ref().unwrap()
    ));
    assert!(
        token
            .set_params(&[
                "m".to_string().into(),
                "[".to_string().into(),
                "x".to_string().into()
            ])
            .is_err()
    );
    assert_eq!(format!("{:?}", token.params), format!("{params:?}"));
    assert!(token.transform("cat".to_string().into(), None).is_err());
    for n in 0..100 {
        clone
            .set_params(&[
                "m".to_string().into(),
                format!("p{n}").into(),
                "x".to_string().into(),
            ])
            .unwrap();
    }
    assert!(token.cache.lock().unwrap().len() <= 64);
}
#[test]
fn unbound_and_invalid_parameters_return_errors() {
    let mut token = Rmapd::default();
    assert!(
        token
            .transform(
                "".to_string().into(),
                Some(&mut GlobalExecutionContext::new())
            )
            .is_err()
    );
    assert!(token.set_params(&[]).is_err());
    assert!(
        token
            .set_params(&[
                1usize.into(),
                "x".to_string().into(),
                "y".to_string().into()
            ])
            .is_err()
    );
}
