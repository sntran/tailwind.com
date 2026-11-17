// The JavaScript engine of a browser for tailwind-small.wasm: the imports
// of the module "tailwind_js" (the feature `host-js` of the crate,
// native/tailwind/src/compat/js/engine.rs). They do what c_src/js.c does
// with QuickJS in tailwind.wasm, for `@plugin` and `@config`:
//
// - Each engine is a realm: a hidden <iframe> of the page. The plugin
//   layer changes the globals of its realm (process, Buffer, the timers),
//   so it does not run in the page itself.
// - JavaScript calls the program with `__host(op, text)`, through the
//   exports tw_host_call and tw_host_release.
// - The JavaScript files of the program (the plugin layer, the TypeScript
//   transform and the three plugins of the standalone program) are not in
//   tailwind-small.wasm: `assets` gives them.
// - A browser cannot import a file that is only in memory: an ES module
//   becomes CommonJS with the `imports` transform of Sucrase.
// - The plugin layer waits for its promises in `tw_js_call_async`, the
//   only import that suspends the program (JS Promise Integration). The
//   program starts with WebAssembly.promising (js/wasi.mjs).

const encoder = new TextEncoder();
const decoder = new TextDecoder();

// True when the browser has JS Promise Integration.
export const hasPromiseIntegration = typeof WebAssembly.Suspending === 'function' && typeof WebAssembly.promising === 'function';

// True when the module has the imports of this engine.
export function needsEngine(module) {
  return WebAssembly.Module.imports(module).some((item) => item.module === 'tailwind_js');
}

// The engine of one run: { imports, dispose }. `imports` are the imports
// of the module "tailwind_js". `exports` gives the exports of the instance
// (after it is made). `assets`: the JavaScript files by name ("compat",
// "typescript", "plugin:@tailwindcss/typography", ...). The program ends
// with proc_exit and does not free its engines: `dispose` removes their
// realms after the run.
export function createEngine(exports, assets = {}) {
  const engines = new Map();
  let nextId = 1;

  const bytes = () => new Uint8Array(exports().memory.buffer);
  const view = () => new DataView(exports().memory.buffer);

  function readText(pointer, length) {
    if (length === undefined) {
      const memory = bytes();
      let end = pointer;
      while (memory[end] !== 0) end++;
      length = end - pointer;
    }
    return decoder.decode(bytes().slice(pointer, pointer + length));
  }

  // Text with a NUL at its end, in memory from tw_host_alloc.
  function writeText(text) {
    const data = encoder.encode(String(text).replaceAll('\0', ''));
    const pointer = exports().tw_host_alloc(data.length + 1);
    const memory = bytes();
    memory.set(data, pointer);
    memory[pointer + data.length] = 0;
    return pointer;
  }

  function setOut(pointer, text) {
    if (pointer) view().setUint32(pointer, text === null || text === undefined ? 0 : writeText(text), true);
  }

  // The message to `out`, and the name, a line end and the stack to
  // `stack`, as take_exception of js.c.
  function exception(error, out, stack) {
    let message = null;
    let name = '';
    let trace = '';
    if (error !== null && (typeof error === 'object' || typeof error === 'function')) {
      try {
        if (typeof error.message === 'string') message = error.message;
        if (typeof error.name === 'string') name = error.name;
        if (typeof error.stack === 'string') trace = error.stack;
      } catch {}
    }
    if (message === null) {
      try {
        message = String(error);
      } catch {
        message = 'JavaScript exception';
      }
    }
    setOut(out, message);
    setOut(stack, `${name}\n${trace}`);
    return 1;
  }

  function engine(id) {
    const found = engines.get(id);
    if (!found) throw new Error(`tailwind.com: no JavaScript engine ${id}`);
    return found;
  }

  // A realm: the window of a hidden <iframe>.
  function realm() {
    if (typeof document === 'undefined') {
      throw new Error('tailwind.com: @plugin and @config in tailwind-small.wasm need a document (a page, not a worker). Use tailwind.wasm.');
    }
    const frame = document.createElement('iframe');
    frame.hidden = true;
    frame.setAttribute('aria-hidden', 'true');
    frame.setAttribute('data-tailwind-com', 'engine');
    document.documentElement.append(frame);
    return { global: frame.contentWindow, dispose: () => frame.remove() };
  }

  // One macrotask: the jobs of the promises that are pending now run
  // before it ends.
  function nextTask() {
    return new Promise((resolve) => {
      const channel = new MessageChannel();
      channel.port1.onmessage = () => {
        channel.port1.close();
        resolve();
      };
      channel.port2.postMessage(null);
    });
  }

  function create(user) {
    const { global, dispose } = realm();
    let strip = null;

    // The TypeScript transform of the program, in the realm.
    function transform(source, file, transforms) {
      if (strip === null) {
        if (typeof assets.typescript !== 'string') throw new global.Error(`Cannot load '${file}': the TypeScript transform is not loaded.`);
        strip = (0, global.eval)(`${assets.typescript}\n//# sourceURL=tailwindcss/typescript.js`);
      }
      return strip(source, file, transforms);
    }

    // An ES module (or a TypeScript file that has its syntax) becomes
    // CommonJS: `import.meta` becomes an object of the file.
    function commonJs(file, result) {
      let { format, source } = result;
      const typescript = /\.[cm]?ts$/.test(file);
      let esm = format === 'esm';
      if (!esm && format !== 'json' && (format === 'unknown' || typescript)) {
        try {
          new global.Function(typescript ? transform(source, file, ['typescript']) : source);
        } catch (error) {
          esm = error instanceof global.SyntaxError;
        }
      }
      if (!esm) return result;
      source = transform(source, file, typescript ? ['typescript', 'imports'] : ['imports']);
      source = source.replace(/\bimport\.meta\b/g, '({ url: __tw_file_url(__filename), filename: __filename, dirname: __dirname })');
      return { ...result, format: 'cjs', source };
    }

    // Call the host function of the program.
    function call(op, text) {
      const api = exports();
      const opPointer = writeText(op);
      const textPointer = writeText(text);
      const errorPointer = api.tw_host_alloc(4);
      view().setInt32(errorPointer, 0, true);
      let result;
      let failed;
      try {
        result = api.tw_host_call(user, opPointer, textPointer, errorPointer);
        failed = view().getInt32(errorPointer, true) !== 0;
      } finally {
        api.tw_host_free(opPointer);
        api.tw_host_free(textPointer);
        api.tw_host_free(errorPointer);
      }
      if (!result) {
        if (failed) throw new global.Error(`The host operation ${op} failed`);
        return undefined;
      }
      const value = readText(result);
      api.tw_host_release(user, result);
      if (failed) throw new global.Error(value);
      return value;
    }

    global.__host = (op, text) => {
      op = String(op);
      if (op === 'asset') {
        const asset = assets[String(text)];
        return typeof asset === 'string' ? asset : undefined;
      }
      const value = call(op, text === undefined ? '' : String(text));
      // The argument is JSON text: the name of the file.
      if (op === 'module.read' && typeof value === 'string') {
        return JSON.stringify(commonJs(String(JSON.parse(text)), JSON.parse(value)));
      }
      return value;
    };
    global.__compile = (source, filename) => (0, global.eval)(`${source}\n//# sourceURL=${filename}`);
    // No synchronous wait here: the plugin layer uses __tw_compat_async.
    global.__await = (value) => {
      if (value !== null && typeof value?.then === 'function') {
        throw new global.Error('This JavaScript engine cannot wait for a promise here.');
      }
      return value;
    };
    global.__tw_yield = () => nextTask();

    const id = nextId++;
    engines.set(id, { global, dispose });
    return id;
  }

  function evaluate(id, sourcePointer, length, filenamePointer, isModule, out, stack) {
    setOut(out, null);
    setOut(stack, null);
    try {
      const { global } = engine(id);
      const filename = readText(filenamePointer);
      let source = readText(sourcePointer, length);
      if (isModule) throw new global.Error('This JavaScript engine has no ES modules of the program.');
      // The program gives an empty plugin layer: the host has it.
      if (filename === 'tailwindcss/compat.js' && source === '') {
        if (typeof assets.compat !== 'string') throw new global.Error('The plugin layer of Tailwind CSS is not loaded.');
        source = assets.compat;
      }
      const value = (0, global.eval)(`${source}\n//# sourceURL=${filename}`);
      if (typeof value === 'string') setOut(out, value);
      return 0;
    } catch (error) {
      return exception(error, out, stack);
    }
  }

  function functionOf(id, namePointer) {
    const { global } = engine(id);
    const fn = global[readText(namePointer)];
    if (typeof fn !== 'function') throw new global.TypeError('not a function');
    return fn;
  }

  const imports = {
    tw_js_new(_memory, _stack, _host, _release, user) {
      return create(user);
    },
    tw_js_free(id) {
      engines.get(id)?.dispose();
      engines.delete(id);
    },
    tw_js_eval: evaluate,
    tw_js_call(id, namePointer, textPointer, out, stack) {
      setOut(out, null);
      setOut(stack, null);
      try {
        const value = functionOf(id, namePointer)(readText(textPointer));
        if (typeof value === 'string') setOut(out, value);
        return 0;
      } catch (error) {
        return exception(error, out, stack);
      }
    },
    tw_js_call_async: new WebAssembly.Suspending(async (id, namePointer, textPointer, out, stack) => {
      setOut(out, null);
      setOut(stack, null);
      try {
        const value = await functionOf(id, namePointer)(readText(textPointer));
        if (typeof value === 'string') setOut(out, value);
        return 0;
      } catch (error) {
        return exception(error, out, stack);
      }
    }),
  };

  function dispose() {
    for (const { dispose } of engines.values()) dispose();
    engines.clear();
  }

  return { imports, dispose };
}
