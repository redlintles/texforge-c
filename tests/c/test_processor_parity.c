/* Processor and error parity against the Rust oracle (examples/processor_reference.rs).
 *
 * Every case in processor_reference.h is replayed through the C API; results
 * and all error fields (stage, code, message, instruction, input) must match.
 */
#define _POSIX_C_SOURCE 200809L
#include <unistd.h>

#include "processor_reference.h"
#include "support.h"

#define CASE_COUNT (sizeof REFERENCE_CASES / sizeof REFERENCE_CASES[0])
static int checked[CASE_COUNT];
static char directory[] = "/tmp/textforge-c-parity-XXXXXX";

static const ReferenceCase *lookup(const char *name) {
    for (size_t i = 0; i < CASE_COUNT; i++) {
        if (strcmp(REFERENCE_CASES[i].name, name) == 0) {
            checked[i] = 1;
            return &REFERENCE_CASES[i];
        }
    }
    fprintf(stderr, "unknown reference case %s\n", name);
    support_failures++;
    return NULL;
}

static int same_text(const char *actual, size_t length, const char *expected) {
    return actual != NULL && length == strlen(expected) && memcmp(actual, expected, length) == 0;
}

/* Compare a call's outcome. `text` may be NULL for successful calls without text. */
static void expect(const char *name, TfStatus status, const char *text, size_t length, TfError *error) {
    const ReferenceCase *reference = lookup(name);
    support_cases++;
    if (reference == NULL) return;
    if (reference->ok) {
        if (status != TF_STATUS_OK) {
            fprintf(stderr, "%s: expected success, got %s: %s\n", name, tf_status_name(status),
                    tf_error_message(error, NULL));
            support_failures++;
        } else if (text != NULL && (length != reference->length || memcmp(text, reference->text, length) != 0)) {
            fprintf(stderr, "%s: output differs from Rust\n", name);
            support_failures++;
        }
    } else {
        size_t message_length = 0, instruction_length = 0, input_length = 0;
        const char *message = tf_error_message(error, &message_length);
        const char *instruction = tf_error_instruction(error, &instruction_length);
        const char *input = tf_error_input(error, &input_length);
        int matches = status != TF_STATUS_OK && error != NULL && tf_error_stage(error) == reference->stage &&
                      tf_error_code(error) == reference->code &&
                      same_text(message, message_length, reference->message) &&
                      same_text(instruction, instruction_length, reference->instruction) &&
                      same_text(input, input_length, reference->input);
        if (!matches) {
            fprintf(stderr, "%s: error differs from Rust (got %s, code %u, '%s' / '%s' / '%s')\n", name,
                    tf_status_name(status), (unsigned)tf_error_code(error), message ? message : "",
                    instruction ? instruction : "", input ? input : "");
            support_failures++;
        }
    }
    tf_error_free(error);
}

static TfFilePath *temporary(const char *file) {
    char path[256];
    snprintf(path, sizeof path, "%s/%s", directory, file);
    TfFilePath *result = NULL;
    CHECK_OK(tf_file_path_from_text(path, TF_NTS, &result, NULL));
    return result;
}

static void touch(const char *file) {
    char path[256];
    snprintf(path, sizeof path, "%s/%s", directory, file);
    FILE *handle = fopen(path, "w");
    CHECK(handle != NULL);
    if (handle) fclose(handle);
}

static char *read_file(const char *file, size_t *length) {
    char path[256];
    snprintf(path, sizeof path, "%s/%s", directory, file);
    FILE *handle = fopen(path, "rb");
    if (handle == NULL) return NULL;
    static char content[4096];
    *length = fread(content, 1, sizeof content, handle);
    fclose(handle);
    return content;
}

/* Run pipeline `id` and record the outcome under `name`. */
static void run_case(TfProcessor *processor, const char *name, const TfString *id, const char *input, size_t length) {
    TfString out = {0};
    TfError *error = NULL;
    TfStatus status = tf_processor_process_all(processor, id->data, id->length, input, length, &out, &error);
    expect(name, status, out.data, out.length, error);
    tf_string_free(&out);
}

static void processor_cases(void) {
    TfProcessor *processor = tf_processor_new();
    TfPipeline *plan = tf_pipeline_new();
    tf_pipeline_add_to_end(tf_pipeline_add_to_beginning(plan, tf_text("🍌")), tf_text("é"));
    TfString id = {0};
    CHECK_OK(tf_pipeline_build(plan, processor, &id, NULL));
    tf_pipeline_free(plan);

    run_case(processor, "unicode", &id, "e\xcc\x81\0", 4);
    run_case(processor, "empty", &id, "", 0);
    TfString missing = {(char *)"missing", 7};
    run_case(processor, "missing_id", &missing, "x", TF_NTS);

    TfInstructionList snapshot = {0};
    TfError *error = NULL;
    TfStatus status = tf_processor_get_transform_vec(processor, "missing", TF_NTS, &snapshot, &error);
    expect("missing_snapshot", status, NULL, 0, error);
    error = NULL;
    status = tf_processor_remove_transform(processor, "missing", TF_NTS, &error);
    expect("missing_remove", status, NULL, 0, error);

    TfString other = {0};
    plan = tf_pipeline_repeat(tf_pipeline_new(), tf_var("missing"));
    CHECK_OK(tf_pipeline_build(plan, processor, &other, NULL));
    tf_pipeline_free(plan);
    run_case(processor, "missing_variable", &other, "é", TF_NTS);
    tf_string_free(&other);
    plan = tf_pipeline_remove_pattern(tf_pipeline_new(), tf_text("["));
    CHECK_OK(tf_pipeline_build(plan, processor, &other, NULL));
    tf_pipeline_free(plan);
    run_case(processor, "invalid_regex", &other, "é", TF_NTS);
    tf_string_free(&other);

    /* Persistence round trips. */
    TfFilePath *text_path = temporary("pipeline.textforge");
    touch("pipeline.textforge");
    CHECK_OK(tf_processor_write_pipeline_to_file(processor, id.data, id.length, text_path, NULL));
    error = NULL;
    status = tf_processor_write_pipeline_to_file(processor, "missing", TF_NTS, text_path, &error);
    expect("persistence_missing", status, NULL, 0, error);
    CHECK_OK(tf_processor_get_pipeline_from_file(processor, text_path, &other, NULL));
    run_case(processor, "text_roundtrip", &other, "x", TF_NTS);
    tf_string_free(&other);
    tf_file_path_free(text_path);

    TfFilePath *byte_path = temporary("pipeline.textforgebc");
    touch("pipeline.textforgebc");
    CHECK_OK(tf_processor_write_to_bytecode_file(processor, id.data, id.length, byte_path, NULL));
    CHECK_OK(tf_processor_read_from_bytecode_file(processor, byte_path, &other, NULL));
    run_case(processor, "bytecode_roundtrip", &other, "x", TF_NTS);
    tf_string_free(&other);
    tf_file_path_free(byte_path);

    /* Buffered processing (spills immediately with a 1-byte threshold). */
    TfFilePath *spill = temporary("");
    TfBufferOptions *options = NULL;
    CHECK_OK(tf_buffer_options_new(1, spill, &options, NULL));
    TfBuffer *input = NULL, *output = NULL;
    CHECK_OK(tf_buffer_from_text("x", TF_NTS, options, &input, NULL));
    error = NULL;
    status = tf_processor_process_all_buffered(processor, id.data, id.length, input, &output, &error);
    TfString text = {0};
    if (status == TF_STATUS_OK) status = tf_buffer_into_string(output, &text, &error);
    expect("buffered", status, text.data, text.length, error);
    tf_string_free(&text);
    tf_buffer_options_free(options);
    tf_file_path_free(spill);

    /* Watchers with a built-in metric and the resulting JSON report. */
    TfObservers *observers = tf_observers_new();
    CHECK_OK(tf_observers_set_builtin(observers, "size", TF_NTS, "current_len_chars", TF_NTS, NULL));
    CHECK_OK(tf_observers_schedule_all(observers, "size", TF_NTS, NULL));
    TfFilePath *report = temporary("report.json");
    TfString watched = {0};
    error = NULL;
    status = tf_processor_process_all_with_watchers(processor, id.data, id.length, "x", TF_NTS, observers, report,
                                                    &watched, &error);
    expect("watched", status, watched.data, watched.length, error);
    tf_string_free(&watched);
    size_t report_length = 0;
    const char *content = read_file("report.json", &report_length);
    expect("report", content ? TF_STATUS_OK : TF_STATUS_EXECUTION, content, report_length, NULL);
    tf_file_path_free(report);
    tf_observers_free(observers);

    tf_string_free(&id);
    tf_processor_free(processor);
}

static void plan_cases(void) {
    TfPipeline *plan = tf_pipeline_add(tf_pipeline_new(), "unknown", TF_NTS, NULL, 0);
    TfError *error = NULL;
    TfStatus status = tf_pipeline_status(plan, &error);
    expect("unknown", status, NULL, 0, error);
    tf_pipeline_clear_error(plan); /* The failed append left the plan unchanged. */
    tf_pipeline_add(plan, "atb", TF_NTS, NULL, 0);
    TfCompiledPipeline *compiled = NULL;
    error = NULL;
    status = tf_pipeline_compile(plan, &compiled, &error);
    expect("arity", status, NULL, 0, error);
    tf_pipeline_free(plan);

    plan = tf_pipeline_add_to_beginning(tf_pipeline_new(), tf_number(1));
    error = NULL;
    status = tf_pipeline_compile(plan, &compiled, &error);
    expect("parameter_type", status, NULL, 0, error);
    tf_pipeline_free(plan);
}

static int failing_observer(const TfObservation *observation, TfObserverResult *result, void *user_data) {
    (void)observation;
    (void)user_data;
    tf_observer_result_set_error(result, "Error: boom", TF_NTS);
    return 1;
}

static int invalid_reader(void *user_data, uint8_t *destination, size_t maximum, size_t *read) {
    int *sent = user_data;
    *read = 0;
    if (!*sent && maximum > 0) {
        destination[0] = 0xFF;
        *read = 1;
        *sent = 1;
    }
    return 0;
}

static void callback_cases(void) {
    TfObservers *observers = tf_observers_new();
    CHECK_OK(tf_observers_set(observers, "bad", TF_NTS, failing_observer, NULL, NULL, NULL));
    CHECK_OK(tf_observers_schedule_all(observers, "bad", TF_NTS, NULL));
    TfProcessor *processor = tf_processor_new();
    TfArg banana = tf_text("🍌");
    TfInstruction *instruction = NULL;
    CHECK_OK(tf_instruction_new("atb", TF_NTS, &banana, 1, &instruction, NULL));
    const TfInstruction *items[] = {instruction};
    TfString id = {0};
    CHECK_OK(tf_processor_add_transform(processor, items, 1, &id, NULL));
    TfFilePath *report = temporary("failed.json");
    TfString out = {0};
    TfError *error = NULL;
    TfStatus status = tf_processor_process_all_with_watchers(processor, id.data, id.length, "x", TF_NTS, observers,
                                                             report, &out, &error);
    expect("callback_error", status, NULL, 0, error);
    error = NULL;
    status = tf_observers_set_builtin(observers, "x", TF_NTS, "missing", TF_NTS, &error);
    expect("builtin_error", status, NULL, 0, error);

    TfFilePath *spill = temporary("");
    TfBufferOptions *options = NULL;
    CHECK_OK(tf_buffer_options_new(1, spill, &options, NULL));
    int sent = 0;
    TfBuffer *buffer = NULL;
    error = NULL;
    status = tf_buffer_from_reader(invalid_reader, &sent, options, &buffer, &error);
    expect("invalid_utf8", status, NULL, 0, error);
    tf_buffer_free(buffer);

    tf_buffer_options_free(options);
    tf_file_path_free(spill);
    tf_file_path_free(report);
    tf_string_free(&id);
    tf_instruction_free(instruction);
    tf_processor_free(processor);
    tf_observers_free(observers);
}

/* Build one pipeline, run it on "a\nb" and compare with the named case. */
static void line(const char *name, TfPipeline *plan) {
    TfProcessor *processor = tf_processor_new();
    TfString id = {0};
    TfError *error = NULL;
    TfStatus status = tf_pipeline_build(plan, processor, &id, &error);
    TfString out = {0};
    if (status == TF_STATUS_OK) status = tf_processor_process_all(processor, id.data, id.length, "a\nb", TF_NTS, &out, &error);
    expect(name, status, out.data, out.length, error);
    tf_string_free(&out);
    tf_string_free(&id);
    tf_pipeline_free(plan);
    tf_processor_free(processor);
}

static void line_cases(void) {
    line("line_append_line_multiline", tf_pipeline_append_line(tf_pipeline_new(), tf_text("x\ny")));
    line("line_prepend_line_multiline", tf_pipeline_prepend_line(tf_pipeline_new(), tf_text("x\ny")));
    line("line_insert_line_multiline", tf_pipeline_insert_line(tf_pipeline_new(), tf_number(1), tf_text("x\ny")));
    line("line_replace_line_multiline", tf_pipeline_replace_line(tf_pipeline_new(), tf_number(1), tf_text("x\ny")));
    line("line_keep_lines_matching_regex", tf_pipeline_keep_lines_matching(tf_pipeline_new(), tf_text("[")));
    line("line_delete_lines_matching_regex", tf_pipeline_delete_lines_matching(tf_pipeline_new(), tf_text("[")));
    line("line_delete_line_zero", tf_pipeline_delete_line(tf_pipeline_new(), tf_number(0)));
    line("line_delete_line_missing", tf_pipeline_delete_line(tf_pipeline_new(), tf_number(100)));
    line("line_insert_line_zero", tf_pipeline_insert_line(tf_pipeline_new(), tf_number(0), tf_text("x")));
    line("line_select_lines_inverted", tf_pipeline_select_lines(tf_pipeline_new(), tf_number(2), tf_number(1)));
    line("line_delete_lines_interval_inverted",
         tf_pipeline_delete_lines_interval(tf_pipeline_new(), tf_number(2), tf_number(1)));
}

static void remove_tree(void) {
    const char *files[] = {"pipeline.textforge", "pipeline.textforgebc", "report.json", "failed.json"};
    char path[256];
    for (size_t i = 0; i < sizeof files / sizeof files[0]; i++) {
        snprintf(path, sizeof path, "%s/%s", directory, files[i]);
        remove(path);
    }
    rmdir(directory);
}

int main(void) {
    if (mkdtemp(directory) == NULL) return EXIT_FAILURE;
    processor_cases();
    plan_cases();
    callback_cases();
    line_cases();
    for (size_t i = 0; i < CASE_COUNT; i++) {
        if (!checked[i]) {
            fprintf(stderr, "reference case %s was not replayed\n", REFERENCE_CASES[i].name);
            support_failures++;
        }
    }
    remove_tree();
    return finish("processor parity");
}
