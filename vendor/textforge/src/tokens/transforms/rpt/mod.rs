#[cfg(feature = "test_access")]
pub mod test;

use std::borrow::Cow;

use crate::{
    context::execution_context::GlobalExecutionContext,
    tokens::InstructionMethods,
    utils::{
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_vec_len,
    },
};

use crate::parser::params::TextForgeParamTypes;

/// RPT - Repeat
///
/// Repeats `input` n `times`
///
/// # Example
///
/// ```rust
/// use textforge::tokens::{InstructionMethods, transforms::rpt::Rpt};
///
/// let token = Rpt::new(3);
///
/// assert_eq!(token.transform("banana".into(),None).unwrap().to_string(), "bananabananabanana");
///
/// ```
#[derive(Clone, Default)]
pub struct Rpt {
    pub times: usize,
    params: Vec<TextForgeParamTypes>,
}

impl Rpt {
    pub fn new(times: usize) -> Self {
        Rpt {
            times,
            params: vec![times.into()],
        }
    }
}

impl InstructionMethods for Rpt {
    fn supports_preparation(&self) -> bool {
        true
    }

    #[cfg(feature = "buffered")]
    fn transform_buffered(
        &self,
        input: crate::api::TextBuffer,
        _context: Option<&mut GlobalExecutionContext>,
    ) -> Result<crate::api::TextBuffer, TextForgeError> {
        input.repeat(self.times)
    }

    fn get_params(&self) -> &Vec<TextForgeParamTypes> {
        &self.params
    }
    fn to_textforge_line(&self) -> Cow<'static, str> {
        format!("rpt {};\n", self.times).into()
    }

    fn transform<'a>(
        &self,
        input: Cow<'a, str>,
        _: Option<&mut GlobalExecutionContext>,
    ) -> Result<Cow<'a, str>, TextForgeError> {
        if input.is_empty() || self.times == 0 {
            return Ok(Cow::Borrowed(""));
        }
        if self.times == 1 {
            return Ok(input);
        }
        let output_len = input.len().checked_mul(self.times).ok_or_else(|| {
            TextForgeError::new(
                TextForgeErrorCode::ValidationError("Repeated output size overflow".into()),
                "rpt",
                format!("input_bytes={}, times={}", input.len(), self.times),
            )
        })?;
        let reserve = |output: &mut String, additional| {
            output.try_reserve_exact(additional).map_err(|error| {
                TextForgeError::new(
                    TextForgeErrorCode::ValidationError("Cannot allocate repeated output".into()),
                    "rpt",
                    error.to_string(),
                )
            })
        };
        let mut output = match input {
            Cow::Owned(mut output) => {
                let additional = output_len - output.len();
                reserve(&mut output, additional)?;
                output
            }
            Cow::Borrowed(text) => {
                let mut output = String::new();
                reserve(&mut output, output_len)?;
                output.push_str(text);
                output
            }
        };
        // Fill the reserved allocation by doubling complete copies. Every prefix
        // below ends on an input boundary, including for multibyte Unicode.
        while output.len() <= output_len / 2 {
            output.extend_from_within(..);
        }
        let remaining = output_len - output.len();
        output.extend_from_within(..remaining);
        Ok(output.into())
    }

    fn get_string_repr(&self) -> &'static str {
        "rpt"
    }
    fn set_params(&mut self, params: &[TextForgeParamTypes]) -> Result<(), TextForgeError> {
        use crate::parse_args;

        check_vec_len(params, 1, "rpt", "")?;

        self.times = parse_args!(params, 0, Usize, "Index should be of usize type");
        self.params = vec![self.times.into()];

        Ok(())
    }
    #[cfg(feature = "bytecode")]
    fn get_opcode(&self) -> u32 {
        0x0d
    }
    #[cfg(feature = "bytecode")]
    fn to_bytecode(&self) -> Result<Vec<u8>, TextForgeError> {
        use crate::to_bytecode;
        let result: Vec<u8> =
            to_bytecode!(self.get_opcode(), [TextForgeParamTypes::Usize(self.times),]);
        Ok(result)
    }
}
