/* Files: paths, process_file, batches (global and caller pool), buffered files. */
#define _POSIX_C_SOURCE 200809L
#include <unistd.h>

#include "support.h"

static char directory[] = "/tmp/textforge-c-files-XXXXXX";

static TfFilePath *path_to(const char *name) {
    char text[256];
    snprintf(text, sizeof text, "%s/%s", directory, name);
    TfFilePath *path = NULL;
    CHECK_OK(tf_file_path_from_text(text, TF_NTS, &path, NULL));
    return path;
}

static void write_file(const char *name, const char *content) {
    char text[256];
    snprintf(text, sizeof text, "%s/%s", directory, name);
    FILE *file = fopen(text, "wb");
    CHECK(file != NULL);
    if (file) {
        fputs(content, file);
        fclose(file);
    }
}

static int file_equals(const char *name, const char *expected) {
    char text[256], content[256] = {0};
    snprintf(text, sizeof text, "%s/%s", directory, name);
    FILE *file = fopen(text, "rb");
    if (file == NULL) return 0;
    size_t length = fread(content, 1, sizeof content - 1, file);
    fclose(file);
    return length == strlen(expected) && memcmp(content, expected, length) == 0;
}

static void remove_file(const char *name) {
    char text[256];
    snprintf(text, sizeof text, "%s/%s", directory, name);
    remove(text);
}

static TfString register_plan(TfProcessor *processor, TfPipeline *plan) {
    TfString id = {0};
    CHECK_OK(tf_pipeline_build(plan, processor, &id, NULL));
    tf_pipeline_free(plan);
    return id;
}

static void paths(void) {
    TfFilePath *path = path_to("é.txt");
    TfString text = {0};
    bool representable = false;
    CHECK_OK(tf_file_path_to_text(path, &text, &representable, NULL));
    CHECK(representable && strstr(text.data, "é.txt") != NULL);
    tf_string_free(&text);
    TfWideString wide = {0};
    CHECK_STATUS(TF_STATUS_CONVERSION, tf_file_path_to_windows_wide(path, &wide, NULL));
    tf_file_path_free(path);
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_file_path_from_text("\xff", 1, &path, NULL));
}

static void process_files(void) {
    TfProcessor *processor = tf_processor_new();
    TfString upper = register_plan(processor, tf_pipeline_to_uppercase_all(tf_pipeline_new()));
    TfString reverse = register_plan(processor, tf_pipeline_to_reverse(tf_pipeline_new()));
    write_file("a.txt", "abc");
    write_file("b.txt", "xyz");

    /* Non-UTF-8 native file name. */
    char raw[256];
    int length = snprintf(raw, sizeof raw, "%s/out-\xff.txt", directory);
    TfFilePath *native = NULL;
    CHECK_OK(tf_file_path_from_native_bytes((const uint8_t *)raw, (size_t)length, &native, NULL));
    TfFilePath *a = path_to("a.txt");
    CHECK_OK(tf_processor_process_file(processor, upper.data, upper.length, a, native, NULL));
    CHECK(file_equals("out-\xff.txt", "ABC"));
    remove_file("out-\xff.txt");

    TfFilePath *b = path_to("b.txt");
    TfFilePath *out_a = path_to("out-a.txt");
    TfFilePath *out_b = path_to("out-b.txt");
    TfFilePath *missing = path_to("missing.txt");
    TfFilePath *out_c = path_to("out-c.txt");
    TfFileTask tasks[] = {
        {a, upper.data, upper.length, out_a},
        {b, reverse.data, reverse.length, out_b},
        {missing, upper.data, upper.length, out_c},
    };
    TfError *results[3];
    CHECK_OK(tf_processor_process_batch(processor, tasks, 3, results, NULL));
    CHECK(results[0] == NULL && results[1] == NULL && results[2] != NULL);
    CHECK(file_equals("out-a.txt", "ABC") && file_equals("out-b.txt", "zyx"));
    for (int i = 0; i < 3; i++) tf_error_free(results[i]);

    TfBatchPool *pool = NULL;
    CHECK_OK(tf_batch_pool_new(2, &pool, NULL));
    tasks[0].pipeline_id = reverse.data;
    tasks[0].pipeline_id_length = reverse.length;
    CHECK_OK(tf_processor_process_batch_in_pool(processor, pool, tasks, 2, results, NULL));
    CHECK(results[0] == NULL && results[1] == NULL && file_equals("out-a.txt", "cba"));
    tf_batch_pool_free(pool);
    CHECK_STATUS(TF_STATUS_INVALID_ARGUMENT, tf_processor_process_batch(processor, tasks, 2, NULL, NULL));

    /* Buffered file processing. */
    TfBufferOptions *options = NULL;
    CHECK_OK(tf_buffer_options_new(1, NULL, &options, NULL));
    TfBuffer *output = NULL;
    CHECK_OK(tf_processor_process_file_buffered(processor, upper.data, upper.length, a, options, &output, NULL));
    TfFilePath *nested = path_to("nested/dir/out.txt");
    uint64_t written = 0;
    CHECK_OK(tf_buffer_write_to_file(output, nested, &written, NULL));
    CHECK(written == 3 && file_equals("nested/dir/out.txt", "ABC"));
    tf_buffer_free(output);
    tf_file_path_free(nested);

    TfBufferTask buffer_tasks[] = {{upper.data, upper.length, b}, {upper.data, upper.length, missing}};
    TfBuffer *outputs[2];
    CHECK_OK(tf_processor_process_batch_buffered(processor, buffer_tasks, 2, options, outputs, results, NULL));
    CHECK(outputs[0] != NULL && results[0] == NULL && outputs[1] == NULL && results[1] != NULL);
    TfString text = {0};
    CHECK_OK(tf_buffer_into_string(outputs[0], &text, NULL));
    CHECK(strcmp(text.data, "XYZ") == 0);
    tf_string_free(&text);
    tf_error_free(results[1]);
    tf_buffer_options_free(options);

    TfFilePath *all[] = {native, a, b, out_a, out_b, missing, out_c};
    for (size_t i = 0; i < sizeof all / sizeof all[0]; i++) tf_file_path_free(all[i]);
    tf_string_free(&upper);
    tf_string_free(&reverse);
    tf_processor_free(processor);
    const char *files[] = {"a.txt", "b.txt", "out-a.txt", "out-b.txt", "nested/dir/out.txt", "nested/dir", "nested"};
    for (size_t i = 0; i < sizeof files / sizeof files[0]; i++) remove_file(files[i]);
}

static void persistence_aliases(void) {
    TfProcessor *processor = tf_processor_new();
    TfPipeline *plan = tf_pipeline_add_to_end(tf_pipeline_new(), tf_text("line\nbreak"));
    TfString id = register_plan(processor, plan);
    /* Text cannot hold a literal line break; nothing is written. Bytecode is lossless. */
    TfFilePath *text = path_to("p.tft");
    TfError *error = NULL;
    CHECK_STATUS(TF_STATUS_PERSISTENCE, tf_processor_write_pipeline_to_file(processor, id.data, id.length, text, &error));
    tf_error_free(error);
    char written[256];
    snprintf(written, sizeof written, "%s/p.tft", directory);
    CHECK(access(written, F_OK) != 0);
    TfFilePath *bytecode = path_to("p.tfb");
    CHECK_OK(tf_processor_write_to_bytecode_file(processor, id.data, id.length, bytecode, NULL));
    TfString restored = {0};
    CHECK_OK(tf_processor_read_from_bytecode_file(processor, bytecode, &restored, NULL));
    TfString out = {0};
    CHECK_OK(tf_processor_process_all(processor, restored.data, restored.length, "a", TF_NTS, &out, NULL));
    CHECK(strcmp(out.data, "aline\nbreak") == 0);
    tf_string_free(&out);
    tf_string_free(&restored);
    tf_file_path_free(text);
    tf_file_path_free(bytecode);
    tf_string_free(&id);
    tf_processor_free(processor);
    remove_file("p.tfb");
}

int main(void) {
    if (mkdtemp(directory) == NULL) return EXIT_FAILURE;
    RUN(paths);
    RUN(process_files);
    RUN(persistence_aliases);
    rmdir(directory);
    return finish("files");
}
