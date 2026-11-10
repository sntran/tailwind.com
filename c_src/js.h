/*
 * A small API over QuickJS. Only UTF-8 text goes across this API, so the
 * Rust code needs no QuickJS types.
 *
 * Text and its owner:
 * - Text that the caller gives (source, names, JSON) stays the property of
 *   the caller. The functions copy what they keep.
 * - Text that a function gives through an "out" or "stack" parameter comes
 *   from malloc. The caller frees it with tw_js_free_text.
 * - Text that the host function returns is the property of the host. The
 *   engine copies it, and then calls the release function of the host.
 */
#ifndef TW_JS_H
#define TW_JS_H

#include <stddef.h>

typedef struct TwJs TwJs;

/*
 * The host function. JavaScript calls `__host(op, text)`, and the engine
 * calls this function. The result is the text that `__host` returns, or
 * NULL for `undefined`. When the function sets `*is_error`, the result is
 * the message of an Error that `__host` throws.
 *
 * The engine also calls it for the ES modules, with the operations
 * "module.resolve" (text: a JSON object with "base" and "name"; result:
 * the name of the module) and "module.load" (text: the name of the module;
 * result: the source of the module).
 */
typedef char *(*TwJsHost)(void *user, const char *op, const char *text, int *is_error);
/* The engine calls this function when it has the copy of a host result. */
typedef void (*TwJsRelease)(void *user, char *text);

/* A new engine. A limit of 0 is no limit. NULL: no memory. */
TwJs *tw_js_new(size_t memory_limit, size_t stack_limit, TwJsHost host, TwJsRelease release, void *user);
void tw_js_free(TwJs *js);
void tw_js_free_text(char *text);

/*
 * Each function below returns 0 for success. If not, `*out` is the message
 * of the exception, and `*stack` is the name of the exception (for example
 * "TypeError"), a line end, and its stack. `*stack` is NULL for an
 * exception that is not an object.
 */

/* Evaluate a script (is_module 0) or an ES module (is_module 1). The
 * result of a script is in `*out` when it is a string. If not, `*out` is
 * NULL. A module runs to its end, with the pending jobs. */
int tw_js_eval(TwJs *js, const char *source, size_t length, const char *filename, int is_module,
               char **out, char **stack);
/* Call the global function `name` with one string argument. The result is
 * in `*out` when it is a string. If not, `*out` is NULL. */
int tw_js_call(TwJs *js, const char *name, const char *text, char **out, char **stack);
/* Run the pending jobs (promises) until none is left. */
int tw_js_run_jobs(TwJs *js, char **out, char **stack);
/* The bytes that the engine uses now. */
size_t tw_js_memory_used(TwJs *js);

#endif
