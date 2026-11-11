//! Typed arguments and owned instruction descriptions.

use crate::{
    error::{Failure, TfError, TfStatus},
    ffi::{self, Out, TF_NTS},
    text::CText,
};
use std::{ffi::c_char, ptr};
use textforge::api::portable::{Argument, InstructionSpec};

/// Kind tag of a [`TfArg`]. Zero is deliberately invalid so a zero-initialized
/// argument is rejected instead of being misread.
pub type TfArgKind = u32;
/// Literal text (`text`, `length`). The engine applies its usual `{{var}}` and
/// `[[expr]]` interpolation.
pub const TF_ARG_TEXT: TfArgKind = 1;
/// Explicit reference to a context variable named by `text`, `length`.
pub const TF_ARG_VARIABLE: TfArgKind = 2;
/// Unsigned number (`number`). Rejected if it exceeds the native `size_t`.
pub const TF_ARG_NUMBER: TfArgKind = 3;
/// Nested instruction (`instruction`), e.g. the body of `iter`.
pub const TF_ARG_INSTRUCTION: TfArgKind = 4;

/// One instruction argument, passed by value.
///
/// Build it with `tf_text`, `tf_text_n`, `tf_var`, `tf_var_n`, `tf_number` or
/// `tf_nested`. The argument only *borrows* its text and instruction: they must
/// stay alive until the function receiving the argument returns, which copies
/// everything it keeps. Arguments exclude DSL-only literals such as `assoc` or `do`.
///
/// ```c
/// TfArg args[] = {tf_text("prefix"), tf_text("> ")};
/// TfPipeline *plan = tf_pipeline_add(tf_pipeline_new(), "val", TF_NTS, args, 2);
/// tf_pipeline_add_to_beginning(plan, tf_var("prefix"));
/// TfCompiledPipeline *pipeline = NULL;
/// assert(tf_pipeline_compile(plan, &pipeline, NULL) == TF_STATUS_OK);
/// TfString out = {0};
/// assert(tf_compiled_pipeline_process(pipeline, "quote", TF_NTS, &out, NULL) == TF_STATUS_OK);
/// assert(strcmp(out.data, "> quote") == 0);
/// tf_string_free(&out);
/// tf_compiled_pipeline_free(pipeline);
/// tf_pipeline_free(plan);
/// ```
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TfArg {
    /// One of the `TF_ARG_*` constants.
    pub kind: TfArgKind,
    /// UTF-8 text for `TF_ARG_TEXT` / `TF_ARG_VARIABLE`.
    pub text: *const c_char,
    /// Byte length of `text`, or `TF_NTS` for a NUL-terminated string.
    pub length: usize,
    /// Value for `TF_ARG_NUMBER`.
    pub number: u64,
    /// Borrowed instruction for `TF_ARG_INSTRUCTION`.
    pub instruction: *const TfInstruction,
}

const EMPTY: TfArg = TfArg {
    kind: 0,
    text: ptr::null(),
    length: 0,
    number: 0,
    instruction: ptr::null(),
};

/// Literal text from a NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub extern "C" fn tf_text(text: *const c_char) -> TfArg {
    tf_text_n(text, TF_NTS)
}

/// Literal text with an explicit byte length (or `TF_NTS`).
#[unsafe(no_mangle)]
pub extern "C" fn tf_text_n(text: *const c_char, length: usize) -> TfArg {
    TfArg {
        kind: TF_ARG_TEXT,
        text,
        length,
        ..EMPTY
    }
}

/// Reference to a context variable, from a NUL-terminated name.
#[unsafe(no_mangle)]
pub extern "C" fn tf_var(name: *const c_char) -> TfArg {
    tf_var_n(name, TF_NTS)
}

/// Reference to a context variable with an explicit name length (or `TF_NTS`).
#[unsafe(no_mangle)]
pub extern "C" fn tf_var_n(name: *const c_char, length: usize) -> TfArg {
    TfArg {
        kind: TF_ARG_VARIABLE,
        text: name,
        length,
        ..EMPTY
    }
}

/// Unsigned number.
#[unsafe(no_mangle)]
pub extern "C" fn tf_number(value: u64) -> TfArg {
    TfArg {
        kind: TF_ARG_NUMBER,
        number: value,
        ..EMPTY
    }
}

/// Nested instruction (borrowed for the duration of the receiving call).
#[unsafe(no_mangle)]
pub extern "C" fn tf_nested(instruction: *const TfInstruction) -> TfArg {
    TfArg {
        kind: TF_ARG_INSTRUCTION,
        instruction,
        ..EMPTY
    }
}

/// Greatest instruction nesting accepted by the adapter (matches the other
/// bindings). The core may impose a smaller, context-dependent limit.
pub(crate) const MAX_NESTING: usize = 32;

/// Convert a C argument into a portable argument.
///
/// # Safety
///
/// Pointers inside `argument` satisfy the contract documented on [`TfArg`].
pub(crate) unsafe fn convert(argument: &TfArg, what: &str) -> Result<Argument, Failure> {
    Ok(match argument.kind {
        TF_ARG_TEXT => {
            // SAFETY: forwarded TfArg contract.
            Argument::Text(unsafe { ffi::text(argument.text, argument.length, what) }?.into())
        }
        TF_ARG_VARIABLE => {
            // SAFETY: forwarded TfArg contract.
            Argument::Variable(unsafe { ffi::text(argument.text, argument.length, what) }?.into())
        }
        TF_ARG_NUMBER => Argument::Number(argument.number),
        TF_ARG_INSTRUCTION => {
            // SAFETY: forwarded TfArg contract.
            let child = unsafe { ffi::handle(argument.instruction, what) }?;
            if child.depth >= MAX_NESTING {
                return Err(Failure::invalid(format!(
                    "instruction nesting exceeds adapter limit {MAX_NESTING}"
                )));
            }
            Argument::Instruction(Box::new(child.spec.clone()))
        }
        other => return Err(Failure::invalid(format!("{what} has unknown kind {other}"))),
    })
}

/// Convert an array of C arguments.
///
/// # Safety
///
/// `arguments` is NULL with `count == 0` or points to `count` valid [`TfArg`]s.
pub(crate) unsafe fn convert_all(
    arguments: *const TfArg,
    count: usize,
) -> Result<Vec<Argument>, Failure> {
    // SAFETY: forwarded caller contract.
    let arguments = unsafe { ffi::array(arguments, count, "arguments") }?;
    arguments
        .iter()
        .enumerate()
        // SAFETY: each element obeys the TfArg contract.
        .map(|(index, argument)| unsafe { convert(argument, &format!("argument {index}")) })
        .collect()
}

/// Owned, immutable instruction description: a mnemonic plus typed arguments.
///
/// Create it with `tf_instruction_new`, inspect it with `tf_instruction_name`
/// and `tf_instruction_argument`, release it with `tf_instruction_free`.
/// Instructions are independent of any pipeline or processor and may be shared
/// between threads.
///
/// ```c
/// TfArg text = tf_text("🍌");
/// TfInstruction *prefix = NULL;
/// assert(tf_instruction_new("atb", TF_NTS, &text, 1, &prefix, NULL) == TF_STATUS_OK);
/// TfPipeline *plan = tf_pipeline_iter(tf_pipeline_new(), tf_number(2), tf_nested(prefix));
/// tf_instruction_free(prefix);  // The pipeline keeps its own copy.
/// TfCompiledPipeline *pipeline = NULL;
/// assert(tf_pipeline_compile(plan, &pipeline, NULL) == TF_STATUS_OK);
/// TfString out = {0};
/// assert(tf_compiled_pipeline_process(pipeline, "x", TF_NTS, &out, NULL) == TF_STATUS_OK);
/// assert(strcmp(out.data, "🍌🍌x") == 0);
/// tf_string_free(&out);
/// tf_compiled_pipeline_free(pipeline);
/// tf_pipeline_free(plan);
/// ```
#[derive(Clone, Debug)]
pub struct TfInstruction {
    pub(crate) spec: InstructionSpec,
    name: CText,
    arguments: Vec<View>,
    depth: usize,
}

/// Borrowable C representation of one argument.
#[derive(Clone, Debug)]
enum View {
    Text(CText),
    Variable(CText),
    Number(u64),
    Instruction(Box<TfInstruction>),
}

impl TfInstruction {
    /// Build the description plus its C-facing views.
    pub(crate) fn from_spec(spec: InstructionSpec) -> Result<Self, Failure> {
        let mut depth = 0;
        let mut arguments = Vec::with_capacity(spec.arguments.len());
        for argument in &spec.arguments {
            arguments.push(match argument {
                Argument::Text(text) => View::Text(CText::from(text.as_str())),
                Argument::Variable(name) => View::Variable(CText::from(name.as_str())),
                Argument::Number(value) => View::Number(*value),
                Argument::Instruction(child) => {
                    let child = Self::from_spec((**child).clone())?;
                    depth = depth.max(child.depth + 1);
                    View::Instruction(Box::new(child))
                }
                _ => return Err(Failure::invalid("unsupported portable argument kind")),
            });
        }
        Ok(Self {
            name: CText::from(spec.name.as_str()),
            spec,
            arguments,
            depth,
        })
    }
}

/// Create an instruction from a mnemonic (e.g. `"atb"`) and arguments.
///
/// The mnemonic and arguments are only checked for well-formed UTF-8 and
/// adapter nesting here; the core registry validates them when the instruction
/// is added to a pipeline or executed. On success `*out` receives a new
/// instruction that the caller must free.
///
/// # Safety
///
/// `name`/`length` follow the text convention; `arguments` points to `count`
/// valid arguments (or is NULL with `count == 0`); `out` is writable; `error`
/// is NULL or a writable slot holding NULL.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_instruction_new(
    name: *const c_char,
    length: usize,
    arguments: *const TfArg,
    count: usize,
    out: *mut *mut TfInstruction,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: every pointer is forwarded under the documented contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let name = ffi::text(name, length, "name")?;
            let arguments = convert_all(arguments, count)?;
            // `convert_all` already bounded the nesting of every child.
            let instruction = TfInstruction::from_spec(InstructionSpec::new(name.into(), arguments))?;
            out.write(ffi::into_handle(instruction));
            Ok(())
        })
    }
}

/// Copy an instruction. `NULL` returns `NULL`.
///
/// # Safety
///
/// `instruction` is NULL or a live instruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_instruction_clone(instruction: *const TfInstruction) -> *mut TfInstruction {
    ffi::guard(ptr::null_mut(), || {
        // SAFETY: forwarded caller contract.
        unsafe { instruction.as_ref() }.map_or(ptr::null_mut(), |i| ffi::into_handle(i.clone()))
    })
}

/// Canonical mnemonic. Valid while the instruction lives; NULL for NULL.
///
/// # Safety
///
/// `instruction` is NULL or a live instruction; `length` is NULL or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_instruction_name(
    instruction: *const TfInstruction,
    length: *mut usize,
) -> *const c_char {
    // SAFETY: forwarded caller contract.
    match unsafe { instruction.as_ref() } {
        Some(instruction) => instruction.name.export(length),
        None => CText::null(length),
    }
}

/// Number of arguments (0 for NULL).
///
/// # Safety
///
/// `instruction` is NULL or a live instruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_instruction_argument_count(instruction: *const TfInstruction) -> usize {
    // SAFETY: forwarded caller contract.
    unsafe { instruction.as_ref() }.map_or(0, |i| i.arguments.len())
}

/// Read argument `index` into `*out`.
///
/// The resulting [`TfArg`] borrows from `instruction` (its text is
/// NUL-terminated and its nested instruction is owned by `instruction`), so it
/// stays valid until `instruction` is freed and can be passed to builder
/// functions as is.
///
/// ```c
/// TfArg number = tf_number(3);
/// TfInstruction *repeat = NULL;
/// assert(tf_instruction_new("rpt", TF_NTS, &number, 1, &repeat, NULL) == TF_STATUS_OK);
/// assert(tf_instruction_argument_count(repeat) == 1);
/// TfArg first;
/// assert(tf_instruction_argument(repeat, 0, &first, NULL) == TF_STATUS_OK);
/// assert(first.kind == TF_ARG_NUMBER && first.number == 3);
/// assert(tf_instruction_argument(repeat, 1, &first, NULL) == TF_STATUS_INVALID_ARGUMENT);
/// tf_instruction_free(repeat);
/// ```
///
/// # Safety
///
/// `instruction` is a live instruction; `out` is writable; `error` follows the
/// error-slot convention.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_instruction_argument(
    instruction: *const TfInstruction,
    index: usize,
    out: *mut TfArg,
    error: *mut *mut TfError,
) -> TfStatus {
    // SAFETY: forwarded caller contract.
    unsafe {
        ffi::call(error, || {
            let out = Out::new(out, "out")?;
            let instruction = ffi::handle(instruction, "instruction")?;
            let view = instruction.arguments.get(index).ok_or_else(|| {
                Failure::invalid(format!(
                    "argument index {index} out of range ({} arguments)",
                    instruction.arguments.len()
                ))
            })?;
            let mut length = 0;
            out.write(match view {
                View::Text(text) => tf_text_n(text.export(&mut length), length),
                View::Variable(name) => tf_var_n(name.export(&mut length), length),
                View::Number(value) => tf_number(*value),
                View::Instruction(child) => tf_nested(&**child),
            });
            Ok(())
        })
    }
}

/// Release an instruction. `NULL` is ignored.
///
/// # Safety
///
/// `instruction` is NULL or a live instruction not used afterwards (including
/// argument views borrowed from it).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_instruction_free(instruction: *mut TfInstruction) {
    // SAFETY: forwarded caller contract.
    ffi::guard((), || drop(unsafe { ffi::take_handle(instruction) }));
}

/// Owned array of instructions. Release with `tf_instruction_list_free`, which
/// also frees every item.
///
/// ```c
/// TfProcessor *processor = tf_processor_new();
/// TfPipeline *plan = tf_pipeline_trim_both_sides(tf_pipeline_new());
/// TfString id = {0};
/// assert(tf_pipeline_build(plan, processor, &id, NULL) == TF_STATUS_OK);
/// TfInstructionList list = {0};
/// assert(tf_processor_get_transform_vec(processor, id.data, id.length, &list, NULL) == TF_STATUS_OK);
/// assert(list.length == 1 && strcmp(tf_instruction_name(list.items[0], NULL), "tbs") == 0);
/// tf_instruction_list_free(&list);
/// tf_string_free(&id);
/// tf_pipeline_free(plan);
/// tf_processor_free(processor);
/// ```
#[repr(C)]
#[derive(Debug)]
pub struct TfInstructionList {
    /// `length` owned instructions; NULL after release.
    pub items: *mut *mut TfInstruction,
    /// Number of instructions.
    pub length: usize,
}

impl From<Vec<TfInstruction>> for TfInstructionList {
    fn from(instructions: Vec<TfInstruction>) -> Self {
        let items: Box<[*mut TfInstruction]> = instructions.into_iter().map(ffi::into_handle).collect();
        let length = items.len();
        Self {
            items: Box::into_raw(items).cast(),
            length,
        }
    }
}

/// Release a list and every instruction in it, then reset it. `NULL` is ignored.
///
/// # Safety
///
/// `list` is NULL or points to a list returned by this library.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tf_instruction_list_free(list: *mut TfInstructionList) {
    // SAFETY: forwarded caller contract.
    let Some(list) = (unsafe { list.as_mut() }) else {
        return;
    };
    if !list.items.is_null() {
        // SAFETY: reconstructs the boxed slice created in `From<Vec<_>>`.
        let items = unsafe { Box::from_raw(ptr::slice_from_raw_parts_mut(list.items, list.length)) };
        for item in items {
            // SAFETY: each item is an owned instruction.
            unsafe { tf_instruction_free(item) };
        }
    }
    list.items = ptr::null_mut();
    list.length = 0;
}
