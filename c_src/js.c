/*
 * A small API over QuickJS: see js.h.
 *
 * JavaScript gets two global functions:
 *   __host(op, text)  call the host function, synchronously
 *   __await(promise)  run the pending jobs until the promise settles, and
 *                     return its value or throw its reason
 * JavaScript has no file access and no network access except through
 * `__host`.
 */
#include "js.h"

#include <stdlib.h>
#include <string.h>

#include "quickjs/quickjs.h"

struct TwJs {
    JSRuntime *rt;
    JSContext *ctx;
    TwJsHost host;
    TwJsRelease release;
    void *user;
    int depth;
};

static char *copy_text(const char *text) {
    size_t n = strlen(text) + 1;
    char *copy = malloc(n);
    if (copy) memcpy(copy, text, n);
    return copy;
}

/* The text of a string value, or NULL when the value is not a string. */
static char *text_of_string(JSContext *ctx, JSValueConst value) {
    const char *text;
    char *copy;
    if (!JS_IsString(value)) return NULL;
    text = JS_ToCString(ctx, value);
    if (!text) return NULL;
    copy = copy_text(text);
    JS_FreeCString(ctx, text);
    return copy;
}

/* Take the pending exception: the message to `*out`, the stack to `*stack`. */
static int take_exception(JSContext *ctx, char **out, char **stack) {
    JSValue exception = JS_GetException(ctx);
    JSValue message = JS_UNDEFINED;
    const char *text;

    if (out) *out = NULL;
    if (stack) *stack = NULL;

    if (JS_IsObject(exception)) {
        message = JS_GetPropertyStr(ctx, exception, "message");
        if (stack) {
            /* The name of the exception, a line end, and its stack. */
            JSValue name = JS_GetPropertyStr(ctx, exception, "name");
            JSValue trace = JS_GetPropertyStr(ctx, exception, "stack");
            char *name_text = text_of_string(ctx, name);
            char *trace_text = text_of_string(ctx, trace);
            size_t name_length = name_text ? strlen(name_text) : 0;
            size_t trace_length = trace_text ? strlen(trace_text) : 0;
            char *both = malloc(name_length + trace_length + 2);
            if (both) {
                if (name_text) memcpy(both, name_text, name_length);
                both[name_length] = '\n';
                if (trace_text) memcpy(both + name_length + 1, trace_text, trace_length);
                both[name_length + 1 + trace_length] = 0;
            }
            *stack = both;
            free(name_text);
            free(trace_text);
            if (JS_IsException(name) || JS_IsException(trace)) {
                JSValue ignored = JS_GetException(ctx);
                JS_FreeValue(ctx, ignored);
            }
            JS_FreeValue(ctx, name);
            JS_FreeValue(ctx, trace);
        }
    }
    if (JS_IsException(message)) {
        JSValue ignored = JS_GetException(ctx);
        JS_FreeValue(ctx, ignored);
        message = JS_UNDEFINED;
    }
    text = JS_ToCString(ctx, JS_IsString(message) ? message : exception);
    if (!text) {
        JSValue ignored = JS_GetException(ctx);
        JS_FreeValue(ctx, ignored);
    }
    if (out) *out = copy_text(text ? text : "JavaScript exception");
    if (text) JS_FreeCString(ctx, text);
    JS_FreeValue(ctx, message);
    JS_FreeValue(ctx, exception);
    return 1;
}

/* Call the host. The result is a string, undefined, or an exception. */
static JSValue call_host(JSContext *ctx, const char *op, const char *text) {
    TwJs *js = JS_GetContextOpaque(ctx);
    int is_error = 0;
    char *result = js->host(js->user, op, text, &is_error);
    JSValue value;

    if (!result) {
        if (is_error) return JS_ThrowInternalError(ctx, "The host operation %s failed", op);
        return JS_UNDEFINED;
    }
    value = JS_NewString(ctx, result);
    js->release(js->user, result);
    if (JS_IsException(value)) return value;
    if (is_error) {
        JSValue error = JS_NewError(ctx);
        JS_DefinePropertyValueStr(ctx, error, "message", value, JS_PROP_WRITABLE | JS_PROP_CONFIGURABLE);
        return JS_Throw(ctx, error);
    }
    return value;
}

/* __host(op, text) */
static JSValue js_host(JSContext *ctx, JSValueConst this_val, int argc, JSValueConst *argv) {
    const char *op, *text = NULL;
    JSValue result;

    (void)this_val;
    if (argc < 1) return JS_ThrowTypeError(ctx, "__host needs an operation");
    op = JS_ToCString(ctx, argv[0]);
    if (!op) return JS_EXCEPTION;
    if (argc > 1 && !JS_IsUndefined(argv[1])) {
        text = JS_ToCString(ctx, argv[1]);
        if (!text) {
            JS_FreeCString(ctx, op);
            return JS_EXCEPTION;
        }
    }
    result = call_host(ctx, op, text ? text : "");
    if (text) JS_FreeCString(ctx, text);
    JS_FreeCString(ctx, op);
    return result;
}

/* __await(promise) */
static JSValue js_await(JSContext *ctx, JSValueConst this_val, int argc, JSValueConst *argv) {
    JSRuntime *rt = JS_GetRuntime(ctx);

    (void)this_val;
    if (argc < 1) return JS_UNDEFINED;
    for (;;) {
        JSContext *job_ctx;
        int state = JS_PromiseState(ctx, argv[0]);
        int ran;

        if (state < 0) return JS_DupValue(ctx, argv[0]);
        if (state == JS_PROMISE_FULFILLED) return JS_PromiseResult(ctx, argv[0]);
        if (state == JS_PROMISE_REJECTED) return JS_Throw(ctx, JS_PromiseResult(ctx, argv[0]));

        ran = JS_ExecutePendingJob(rt, &job_ctx);
        if (ran < 0) return JS_EXCEPTION;
        if (ran == 0) return JS_ThrowInternalError(ctx, "The promise cannot settle: no job is pending");
    }
}

/* The name of a module, from the name in the import and its base. */
static char *module_normalize(JSContext *ctx, const char *base, const char *name, void *opaque) {
    JSValue request = JS_NewObject(ctx);
    JSValue json, resolved;
    const char *text;
    char *result = NULL;

    (void)opaque;
    JS_DefinePropertyValueStr(ctx, request, "base", JS_NewString(ctx, base), JS_PROP_C_W_E);
    JS_DefinePropertyValueStr(ctx, request, "name", JS_NewString(ctx, name), JS_PROP_C_W_E);
    json = JS_JSONStringify(ctx, request, JS_UNDEFINED, JS_UNDEFINED);
    JS_FreeValue(ctx, request);
    if (JS_IsException(json)) return NULL;
    text = JS_ToCString(ctx, json);
    JS_FreeValue(ctx, json);
    if (!text) return NULL;

    resolved = call_host(ctx, "module.resolve", text);
    JS_FreeCString(ctx, text);
    if (JS_IsException(resolved)) return NULL;
    if (!JS_IsString(resolved)) {
        JS_FreeValue(ctx, resolved);
        JS_ThrowReferenceError(ctx, "Cannot find module '%s'", name);
        return NULL;
    }
    text = JS_ToCString(ctx, resolved);
    if (text) {
        result = js_strdup(ctx, text);
        JS_FreeCString(ctx, text);
    }
    JS_FreeValue(ctx, resolved);
    return result;
}

/* Compile the module `name`. The host gives the source. */
static JSModuleDef *module_loader(JSContext *ctx, const char *name, void *opaque) {
    JSValue source = call_host(ctx, "module.load", name);
    JSValue function, meta;
    JSModuleDef *module;
    const char *text;
    size_t length;

    (void)opaque;
    if (JS_IsException(source)) return NULL;
    if (!JS_IsString(source)) {
        JS_FreeValue(ctx, source);
        JS_ThrowReferenceError(ctx, "Cannot load module '%s'", name);
        return NULL;
    }
    text = JS_ToCStringLen(ctx, &length, source);
    JS_FreeValue(ctx, source);
    if (!text) return NULL;
    function = JS_Eval(ctx, text, length, name, JS_EVAL_TYPE_MODULE | JS_EVAL_FLAG_COMPILE_ONLY);
    JS_FreeCString(ctx, text);
    if (JS_IsException(function)) return NULL;

    /* The module stays alive in the list of modules of the context. */
    module = JS_VALUE_GET_PTR(function);
    JS_FreeValue(ctx, function);

    meta = JS_GetImportMeta(ctx, module);
    if (JS_IsException(meta)) return NULL;
    JS_DefinePropertyValueStr(ctx, meta, "filename", JS_NewString(ctx, name), JS_PROP_C_W_E);
    JS_FreeValue(ctx, meta);
    return module;
}

TwJs *tw_js_new(size_t memory_limit, size_t stack_limit, TwJsHost host, TwJsRelease release, void *user) {
    TwJs *js = calloc(1, sizeof(*js));
    JSValue global;

    if (!js) return NULL;
    js->host = host;
    js->release = release;
    js->user = user;
    js->rt = JS_NewRuntime();
    if (!js->rt) {
        free(js);
        return NULL;
    }
    if (memory_limit) JS_SetMemoryLimit(js->rt, memory_limit);
    JS_SetMaxStackSize(js->rt, stack_limit);
    js->ctx = JS_NewContext(js->rt);
    if (!js->ctx) {
        JS_FreeRuntime(js->rt);
        free(js);
        return NULL;
    }
    JS_SetContextOpaque(js->ctx, js);
    JS_SetModuleLoaderFunc(js->rt, module_normalize, module_loader, NULL);

    global = JS_GetGlobalObject(js->ctx);
    JS_DefinePropertyValueStr(js->ctx, global, "__host", JS_NewCFunction(js->ctx, js_host, "__host", 2),
                              JS_PROP_WRITABLE | JS_PROP_CONFIGURABLE);
    JS_DefinePropertyValueStr(js->ctx, global, "__await", JS_NewCFunction(js->ctx, js_await, "__await", 1),
                              JS_PROP_WRITABLE | JS_PROP_CONFIGURABLE);
    JS_FreeValue(js->ctx, global);
    return js;
}

void tw_js_free(TwJs *js) {
    if (!js) return;
    JS_FreeContext(js->ctx);
    JS_FreeRuntime(js->rt);
    free(js);
}

void tw_js_free_text(char *text) { free(text); }

size_t tw_js_memory_used(TwJs *js) {
    JSMemoryUsage usage;
    JS_ComputeMemoryUsage(js->rt, &usage);
    return (size_t)usage.malloc_size;
}

/* The stack limit is relative to the first call into the engine. */
static void enter(TwJs *js) {
    if (js->depth++ == 0) JS_UpdateStackTop(js->rt);
}

static void leave(TwJs *js) { js->depth--; }

static int run_jobs(TwJs *js, char **out, char **stack) {
    for (;;) {
        JSContext *job_ctx;
        int ran = JS_ExecutePendingJob(js->rt, &job_ctx);
        if (ran == 0) return 0;
        if (ran < 0) return take_exception(job_ctx, out, stack);
    }
}

int tw_js_run_jobs(TwJs *js, char **out, char **stack) {
    int status;
    if (out) *out = NULL;
    if (stack) *stack = NULL;
    enter(js);
    status = run_jobs(js, out, stack);
    leave(js);
    return status;
}

int tw_js_eval(TwJs *js, const char *source, size_t length, const char *filename, int is_module,
               char **out, char **stack) {
    JSContext *ctx = js->ctx;
    JSValue value;
    int status = 0;

    if (out) *out = NULL;
    if (stack) *stack = NULL;
    enter(js);
    value = JS_Eval(ctx, source, length, filename, is_module ? JS_EVAL_TYPE_MODULE : JS_EVAL_TYPE_GLOBAL);
    if (JS_IsException(value)) {
        status = take_exception(ctx, out, stack);
    } else if (is_module) {
        /* The value is the promise of the evaluation of the module. */
        status = run_jobs(js, out, stack);
        if (status == 0 && JS_PromiseState(ctx, value) == JS_PROMISE_REJECTED) {
            JS_Throw(ctx, JS_PromiseResult(ctx, value));
            status = take_exception(ctx, out, stack);
        }
    } else if (out) {
        *out = text_of_string(ctx, value);
    }
    JS_FreeValue(ctx, value);
    leave(js);
    return status;
}

int tw_js_call(TwJs *js, const char *name, const char *text, char **out, char **stack) {
    JSContext *ctx = js->ctx;
    JSValue global, function, argument, value;
    int status = 0;

    if (out) *out = NULL;
    if (stack) *stack = NULL;
    enter(js);
    global = JS_GetGlobalObject(ctx);
    function = JS_GetPropertyStr(ctx, global, name);
    argument = JS_NewString(ctx, text ? text : "");
    if (JS_IsException(function) || JS_IsException(argument)) {
        status = take_exception(ctx, out, stack);
    } else {
        value = JS_Call(ctx, function, global, 1, &argument);
        if (JS_IsException(value)) {
            status = take_exception(ctx, out, stack);
        } else if (out) {
            *out = text_of_string(ctx, value);
        }
        JS_FreeValue(ctx, value);
    }
    JS_FreeValue(ctx, argument);
    JS_FreeValue(ctx, function);
    JS_FreeValue(ctx, global);
    leave(js);
    return status;
}
