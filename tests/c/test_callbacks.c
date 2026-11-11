/* Watcher and stream callbacks: data passed, user_data lifetime, failures and re-entrancy. */
#include "support.h"

typedef struct {
    int calls;
    int destroyed;
    char last_instruction[16];
    TfObservers *observers;  /* for re-entrancy checks */
    TfProcessor *processor;
    TfCompiledPipeline *independent;
    TfStatus reentrant_observers;
    TfStatus reentrant_processor;
    TfStatus independent_status;
} Probe;

static void destroy_probe(void *user_data) { ((Probe *)user_data)->destroyed++; }

static int record_observation(const TfObservation *observation, TfObserverResult *result, void *user_data) {
    Probe *probe = user_data;
    probe->calls++;
    size_t length = 0;
    const char *instruction = tf_observation_instruction(observation, &length);
    snprintf(probe->last_instruction, sizeof probe->last_instruction, "%.*s", (int)length, instruction);
    const char *before = tf_observation_before(observation, NULL);
    const char *after = tf_observation_after(observation, NULL);
    const char *current = tf_observation_current(observation, NULL);
    char text[64];
    snprintf(text, sizeof text, "%s>%s|%s", before, after ? after : "(none)", current);
    return tf_observer_result_set(result, text, TF_NTS) == TF_STATUS_OK ? 0 : 1;
}

static int reenter(const TfObservation *observation, TfObserverResult *result, void *user_data) {
    (void)observation;
    Probe *probe = user_data;
    probe->reentrant_observers = tf_observers_reset(probe->observers, NULL);
    TfString out = {0};
    probe->reentrant_processor = tf_processor_process_all(probe->processor, "any", TF_NTS, "x", TF_NTS, &out, NULL);
    probe->independent_status = tf_compiled_pipeline_process(probe->independent, "x", TF_NTS, &out, NULL);
    tf_string_free(&out);
    return tf_observer_result_set(result, "ok", TF_NTS) == TF_STATUS_OK ? 0 : 1;
}

static int no_result(const TfObservation *observation, TfObserverResult *result, void *user_data) {
    (void)observation;
    (void)result;
    (void)user_data;
    return 0;
}

static int bad_status(const TfObservation *observation, TfObserverResult *result, void *user_data) {
    (void)observation;
    (void)result;
    (void)user_data;
    return 42;
}

static TfString register_upper(TfProcessor *processor) {
    TfPipeline *plan = tf_pipeline_to_uppercase_all(tf_pipeline_new());
    TfString id = {0};
    CHECK_OK(tf_pipeline_build(plan, processor, &id, NULL));
    tf_pipeline_free(plan);
    return id;
}

static TfFilePath *report_path(void) {
    TfFilePath *path = NULL;
    CHECK_OK(tf_file_path_from_text("/tmp/textforge-c-callbacks/report.json", TF_NTS, &path, NULL));
    return path;
}

static void observations_and_destroy(void) {
    Probe probe = {0};
    TfObservers *observers = tf_observers_new();
    CHECK_OK(tf_observers_set(observers, "probe", TF_NTS, record_observation, &probe, destroy_probe, NULL));
    CHECK_OK(tf_observers_schedule_all(observers, "probe", TF_NTS, NULL));

    TfProcessor *processor = tf_processor_new();
    TfString id = register_upper(processor);
    TfFilePath *report = report_path();
    TfString out = {0};
    CHECK_OK(tf_processor_process_all_with_watchers(processor, id.data, id.length, "ab", TF_NTS, observers, report,
                                                    &out, NULL));
    CHECK(strcmp(out.data, "AB") == 0);
    CHECK(probe.calls >= 1 && strncmp(probe.last_instruction, "tua;", 4) == 0);
    tf_string_free(&out);

    /* Manual run with no "after" text. */
    int before = probe.calls;
    CHECK_OK(tf_observers_run(observers, "c", TF_NTS, "b", TF_NTS, NULL, 0, "manual", TF_NTS, NULL));
    CHECK(probe.calls == before + 1 && strcmp(probe.last_instruction, "manual") == 0);
    CHECK_OK(tf_observers_write_report(observers, report, NULL));
    CHECK_OK(tf_observers_reset(observers, NULL));

    /* Replacing a registration destroys the previous user_data exactly once. */
    Probe second = {0};
    CHECK_OK(tf_observers_set(observers, "probe", TF_NTS, record_observation, &second, destroy_probe, NULL));
    CHECK(probe.destroyed == 1 && second.destroyed == 0);
    tf_observers_free(observers);
    CHECK(probe.destroyed == 1 && second.destroyed == 1);

    tf_file_path_free(report);
    tf_string_free(&id);
    tf_processor_free(processor);
    remove("/tmp/textforge-c-callbacks/report.json");
    remove("/tmp/textforge-c-callbacks");
}

static void failed_registration_destroys_user_data(void) {
    Probe probe = {0};
    TfObservers *observers = tf_observers_new();
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT,
                 tf_observers_set(observers, "\xff", 1, record_observation, &probe, destroy_probe, NULL));
    CHECK(probe.destroyed == 1);
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_observers_set(observers, "x", TF_NTS, NULL, &probe, destroy_probe, NULL));
    CHECK(probe.destroyed == 2);
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_observers_set(NULL, "x", TF_NTS, record_observation, &probe, destroy_probe, NULL));
    CHECK(probe.destroyed == 3);
    tf_observers_free(observers);
}

static void callback_failures(void) {
    TfProcessor *processor = tf_processor_new();
    TfString id = register_upper(processor);
    TfFilePath *report = report_path();
    int (*callbacks[])(const TfObservation *, TfObserverResult *, void *) = {no_result, bad_status};
    const char *messages[] = {"without a result", "status 42"};
    for (int i = 0; i < 2; i++) {
        TfObservers *observers = tf_observers_new();
        CHECK_OK(tf_observers_set(observers, "w", TF_NTS, callbacks[i], NULL, NULL, NULL));
        CHECK_OK(tf_observers_schedule_all(observers, "w", TF_NTS, NULL));
        TfString out = {0};
        TfError *error = NULL;
        CHECK_STATUS(TF_STATUS_CALLBACK, tf_processor_process_all_with_watchers(processor, id.data, id.length, "a",
                                                                               TF_NTS, observers, report, &out, &error));
        CHECK(tf_error_stage(error) == TF_STAGE_CALLBACK && strstr(tf_error_message(error, NULL), messages[i]) != NULL);
        CHECK(out.data == NULL);
        tf_error_free(error);
        tf_observers_free(observers);
    }
    tf_file_path_free(report);
    tf_string_free(&id);
    tf_processor_free(processor);
}

static void reentrancy_is_reported(void) {
    TfProcessor *processor = tf_processor_new();
    TfString id = register_upper(processor);
    TfPipeline *plan = tf_pipeline_new();
    Probe probe = {0};
    CHECK_OK(tf_pipeline_compile(plan, &probe.independent, NULL));
    tf_pipeline_free(plan);
    probe.observers = tf_observers_new();
    probe.processor = processor;
    CHECK_OK(tf_observers_set(probe.observers, "r", TF_NTS, reenter, &probe, NULL, NULL));
    CHECK_OK(tf_observers_schedule_all(probe.observers, "r", TF_NTS, NULL));
    TfFilePath *report = report_path();
    TfString out = {0};
    CHECK_OK(tf_processor_process_all_with_watchers(processor, id.data, id.length, "a", TF_NTS, probe.observers, report,
                                                    &out, NULL));
    CHECK(probe.reentrant_observers == TF_STATUS_BUSY);
    CHECK(probe.reentrant_processor == TF_STATUS_BUSY);
    CHECK(probe.independent_status == TF_STATUS_OK);
    tf_string_free(&out);
    tf_file_path_free(report);
    tf_observers_free(probe.observers);
    tf_compiled_pipeline_free(probe.independent);
    tf_string_free(&id);
    tf_processor_free(processor);
    remove("/tmp/textforge-c-callbacks/report.json");
    remove("/tmp/textforge-c-callbacks");
}

/* ---- streams ---- */

typedef struct {
    const char *data;
    size_t length;
    size_t offset;
    size_t chunk;
} Source;

static int read_chunks(void *user_data, uint8_t *destination, size_t maximum, size_t *read) {
    Source *source = user_data;
    size_t count = source->length - source->offset;
    if (count > source->chunk) count = source->chunk;
    if (count > maximum) count = maximum;
    memcpy(destination, source->data + source->offset, count);
    source->offset += count;
    *read = count;
    return 0;
}

static int read_too_much(void *user_data, uint8_t *destination, size_t maximum, size_t *read) {
    (void)user_data;
    (void)destination;
    *read = maximum + 1;
    return 0;
}

typedef struct {
    char data[64];
    size_t length;
    int calls;
    TfBuffer *self;
    TfStatus reentrant;
} Sink;

static int write_one_byte(void *user_data, const uint8_t *bytes, size_t length, size_t *written) {
    Sink *sink = user_data;
    sink->calls++;
    if (sink->self != NULL) {
        TfString text = {0};
        sink->reentrant = tf_buffer_into_string(sink->self, &text, NULL); /* must not free a busy buffer */
        sink->self = NULL;
    }
    if (length == 0 || sink->length >= sizeof sink->data) {
        *written = 0;
        return length == 0 ? 0 : 1;
    }
    sink->data[sink->length++] = (char)bytes[0];
    *written = 1;
    return 0;
}

static int write_fails(void *user_data, const uint8_t *bytes, size_t length, size_t *written) {
    (void)user_data;
    (void)bytes;
    (void)length;
    *written = 0;
    return 7;
}

static void streams(void) {
    TfBufferOptions *options = NULL;
    CHECK_OK(tf_buffer_options_new(2, NULL, &options, NULL));
    const char text[] = "h\xc3\xa9llo \xf0\x9f\x8d\x8c"; /* 1-byte chunks split UTF-8 sequences */
    Source source = {text, sizeof text - 1, 0, 1};
    TfBuffer *buffer = NULL;
    CHECK_OK(tf_buffer_from_reader(read_chunks, &source, options, &buffer, NULL));
    uint64_t length = 0;
    bool spilled = false, empty = true;
    CHECK_OK(tf_buffer_len_bytes(buffer, &length, NULL));
    CHECK_OK(tf_buffer_is_spilled(buffer, &spilled, NULL));
    CHECK_OK(tf_buffer_is_empty(buffer, &empty, NULL));
    CHECK(length == sizeof text - 1 && spilled && !empty);

    TfBytes past_end = {0};
    CHECK_OK(tf_buffer_read_at(buffer, 1000, 10, &past_end, NULL));
    CHECK(past_end.length == 0 && past_end.data != NULL);
    tf_bytes_free(&past_end);

    Sink sink = {0};
    sink.self = buffer;
    uint64_t copied = 0;
    CHECK_OK(tf_buffer_copy_to(buffer, write_one_byte, &sink, &copied, NULL));
    CHECK(sink.reentrant == TF_STATUS_BUSY);
    CHECK(copied == sizeof text - 1 && sink.length == copied && memcmp(sink.data, text, sink.length) == 0);

    TfError *error = NULL;
    CHECK_STATUS(TF_STATUS_EXECUTION, tf_buffer_copy_to(buffer, write_fails, NULL, &copied, &error));
    CHECK(strstr(tf_error_message(error, NULL), "status 7") != NULL || tf_error_code(error) != 0);
    tf_error_free(error);
    error = NULL;
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_buffer_copy_to(buffer, NULL, NULL, &copied, NULL));
    tf_buffer_free(buffer);

    CHECK_STATUS(TF_STATUS_EXECUTION, tf_buffer_from_reader(read_too_much, NULL, options, &buffer, &error));
    tf_error_free(error);
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_buffer_from_reader(NULL, NULL, options, &buffer, NULL));

    CHECK_OK(tf_buffer_from_text("", 0, options, &buffer, NULL));
    CHECK_OK(tf_buffer_is_empty(buffer, &empty, NULL));
    CHECK(empty);
    CHECK_OK(tf_buffer_spill(buffer, NULL));
    tf_buffer_free(buffer);
    tf_buffer_options_free(options);
}

int main(void) {
    RUN(observations_and_destroy);
    RUN(failed_registration_destroys_user_data);
    RUN(callback_failures);
    RUN(reentrancy_is_reported);
    RUN(streams);
    return finish("callbacks");
}
