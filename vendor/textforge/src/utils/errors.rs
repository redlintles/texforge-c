use std::borrow::Cow;
use std::error::Error;
use std::fmt::Display;

// Colored formatting (presentation only)
use colored::{Color, Colorize};

#[derive(Default, Clone)]
pub struct ErrorManager {
    panic_with_error: bool,
    show_warnings: bool,
    error_vec: Vec<TextForgeError>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextForgeError {
    pub error_code: TextForgeErrorCode,
    pub instruction: Cow<'static, str>,
    pub input: Cow<'static, str>,
}

impl Error for TextForgeError {}

impl Display for TextForgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Presentation only. No semantic changes.
        let title = "Erro".bold().red();
        let instr_label = "Instruction:".bold().cyan();
        let input_label = "Input:".bold().dimmed();

        write!(
            f,
            "{}\n{}\n{} {}\n{} {}",
            title,
            self.error_code,
            instr_label,
            self.instruction.as_ref().cyan(),
            input_label,
            self.input.as_ref().dimmed()
        )?;
        Ok(())
    }
}

impl TextForgeError {
    pub fn new<I, T>(error_code: TextForgeErrorCode, instruction: I, input: T) -> Self
    where
        I: Into<Cow<'static, str>>,
        T: Into<Cow<'static, str>>,
    {
        TextForgeError {
            error_code,
            instruction: instruction.into(),
            input: input.into(),
        }
    }

    // Cheap helpers (do not change public fields; just convenience)
    pub fn instruction_str(&self) -> &str {
        self.instruction.as_ref()
    }
    pub fn input_str(&self) -> &str {
        self.input.as_ref()
    }
}

impl ErrorManager {
    pub fn will_panic(&mut self, val: bool) {
        self.panic_with_error = val;
    }

    pub fn will_log(&mut self, val: bool) {
        self.show_warnings = val;

        // Tie "log mode" to color output by default.
        colored::control::set_override(val);
    }

    pub fn add_error(&mut self, err: TextForgeError) {
        self.error_vec.push(err);
    }

    pub fn has_errors(&self) -> bool {
        !self.error_vec.is_empty()
    }

    /// Optional: reserve capacity when you expect many errors in a big pipeline.
    pub fn reserve_errors(&mut self, additional: usize) {
        self.error_vec.reserve(additional);
    }

    /// Keeps existing API: prints to stdout.
    /// Internally uses a writer so it can be unit-tested and buffered.
    pub fn print_errors(&self) {
        use std::io::{self, BufWriter};
        let stdout = io::stdout();
        let mut w = BufWriter::new(stdout.lock());
        let _ = self.write_errors(&mut w);
    }

    /// Internal helper: write formatted errors into any writer (test-friendly).
    fn write_errors<W: std::io::Write>(&self, mut w: W) -> std::io::Result<()> {
        for (index, error) in self.error_vec.iter().enumerate() {
            writeln!(
                w,
                "[{}] {} {}\n\t{} {}\n\t{} {}\n\t{} {}",
                index.to_string().blue(),
                "Code:".bold().red(),
                error.error_code.get_error_code().to_string().red().bold(),
                "Instruction:".bold().cyan(),
                error.instruction.as_ref().cyan(),
                "Message:".bold().yellow(),
                // IMPORTANT: avoid allocating; get_message returns &Cow now.
                error.error_code.get_message().as_ref().yellow(),
                "Processing:".bold().dimmed(),
                error.input.as_ref().dimmed()
            )?;
        }
        Ok(())
    }

    /// Optional: consistent behavior when you want to both store and act on an error.
    /// Does not change any existing behavior unless you choose to use it.
    pub fn handle_error(&mut self, err: TextForgeError) {
        self.add_error(err.clone());

        if self.show_warnings {
            println!("{err}");
        }

        if self.panic_with_error {
            panic!("{err}");
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TextForgeErrorCode {
    FileNotFound(Cow<'static, str>),
    BytecodeParamNotRecognized(Cow<'static, str>),
    TokenNotFound(Cow<'static, str>),
    TokenArrayNotFound(Cow<'static, str>),
    BlockNotFound(Cow<'static, str>),
    VariableNotFound(Cow<'static, str>),
    NonMutableVariableError(Cow<'static, str>),
    FileReadingError(Cow<'static, str>),
    FileWritingError(Cow<'static, str>),
    FileOpeningError(Cow<'static, str>),
    BytecodeNotFound(Cow<'static, str>),
    TextParsingError(Cow<'static, str>),
    BytecodeParsingError(Cow<'static, str>),
    BytecodeParamParsingError(Cow<'static, str>),
    InvalidIndex(Cow<'static, str>),
    IndexOutOfRange(Cow<'static, str>),
    InvalidOperands(Cow<'static, str>),
    InvalidParameters(Cow<'static, str>),
    ValidationError(Cow<'static, str>),
    InvalidArgumentNumber(Cow<'static, str>),
    ZeroDivisionError(Cow<'static, str>),
    TryIntoFailError(Cow<'static, str>),
    IncompatibleTypeError(Cow<'static, str>),
    RequiredContextError(Cow<'static, str>),
    WatcherNotFoundError(Cow<'static, str>),
    SerializationError(Cow<'static, str>),
    GenericError(Cow<'static, str>),
    NestedBlocksNotAllowedError(Cow<'static, str>),
    InvalidUtf8Error(Cow<'static, str>),
    InvalidExprError(Cow<'static, str>),
}

impl Display for TextForgeErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use severity to color the code/message, but keep raw values available via getters.
        let severity = self.severity_color();

        let code_label = "Código:".bold().red();
        let msg_label = "Mensagem:".bold().yellow();

        let code_val = self.get_error_code().to_string().color(severity).bold();
        let msg_val = self.message().as_ref().color(severity);

        write!(f, "{} {}\n{} {}", code_label, code_val, msg_label, msg_val)?;
        Ok(())
    }
}

impl TextForgeErrorCode {
    pub fn get_error_code(&self) -> u16 {
        match self {
            Self::FileNotFound(_) => 1u16,
            Self::TokenNotFound(_) => 2u16,
            Self::TokenArrayNotFound(_) => 3u16,
            Self::FileReadingError(_) => 4u16,
            Self::FileWritingError(_) => 5u16,
            Self::FileOpeningError(_) => 6u16,
            Self::BytecodeNotFound(_) => 7u16,
            Self::BlockNotFound(_) => 8u16,
            Self::VariableNotFound(_) => 9u16,
            Self::NonMutableVariableError(_) => 10u16,
            Self::InvalidOperands(_) => 11u16,
            Self::IndexOutOfRange(_) => 12u16,
            Self::InvalidIndex(_) => 13u16,
            Self::InvalidParameters(_) => 14u16,
            Self::InvalidArgumentNumber(_) => 15u16,
            Self::BytecodeParamNotRecognized(_) => 16u16,
            Self::TextParsingError(_) => 17u16,
            Self::BytecodeParsingError(_) => 18u16,
            Self::BytecodeParamParsingError(_) => 19u16,
            Self::ValidationError(_) => 20u16,
            Self::ZeroDivisionError(_) => 21u16,
            Self::TryIntoFailError(_) => 22u16,
            Self::IncompatibleTypeError(_) => 23u16,
            Self::RequiredContextError(_) => 24u16,
            Self::WatcherNotFoundError(_) => 25u16,
            Self::SerializationError(_) => 26u16,
            Self::GenericError(_) => 27u16,
            Self::NestedBlocksNotAllowedError(_) => 28u16,
            Self::InvalidUtf8Error(_) => 29u16,
            Self::InvalidExprError(_) => 30u16,
        }
    }

    /// Kept name, but now returns borrowed data (no allocation).
    pub fn get_message(&self) -> &Cow<'static, str> {
        self.message()
    }

    /// Zero-allocation access to the inner message.
    pub fn message(&self) -> &Cow<'static, str> {
        match self {
            Self::InvalidExprError(x)
            | Self::InvalidUtf8Error(x)
            | Self::FileNotFound(x)
            | Self::IndexOutOfRange(x)
            | Self::InvalidIndex(x)
            | Self::InvalidOperands(x)
            | Self::TextParsingError(x)
            | Self::TokenNotFound(x)
            | Self::BytecodeNotFound(x)
            | Self::BytecodeParsingError(x)
            | Self::InvalidParameters(x)
            | Self::ValidationError(x)
            | Self::InvalidArgumentNumber(x)
            | Self::FileReadingError(x)
            | Self::FileWritingError(x)
            | Self::FileOpeningError(x)
            | Self::BytecodeParamParsingError(x)
            | Self::ZeroDivisionError(x)
            | Self::TokenArrayNotFound(x)
            | Self::BlockNotFound(x)
            | Self::VariableNotFound(x)
            | Self::NonMutableVariableError(x)
            | Self::TryIntoFailError(x)
            | Self::IncompatibleTypeError(x)
            | Self::BytecodeParamNotRecognized(x)
            | Self::WatcherNotFoundError(x)
            | Self::SerializationError(x)
            | Self::GenericError(x)
            | Self::NestedBlocksNotAllowedError(x)
            | Self::RequiredContextError(x) => x,
        }
    }

    fn severity_color(&self) -> Color {
        match self {
            // "Hard" errors
            Self::InvalidExprError(_)
            | Self::InvalidUtf8Error(_)
            | Self::ZeroDivisionError(_)
            | Self::IncompatibleTypeError(_)
            | Self::InvalidOperands(_)
            | Self::ValidationError(_)
            | Self::InvalidParameters(_)
            | Self::InvalidArgumentNumber(_)
            | Self::BytecodeParsingError(_)
            | Self::BytecodeParamParsingError(_)
            | Self::WatcherNotFoundError(_)
            | Self::SerializationError(_)
            | Self::GenericError(_)
            | Self::NestedBlocksNotAllowedError(_)
            | Self::TextParsingError(_) => Color::Red,
            // "Missing things" / lookup failures
            Self::TryIntoFailError(_)
            | Self::FileNotFound(_)
            | Self::FileOpeningError(_)
            | Self::FileReadingError(_)
            | Self::FileWritingError(_)
            | Self::TokenNotFound(_)
            | Self::TokenArrayNotFound(_)
            | Self::BlockNotFound(_)
            | Self::VariableNotFound(_)
            | Self::NonMutableVariableError(_)
            | Self::BytecodeNotFound(_)
            | Self::RequiredContextError(_)
            | Self::BytecodeParamNotRecognized(_) => Color::Yellow,

            // Index-ish
            Self::InvalidIndex(_) | Self::IndexOutOfRange(_) => Color::Magenta,
        }
    }
}

pub fn token_array_not_found(identifier: &str) -> impl Fn() -> TextForgeError {
    let message = TextForgeError::new(
        TextForgeErrorCode::TokenArrayNotFound(
            format!(
                "Token array not found, is {} a valid identifier for this processor?",
                identifier
            )
            .into(),
        ),
        Cow::Borrowed("get identifier"),
        Cow::Borrowed(""),
    );
    move || message.clone()
}

#[cfg(feature = "test_access")]
#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    fn disable_colors() {
        // Mantém os testes determinísticos (sem ANSI).
        colored::control::set_override(false);
    }

    // ============================================================
    // TextForgeError
    // ============================================================

    #[test]
    fn textforge_error_new_sets_fields_and_helpers_return_strs() {
        disable_colors();

        let err = TextForgeError::new(
            TextForgeErrorCode::ValidationError(Cow::Borrowed("bad params")),
            Cow::Borrowed("raw"),
            Cow::Borrowed("banana"),
        );

        assert_eq!(err.instruction, Cow::Borrowed("raw"));
        assert_eq!(err.input, Cow::Borrowed("banana"));

        assert_eq!(err.instruction_str(), "raw");
        assert_eq!(err.input_str(), "banana");
    }

    #[test]
    fn textforge_error_display_contains_sections_and_no_weird_comma() {
        disable_colors();

        let err = TextForgeError::new(
            TextForgeErrorCode::InvalidIndex(Cow::Borrowed("index inválido")),
            "dlc",
            "banana",
        );

        let s = format!("{err}");

        // Título e seções
        assert!(s.contains("Erro"));
        assert!(s.contains("Código:"));
        assert!(s.contains("Mensagem:"));
        assert!(s.contains("Instruction:"));
        assert!(s.contains("Input:"));

        // Conteúdo
        assert!(s.contains("dlc"));
        assert!(s.contains("banana"));

        // Bug clássico de formatação
        assert!(
            !s.contains("\n,Input"),
            "Não deve existir '\\n,Input' no Display"
        );
    }

    // ============================================================
    // TextForgeErrorCode (message/get_message, Display, get_error_code, severity_color)
    // ============================================================

    #[test]
    fn error_code_message_and_get_message_match_and_are_borrowed() {
        let code = TextForgeErrorCode::TokenNotFound(Cow::Borrowed("no token"));
        assert_eq!(code.message().as_ref(), "no token");
        assert_eq!(code.get_message().as_ref(), "no token");
    }

    #[test]
    fn error_code_display_includes_labels_code_and_message() {
        disable_colors();

        let code = TextForgeErrorCode::FileNotFound(Cow::Borrowed("missing file"));
        let s = format!("{code}");

        assert!(s.contains("Código:"));
        assert!(s.contains("Mensagem:"));
        // FileNotFound => 1
        assert!(
            s.contains("1"),
            "Display deve conter o código numérico esperado (1)"
        );
        assert!(s.contains("missing file"));
    }

    #[test]
    fn get_error_code_matches_all_variants() {
        use TextForgeErrorCode::*;

        // cobre todos os braços do match de get_error_code()
        assert_eq!(FileNotFound(Cow::Borrowed("x")).get_error_code(), 1);
        assert_eq!(TokenNotFound(Cow::Borrowed("x")).get_error_code(), 2);
        assert_eq!(TokenArrayNotFound(Cow::Borrowed("x")).get_error_code(), 3);
        assert_eq!(FileReadingError(Cow::Borrowed("x")).get_error_code(), 4);
        assert_eq!(FileWritingError(Cow::Borrowed("x")).get_error_code(), 5);
        assert_eq!(FileOpeningError(Cow::Borrowed("x")).get_error_code(), 6);
        assert_eq!(BytecodeNotFound(Cow::Borrowed("x")).get_error_code(), 7);
        assert_eq!(BlockNotFound(Cow::Borrowed("x")).get_error_code(), 8);
        assert_eq!(VariableNotFound(Cow::Borrowed("x")).get_error_code(), 9);
        assert_eq!(
            NonMutableVariableError(Cow::Borrowed("x")).get_error_code(),
            10
        );
        assert_eq!(InvalidOperands(Cow::Borrowed("x")).get_error_code(), 11);
        assert_eq!(IndexOutOfRange(Cow::Borrowed("x")).get_error_code(), 12);
        assert_eq!(InvalidIndex(Cow::Borrowed("x")).get_error_code(), 13);
        assert_eq!(InvalidParameters(Cow::Borrowed("x")).get_error_code(), 14);
        assert_eq!(
            InvalidArgumentNumber(Cow::Borrowed("x")).get_error_code(),
            15
        );
        assert_eq!(
            BytecodeParamNotRecognized(Cow::Borrowed("x")).get_error_code(),
            16
        );
        assert_eq!(TextParsingError(Cow::Borrowed("x")).get_error_code(), 17);
        assert_eq!(
            BytecodeParsingError(Cow::Borrowed("x")).get_error_code(),
            18
        );
        assert_eq!(
            BytecodeParamParsingError(Cow::Borrowed("x")).get_error_code(),
            19
        );
        assert_eq!(ValidationError(Cow::Borrowed("x")).get_error_code(), 20);
        assert_eq!(ZeroDivisionError(Cow::Borrowed("x")).get_error_code(), 21);
        assert_eq!(TryIntoFailError(Cow::Borrowed("x")).get_error_code(), 22);
        assert_eq!(
            IncompatibleTypeError(Cow::Borrowed("x")).get_error_code(),
            23
        );
        assert_eq!(
            RequiredContextError(Cow::Borrowed("x")).get_error_code(),
            24
        );
    }

    #[test]
    fn severity_color_red_for_hard_errors() {
        use TextForgeErrorCode::*;

        assert_eq!(
            ZeroDivisionError(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            InvalidOperands(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            ValidationError(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            InvalidParameters(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            InvalidArgumentNumber(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            BytecodeParsingError(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            BytecodeParamParsingError(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
        assert_eq!(
            TextParsingError(Cow::Borrowed("x")).severity_color(),
            Color::Red
        );
    }

    #[test]
    fn severity_color_yellow_for_missing_things() {
        use TextForgeErrorCode::*;

        assert_eq!(
            FileNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            FileOpeningError(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            FileReadingError(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            FileWritingError(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            TokenNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            TokenArrayNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            BlockNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            VariableNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            NonMutableVariableError(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            BytecodeNotFound(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
        assert_eq!(
            BytecodeParamNotRecognized(Cow::Borrowed("x")).severity_color(),
            Color::Yellow
        );
    }

    #[test]
    fn severity_color_magenta_for_indexish() {
        use TextForgeErrorCode::*;

        assert_eq!(
            InvalidIndex(Cow::Borrowed("x")).severity_color(),
            Color::Magenta
        );
        assert_eq!(
            IndexOutOfRange(Cow::Borrowed("x")).severity_color(),
            Color::Magenta
        );
    }

    #[test]
    fn message_returns_the_same_inner_reference_for_all_variants() {
        // cobre o match "achatado" (| A(x) | B(x) => x) do method message()
        // com alguns exemplos de grupos diferentes.
        use TextForgeErrorCode::*;

        let m1 = FileNotFound(Cow::Borrowed("a")).message().as_ref();
        let m2 = InvalidOperands(Cow::Borrowed("b")).message().as_ref();
        let m3 = IndexOutOfRange(Cow::Borrowed("c")).message().as_ref();
        let m4 = BytecodeParamNotRecognized(Cow::Borrowed("d"))
            .message()
            .as_ref();

        assert_eq!(m1, "a");
        assert_eq!(m2, "b");
        assert_eq!(m3, "c");
        assert_eq!(m4, "d");
    }

    // ============================================================
    // ErrorManager (add_error/has_errors/reserve_errors/will_log/will_panic/write_errors/handle_error)
    // ============================================================

    #[test]
    fn error_manager_default_and_add_error_has_errors() {
        disable_colors();

        let mut mgr = ErrorManager::default();
        assert!(!mgr.has_errors());

        mgr.add_error(TextForgeError::new(
            TextForgeErrorCode::TokenNotFound(Cow::Borrowed("x")),
            "inst",
            "in",
        ));

        assert!(mgr.has_errors());
        assert_eq!(mgr.error_vec.len(), 1);
    }

    #[test]
    fn error_manager_reserve_errors_does_not_decrease_capacity() {
        let mut mgr = ErrorManager::default();

        let before = mgr.error_vec.capacity();
        mgr.reserve_errors(256);
        let after = mgr.error_vec.capacity();

        assert!(after >= before, "reserve não deve reduzir capacity");
    }

    #[test]
    fn will_log_toggles_show_warnings_and_does_not_panic() {
        let mut mgr = ErrorManager::default();

        mgr.will_log(true);
        assert!(mgr.show_warnings);

        mgr.will_log(false);
        assert!(!mgr.show_warnings);
    }

    #[test]
    fn will_panic_toggles_panic_mode_flag() {
        let mut mgr = ErrorManager::default();

        mgr.will_panic(true);
        assert!(mgr.panic_with_error);

        mgr.will_panic(false);
        assert!(!mgr.panic_with_error);
    }

    #[test]
    fn write_errors_writes_expected_sections_and_indices_for_multiple_errors() {
        disable_colors();

        let mut mgr = ErrorManager::default();
        mgr.add_error(TextForgeError::new(
            TextForgeErrorCode::TokenNotFound(Cow::Borrowed("first")),
            "inst1",
            "in1",
        ));
        mgr.add_error(TextForgeError::new(
            TextForgeErrorCode::InvalidIndex(Cow::Borrowed("second")),
            "inst2",
            "in2",
        ));

        let mut buf: Vec<u8> = vec![];
        mgr.write_errors(&mut buf).unwrap();

        let out = String::from_utf8(buf).unwrap();

        // seções do writer helper
        assert!(out.contains("Code:"));
        assert!(out.contains("Instruction:"));
        assert!(out.contains("Message:"));
        assert!(out.contains("Processing:"));

        // índices enumerados
        assert!(out.contains("[0]"));
        assert!(out.contains("[1]"));

        // conteúdo das mensagens/instruções/inputs
        assert!(out.contains("first"));
        assert!(out.contains("second"));
        assert!(out.contains("inst1"));
        assert!(out.contains("inst2"));
        assert!(out.contains("in1"));
        assert!(out.contains("in2"));
    }

    #[test]
    fn handle_error_stores_error_and_does_not_panic_when_panic_mode_off() {
        disable_colors();

        let mut mgr = ErrorManager::default();
        mgr.will_log(false);
        mgr.will_panic(false);

        mgr.handle_error(TextForgeError::new(
            TextForgeErrorCode::ValidationError(Cow::Borrowed("oops")),
            "inst",
            "input",
        ));

        assert!(mgr.has_errors());
        assert_eq!(mgr.error_vec.len(), 1);
        assert_eq!(mgr.error_vec[0].instruction_str(), "inst");
        assert_eq!(mgr.error_vec[0].input_str(), "input");
    }

    #[test]
    fn handle_error_panics_when_panic_mode_on_and_still_stores_error_first() {
        disable_colors();

        let mut mgr = ErrorManager::default();
        mgr.will_log(false);
        mgr.will_panic(true);

        let err = TextForgeError::new(
            TextForgeErrorCode::ZeroDivisionError(Cow::Borrowed("div0")),
            "div",
            "10/0",
        );

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            mgr.handle_error(err);
        }));

        assert!(result.is_err(), "deve panicar quando panic_with_error=true");
        assert!(
            mgr.has_errors(),
            "deve ter armazenado o erro antes do panic"
        );
        assert_eq!(mgr.error_vec.len(), 1);
    }

    // ============================================================
    // token_array_not_found
    // ============================================================

    #[test]
    fn token_array_not_found_closure_returns_stable_clone_and_contains_identifier() {
        disable_colors();

        let f = token_array_not_found("abc");

        let e1 = f();
        let e2 = f();

        assert_eq!(e1, e2);
        assert_eq!(e1.instruction, Cow::Borrowed("get identifier"));
        assert_eq!(e1.input, Cow::Borrowed(""));

        match &e1.error_code {
            TextForgeErrorCode::TokenArrayNotFound(m) => {
                assert!(m.contains("Token array not found"));
                assert!(m.contains("abc"));
            }
            _ => panic!("Esperado TokenArrayNotFound"),
        }
    }
}
