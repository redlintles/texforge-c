use tempfile::NamedTempFile;

use super::*;
use std::fs;

fn ctx(before: &str, current: &str, after: Option<&str>, instruction: &str) -> WatcherContext {
    WatcherContext::new(
        current.to_string(),
        before.to_string(),
        after.map(|s| s.to_string()),
        instruction.to_string(),
    )
}

#[test]
fn test_watcher_context_new_stores_fields_verbatim() {
    let c = ctx("a", "ab", Some("abc"), "add_to_end");
    assert_eq!(c.before.to_string(), "a");
    assert_eq!(c.current.to_string(), "ab");
    assert_eq!(c.after.unwrap().to_string(), "abc".to_string());
    assert_eq!(c.instruction.to_string(), "add_to_end");
}

#[test]
fn test_set_watcher_without_schedule_does_not_run() {
    let mut wl = WatcherList::default();
    wl.set_watcher("len", |c: WatcherContext| c.current.len().to_string());
    // Nunca agendado -> run_watchers não deve produzir nenhuma entrada
    wl.run_watchers(ctx("", "banana", None::<&str>, "add_to_end"))
        .unwrap();
    assert!(wl.result.is_empty());
}

#[test]
fn test_schedule_watcher_fails_for_unregistered_name() {
    let mut wl = WatcherList::default();
    let res = wl.schedule_watcher("nao_existe", ExecutionWindows::All);
    assert!(res.is_err());
}

#[test]
fn test_schedule_watcher_succeeds_for_registered_name() {
    let mut wl = WatcherList::default();
    wl.set_watcher("len", |c: WatcherContext| c.current.len().to_string());
    assert!(wl.schedule_watcher("len", ExecutionWindows::All).is_ok());
}

#[test]
fn test_run_watchers_stores_result_under_current_counter() {
    let mut wl = WatcherList::default();
    wl.set_watcher("len", |c: WatcherContext| c.current.len().to_string());
    wl.schedule_watcher("len", ExecutionWindows::All).unwrap();

    wl.run_watchers(ctx("", "banana", None::<&str>, "add_to_end"))
        .unwrap();

    let step0 = wl.result.first().expect("step 0 deveria existir");
    assert_eq!(step0.get("len").unwrap(), "6");
}

#[test]
fn test_run_watchers_increments_counter_across_calls() {
    let mut wl = WatcherList::default();
    wl.set_watcher("len", |c: WatcherContext| c.current.len().to_string());
    wl.schedule_watcher("len", ExecutionWindows::All).unwrap();

    wl.run_watchers(ctx("", "a", None::<&str>, "add_to_end"))
        .unwrap();
    wl.run_watchers(ctx("a", "ab", None::<&str>, "add_to_end"))
        .unwrap();
    wl.run_watchers(ctx("ab", "abc", None::<&str>, "add_to_end"))
        .unwrap();

    assert_eq!(wl.result.first().unwrap().get("len").unwrap(), "1");
    assert_eq!(wl.result.get(1).unwrap().get("len").unwrap(), "2");
    assert_eq!(wl.result.get(2).unwrap().get("len").unwrap(), "3");
}

#[test]
fn test_run_watchers_runs_multiple_scheduled_watchers_per_call() {
    let mut wl = WatcherList::default();
    wl.set_watcher("len", |c: WatcherContext| c.current.len().to_string());
    wl.set_watcher("is_empty", |c: WatcherContext| {
        c.current.is_empty().to_string()
    });
    wl.schedule_watcher("len", ExecutionWindows::All).unwrap();
    wl.schedule_watcher("is_empty", ExecutionWindows::All)
        .unwrap();

    wl.run_watchers(ctx("", "banana", None::<&str>, "add_to_end"))
        .unwrap();

    let step0 = wl.result.first().unwrap();
    assert_eq!(step0.get("len").unwrap(), "6");
    assert_eq!(step0.get("is_empty").unwrap(), "false");
}

#[test]
fn test_reset_clears_result_and_counter_but_keeps_registration() {
    let mut wl = WatcherList::default();
    wl.set_watcher("len", |c: WatcherContext| c.current.len().to_string());
    wl.schedule_watcher("len", ExecutionWindows::All).unwrap();
    wl.run_watchers(ctx("", "banana", None::<&str>, "add_to_end"))
        .unwrap();

    wl.reset();
    assert!(wl.result.is_empty());
    assert_eq!(wl.counter, 0);

    // "len" continua registrado e agendado: run_watchers volta a produzir resultado
    // sem precisar chamar set_watcher/schedule_watcher de novo.
    wl.run_watchers(ctx("", "abcd", None::<&str>, "add_to_end"))
        .unwrap();
    assert_eq!(wl.result.first().unwrap().get("len").unwrap(), "4");
}

// NOTA: o branch de erro "Watcher Not Found" dentro de run_watchers (o segundo
// ok_or_else, distinto do de schedule_watcher) não é alcançável pela API pública
// atual — schedule_watcher só agenda nomes já presentes em `watchers`, e não existe
// remove_watcher pra desincronizar os dois mapas depois. Por isso não há teste
// exercitando esse caminho; ele é só uma checagem defensiva.

// ... ctx() e os testes anteriores continuam iguais ...

#[test]
fn test_to_json_writes_valid_json_matching_result() {
    let mut wl = WatcherList::default();
    wl.set_watcher("len", |c: WatcherContext| c.current.len().to_string());
    wl.schedule_watcher("len", ExecutionWindows::All).unwrap();
    wl.run_watchers(ctx("", "banana", None::<&str>, "add_to_end"))
        .unwrap();
    wl.run_watchers(ctx("banana", "banana!", None::<&str>, "add_to_end"))
        .unwrap();

    let file = NamedTempFile::new().unwrap();
    wl.to_json(file.path()).unwrap();

    let content = fs::read_to_string(file.path()).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

    assert_eq!(parsed[0]["len"], "6");
    assert_eq!(parsed[1]["len"], "7");
    // arquivo é removido automaticamente quando `file` sai de escopo
}

#[test]
fn test_to_json_overwrites_existing_file() {
    let mut wl = WatcherList::default();
    wl.set_watcher("len", |c: WatcherContext| c.current.len().to_string());
    wl.schedule_watcher("len", ExecutionWindows::All).unwrap();

    let file = NamedTempFile::new().unwrap();
    fs::write(file.path(), "conteudo antigo que deve ser substituido").unwrap();

    wl.run_watchers(ctx("", "ab", None::<&str>, "add_to_end"))
        .unwrap();
    wl.to_json(file.path()).unwrap();

    let content = fs::read_to_string(file.path()).unwrap();
    assert!(!content.contains("conteudo antigo"));
}

fn populated_list() -> WatcherList {
    let mut list = WatcherList::new();
    list.set_watcher("z_bytes", |ctx| ctx.current.len().to_string());
    list.set_watcher("a_chars", |ctx| ctx.current.chars().count().to_string());
    list.schedule_watcher("z_bytes", All).unwrap();
    list.schedule_watcher("a_chars", All).unwrap();
    list
}

#[test]
fn one_complete_row_per_step_with_sorted_keys_and_no_extra_entries() {
    let mut list = populated_list();
    for text in ["", "é", "👩🏽‍💻"] {
        list.run_watchers(ctx("", text, None, "null")).unwrap();
    }
    assert_eq!(list.result.len(), 3);
    assert!(list.result.iter().all(|row| row.len() == 2));
    let mut bytes = Vec::new();
    list.write_json(&mut bytes).unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 3);
    assert_eq!(json[1]["a_chars"], "2");
    assert_eq!(json[2]["z_bytes"], "15");
    let mut again = Vec::new();
    list.write_json(&mut again).unwrap();
    let first = String::from_utf8(again).unwrap();
    assert!(first.find("a_chars").unwrap() < first.find("z_bytes").unwrap());
}

#[test]
fn prepared_schedule_runs_replaced_callbacks_once_in_name_order() {
    use std::cell::RefCell;
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut list = WatcherList::new();
    for name in ["z", "a"] {
        let calls = Rc::clone(&calls);
        list.set_watcher(name, move |_| {
            calls.borrow_mut().push(name);
            name.to_string()
        });
        list.schedule_watcher(name, All).unwrap();
        list.schedule_watcher(name, All).unwrap();
    }
    list.set_watcher("z", |_| "replacement".into());
    list.run_watchers(ctx("", "", None, "null")).unwrap();
    assert_eq!(&*calls.borrow(), &["a"]);
    assert_eq!(list.result[0]["z"], "replacement");
    assert_eq!(list.schedule.len(), 2);
}

#[test]
fn reset_reuses_bounded_empty_maps_and_drops_large_values() {
    let mut list = populated_list();
    for _ in 0..600 {
        list.run_watchers(ctx("", "é", None, "null")).unwrap();
    }
    list.reset();
    assert!(list.result.is_empty());
    assert_eq!(list.counter, 0);
    assert_eq!(list.spare_rows.len(), MAX_RETAINED_ROWS);
    assert!(list.spare_rows.iter().all(HashMap::is_empty));
    let capacity = list.result.capacity();
    for _ in 0..3 {
        list.run_watchers(ctx("", "abc", None, "null")).unwrap();
        assert_eq!(list.result.len(), 1);
        assert_eq!(list.result[0]["z_bytes"], "3");
        list.reset();
        assert_eq!(list.result.capacity(), capacity);
    }
    list.result.reserve(MAX_RETAINED_RESULT_CAPACITY + 1);
    list.reset();
    assert_eq!(list.result.capacity(), 0);
    list.spare_rows.clear();
    let mut oversized = HashMap::new();
    oversized.reserve(MAX_RETAINED_COLUMNS + 1);
    oversized.insert(
        std::sync::Arc::<str>::from("huge"),
        "x".repeat(2 * 1024 * 1024),
    );
    list.result.push(oversized);
    list.reset();
    assert!(list.spare_rows.is_empty());
}

/// Fault injection writer exercising short writes, write-zero and flush errors.
struct FaultWriter {
    bytes: Vec<u8>,
    max_chunk: usize,
    fail_after: Option<usize>,
    fail_flush: bool,
}
impl Write for FaultWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self
            .fail_after
            .is_some_and(|limit| self.bytes.len() >= limit)
        {
            return Err(std::io::Error::other("injected write failure"));
        }
        let count = bytes.len().min(self.max_chunk).min(
            self.fail_after
                .map_or(usize::MAX, |limit| limit - self.bytes.len()),
        );
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if self.fail_flush {
            Err(std::io::Error::other("injected flush failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn serialization_handles_short_writes_and_reports_write_and_flush_failures() {
    let mut list = populated_list();
    list.run_watchers(ctx("", "é", None, "null")).unwrap();
    let mut short = FaultWriter {
        bytes: vec![],
        max_chunk: 1,
        fail_after: None,
        fail_flush: false,
    };
    list.write_json(&mut short).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&short.bytes).unwrap();
    assert_eq!(parsed[0]["z_bytes"], "2");
    for (max_chunk, fail_after, fail_flush) in
        [(0, None, false), (3, Some(17), false), (100, None, true)]
    {
        let mut writer = FaultWriter {
            bytes: vec![],
            max_chunk,
            fail_after,
            fail_flush,
        };
        let error = list.write_json(&mut writer).unwrap_err();
        assert!(matches!(
            error.error_code,
            TextForgeErrorCode::FileWritingError(_)
        ));
    }
}

#[test]
fn atomic_export_keeps_old_report_and_cleans_up_after_write_or_publish_failure() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.json");
    std::fs::write(&path, "old report").unwrap();
    let error = publish_report(&path, |writer| {
        writer.write_all(b"partial").map_err(write_error)?;
        Err(report_error("injected serialization failure"))
    });
    assert!(error.is_err());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old report");
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    let destination_directory = directory.path().join("destination");
    std::fs::create_dir(&destination_directory).unwrap();
    assert!(
        publish_report(&destination_directory, |writer| {
            writer.write_all(b"[]").map_err(write_error)
        })
        .is_err()
    );
    assert!(destination_directory.is_dir());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    let mut list = populated_list();
    list.run_watchers(ctx("", "abc", None, "null")).unwrap();
    list.to_json(&path).unwrap();
    let report: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(report[0]["z_bytes"], "3");
}

#[test]
fn concurrent_publishers_leave_one_complete_valid_report_without_temporary_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("report.json");
    let handles: Vec<_> = (0..4)
        .map(|index| {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut list = WatcherList::new();
                list.set_watcher("publisher", move |_| index.to_string());
                list.schedule_watcher("publisher", All).unwrap();
                list.run_watchers(ctx("", "", None, "null")).unwrap();
                for _ in 0..10 {
                    list.to_json(&path).unwrap();
                    let report: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                    assert_eq!(report.as_array().unwrap().len(), 1);
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap();
    }
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
}

#[test]
fn counter_overflow_is_an_error_without_running_callbacks_or_adding_rows() {
    let mut list = populated_list();
    list.counter = u64::MAX;
    assert!(list.run_watchers(ctx("", "abc", None, "null")).is_err());
    assert!(list.result.is_empty());
}

#[test]
fn shared_standard_statistics_match_pure_functions_and_custom_overrides_stay_opaque() {
    use super::default_watchers as defaults;
    let directory = tempfile::tempdir().unwrap();
    let mut list = WatcherList::new();
    list.set_watcher("chars", defaults::current_len_chars);
    list.set_watcher("words", defaults::word_count);
    list.set_watcher("lines", defaults::line_count);
    list.set_watcher("char_delta", defaults::char_delta);
    list.set_watcher("word_delta", defaults::word_count_delta);
    for name in ["chars", "words", "lines", "char_delta", "word_delta"] {
        list.schedule_watcher(name, All).unwrap();
    }
    for text in ["", "a\r\nb\n", "é\u{85}👩🏽‍💻 🇧🇷\u{3000}"] {
        let context = ctx("old words", text, None, "null");
        list.run_watchers(context.clone()).unwrap();
        let path = directory.path().join("report.json");
        list.to_json(&path).unwrap();
        let rows: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        for (key, expected) in [
            ("chars", defaults::current_len_chars(context.clone())),
            ("words", defaults::word_count(context.clone())),
            ("lines", defaults::line_count(context.clone())),
            ("char_delta", defaults::char_delta(context.clone())),
            ("word_delta", defaults::word_count_delta(context)),
        ] {
            assert_eq!(rows[0][key], expected);
        }
        list.reset();
    }
    list.set_watcher("words", |_| "custom".to_string());
    assert!(
        list.schedule
            .iter()
            .find(|entry| entry.name.as_ref() == "words")
            .unwrap()
            .metric
            .is_none()
    );
    list.run_watchers(ctx("", "abc", None, "null")).unwrap();
    assert_eq!(list.result[0]["words"], "custom");
}
