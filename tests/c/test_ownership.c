/* Ownership: consumed arguments, instruction snapshots and registry independence.
 * Run under ASan and valgrind, these tests also prove that no path leaks. */
#include "support.h"

static void block_body_consumed_on_every_path(void) {
    /* Pipeline already failed: the body is still released. */
    TfPipeline *plan = tf_pipeline_add(tf_pipeline_new(), "unknown", TF_NTS, NULL, 0);
    tf_pipeline_block(plan, "b", TF_NTS, tf_pipeline_add_to_end(tf_pipeline_new(), tf_text("!")));
    CHECK_STATUS(TF_STATUS_CONVERSION, tf_pipeline_status(plan, NULL));
    tf_pipeline_free(plan);

    /* NULL pipeline: the body is released and NULL propagates. */
    CHECK(tf_pipeline_block(NULL, "b", TF_NTS, tf_pipeline_new()) == NULL);
    CHECK(tf_pipeline_contains_each(NULL, "x", TF_NTS, tf_pipeline_new()) == NULL);

    /* Invalid name: the body is released and the error is sticky. */
    plan = tf_pipeline_block(tf_pipeline_new(), "\xff", 1, tf_pipeline_new());
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_pipeline_status(plan, NULL));
    tf_pipeline_free(plan);

    /* A body's own sticky error becomes the parent's error. */
    TfPipeline *body = tf_pipeline_add(tf_pipeline_new(), "unknown", TF_NTS, NULL, 0);
    plan = tf_pipeline_block_assoc(tf_pipeline_new(), "b", TF_NTS, body);
    CHECK_STATUS(TF_STATUS_CONVERSION, tf_pipeline_status(plan, NULL));
    tf_pipeline_free(plan);

    /* NULL body is an error, not a crash. */
    plan = tf_pipeline_if_do_contains_each(tf_pipeline_new(), "x", TF_NTS, NULL);
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_pipeline_status(plan, NULL));
    tf_pipeline_free(plan);
}

static void self_insertion_is_rejected(void) {
    TfPipeline *plan = tf_pipeline_new();
    CHECK(tf_pipeline_block(plan, "self", TF_NTS, plan) == plan);
    TfError *error = NULL;
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_pipeline_status(plan, &error));
    CHECK(strstr(tf_error_message(error, NULL), "own body") != NULL);
    tf_error_free(error);
    tf_pipeline_free(plan); /* Still owned by the caller: freed exactly once. */
}

static void replace_maps(void) {
    /* Consumed when the pipeline is NULL. */
    TfReplaceMapBuilder *map = tf_replace_map_builder_add_replace(tf_replace_map_builder_new(), tf_text("a"), tf_text("b"));
    CHECK(tf_pipeline_define_replace_map(NULL, tf_text("m"), map) == NULL);

    /* An empty map is rejected by the core; the map is still consumed. */
    TfPipeline *plan = tf_pipeline_define_replace_map(tf_pipeline_new(), tf_text("m"), tf_replace_map_builder_new());
    CHECK_STATUS(TF_STATUS_VALIDATION, tf_pipeline_status(plan, NULL));
    tf_pipeline_free(plan);

    /* A sticky error inside the map builder surfaces on the pipeline. */
    map = tf_replace_map_builder_add_replace(tf_replace_map_builder_new(), tf_text_n("\xff", 1), tf_text("b"));
    tf_replace_map_builder_add_replace(map, tf_text("ignored"), tf_text("x"));
    plan = tf_pipeline_define_replace_map(tf_pipeline_new(), tf_text("m"), map);
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_pipeline_status(plan, NULL));
    tf_pipeline_free(plan);

    /* An unused builder can simply be freed. */
    tf_replace_map_builder_free(tf_replace_map_builder_new());

    /* Variable-valued names and rules. */
    plan = tf_pipeline_val(tf_pipeline_new(), tf_text("pet"), tf_text("cat"));
    map = tf_replace_map_builder_add_replace(tf_replace_map_builder_new(), tf_var("pet"), tf_text("dog"));
    tf_pipeline_define_replace_map(plan, tf_text("m"), map);
    tf_pipeline_execute_replace_map(plan, tf_text("m"));
    CHECK(run_text(plan, "cat!", "dog!"));
    tf_pipeline_free(plan);
}

static void instruction_views(void) {
    TfArg inner_args[] = {tf_text("🍌")};
    TfInstruction *inner = NULL;
    CHECK_OK(tf_instruction_new("atb", TF_NTS, inner_args, 1, &inner, NULL));
    TfArg outer_args[] = {tf_var("times"), tf_nested(inner)};
    TfInstruction *outer = NULL;
    CHECK_OK(tf_instruction_new("iter", TF_NTS, outer_args, 2, &outer, NULL));
    tf_instruction_free(inner); /* The outer instruction owns a copy. */

    size_t length = 0;
    CHECK(strcmp(tf_instruction_name(outer, &length), "iter") == 0 && length == 4);
    CHECK(tf_instruction_argument_count(outer) == 2);
    TfArg times, body;
    CHECK_OK(tf_instruction_argument(outer, 0, &times, NULL));
    CHECK(times.kind == TF_ARG_VARIABLE && strcmp(times.text, "times") == 0 && times.length == 5);
    CHECK_OK(tf_instruction_argument(outer, 1, &body, NULL));
    CHECK(body.kind == TF_ARG_INSTRUCTION && strcmp(tf_instruction_name(body.instruction, NULL), "atb") == 0);

    /* Views can be passed straight back to builders. */
    TfInstruction *copy = tf_instruction_clone(outer);
    TfPipeline *plan = tf_pipeline_val(tf_pipeline_new(), tf_text("times"), tf_number(2));
    tf_pipeline_iter(plan, times, body);
    tf_pipeline_push_token(plan, copy);
    CHECK(run_text(plan, "x", "🍌🍌🍌🍌x"));
    tf_pipeline_free(plan);
    tf_instruction_free(copy);
    tf_instruction_free(outer);
}

static void nesting_limit(void) {
    TfArg leaf_args[] = {tf_text("!")};
    TfInstruction *current = NULL;
    CHECK_OK(tf_instruction_new("ate", TF_NTS, leaf_args, 1, &current, NULL));
    TfStatus status = TF_STATUS_OK;
    int depth = 0;
    while (status == TF_STATUS_OK && depth < 40) {
        TfArg args[] = {tf_number(1), tf_nested(current)};
        TfInstruction *next = NULL;
        status = tf_instruction_new("iter", TF_NTS, args, 2, &next, NULL);
        if (status == TF_STATUS_OK) {
            tf_instruction_free(current);
            current = next;
            depth++;
        }
    }
    CHECK(status == TF_STATUS_INVALID_ARGUMENT && depth == 32);
    tf_instruction_free(current);
}

static void registry_snapshots(void) {
    TfProcessor *processor = tf_processor_new();
    TfPipeline *plan = tf_pipeline_add_to_end(tf_pipeline_trim_both_sides(tf_pipeline_new()), tf_text("."));
    TfString id = {0};
    CHECK_OK(tf_pipeline_build(plan, processor, &id, NULL));
    tf_pipeline_free(plan); /* Registered snapshots are independent of the plan. */

    TfInstructionList list = {0};
    CHECK_OK(tf_processor_get_transform_vec(processor, id.data, id.length, &list, NULL));
    CHECK(list.length == 2);
    TfString copy = {0};
    CHECK_OK(tf_processor_add_transform(processor, (const TfInstruction *const *)list.items, list.length, &copy, NULL));
    tf_instruction_list_free(&list);
    CHECK(list.items == NULL && list.length == 0);

    TfStringList lines = {0};
    CHECK_OK(tf_processor_get_text_transform_vec(processor, copy.data, copy.length, &lines, NULL));
    CHECK(lines.length == 2 && strstr(lines.items[1].data, "ate") != NULL);
    tf_string_list_free(&lines);

    TfString clone = {0};
    CHECK_OK(tf_processor_clone_pipeline(processor, id.data, id.length, &clone, NULL));
    TfStringList ids = {0};
    CHECK_OK(tf_processor_list_transforms(processor, &ids, NULL));
    CHECK(ids.length == 3);
    for (size_t i = 1; i < ids.length; i++) CHECK(strcmp(ids.items[i - 1].data, ids.items[i].data) < 0);
    tf_string_list_free(&ids);

    CHECK_OK(tf_processor_remove_transform(processor, id.data, id.length, NULL));
    TfString out = {0};
    CHECK_OK(tf_processor_run_transform(processor, clone.data, clone.length, "  x  ", TF_NTS, &out, NULL));
    CHECK(strcmp(out.data, "x.") == 0);
    tf_string_free(&out);

    TfInstruction *single = NULL;
    TfArg args[] = {tf_text(">")};
    CHECK_OK(tf_instruction_new("atb", TF_NTS, args, 1, &single, NULL));
    CHECK_OK(tf_processor_process_single(processor, single, "x", TF_NTS, &out, NULL));
    CHECK(strcmp(out.data, ">x") == 0);
    tf_string_free(&out);
    tf_instruction_free(single);

    tf_string_free(&clone);
    tf_string_free(&copy);
    tf_string_free(&id);
    tf_processor_free(processor);
}

static void buffers_consumed_on_failure(void) {
    TfBufferOptions *options = NULL;
    CHECK_OK(tf_buffer_options_new(TF_DEFAULT_MEMORY_THRESHOLD, NULL, &options, NULL));
    TfProcessor *processor = tf_processor_new();
    TfBuffer *input = NULL, *output = NULL;
    CHECK_OK(tf_buffer_from_text("abc", TF_NTS, options, &input, NULL));
    TfError *error = NULL;
    CHECK_STATUS(TF_STATUS_EXECUTION, tf_processor_process_all_buffered(processor, "missing", TF_NTS, input, &output, &error));
    CHECK(output == NULL);
    tf_error_free(error);

    CHECK_OK(tf_buffer_from_text("abc", TF_NTS, options, &input, NULL));
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_buffer_into_string(input, NULL, NULL)); /* consumed anyway */

    CHECK_OK(tf_buffer_from_text("abc", TF_NTS, options, &input, NULL));
    CHECK_OK(tf_processor_process_single_buffered(processor, "atb \"> \";", TF_NTS, input, &output, NULL));
    TfString text = {0};
    CHECK_OK(tf_buffer_into_string(output, &text, NULL));
    CHECK(strcmp(text.data, "> abc") == 0);
    tf_string_free(&text);

    tf_processor_free(processor);
    tf_buffer_options_free(options);
}

int main(void) {
    RUN(block_body_consumed_on_every_path);
    RUN(self_insertion_is_rejected);
    RUN(replace_maps);
    RUN(instruction_views);
    RUN(nesting_limit);
    RUN(registry_snapshots);
    RUN(buffers_consumed_on_failure);
    return finish("ownership");
}
