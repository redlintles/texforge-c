//! Tests for token `Atb`.
//!
//! Parent module has: `#[cfg(feature = "test_access")] pub mod test;`

#[cfg(test)]
mod common {
    use crate::{
        context::execution_context::GlobalExecutionContext,
        parser::params::TextForgeParamTypes,
        tokens::{InstructionMethods, transforms::atb::Atb},
        utils::errors::TextForgeErrorCode,
    };

    #[test]
    fn params_sets_text() {
        let t = Atb::new("foo");
        assert_eq!(t.text, "foo");
    }

    #[test]
    fn transform_prepends_text() {
        let mut ctx = GlobalExecutionContext::new();
        let t = Atb::new("foo");
        assert_eq!(
            t.transform(" bar".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "foo bar"
        );
    }

    #[test]
    fn transform_empty_input() {
        let mut ctx = GlobalExecutionContext::new();

        let t = Atb::new("foo");
        assert_eq!(
            t.transform("".into(), Some(&mut ctx)).unwrap().to_string(),
            "foo"
        );
    }

    #[test]
    fn transform_empty_prefix() {
        let mut ctx = GlobalExecutionContext::new();

        let t = Atb::new("");
        assert_eq!(
            t.transform("bar".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "bar"
        );
    }

    #[test]
    fn to_textforge_line_exact_format() {
        let t = Atb::new("foo");
        let line = t.to_textforge_line();
        assert_eq!(line.as_ref(), "atb foo;\n");
    }

    #[test]
    fn get_string_repr_is_atb() {
        let t = Atb::default();
        assert_eq!(t.get_string_repr(), "atb");
    }
    #[test]
    fn atb_set_params_accepts_single_string_param() {
        let mut t = Atb::default();
        let mut ctx = GlobalExecutionContext::new();

        let params = vec![TextForgeParamTypes::String("foo".to_string())];

        t.set_params(&params).unwrap();
        assert_eq!(t.text, "foo");

        assert_eq!(
            t.transform(" bar".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "foo bar"
        );
    }

    #[test]
    fn atb_set_params_rejects_wrong_param_count() {
        let mut t = Atb::default();
        let params = vec![
            TextForgeParamTypes::String("a".to_string()),
            TextForgeParamTypes::String("b".to_string()),
        ];

        let err = t.set_params(&params).unwrap_err();

        assert!(matches!(
            err.error_code,
            TextForgeErrorCode::InvalidArgumentNumber(_)
        ));
    }

    #[test]
    fn atb_to_bytecode_can_be_parsed_into_params_and_feed_set_params() {
        // Fluxo real:
        // Atb::to_bytecode -> extrair param -> TextForgeParamTypes::from_bytecode(param bytes) -> Atb::set_params
        //
        // Layout:
        // [u64 instruction_total_size][u32 opcode][u8 param_count]
        // [u64 param_total_size][u32 param_type][u32 payload_size][payload]
        //
        // E TextForgeParamTypes::from_bytecode espera:
        // [u32 type][u32 payload_size][payload]
        //
        // Então pulamos o u64 param_total_size.
        let mut ctx = GlobalExecutionContext::new();

        let original = Atb::new("hello");
        let bytes = original.to_bytecode().unwrap();

        // sanity: param_count deve ser 1
        assert_eq!(bytes[12], 1);

        // pulo: 8 (total) + 4 (opcode) + 1 (param_count) = 13
        let mut idx = 13;

        // lê u64 param_total_size, mas não usa (8 bytes)
        let _param_total_size = u64::from_be_bytes(bytes[idx..idx + 8].try_into().unwrap());
        idx += 8;

        // Agora o sub-slice começa em [u32 param_type][u32 payload_size][payload...]
        let param_slice = bytes[idx..].to_vec();

        let parsed_param = TextForgeParamTypes::from_bytecode(param_slice).unwrap();

        let mut rebuilt = Atb::default();
        rebuilt.set_params(&[parsed_param]).unwrap();

        assert_eq!(rebuilt.text, "hello");
        assert_eq!(
            rebuilt
                .transform(" world".into(), Some(&mut ctx))
                .unwrap()
                .to_string(),
            "hello world"
        );
    }
}

#[cfg(all(test, feature = "bytecode"))]
mod bytecode {
    use crate::{
        context::execution_context::GlobalExecutionContext,
        parser::params::TextForgeParamTypes,
        tokens::{InstructionMethods, transforms::atb::Atb},
        utils::errors::TextForgeError,
    };

    // Helper: encode a param in the exact format that TextForgeParamTypes::from_bytecode expects:
    // [type: u32][payload_size: u32][payload: bytes]
    fn encode_param_for_decoder(param_type: u32, payload: &[u8]) -> Vec<u8> {
        let mut v = Vec::with_capacity(4 + 4 + payload.len());
        v.extend_from_slice(&param_type.to_be_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        v.extend_from_slice(payload);
        v
    }

    #[test]
    fn opcode_is_expected() {
        let t = Atb::default();
        assert_eq!(t.get_opcode(), 0x01);
    }

    #[test]
    fn textforgeparam_from_bytecode_string_roundtrip_decoder_format() {
        let raw = encode_param_for_decoder(0x01, b"abc");
        let parsed = TextForgeParamTypes::from_bytecode(raw).unwrap();

        match parsed {
            TextForgeParamTypes::String(s) => assert_eq!(s, "abc"),
            other => panic!(
                "Expected String, got type code {}",
                other.get_param_type_code()
            ),
        }
    }

    #[test]
    fn textforgeparam_from_bytecode_usize_decoder_format() {
        let n: usize = 123;
        let raw = encode_param_for_decoder(0x02, &n.to_be_bytes());
        let parsed = TextForgeParamTypes::from_bytecode(raw).unwrap();

        match parsed {
            TextForgeParamTypes::Usize(x) => assert_eq!(x, 123),
            other => panic!(
                "Expected Usize, got type code {}",
                other.get_param_type_code()
            ),
        }
    }

    #[test]
    fn textforgeparam_param_to_bytecode_has_internal_structure() -> Result<(), TextForgeError> {
        let p = TextForgeParamTypes::String("abc".to_string());
        let mut dummy_context = GlobalExecutionContext::new();
        let (total_u64, b) = p.param_to_bytecode(&mut dummy_context)?;

        // total u64 (8)
        let total = u64::from_be_bytes(b[0..8].try_into().unwrap());
        assert_eq!(total, total_u64);
        assert_eq!(total as usize, b.len());

        // type u32 (4)
        let ty = u32::from_be_bytes(b[8..12].try_into().unwrap());
        assert_eq!(ty, 0x01);

        // payload_size u32 (4)
        let size = u32::from_be_bytes(b[12..16].try_into().unwrap());
        assert_eq!(size, 3);

        // payload
        assert_eq!(&b[16..19], b"abc");

        Ok(())
    }
}
