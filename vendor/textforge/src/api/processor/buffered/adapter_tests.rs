use super::*;
use crate::api::processor::TextForgeProcessorMethods;
use std::borrow::Cow;

fn pipeline(processor: &mut TextForgeProcessor, source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.tft");
    std::fs::write(&path, source).unwrap();
    processor.get_pipeline_from_file(&path).unwrap()
}

#[test]
fn adapters_preserve_legacy_transforms_context_and_halt() {
    for source in [
        "atb <;\nate >;\n",
        "raw alice USER;\ntua;\n",
        "atb A;\nend;\nate B;\n",
        "blk x assoc tua;\ncblk x;\n",
        "var name = João;\natb \"{{name}}: \";\n",
    ] {
        for input in [
            String::new(),
            "alice é 👩🏽‍💻 🇧🇷".into(),
            "alice é 🦀".repeat(180000),
        ] {
            let mut processor = TextForgeProcessor::new();
            let id = pipeline(&mut processor, source);
            let expected = processor
                .run_transform(&id, Cow::Borrowed(&input))
                .unwrap()
                .into_owned();
            assert_eq!(processor.process_all(&id, &input).unwrap(), expected);
            let mut log = Vec::new();
            let document =
                TextBuffer::from_text(&input, TextBufferOptions::new(1, std::env::temp_dir()))
                    .unwrap();
            assert_eq!(
                processor
                    .run_buffered_debug(&id, document, &mut log)
                    .unwrap()
                    .into_string()
                    .unwrap(),
                expected
            );
            assert!(log.starts_with(b"PROCESSING STEP BY STEP:\n----------\n\n"));
        }
    }
}

#[test]
fn batch_keeps_order_and_continues_after_invalid_input() {
    let dir = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = pipeline(&mut processor, "atb <;\nate >;\n");
    let input = dir.path().join("input");
    let bad = dir.path().join("bad");
    let missing = dir.path().join("missing");
    let output = dir.path().join("nested/output");
    let output2 = dir.path().join("output2");
    std::fs::write(&input, "é 👩🏽‍💻".repeat(100000)).unwrap();
    std::fs::write(&bad, [0xff]).unwrap();
    let results = processor.process_batch(vec![
        (&bad, &id, &output2),
        (&input, &id, &output),
        (&missing, &id, &output2),
    ]);
    assert!(results[0].is_err());
    assert!(results[1].is_ok());
    assert!(results[2].is_err());
    assert_eq!(
        std::fs::read_to_string(output).unwrap(),
        format!("<{}>", std::fs::read_to_string(input).unwrap())
    );
    assert!(processor.process_batch(vec![]).is_empty());
}

#[test]
fn adapters_record_pipeline_and_parameter_errors() {
    let mut processor = TextForgeProcessor::new();
    assert!(processor.process_all("missing", "").is_err());
    assert!(processor.process_all_with_debug("missing", "").is_err());
    let id = pipeline(&mut processor, "atb \"{{missing}}\";\n");
    assert!(processor.process_all(&id, "abc").is_err());
    assert!(processor.process_all_with_debug(&id, "abc").is_err());
}

#[test]
#[cfg(feature = "watchers")]
fn watchers_preserve_snapshots_lookahead_and_reset() {
    use crate::watchers::{ExecutionWindows, WatcherContext, WatcherList};
    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let id = pipeline(&mut processor, "atb <;\nate >;\n");
    let mut list = WatcherList::new();
    list.set_watcher("snapshot", |ctx: WatcherContext| {
        serde_json::json!({
            "before": ctx.before.as_ref(), "current": ctx.current.as_ref(),
            "after": ctx.after.as_deref(), "instruction": ctx.instruction.as_ref()
        })
        .to_string()
    });
    list.schedule_watcher("snapshot", ExecutionWindows::All)
        .unwrap();
    for input in [String::new(), "é 👩🏽‍💻 🇧🇷".into(), "é 🦀".repeat(250000)] {
        let report = directory.path().join("report.json");
        assert_eq!(
            processor
                .process_all_with_watchers(&id, &input, &mut list, &report)
                .unwrap(),
            format!("<{input}>")
        );
        let rows: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 2);
        let first: serde_json::Value =
            serde_json::from_str(rows[0]["snapshot"].as_str().unwrap()).unwrap();
        let last: serde_json::Value =
            serde_json::from_str(rows[1]["snapshot"].as_str().unwrap()).unwrap();
        assert_eq!(first["before"], input);
        assert_eq!(first["current"], format!("<{input}"));
        assert_eq!(first["after"], format!("<{input}>"));
        assert_eq!(last["before"], format!("<{input}"));
        assert_eq!(last["current"], format!("<{input}>"));
        assert!(last["after"].is_null());
    }
}

#[test]
#[cfg(feature = "watchers")]
fn watchers_propagate_errors_and_retry_export_without_resetting_results() {
    use crate::watchers::{ExecutionWindows, WatcherContext, WatcherList};
    let directory = tempfile::tempdir().unwrap();
    let mut processor = TextForgeProcessor::new();
    let mut list = WatcherList::new();
    list.set_watcher("bytes", |ctx: WatcherContext| ctx.current.len().to_string());
    list.schedule_watcher("bytes", ExecutionWindows::All)
        .unwrap();
    let report = directory.path().join("report.json");
    assert!(
        processor
            .process_all_with_watchers("missing", "abc", &mut list, &report)
            .is_err()
    );
    assert!(!report.exists());
    let invalid = pipeline(&mut processor, "atb \"{{missing}}\";\n");
    assert!(
        processor
            .process_all_with_watchers(&invalid, "abc", &mut list, &report)
            .is_err()
    );
    assert!(!report.exists());
    list.reset();
    let valid = pipeline(&mut processor, "atb <;\nate >;\n");
    let invalid_report = directory.path().join("missing/report.json");
    assert!(
        processor
            .process_all_with_watchers(&valid, "abc", &mut list, &invalid_report)
            .is_err()
    );
    list.to_json(&report).unwrap();
    let rows: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap();
    assert_eq!(rows[0]["bytes"], "4");
    assert_eq!(rows[1]["bytes"], "5");
    list.reset();
    assert_eq!(
        processor
            .process_all_with_watchers(&valid, "abc", &mut list, &report)
            .unwrap(),
        "<abc>"
    );
}
