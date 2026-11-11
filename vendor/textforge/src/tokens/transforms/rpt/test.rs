#![cfg(feature = "test_access")]
#[cfg(test)]
mod tests {
    use crate::context::execution_context::GlobalExecutionContext;
    use crate::parser::params::TextForgeParamTypes;
    use crate::tokens::{InstructionMethods, transforms::rpt::Rpt};
    use crate::utils::errors::TextForgeErrorCode;

    #[test]
    fn rejects_output_size_overflow_without_panicking() {
        let error = Rpt::new(usize::MAX)
            .transform("ab".into(), None)
            .unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::ValidationError(_)
        ));
        assert_eq!(error.instruction, "rpt");
    }

    #[test]
    fn rejects_capacity_overflow_without_panicking() {
        let error = Rpt::new(usize::MAX)
            .transform("x".into(), None)
            .unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::ValidationError(_)
        ));
    }

    #[test]
    fn huge_repeat_of_empty_input_stays_empty() {
        assert_eq!(Rpt::new(usize::MAX).transform("".into(), None).unwrap(), "");
    }

    #[test]
    fn fallible_repetition_matches_standard_repeat_for_unicode() {
        for input in ["", "a", "é", "é", "👍🏽", "👨‍👩‍👧‍👦", "🇧🇷 x"] {
            for times in 0..65 {
                assert_eq!(
                    Rpt::new(times).transform(input.into(), None).unwrap(),
                    input.repeat(times),
                    "input={input:?}, times={times}",
                );
            }
        }
    }

    #[test]
    fn get_string_repr_is_rpt() {
        let t = Rpt::default();
        assert_eq!(t.get_string_repr(), "rpt");
    }

    #[test]
    fn to_textforge_line_contains_times() {
        let t = Rpt::new(3);
        assert_eq!(t.to_textforge_line().as_ref(), "rpt 3;\n");
    }

    #[test]
    fn transform_repeats_input_n_times() {
        let t = Rpt::new(3);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bananabananabanana"
        );
    }

    #[test]
    fn transform_zero_times_returns_empty_string() {
        let t = Rpt::new(0);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            ""
        );
    }

    #[test]
    fn transform_empty_input_still_empty() {
        let t = Rpt::new(5);
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn set_params_parses_single_usize() {
        let mut t = Rpt::default();

        let params = vec![TextForgeParamTypes::Usize(5)];

        assert_eq!(t.set_params(&params), Ok(()));
        assert_eq!(t.times, 5);
    }

    #[test]
    fn set_params_rejects_wrong_param_count() {
        let mut t = Rpt::default();

        let params = vec![];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn set_params_rejects_wrong_type() {
        let mut t = Rpt::default();

        let params = vec![TextForgeParamTypes::String("x".to_string())];

        assert!(t.set_params(&params).is_err());
    }

    // ============================
    // Bytecode tests
    // ============================
    #[cfg(feature = "bytecode")]
    mod bytecode_tests {
        use super::*;

        #[test]
        fn get_opcode_is_0x0d() {
            let t = Rpt::default();
            assert_eq!(t.get_opcode(), 0x0d);
        }

        #[test]
        fn to_bytecode_contains_opcode_and_one_param() {
            let t = Rpt::new(3);
            let bc = t.to_bytecode().unwrap();

            assert!(!bc.is_empty());
            assert!(bc.len() >= 13); // header mínimo

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x0d);

            let param_count = bc[i] as usize;
            i += 1;
            assert_eq!(param_count, 1);

            // Param 1: Usize
            let _p_total = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;

            let p_type = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(p_type, 0x02);

            let p_size = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap()) as usize;
            i += 4;
            assert_eq!(p_size, 8);

            let payload = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            assert_eq!(payload, 3);
        }
    }
}
