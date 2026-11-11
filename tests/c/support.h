/* Minimal assertion helpers shared by the C test programs (no external framework). */
#ifndef TEXTFORGE_TEST_SUPPORT_H
#define TEXTFORGE_TEST_SUPPORT_H

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "textforge.h"

static int support_failures = 0;
static int support_cases = 0;

/* Record a failed condition without aborting, so one run reports every failure. */
#define CHECK(condition)                                                              \
    do {                                                                              \
        if (!(condition)) {                                                           \
            fprintf(stderr, "%s:%d: CHECK failed: %s\n", __FILE__, __LINE__, #condition); \
            support_failures++;                                                       \
        }                                                                             \
    } while (0)

/* Expect TF_STATUS_OK; print the error message otherwise. */
#define CHECK_OK(call) CHECK_STATUS(TF_STATUS_OK, call)

/* Expect a specific status from a call taking a trailing `TfError **` set to NULL. */
#define CHECK_STATUS(expected, call)                                                  \
    do {                                                                              \
        TfStatus check_status_ = (call);                                              \
        if (check_status_ != (expected)) {                                            \
            fprintf(stderr, "%s:%d: %s returned %s, expected %s\n", __FILE__, __LINE__, \
                    #call, tf_status_name(check_status_), tf_status_name(expected));  \
            support_failures++;                                                       \
        }                                                                             \
    } while (0)

/* Run one test function. */
#define RUN(test)               \
    do {                        \
        support_cases++;        \
        test();                 \
    } while (0)

/* Report the summary and the process exit status. */
static inline int finish(const char *suite) {
    printf("%s: %d cases, %d failures\n", suite, support_cases, support_failures);
    return support_failures == 0 ? EXIT_SUCCESS : EXIT_FAILURE;
}

/* Compile `plan`, run it on `input` and compare with `expected` (NUL-terminated). */
static inline int run_text(const TfPipeline *plan, const char *input, const char *expected) {
    TfCompiledPipeline *pipeline = NULL;
    TfError *error = NULL;
    if (tf_pipeline_compile(plan, &pipeline, &error) != TF_STATUS_OK) {
        fprintf(stderr, "compile failed: %s\n", tf_error_message(error, NULL));
        tf_error_free(error);
        return 0;
    }
    TfString out = {0};
    int matches = 0;
    if (tf_compiled_pipeline_process(pipeline, input, TF_NTS, &out, &error) == TF_STATUS_OK) {
        matches = strcmp(out.data, expected) == 0;
        if (!matches) fprintf(stderr, "got \"%s\", expected \"%s\"\n", out.data, expected);
    } else {
        fprintf(stderr, "process failed: %s\n", tf_error_message(error, NULL));
        tf_error_free(error);
    }
    tf_string_free(&out);
    tf_compiled_pipeline_free(pipeline);
    return matches;
}

#endif
