use clap::{Arg, ArgAction, Command, value_parser};
use std::{
    borrow::Cow,
    fs::OpenOptions,
    io::{self, Error, Read, Write},
    path::{Path, PathBuf},
};
use textforge::{
    api::processor::{TextForgeProcessor, TextForgeProcessorMethods},
    utils::{
        cli::{process_input_by_chunks, process_input_line_by_line, process_input_single_chunk},
        errors::{TextForgeError, TextForgeErrorCode},
        validations::check_file_path,
    },
};

#[derive(Clone, Copy, PartialEq, Debug)]
enum ReadMode {
    All,
    Line,
    Chunk(usize),
}

fn build_cli() -> Command {
    Command::new("txf")
        .version(env!("CARGO_PKG_VERSION"))
        .about("CLI for TextForge")
        .arg(
            Arg::new("file")
                .short('f')
                .long("file")
                .required(true)
                .value_name("FILE")
                .value_parser(value_parser!(PathBuf))
                .help("Arquivo .textforge ou .textforgebc")
        )
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .required(false)
                .value_name("INPUT")
                .value_parser(value_parser!(PathBuf))
                .help(
                    "The file that will be processed, if not specified, will read from stdin, otherwise, will use an empty string"
                )
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .required(false)
                .value_name("OUTPUT")
                .value_parser(value_parser!(PathBuf))
                .help(
                    "The file where the output will be stored, if not specified, the content will just be printed in stdout"
                )
        )
        .arg(
            Arg::new("read_mode")
                .short('r')
                .long("read-mode")
                .default_value("all")
                .required(false)
                .value_name("READ_MODE")
                .value_parser(
                    |s: &str| -> Result<ReadMode, Error> {
                        if s == "all" {
                            Ok(ReadMode::All)
                        } else if s == "line" {
                            Ok(ReadMode::Line)
                        } else if let Some(num) = s.strip_prefix("chunk-") {
                            num.parse::<usize>()
                                .map(ReadMode::Chunk)
                                .map_err(|_|
                                    Error::new(io::ErrorKind::InvalidInput, "Chunk size inválido")
                                )
                        } else {
                            Err(Error::new(io::ErrorKind::InvalidInput, "Modo inválido"))
                        }
                    }
                )
                .help(
                    "Input Read mode, default value is 'all', meaning it will read all file contents as a single string, other possible values are 'line', to read the file line by line, and 'chunk-X', meaning it will read the file in chunks of X characters"
                )
        )
        .arg(
            Arg::new("mode")
                .short('m')
                .long("mode")
                .required(false)
                .value_name("mode")
                .value_parser(["b", "t"])
                .default_value("t")
                .help(
                    "The TextForge mode that will be used, default is 't', for text mode, you can also use 'b' for bytecode mode"
                )
        )
        .arg(
            Arg::new("debug")
                .short('d')
                .long("debug")
                .required(false)
                .value_name("debug")
                .action(ArgAction::SetTrue)
                .help(
                    "Determines whether TextForge will run in debug mode or not, default is false"
                )
        )
}

fn process_by_mode(
    read_mode: &ReadMode,
    id: &str,
    data: &str,
    debug: bool,
    processor: &mut TextForgeProcessor,
) -> Result<String, TextForgeError> {
    match read_mode {
        ReadMode::All => process_input_single_chunk(processor, id, data, debug),
        ReadMode::Line => process_input_line_by_line(processor, id, data, debug),
        ReadMode::Chunk(s) => process_input_by_chunks(processor, id, data, *s, debug),
    }
}

/// Carrega o pipeline de `file` no modo pedido e devolve (id, processor).
/// Modo 'b' só existe quando o binário é compilado com `--features bytecode`;
/// sem o feature, escolher 'b' falha em runtime com uma mensagem clara, em
/// vez de o binário inteiro deixar de compilar sem esse feature ligado.
fn load_pipeline(mode: &str, file: &Path) -> Result<(String, TextForgeProcessor), TextForgeError> {
    match mode {
        "b" => {
            #[cfg(feature = "bytecode")]
            {
                check_file_path(file, Some("textforgebc"))?;
                let mut processor = TextForgeProcessor::new();
                let id = processor.read_from_bytecode_file(file)?;
                Ok((id, processor))
            }
            #[cfg(not(feature = "bytecode"))]
            {
                Err(TextForgeError::new(
                    TextForgeErrorCode::ValidationError(
                        "Bytecode mode requires building txf with `--features bytecode`".into(),
                    ),
                    Cow::from("main.load_pipeline"),
                    Cow::from("mode=b"),
                ))
            }
        }
        "t" => {
            check_file_path(file, Some("textforge"))?;
            let mut processor = TextForgeProcessor::new();
            let id = processor.get_pipeline_from_file(file)?;
            Ok((id, processor))
        }
        // clap já restringe "mode" a ["b", "t"] via value_parser — qualquer
        // outro valor nunca chega aqui.
        _ => unreachable!("clap value_parser restricts mode to b/t"),
    }
}

fn main() -> Result<(), TextForgeError> {
    let matches = build_cli().get_matches();

    let file = matches.get_one::<PathBuf>("file").unwrap();
    let input = matches.get_one::<PathBuf>("input");
    let output = matches.get_one::<PathBuf>("output");
    let textforge_mode = matches.get_one::<String>("mode").unwrap();
    let read_mode = matches.get_one::<ReadMode>("read_mode").unwrap();
    let debug = matches.get_one::<bool>("debug").unwrap();

    let data: String = match input {
        Some(path) => {
            if !path.exists() {
                return Err(TextForgeError::new(
                    TextForgeErrorCode::FileNotFound("Input file does not exist".into()),
                    Cow::from("main"),
                    Cow::from(path.display().to_string()),
                ));
            }

            let mut input_file = OpenOptions::new().read(true).open(path).map_err(|e| {
                TextForgeError::new(
                    TextForgeErrorCode::FileOpeningError(Cow::from(e.to_string())),
                    Cow::from("main"),
                    Cow::from(path.display().to_string()),
                )
            })?;

            let mut b = String::new();
            input_file.read_to_string(&mut b).map_err(|e| {
                TextForgeError::new(
                    TextForgeErrorCode::FileReadingError(Cow::from(e.to_string())),
                    Cow::from("main"),
                    Cow::from(path.display().to_string()),
                )
            })?;

            b
        }
        None => {
            let mut b = String::new();
            io::stdin().read_to_string(&mut b).map_err(|e| {
                TextForgeError::new(
                    TextForgeErrorCode::FileReadingError(Cow::from(e.to_string())),
                    Cow::from("main"),
                    Cow::from("<stdin>"),
                )
            })?;
            b
        }
    };

    let (id, mut processor) = load_pipeline(textforge_mode.as_str(), file)?;
    let result = process_by_mode(read_mode, &id, &data, *debug, &mut processor)?;

    match output {
        Some(p) => {
            let mut f = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(p)
                .map_err(|e| {
                    TextForgeError::new(
                        TextForgeErrorCode::FileOpeningError(Cow::from(e.to_string())),
                        Cow::from("main"),
                        Cow::from(p.display().to_string()),
                    )
                })?;

            f.write_all(result.as_bytes()).map_err(|e| {
                TextForgeError::new(
                    TextForgeErrorCode::FileWritingError(Cow::from(e.to_string())),
                    Cow::from("main"),
                    Cow::from(p.display().to_string()),
                )
            })?;
        }
        None => {
            println!("Resultado do processamento: {}", result);
        }
    }

    Ok(())
}

#[cfg(feature = "test_access")]
#[cfg(test)]
mod textforge_tests {
    mod parser_tests {
        use crate::{ReadMode, build_cli};
        use std::{path::PathBuf, str::FromStr};

        #[test]
        fn test_all_with_long_params() {
            let parser = build_cli();
            let c = "txf --file ./instructions.textforgebc --input ./example.txt --output output.txt --debug --mode b --read-mode line";

            let arg_vec = shell_words::split(c).unwrap();

            let m = parser.try_get_matches_from(arg_vec).unwrap();

            let file = m.get_one::<PathBuf>("file").unwrap();
            let input = m.get_one::<PathBuf>("input").unwrap();
            let output = m.get_one::<PathBuf>("output").unwrap();
            let textforge_mode = m.get_one::<String>("mode").unwrap();
            let read_mode = m.get_one::<ReadMode>("read_mode").unwrap();
            let debug = m.get_one::<bool>("debug").unwrap();

            assert_eq!(
                *file,
                PathBuf::from_str("./instructions.textforgebc").unwrap()
            );
            assert_eq!(*input, PathBuf::from_str("./example.txt").unwrap());
            assert_eq!(*output, PathBuf::from_str("output.txt").unwrap());
            assert_eq!(*textforge_mode, "b".to_string());
            assert_eq!(*read_mode, ReadMode::Line);
            assert!(*debug);
        }
        #[test]
        fn test_all_with_short_params() {
            let parser = build_cli();
            let c =
                "txf -f ./instructions.textforgebc -i ./example.txt -o output.txt -d -m b -r line";

            let arg_vec = shell_words::split(c).unwrap();

            let m = parser.try_get_matches_from(arg_vec).unwrap();

            let file = m.get_one::<PathBuf>("file").unwrap();
            let input = m.get_one::<PathBuf>("input").unwrap();
            let output = m.get_one::<PathBuf>("output").unwrap();
            let textforge_mode = m.get_one::<String>("mode").unwrap();
            let read_mode = m.get_one::<ReadMode>("read_mode").unwrap();
            let debug = m.get_one::<bool>("debug").unwrap();

            assert_eq!(
                *file,
                PathBuf::from_str("./instructions.textforgebc").unwrap()
            );
            assert_eq!(*input, PathBuf::from_str("./example.txt").unwrap());
            assert_eq!(*output, PathBuf::from_str("output.txt").unwrap());
            assert_eq!(*textforge_mode, "b".to_string());
            assert_eq!(*read_mode, ReadMode::Line);
            assert!(*debug);
        }
    }
}
