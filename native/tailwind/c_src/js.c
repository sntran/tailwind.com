/*
 * A small API over QuickJS: see js.h.
 *
 * JavaScript gets two global functions:
 *   __host(op, text)  call the host function, synchronously
 *   __await(promise)  run the pending jobs until the promise settles, and
 *                     return its value or throw its reason
 *   __compile(source, filename)
 *                     evaluate a script that has the name of its file: an
 *                     `import()` in it resolves from that file
 * When no job is pending, the job loop calls the global function
 * `__tw_run_timer` of JavaScript (the timers of `setTimeout`). It returns
 * true when it ran a timer.
 * JavaScript has no file access and no network access except through
 * `__host`.
 */
#include "js.h"

#include <stdlib.h>
#include <string.h>

#include "quickjs/quickjs.h"

#if defined(__COSMOPOLITAN__)
#include <cosmo.h>
#endif

struct TwJs {
    JSRuntime *rt;
    JSContext *ctx;
    TwJsHost host;
    TwJsRelease release;
    void *user;
    int depth;
    TwJsInterrupted interrupted;
};

static int interrupt_handler(JSRuntime *rt, void *opaque) {
    TwJs *js = opaque;
    (void)rt;
    return js->interrupted ? js->interrupted() : 0;
}

void tw_js_set_interrupt(TwJs *js, TwJsInterrupted interrupted) {
    js->interrupted = interrupted;
    JS_SetInterruptHandler(js->rt, interrupted ? interrupt_handler : NULL, js);
}

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

/*
 * The compiled scripts and modules of the process. A new engine compiles
 * the same text again: the plugin layer of Tailwind CSS in each compile,
 * and the plugins in each full build of the watch, and in each run of the
 * NIF. The cache keeps the bytecode of each text, with a copy of the text:
 * an entry is used only for the same name and the same text, byte for
 * byte. The bytecode has the source and the debug information, so a
 * function from the cache is the same as a function from the text.
 */
#define CACHE_SIZE 32

typedef struct {
    char *name;
    char *source;
    size_t length;
    int is_module;
    uint8_t *bytecode;
    size_t bytecode_length;
} CacheEntry;

static CacheEntry cache[CACHE_SIZE];
static unsigned cache_next;

#if defined(__wasi__)
/* The WASI program has one thread. */
static void cache_lock(void) {}
static void cache_unlock(void) {}
#else
#include <pthread.h>
static pthread_mutex_t cache_mutex = PTHREAD_MUTEX_INITIALIZER;
static void cache_lock(void) { pthread_mutex_lock(&cache_mutex); }
static void cache_unlock(void) { pthread_mutex_unlock(&cache_mutex); }
#endif

static void cache_clear_entry(CacheEntry *entry) {
    free(entry->name);
    free(entry->source);
    free(entry->bytecode);
    memset(entry, 0, sizeof(*entry));
}

/* The function or the module of the bytecode of this text, or
 * JS_UNINITIALIZED when the cache does not have it. */
static JSValue cache_read(JSContext *ctx, const char *source, size_t length, const char *name, int is_module) {
    JSValue value = JS_UNINITIALIZED;
    cache_lock();
    for (int i = 0; i < CACHE_SIZE; i++) {
        CacheEntry *entry = &cache[i];
        if (!entry->name || entry->is_module != is_module || entry->length != length) continue;
        if (strcmp(entry->name, name) != 0 || memcmp(entry->source, source, length) != 0) continue;
        value = JS_ReadObject(ctx, entry->bytecode, entry->bytecode_length, JS_READ_OBJ_BYTECODE);
        if (JS_IsException(value)) {
            /* The text compiles again. */
            JSValue ignored = JS_GetException(ctx);
            JS_FreeValue(ctx, ignored);
            cache_clear_entry(entry);
            value = JS_UNINITIALIZED;
        }
        break;
    }
    cache_unlock();
    /* The compile of a module also resolves its imports: here too. This
     * can load other modules, so the cache is not locked. */
    if (is_module && !JS_IsUninitialized(value) && JS_ResolveModule(ctx, value) < 0) {
        JS_FreeValue(ctx, value);
        return JS_EXCEPTION;
    }
    return value;
}

/* Keep the bytecode of a compiled text. A new text of the same name takes
 * the place of the old one. */
static void cache_write(JSContext *ctx, const char *source, size_t length, const char *name, int is_module,
                        JSValueConst compiled) {
    size_t bytecode_length;
    uint8_t *bytecode = JS_WriteObject(ctx, &bytecode_length, compiled, JS_WRITE_OBJ_BYTECODE);
    CacheEntry entry = {0};
    CacheEntry *slot = NULL;

    if (!bytecode) {
        JSValue ignored = JS_GetException(ctx);
        JS_FreeValue(ctx, ignored);
        return;
    }
    entry.name = malloc(strlen(name) + 1);
    entry.source = malloc(length + 1);
    entry.bytecode = malloc(bytecode_length);
    if (!entry.name || !entry.source || !entry.bytecode) {
        cache_clear_entry(&entry);
        js_free(ctx, bytecode);
        return;
    }
    strcpy(entry.name, name);
    memcpy(entry.source, source, length);
    entry.source[length] = 0;
    entry.length = length;
    entry.is_module = is_module;
    memcpy(entry.bytecode, bytecode, bytecode_length);
    entry.bytecode_length = bytecode_length;
    js_free(ctx, bytecode);

    cache_lock();
    for (int i = 0; i < CACHE_SIZE && !slot; i++)
        if (cache[i].name && cache[i].is_module == is_module && strcmp(cache[i].name, name) == 0) slot = &cache[i];
    for (int i = 0; i < CACHE_SIZE && !slot; i++)
        if (!cache[i].name) slot = &cache[i];
    if (!slot) slot = &cache[cache_next++ % CACHE_SIZE];
    cache_clear_entry(slot);
    *slot = entry;
    cache_unlock();
}

/* Compile a script or a module (JS_EVAL_FLAG_COMPILE_ONLY), with the
 * cache of the process. */
static JSValue compile_text(JSContext *ctx, const char *source, size_t length, const char *name, int is_module) {
    JSValue compiled = cache_read(ctx, source, length, name, is_module);
    if (!JS_IsUninitialized(compiled)) return compiled;
    compiled = JS_Eval(ctx, source, length, name,
                       (is_module ? JS_EVAL_TYPE_MODULE : JS_EVAL_TYPE_GLOBAL) | JS_EVAL_FLAG_COMPILE_ONLY);
    if (!JS_IsException(compiled)) cache_write(ctx, source, length, name, is_module, compiled);
    return compiled;
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

/* Run one timer of JavaScript. 1: a timer ran. 0: no timer. -1: exception. */
static int run_timer(JSContext *ctx) {
    JSValue global = JS_GetGlobalObject(ctx);
    JSValue function = JS_GetPropertyStr(ctx, global, "__tw_run_timer");
    JSValue value;
    int ran = 0;

    if (JS_IsFunction(ctx, function)) {
        value = JS_Call(ctx, function, global, 0, NULL);
        if (JS_IsException(value)) {
            ran = -1;
        } else {
            ran = JS_ToBool(ctx, value) > 0;
        }
        JS_FreeValue(ctx, value);
    }
    JS_FreeValue(ctx, function);
    JS_FreeValue(ctx, global);
    return ran;
}

/* __compile(source, filename) */
static JSValue js_compile(JSContext *ctx, JSValueConst this_val, int argc, JSValueConst *argv) {
    const char *source, *filename;
    size_t length;
    JSValue value;

    (void)this_val;
    if (argc < 2) return JS_ThrowTypeError(ctx, "__compile needs a source and a file name");
    source = JS_ToCStringLen(ctx, &length, argv[0]);
    if (!source) return JS_EXCEPTION;
    filename = JS_ToCString(ctx, argv[1]);
    if (!filename) {
        JS_FreeCString(ctx, source);
        return JS_EXCEPTION;
    }
    value = compile_text(ctx, source, length, filename, 0);
    JS_FreeCString(ctx, filename);
    JS_FreeCString(ctx, source);
    if (JS_IsException(value)) return value;
    return JS_EvalFunction(ctx, value);
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
        if (ran == 0) {
            /* No job: the next timer of `setTimeout` runs now. */
            ran = run_timer(ctx);
            if (ran < 0) return JS_EXCEPTION;
            if (ran == 0) return JS_ThrowInternalError(ctx, "The promise cannot settle: no job and no timer is pending");
        }
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
    function = compile_text(ctx, text, length, name, 1);
    JS_FreeCString(ctx, text);
    if (JS_IsException(function)) return NULL;

    /* The module stays alive in the list of modules of the context. */
    module = JS_VALUE_GET_PTR(function);
    JS_FreeValue(ctx, function);

    meta = JS_GetImportMeta(ctx, module);
    if (JS_IsException(meta)) return NULL;
    JS_DefinePropertyValueStr(ctx, meta, "filename", JS_NewString(ctx, name), JS_PROP_C_W_E);
    if (name[0] == '/') {
        /* A file: `import.meta.url` and `import.meta.dirname`, as Node.js. */
        const char *slash = strrchr(name, '/');
        JSValue global = JS_GetGlobalObject(ctx);
        JSValue to_url = JS_GetPropertyStr(ctx, global, "__tw_file_url");
        JSValue file = JS_NewString(ctx, name);
        JSValue url = JS_IsFunction(ctx, to_url) ? JS_Call(ctx, to_url, global, 1, &file) : JS_UNDEFINED;
        if (JS_IsException(url)) {
            JSValue ignored = JS_GetException(ctx);
            JS_FreeValue(ctx, ignored);
            url = JS_UNDEFINED;
        }
        JS_DefinePropertyValueStr(ctx, meta, "url", url, JS_PROP_C_W_E);
        JS_DefinePropertyValueStr(ctx, meta, "dirname",
                                  JS_NewStringLen(ctx, name, slash == name ? 1 : (size_t)(slash - name)), JS_PROP_C_W_E);
        JS_FreeValue(ctx, file);
        JS_FreeValue(ctx, to_url);
        JS_FreeValue(ctx, global);
    }
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
    JS_DefinePropertyValueStr(js->ctx, global, "__compile", JS_NewCFunction(js->ctx, js_compile, "__compile", 2),
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

/* `process.platform` of Node.js for the system that runs the program. */
const char *tw_js_platform(void) {
#if defined(__COSMOPOLITAN__)
    if (IsWindows()) return "win32";
    if (IsXnu()) return "darwin";
    if (IsFreebsd()) return "freebsd";
    if (IsOpenbsd()) return "openbsd";
    if (IsNetbsd()) return "netbsd";
    return "linux";
#elif defined(_WIN32)
    return "win32";
#elif defined(__APPLE__)
    return "darwin";
#elif defined(__FreeBSD__)
    return "freebsd";
#elif defined(__OpenBSD__)
    return "openbsd";
#elif defined(__NetBSD__)
    return "netbsd";
#elif defined(__wasi__)
    return "wasi";
#else
    return "linux";
#endif
}

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
        if (ran < 0) return take_exception(job_ctx, out, stack);
        if (ran == 0) {
            ran = run_timer(js->ctx);
            if (ran < 0) return take_exception(js->ctx, out, stack);
            if (ran == 0) return 0;
        }
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
    value = compile_text(ctx, source, length, filename, is_module);
    if (!JS_IsException(value)) value = JS_EvalFunction(ctx, value);
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

int tw_js_eval_bytecode(TwJs *js, const unsigned char *bytecode, size_t length, char **out, char **stack) {
    JSContext *ctx = js->ctx;
    JSValue value;
    int status = 0;

    if (out) *out = NULL;
    if (stack) *stack = NULL;
    enter(js);
    /* A copy: with JS_READ_OBJ_ROM_DATA, QuickJS can write into the bytes,
     * and the bytes of the program are read-only. */
    value = JS_ReadObject(ctx, bytecode, length, JS_READ_OBJ_BYTECODE);
    if (JS_IsException(value) || JS_VALUE_GET_TAG(value) != JS_TAG_FUNCTION_BYTECODE) {
        /* Another version of QuickJS, or no bytecode: nothing ran. */
        JSValue ignored = JS_GetException(ctx);
        JS_FreeValue(ctx, ignored);
        JS_FreeValue(ctx, value);
        leave(js);
        return TW_JS_NO_BYTECODE;
    }
    value = JS_EvalFunction(ctx, value);
    if (JS_IsException(value)) {
        status = take_exception(ctx, out, stack);
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
