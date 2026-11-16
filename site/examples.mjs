// The starter examples of the site. Each one has the markup, the input
// CSS, and the selection that it opens with: one version, two versions,
// or the Rust compiler against the official compiler of one version.
export const examples = [
  {
    name: 'A card',
    mode: 'one',
    a: '4.3.5',
    markup: `<div class="mx-auto max-w-sm rounded-2xl bg-white p-6 shadow-lg ring-1 ring-slate-900/5 dark:bg-slate-800">
  <p class="text-sm font-semibold text-sky-600 uppercase">tailwind.com</p>
  <h1 class="mt-2 text-2xl font-bold text-slate-900 dark:text-white">One file, each version</h1>
  <p class="mt-3 text-slate-600 dark:text-slate-300">
    Select another version of Tailwind CSS above. The output CSS changes, and the card changes with it.
  </p>
  <button class="mt-5 rounded-lg bg-sky-600 px-4 py-2 font-medium text-white hover:bg-sky-500 focus-visible:outline-2 focus-visible:outline-offset-2">
    Build again
  </button>
</div>
`,
    css: `@import "tailwindcss";
`,
  },
  {
    name: 'New utilities of 4.1',
    mode: 'two',
    a: '4.0.17',
    b: '4.1.0',
    markup: `<div class="grid gap-6 p-6">
  <h1 class="text-4xl font-black text-sky-500 text-shadow-lg text-shadow-sky-900/40">Text shadow</h1>
  <img class="h-40 w-full mask-b-from-20% mask-b-to-90% bg-linear-to-r from-pink-500 to-amber-400" alt="">
  <p class="w-24 wrap-break-word">Averyveryverylongwordwithnospace</p>
</div>
`,
    css: `@import "tailwindcss";
`,
  },
  {
    name: 'Theme variables',
    mode: 'two',
    a: '4.3.5',
    b: '4.0.0',
    markup: `<div class="p-6 font-display">
  <p class="inline-block rounded-full bg-brand px-4 py-1 text-white">brand</p>
  <p class="mt-4 text-brand-soft">A soft brand color, from a variable.</p>
</div>
`,
    css: `@import "tailwindcss";

@theme {
  --font-display: "Avenir Next", ui-sans-serif, system-ui, sans-serif;
  --color-brand: oklch(0.6 0.2 260);
  --color-brand-soft: color-mix(in oklab, var(--color-brand) 60%, white);
}
`,
  },
  {
    name: 'A custom utility and variant',
    mode: 'one',
    a: '4.3.5',
    markup: `<pre class="tab-4 rounded bg-slate-100 p-4 coarse:p-8">	one tab
		two tabs</pre>
`,
    css: `@import "tailwindcss";

@utility tab-* {
  tab-size: --value(integer);
}

@custom-variant coarse (@media (pointer: coarse));
`,
  },
  {
    // The optimizer lowers CSS functions and mixins (docs/MIXINS.md). With
    // no --minify, the output keeps them, and a browser does not know them.
    name: 'CSS functions and mixins',
    mode: 'one',
    a: '4.3.5',
    minify: true,
    markup: `<div class="grid gap-4 p-6">
  <article class="card">
    <h2 class="font-bold">A mixin</h2>
    <p>Clear --minify: the output keeps @mixin and @apply, and the browser does not know them yet.</p>
  </article>
  <article class="card-warm">
    <h2 class="font-bold">Arguments and a block</h2>
    <p>The gap and the padding come from a function. Make the preview wider than 40rem: they become two times larger.</p>
  </article>
</div>
`,
    css: `@import "tailwindcss";

@function --space(--n) {
  --step: 0.5rem;
  @media (width >= 40rem) {
    --step: 1rem;
  }
  result: calc(var(--n) * var(--step));
}

@mixin --card(--bg: white, --fg: var(--color-slate-700)) {
  @apply rounded-xl shadow-lg ring-1 ring-slate-900/5;
  display: grid;
  gap: --space(1);
  padding: --space(3);
  background: var(--bg);
  & > p {
    color: var(--fg);
  }
  @contents;
}

.card {
  @apply --card;
}

.card-warm {
  @apply --card(var(--color-amber-100), var(--color-amber-900)) {
    border: 1px solid var(--color-amber-300);
  }
}
`,
  },
  {
    // The optimizer lowers if(), @when and @else (docs/MIXINS.md).
    name: 'CSS conditions: if() and @when',
    mode: 'one',
    a: '4.3.5',
    minify: true,
    markup: `<div class="p-6">
  <nav class="nav">
    <a href="#">One</a>
    <a href="#">Two</a>
    <a href="#">Three</a>
  </nav>
  <p class="note">Clear --minify: the output keeps if() and @when. Make the preview narrower than 40rem: the list and the colors change.</p>
</div>
`,
    css: `@import "tailwindcss";

@function --tone(--mode: light) {
  result: if(style(--mode: dark): var(--color-slate-900); else: var(--color-sky-100));
}

.nav {
  gap: if(media(width >= 40rem): 2rem; else: 0.5rem);
  padding: 1rem;
  border-radius: var(--radius-lg);
  background: --tone();
}

@when media(width >= 40rem) {
  .nav { display: flex; }
} @else supports(display: grid) {
  .nav { display: grid; background: --tone(dark); color: white; }
} @else {
  .nav { display: block; }
}

.note {
  margin-top: 1rem;
  color: if(media(width >= 40rem): var(--color-slate-600); else: var(--color-rose-600));
}
`,
  },
  {
    name: 'The typography plugin',
    mode: 'two',
    a: '4.3.5',
    b: '4.1.0',
    markup: `<article class="prose prose-slate mx-auto p-6 dark:prose-invert">
  <h1>A heading</h1>
  <p>A paragraph with <a href="#">a link</a> and <code>code</code>.</p>
  <ul><li>One</li><li>Two</li></ul>
</article>
`,
    css: `@import "tailwindcss";
@plugin "@tailwindcss/typography";
`,
  },
  {
    name: 'The Rust compiler against the official compiler',
    mode: 'engines',
    a: '4.2.0',
    markup: `<nav class="flex items-center gap-4 p-4 *:rounded-md *:px-3 *:py-1 [&>a:hover]:bg-slate-100">
  <a class="font-semibold not-hover:text-slate-600" href="#">Home</a>
  <a class="group-hover:underline md:max-lg:hidden" href="#">Docs</a>
  <a class="supports-[display:grid]:grid data-[state=open]:bg-sky-100" href="#">Blog</a>
</nav>
`,
    css: `@import "tailwindcss";
`,
  },
]
