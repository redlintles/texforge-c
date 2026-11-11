// src/tokens/transforms/rmws/test.rs
#![cfg(feature = "test_access")]
#[cfg(test)]
mod tests {
    use crate::{
        context::execution_context::GlobalExecutionContext,
        parser::params::TextForgeParamTypes,
        tokens::{InstructionMethods, transforms::rmws::Rmws},
    };

    #[test]
    fn rmws_get_string_repr_ok() {
        let t = Rmws::default();
        assert_eq!(t.get_string_repr(), "rmws");
    }

    #[test]
    fn rmws_to_textforge_line_ok() {
        let t = Rmws::default();
        assert_eq!(t.to_textforge_line().as_ref(), "rmws;\n");
    }

    #[test]
    fn rmws_transform_basic_ok() {
        let t = Rmws::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("banana laranja cheia de canja".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bananalaranjacheiadecanja"
        );
    }

    #[test]
    fn rmws_transform_preserves_non_whitespace_ok() {
        let t = Rmws::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("  a\tb\nc\r\nd  ".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "abcd"
        );
    }

    #[test]
    fn rmws_transform_empty_ok() {
        let t = Rmws::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            ""
        );
    }

    #[test]
    fn rmws_transform_only_whitespace_ok() {
        let t = Rmws::default();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform(" \t\n\r  ".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            ""
        );
    }

    #[test]
    fn rmws_transform_unicode_whitespace_ok() {
        // split_whitespace cobre vários espaços unicode.
        // Ex.: NBSP (\u{00A0}) e EM SPACE (\u{2003}) podem variar por versão,
        // então uso um que costuma ser reconhecido (EM SPACE).
        let t = Rmws::default();
        let input = "a\u{2003}b\u{2003}c".to_string();
        let mut ctx = GlobalExecutionContext::new();

        assert_eq!(
            t.transform(input.into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "abc"
        );
    }

    #[test]
    fn rmws_set_params_ok_empty() {
        let mut t = Rmws::default();
        let v: Vec<TextForgeParamTypes> = vec![];
        assert!(t.set_params(&v).is_ok());
    }

    #[test]
    fn rmws_set_params_err_when_not_empty() {
        let mut t = Rmws::default();
        let v: Vec<TextForgeParamTypes> = vec![TextForgeParamTypes::Usize(0)];
        assert!(t.set_params(&v).is_err());
    }
    #[cfg(feature = "bytecode")]
    mod bytecode {
        use super::*;

        #[test]
        fn rmws_opcode_ok() {
            let t = Rmws::default();
            assert_eq!(t.get_opcode(), 0x31);
        }

        #[test]
        fn rmws_to_bytecode_non_empty_and_no_params() {
            let t = Rmws::default();
            let bc = t.to_bytecode().unwrap();

            // header mínimo: 8 + 4 + 1 = 13 bytes
            assert!(bc.len() >= 13);

            let mut i = 0;

            let total_size = u64::from_be_bytes(bc[i..i + 8].try_into().unwrap());
            i += 8;

            // total_size = tamanho do "body" (opcode+count+params...)
            assert_eq!(total_size as usize, bc.len() - 8);

            let opcode = u32::from_be_bytes(bc[i..i + 4].try_into().unwrap());
            i += 4;
            assert_eq!(opcode, 0x31);

            let param_count = bc[i] as usize;
            assert_eq!(param_count, 0);
        }
    }
}
