use super::*;
use crate::api::processor::{TextForgeProcessor, TextForgeProcessorMethods};
use crate::context::execution_context::{VarEntry, VarValues};
use crate::tokens::transforms::{atb::Atb, ate::Ate};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// Counts parameter binding while executing an ordinary prefix transform.
#[derive(Clone)]
struct CountingPrefix {
    inner: Atb,
    binds: Arc<AtomicUsize>,
    opt_in: bool,
}
impl InstructionMethods for CountingPrefix {
    fn supports_preparation(&self) -> bool {
        self.opt_in
    }
    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        self.inner.get_params()
    }
    fn get_string_repr(&self) -> &'static str {
        "atb"
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        self.inner.to_textforge_line()
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        self.binds.fetch_add(1, Ordering::SeqCst);
        self.inner.set_params(params)
    }
    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        context: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        self.inner.transform(input, context)
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        self.inner.get_opcode()
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        self.inner.to_bytecode()
    }
}

#[test]
fn literal_binding_happens_once_but_custom_instructions_remain_opt_in() {
    for opt_in in [true, false] {
        let binds = Arc::new(AtomicUsize::new(0));
        let token = CountingPrefix {
            inner: Atb::default(),
            binds: Arc::clone(&binds),
            opt_in,
        };
        let wrapper = TokenWrapper::new(
            Box::new(token),
            Some(vec![ValType::Literal("!".to_string().into())]),
        );
        let mut processor = TextForgeProcessor::new();
        let id = processor.add_transform(vec![wrapper]);
        assert_eq!(binds.load(Ordering::SeqCst), usize::from(opt_in));
        for _ in 0..3 {
            assert_eq!(
                processor.run_transform(&id, Cow::Borrowed("x")).unwrap(),
                "!x"
            );
        }
        assert_eq!(binds.load(Ordering::SeqCst), if opt_in { 1 } else { 3 });
    }
}

#[test]
fn exported_clones_drop_cached_binding_before_public_template_mutation() {
    let mut processor = TextForgeProcessor::new();
    let id = processor.add_transform(vec![TokenWrapper::new(
        Box::new(Atb::default()),
        Some(vec![ValType::Literal("!".to_string().into())]),
    )]);
    let mut exported = processor.get_transform_vec(&id).unwrap().remove(0);
    assert!(exported.prepared.is_none());
    exported.token = Box::new(Ate::default());
    assert_eq!(
        exported
            .apply_token(Cow::Borrowed("x"), &mut GlobalExecutionContext::new())
            .unwrap(),
        "x!"
    );
    assert_eq!(
        processor.run_transform(&id, Cow::Borrowed("x")).unwrap(),
        "!x"
    );
}

#[test]
fn variables_and_expressions_are_resolved_against_each_current_context() {
    for (source, expected) in [
        ("atb \"{{name}}\";", ["onex", "twox"]),
        ("atb \"[[ n + 1 ]]\";", ["3x", "10x"]),
        ("raw \"{{pattern}}\" X;", ["Xbc", "aXc"]),
    ] {
        let mut token = crate::text::reader::read_from_text(source).unwrap();
        token.prepare_constant();
        assert!(token.prepared.is_none());
        let mut context = GlobalExecutionContext::new();
        for (i, (name, n, pattern)) in [("one", 2, "a"), ("two", 9, "b")].into_iter().enumerate() {
            for (key, value) in [
                ("name", VarValues::String(name.into())),
                ("n", VarValues::Usize(n)),
                ("pattern", VarValues::String(pattern.into())),
            ] {
                context
                    .add_var(
                        key,
                        VarEntry {
                            value,
                            mutable: true,
                        },
                    )
                    .unwrap();
            }
            let input = if source.starts_with("raw") {
                "abc"
            } else {
                "x"
            };
            assert_eq!(
                token
                    .apply_token(Cow::Borrowed(input), &mut context)
                    .unwrap(),
                expected[i]
            );
        }
    }
}

#[test]
fn invalid_parameters_still_fail_only_when_the_instruction_executes() {
    let mut processor = TextForgeProcessor::new();
    let invalid = crate::text::reader::read_from_text("raw \"[\" X;").unwrap();
    let id = processor.add_transform(vec![
        crate::text::reader::read_from_text("end;").unwrap(),
        invalid.clone(),
    ]);
    assert_eq!(
        processor.run_transform(&id, Cow::Borrowed("abc")).unwrap(),
        "abc"
    );
    let id = processor.add_transform(vec![invalid]);
    assert!(processor.run_transform(&id, Cow::Borrowed("abc")).is_err());
}
