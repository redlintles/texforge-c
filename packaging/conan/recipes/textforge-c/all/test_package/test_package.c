#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "textforge.h"

int main(void) {
    TfPipeline *plan = tf_pipeline_to_uppercase_all(tf_pipeline_new());
    tf_pipeline_add_to_end(plan, tf_text("!"));
    TfCompiledPipeline *pipeline = NULL;
    TfString out = {0};
    int ok = tf_pipeline_compile(plan, &pipeline, NULL) == TF_STATUS_OK &&
             tf_compiled_pipeline_process(pipeline, "textforge", TF_NTS, &out, NULL) == TF_STATUS_OK &&
             strcmp(out.data, "TEXTFORGE!") == 0;
    printf("textforge-c %s (core %s): %s\n", tf_version(), tf_core_version(), ok ? out.data : "FAILED");
    tf_string_free(&out);
    tf_compiled_pipeline_free(pipeline);
    tf_pipeline_free(plan);
    return ok ? EXIT_SUCCESS : EXIT_FAILURE;
}
