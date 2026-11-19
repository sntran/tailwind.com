/*
 * The bytecode of a script, for the program: build.rs compiles this file
 * and QuickJS for the computer of the build, and runs it on
 * assets/compat.js (the plugin layer of the version of the port).
 *
 *   bytecode INPUT.js NAME OUTPUT
 *
 * NAME is the file name of the script in the program
 * ("tailwindcss/compat.js"): its stack traces show it. The bytecode has
 * the source and the debug information, as a compile of the text. The
 * format of QuickJS is the same on each computer (the numbers are little
 * endian), and it has the version of the format: an engine of another
 * version does not read it, and the program then compiles the text.
 */
#include <stdio.h>
#include <stdlib.h>

#include "quickjs/quickjs.h"

int main(int argc, char **argv) {
    FILE *file;
    long length;
    char *source;
    size_t size;
    uint8_t *bytecode;
    JSRuntime *rt;
    JSContext *ctx;
    JSValue compiled;

    if (argc != 4) {
        fprintf(stderr, "usage: bytecode INPUT.js NAME OUTPUT\n");
        return 2;
    }
    file = fopen(argv[1], "rb");
    if (!file || fseek(file, 0, SEEK_END) != 0 || (length = ftell(file)) < 0 || fseek(file, 0, SEEK_SET) != 0) {
        fprintf(stderr, "bytecode: cannot read %s\n", argv[1]);
        return 1;
    }
    source = malloc((size_t)length + 1);
    if (!source || fread(source, 1, (size_t)length, file) != (size_t)length) {
        fprintf(stderr, "bytecode: cannot read %s\n", argv[1]);
        return 1;
    }
    source[length] = 0;
    fclose(file);

    rt = JS_NewRuntime();
    ctx = rt ? JS_NewContext(rt) : NULL;
    if (!ctx) {
        fprintf(stderr, "bytecode: no memory\n");
        return 1;
    }
    compiled = JS_Eval(ctx, source, (size_t)length, argv[2], JS_EVAL_TYPE_GLOBAL | JS_EVAL_FLAG_COMPILE_ONLY);
    if (JS_IsException(compiled)) {
        JSValue exception = JS_GetException(ctx);
        const char *message = JS_ToCString(ctx, exception);
        fprintf(stderr, "bytecode: %s: %s\n", argv[1], message ? message : "syntax error");
        return 1;
    }
    bytecode = JS_WriteObject(ctx, &size, compiled, JS_WRITE_OBJ_BYTECODE);
    if (!bytecode) {
        fprintf(stderr, "bytecode: cannot write the bytecode of %s\n", argv[1]);
        return 1;
    }
    file = fopen(argv[3], "wb");
    if (!file || fwrite(bytecode, 1, size, file) != size || fclose(file) != 0) {
        fprintf(stderr, "bytecode: cannot write %s\n", argv[3]);
        return 1;
    }
    js_free(ctx, bytecode);
    JS_FreeValue(ctx, compiled);
    JS_FreeContext(ctx);
    JS_FreeRuntime(rt);
    free(source);
    return 0;
}
