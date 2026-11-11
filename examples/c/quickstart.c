/* TextForge from C: build a pipeline, compile it and run it.
 *
 * Build against an installed library:
 *   cc -std=c11 quickstart.c $(pkg-config --cflags --libs textforge-c) -o quickstart
 */
#include <stdio.h>
#include <stdlib.h>
#include <textforge.h>

int main(void) {
    /* Builder calls chain; the first failure is kept and reported by compile. */
    TfPipeline *plan = tf_pipeline_new();
    tf_pipeline_trim_both_sides(plan);
    tf_pipeline_val(plan, tf_text("greeting"), tf_text("Hello"));
    tf_pipeline_add_to_beginning(plan, tf_text("{{greeting}}, "));
    tf_pipeline_add_to_end(plan, tf_text("!"));

    TfCompiledPipeline *pipeline = NULL;
    TfError *error = NULL;
    if (tf_pipeline_compile(plan, &pipeline, &error) != TF_STATUS_OK) {
        fprintf(stderr, "compile failed: %s\n", tf_error_message(error, NULL));
        tf_error_free(error);
        tf_pipeline_free(plan);
        return EXIT_FAILURE;
    }
    tf_pipeline_free(plan); /* The compiled pipeline is independent. */

    TfString out = {0};
    if (tf_compiled_pipeline_process(pipeline, "  world 🌍  ", TF_NTS, &out, &error) != TF_STATUS_OK) {
        fprintf(stderr, "processing failed: %s\n", tf_error_message(error, NULL));
        tf_error_free(error);
        tf_compiled_pipeline_free(pipeline);
        return EXIT_FAILURE;
    }
    printf("%s\n", out.data); /* Hello, world 🌍! */
    tf_string_free(&out);
    tf_compiled_pipeline_free(pipeline);
    return EXIT_SUCCESS;
}
