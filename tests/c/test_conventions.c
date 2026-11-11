/* Adapter conventions: NULL handling, error slots, out-parameters, UTF-8 and lossless text. */
#include "support.h"

static void null_arguments(void) {
    TfError *error = NULL;
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_pipeline_compile(NULL, NULL, &error));
    CHECK(tf_error_stage(error) == TF_STAGE_ADAPTER && tf_error_code(error) == 0);
    CHECK(strstr(tf_error_message(error, NULL), "must not be NULL") != NULL);
    tf_error_free(error);

    /* Errors are optional. */
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_pipeline_compile(NULL, NULL, NULL));

    /* Builders propagate NULL instead of crashing. */
    CHECK(tf_pipeline_add_to_end(NULL, tf_text("x")) == NULL);
    CHECK(tf_pipeline_delete_first(tf_pipeline_trim_both_sides(NULL)) == NULL);
    CHECK(tf_replace_map_builder_add_replace(NULL, tf_text("a"), tf_text("b")) == NULL);
    CHECK(tf_processor_create_pipeline(NULL) == NULL);

    /* Free functions accept NULL. */
    tf_pipeline_free(NULL);
    tf_compiled_pipeline_free(NULL);
    tf_processor_free(NULL);
    tf_instruction_free(NULL);
    tf_error_free(NULL);
    tf_file_path_free(NULL);
    tf_observers_free(NULL);
    tf_buffer_free(NULL);
    tf_buffer_options_free(NULL);
    tf_batch_pool_free(NULL);
    tf_replace_map_builder_free(NULL);
    tf_string_free(NULL);
    tf_bytes_free(NULL);
    tf_wide_string_free(NULL);
    tf_string_list_free(NULL);
    tf_instruction_list_free(NULL);

    /* NULL getters are safe and report zero length. */
    size_t length = 99;
    CHECK(tf_error_message(NULL, &length) == NULL && length == 0);
    CHECK(tf_instruction_name(NULL, NULL) == NULL);
    CHECK(tf_instruction_argument_count(NULL) == 0);
    CHECK(tf_error_status(NULL) == TF_STATUS_INVALID_ARGUMENT);
}

static void error_slot_must_be_empty(void) {
    TfError *first = NULL;
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_pipeline_compile(NULL, NULL, &first));
    TfError *kept = first;
    /* A non-NULL slot is refused and left untouched, so the first error does not leak. */
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_pipeline_compile(NULL, NULL, &first));
    CHECK(first == kept);
    tf_error_free(first);
}

static void out_parameters_written_only_on_success(void) {
    TfPipeline *plan = tf_pipeline_add(tf_pipeline_new(), "atb", TF_NTS, NULL, 0); /* arity error */
    TfCompiledPipeline *sentinel = (TfCompiledPipeline *)&plan;
    TfCompiledPipeline *compiled = sentinel;
    TfError *error = NULL;
    CHECK_STATUS(TF_STATUS_VALIDATION, tf_pipeline_compile(plan, &compiled, &error));
    CHECK(compiled == sentinel);
    tf_error_free(error);
    tf_pipeline_free(plan);
}

static void utf8_and_lengths(void) {
    TfPipeline *plan = tf_pipeline_new();
    TfCompiledPipeline *pipeline = NULL;
    CHECK_OK(tf_pipeline_compile(plan, &pipeline, NULL));
    TfString out = {0};
    TfError *error = NULL;

    /* Invalid UTF-8 is rejected with its byte offset. */
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_compiled_pipeline_process(pipeline, "ab\xff", 3, &out, &error));
    CHECK(strstr(tf_error_message(error, NULL), "byte offset 2") != NULL);
    CHECK(out.data == NULL);
    tf_error_free(error);
    error = NULL;

    /* TF_NTS requires a non-NULL pointer; an explicit zero length accepts NULL. */
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_compiled_pipeline_process(pipeline, NULL, TF_NTS, &out, NULL));
    CHECK_OK(tf_compiled_pipeline_process(pipeline, NULL, 0, &out, NULL));
    CHECK(out.data != NULL && out.length == 0 && out.data[0] == '\0');
    tf_string_free(&out);
    CHECK(out.data == NULL && out.length == 0);
    tf_string_free(&out); /* Second release of a reset string is harmless. */

    /* Embedded NUL, emoji and combining characters round-trip byte for byte. */
    const char input[] = "e\xcc\x81\0\xf0\x9f\x8d\x8c";
    CHECK_OK(tf_compiled_pipeline_process(pipeline, input, sizeof input - 1, &out, NULL));
    CHECK(out.length == sizeof input - 1 && memcmp(out.data, input, out.length) == 0);
    tf_string_free(&out);

    tf_compiled_pipeline_free(pipeline);
    tf_pipeline_free(plan);
}

static void unicode_transforms(void) {
    TfPipeline *plan = tf_pipeline_to_uppercase_all(tf_pipeline_new());
    CHECK(run_text(plan, "", ""));
    CHECK(run_text(plan, "ß🍌é", "SS🍌É"));
    tf_pipeline_free(plan);

    plan = tf_pipeline_delete_last(tf_pipeline_new());
    CHECK(run_text(plan, "a🍌", "a"));
    tf_pipeline_free(plan);

    plan = tf_pipeline_normalize_unicode(tf_pipeline_new(), tf_text("nfc"));
    CHECK(run_text(plan, "e\xcc\x81", "\xc3\xa9"));
    tf_pipeline_free(plan);
}

static void invalid_arguments_are_sticky(void) {
    TfArg zeroed = {0}; /* kind 0 is never valid */
    TfPipeline *plan = tf_pipeline_add_to_end(tf_pipeline_new(), zeroed);
    tf_pipeline_add_to_end(plan, tf_text("ignored after the first failure"));
    TfError *error = NULL;
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_pipeline_status(plan, &error));
    CHECK(strstr(tf_error_message(error, NULL), "unknown kind 0") != NULL);
    tf_error_free(error);
    error = NULL;

    /* The status does not clear the error; compile reports the same failure. */
    TfCompiledPipeline *compiled = NULL;
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_pipeline_compile(plan, &compiled, NULL));
    tf_pipeline_clear_error(plan);
    CHECK_OK(tf_pipeline_status(plan, NULL));
    tf_pipeline_add_to_end(plan, tf_text("!"));
    CHECK(run_text(plan, "a", "a!"));
    tf_pipeline_free(plan);

    /* Invalid UTF-8 in an argument, and an explicit length that keeps NUL bytes. */
    plan = tf_pipeline_add_to_end(tf_pipeline_new(), tf_text_n("\xc3", 1));
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_pipeline_status(plan, NULL));
    tf_pipeline_free(plan);
    plan = tf_pipeline_add_to_end(tf_pipeline_new(), tf_text_n("a\0b", 3));
    TfCompiledPipeline *pipeline = NULL;
    CHECK_OK(tf_pipeline_compile(plan, &pipeline, NULL));
    TfString out = {0};
    CHECK_OK(tf_compiled_pipeline_process(pipeline, "x", TF_NTS, &out, NULL));
    CHECK(out.length == 4 && memcmp(out.data, "xa\0b", 4) == 0);
    tf_string_free(&out);
    tf_compiled_pipeline_free(pipeline);
    tf_pipeline_free(plan);
}

static void numbers_and_variables(void) {
    /* Numeric assignment values are normalized to decimal text like the Rust builder. */
    TfPipeline *plan = tf_pipeline_val(tf_pipeline_new(), tf_text("n"), tf_number(3));
    tf_pipeline_repeat(plan, tf_var("n"));
    CHECK(run_text(plan, "ab", "ababab"));
    tf_pipeline_free(plan);

    /* Text interpolation still applies to literal text. */
    plan = tf_pipeline_val(tf_pipeline_new(), tf_text("who"), tf_text("C"));
    tf_pipeline_add_to_beginning(plan, tf_text("{{who}} says "));
    CHECK(run_text(plan, "hi", "C says hi"));
    tf_pipeline_free(plan);
}

static void metadata(void) {
    CHECK(strcmp(tf_version(), "0.1.0") == 0);
    CHECK(strcmp(tf_core_version(), "0.2.0") == 0);
    TfFeatures features = tf_features();
    CHECK(features.bytecode && features.watchers && features.buffered);
    CHECK(strcmp(tf_status_name(TF_STATUS_OK), "TF_STATUS_OK") == 0);
    CHECK(strcmp(tf_status_name(1234), "TF_STATUS_UNRECOGNIZED") == 0);
}

int main(void) {
    RUN(null_arguments);
    RUN(error_slot_must_be_empty);
    RUN(out_parameters_written_only_on_success);
    RUN(utf8_and_lengths);
    RUN(unicode_transforms);
    RUN(invalid_arguments_are_sticky);
    RUN(numbers_and_variables);
    RUN(metadata);
    return finish("conventions");
}
