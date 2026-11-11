/* Concurrency: shared compiled pipelines and serialized processor access (POSIX threads). */
#define _POSIX_C_SOURCE 200809L
#include <pthread.h>

#include "support.h"

enum { THREADS = 8, ITERATIONS = 200 };

typedef struct {
    const TfCompiledPipeline *pipeline;
    TfProcessor *processor;
    const TfString *id;
    int failures;
} Worker;

static void *use_compiled(void *argument) {
    Worker *worker = argument;
    for (int i = 0; i < ITERATIONS; i++) {
        TfString out = {0};
        if (tf_compiled_pipeline_process(worker->pipeline, "é🍌", TF_NTS, &out, NULL) != TF_STATUS_OK ||
            strcmp(out.data, "[É🍌]") != 0)
            worker->failures++;
        tf_string_free(&out);
    }
    return NULL;
}

static void *use_processor(void *argument) {
    Worker *worker = argument;
    for (int i = 0; i < ITERATIONS; i++) {
        TfString out = {0};
        /* Different threads wait for each other instead of reporting TF_STATUS_BUSY. */
        if (tf_processor_process_all(worker->processor, worker->id->data, worker->id->length, "a", TF_NTS, &out, NULL) !=
                TF_STATUS_OK ||
            strcmp(out.data, "[A]") != 0)
            worker->failures++;
        tf_string_free(&out);
        TfStringList ids = {0};
        if (tf_processor_list_transforms(worker->processor, &ids, NULL) != TF_STATUS_OK) worker->failures++;
        tf_string_list_free(&ids);
    }
    return NULL;
}

static TfPipeline *bracket_upper(void) {
    TfPipeline *plan = tf_pipeline_to_uppercase_all(tf_pipeline_new());
    tf_pipeline_add_to_end(tf_pipeline_add_to_beginning(plan, tf_text("[")), tf_text("]"));
    return plan;
}

static void run_workers(void *(*body)(void *), Worker *template) {
    pthread_t threads[THREADS];
    Worker workers[THREADS];
    for (int i = 0; i < THREADS; i++) {
        workers[i] = *template;
        CHECK(pthread_create(&threads[i], NULL, body, &workers[i]) == 0);
    }
    for (int i = 0; i < THREADS; i++) {
        CHECK(pthread_join(threads[i], NULL) == 0);
        CHECK(workers[i].failures == 0);
    }
}

static void shared_compiled_pipeline(void) {
    TfPipeline *plan = bracket_upper();
    TfCompiledPipeline *pipeline = NULL;
    CHECK_OK(tf_pipeline_compile(plan, &pipeline, NULL));
    tf_pipeline_free(plan);
    Worker template = {pipeline, NULL, NULL, 0};
    run_workers(use_compiled, &template);
    tf_compiled_pipeline_free(pipeline);
}

static void shared_processor(void) {
    TfProcessor *processor = tf_processor_new();
    TfPipeline *plan = bracket_upper();
    TfString id = {0};
    CHECK_OK(tf_pipeline_build(plan, processor, &id, NULL));
    tf_pipeline_free(plan);
    Worker template = {NULL, processor, &id, 0};
    run_workers(use_processor, &template);
    tf_string_free(&id);
    tf_processor_free(processor);
}

int main(void) {
    RUN(shared_compiled_pipeline);
    RUN(shared_processor);
    return finish("threads");
}
