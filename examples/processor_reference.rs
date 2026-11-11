//! Independent processor/error oracle for the C parity test.
//!
//! Run through `python3 scripts/generate_processor_reference.py`, which stores
//! the output in tests/fixtures/processor_reference.json and the C expectation
//! table tests/c/processor_reference.h. Uses the public Rust processor and
//! builder (feature `test-reference`); the C library never enables it.
use textforge::api::{
    TextForgeBuilderMethods,
    builder::TextForgeBuilder,
    portable::{self, Argument, BindingError, ErrorStage, InstructionSpec, PipelinePlan},
    processor::{TextForgeProcessor, TextForgeProcessorMethods},
};
use textforge::utils::errors::TextForgeError;

fn hex(text: &str) -> String {
    text.as_bytes().iter().map(|byte| format!("{byte:02x}")).collect()
}

fn record(name: &str, result: Result<String, BindingError>) {
    match result {
        Ok(text) => println!("REF\t{name}\tok\t{}", hex(&text)),
        Err(e) => println!(
            "REF\t{name}\terror\t{}\t{:?}\t{}\t{}\t{}",
            e.code,
            e.stage,
            hex(&e.message),
            hex(&e.instruction),
            hex(&e.input)
        ),
    }
}

fn core_error(stage: ErrorStage, e: TextForgeError) -> BindingError {
    BindingError {
        stage,
        code: e.error_code.get_error_code(),
        message: e.error_code.message().to_string(),
        instruction: e.instruction.into_owned(),
        input: e.input.into_owned(),
    }
}

/// Build one pipeline with the public builder and run it on "a\nb".
fn line_case(
    name: &str,
    build: impl FnOnce(&mut TextForgeBuilder<'_>) -> Result<(), TextForgeError>,
) -> Result<(), TextForgeError> {
    let mut native = TextForgeProcessor::new();
    let mut builder = native.create_pipeline();
    build(&mut builder)?;
    let id = builder.build()?;
    record(
        name,
        native.process_all(&id, "a\nb").map_err(|error| core_error(ErrorStage::Execution, error)),
    );
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::path::PathBuf::from(std::env::args().nth(1).ok_or("expected temporary directory")?);
    let mut core = TextForgeProcessor::new();
    let id = core.create_pipeline().add_to_beginning("🍌")?.add_to_end("é")?.build()?;
    for (name, input) in [("unicode", "e\u{301}\0"), ("empty", "")] {
        record(name, core.process_all(&id, input).map_err(|e| core_error(ErrorStage::Execution, e)));
    }
    record("missing_id", core.process_all("missing", "x").map_err(|e| core_error(ErrorStage::Execution, e)));
    record(
        "missing_snapshot",
        core.get_transform_vec("missing")
            .map(|v| v.len().to_string())
            .map_err(|e| core_error(ErrorStage::Execution, e)),
    );
    record(
        "missing_remove",
        core.remove_transform("missing")
            .map(|()| String::new())
            .map_err(|e| core_error(ErrorStage::Execution, e)),
    );
    let missing_var = core.create_pipeline().repeat(textforge::api::get_var("missing"))?.build()?;
    record(
        "missing_variable",
        core.process_all(&missing_var, "é").map_err(|e| core_error(ErrorStage::Execution, e)),
    );
    let invalid_regex = core.create_pipeline().remove_pattern("[")?.build()?;
    record(
        "invalid_regex",
        core.process_all(&invalid_regex, "é").map_err(|e| core_error(ErrorStage::Execution, e)),
    );
    let mut plan = PipelinePlan::new();
    record("unknown", plan.add(InstructionSpec::new("unknown".into(), vec![])).map(|()| String::new()));
    plan.add(InstructionSpec::new("atb".into(), vec![]))?;
    record("arity", plan.compile().map(|_| String::new()));
    let mut typed = PipelinePlan::new();
    typed.add(InstructionSpec::new("atb".into(), vec![Argument::Number(1)]))?;
    record("parameter_type", typed.compile().map(|_| String::new()));

    let text_path = dir.join("pipeline.textforge");
    std::fs::write(&text_path, "")?;
    core.write_pipeline_to_file(&id, &text_path)?;
    record(
        "persistence_missing",
        core.write_pipeline_to_file("missing", &text_path)
            .map(|()| String::new())
            .map_err(|e| core_error(ErrorStage::Persistence, e)),
    );
    let restored = core.get_pipeline_from_file(&text_path)?;
    record("text_roundtrip", core.process_all(&restored, "x").map_err(|e| core_error(ErrorStage::Execution, e)));
    let byte_path = dir.join("pipeline.textforgebc");
    std::fs::write(&byte_path, "")?;
    core.write_to_bytecode_file(&id, &byte_path)?;
    let restored = core.read_from_bytecode_file(&byte_path)?;
    record(
        "bytecode_roundtrip",
        core.process_all(&restored, "x").map_err(|e| core_error(ErrorStage::Execution, e)),
    );
    let options = textforge::api::TextBufferOptions::new(1, &dir);
    record(
        "buffered",
        core.process_all_buffered(&id, textforge::api::TextBuffer::from_text("x", options)?)
            .and_then(|b| b.into_string())
            .map_err(|e| core_error(ErrorStage::Execution, e)),
    );
    let mut watchers = textforge::watchers::WatcherList::new();
    watchers.set_watcher("size", textforge::watchers::default_watchers::current_len_chars);
    watchers.schedule_watcher("size", textforge::watchers::ExecutionWindows::All)?;
    let report = dir.join("report.json");
    record(
        "watched",
        core.process_all_with_watchers(&id, "x", &mut watchers, &report)
            .map_err(|e| core_error(ErrorStage::Execution, e)),
    );
    record("report", Ok(std::fs::read_to_string(report)?));

    let mut observers = portable::Observers::new();
    observers.set("bad".into(), Box::new(|_: portable::Observation| Err("Error: boom".into())));
    observers.schedule_all("bad")?;
    let mut processor = portable::PortableProcessor::new();
    let id = processor.add_transform(vec![InstructionSpec::new("atb".into(), vec![Argument::Text("🍌".into())])])?;
    let failed = dir.join("failed.json").to_str().ok_or("temporary path is not Unicode")?.to_owned();
    record(
        "callback_error",
        processor.process_all_with_watchers(&id, "x", &mut observers, &portable::FilePath::from_text(failed)),
    );
    record("builtin_error", observers.set_builtin("x".into(), "missing").map(|()| String::new()));
    let mut sent = false;
    let options = portable::BufferOptions {
        memory_threshold: 1,
        spill_directory: portable::FilePath::from_text(
            dir.to_str().ok_or("temporary path is not Unicode")?.to_owned(),
        ),
    };
    record(
        "invalid_utf8",
        portable::PortableBuffer::from_reader(
            Box::new(move |_| {
                if sent {
                    Ok(vec![])
                } else {
                    sent = true;
                    Ok(vec![0xff])
                }
            }),
            options,
        )
        .map(|_| String::new()),
    );

    // Line-instruction runtime errors (C replays them in test_processor_parity.c).
    line_case("line_append_line_multiline", |p| p.append_line("x\ny").map(drop))?;
    line_case("line_prepend_line_multiline", |p| p.prepend_line("x\ny").map(drop))?;
    line_case("line_insert_line_multiline", |p| p.insert_line(1, "x\ny").map(drop))?;
    line_case("line_replace_line_multiline", |p| p.replace_line(1, "x\ny").map(drop))?;
    line_case("line_keep_lines_matching_regex", |p| p.keep_lines_matching("[").map(drop))?;
    line_case("line_delete_lines_matching_regex", |p| p.delete_lines_matching("[").map(drop))?;
    line_case("line_delete_line_zero", |p| p.delete_line(0).map(drop))?;
    line_case("line_delete_line_missing", |p| p.delete_line(100).map(drop))?;
    line_case("line_insert_line_zero", |p| p.insert_line(0, "x").map(drop))?;
    line_case("line_select_lines_inverted", |p| p.select_lines(2, 1).map(drop))?;
    line_case("line_delete_lines_interval_inverted", |p| p.delete_lines_interval(2, 1).map(drop))?;
    Ok(())
}
