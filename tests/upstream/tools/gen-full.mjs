#!/usr/bin/env node
// Usage: node gen-full.mjs <path of the upstream clone> <output directory>
// This script writes full.json.gz. The file is a JSON array with two entries.
// It also copies the four upstream CSS files into <output directory>/css.
import fs from 'node:fs'
import path from 'node:path'
import { loadUpstream, writeGzip } from './lib.mjs'

let [clone, outDir] = process.argv.slice(2)
if (!clone || !outDir) {
  console.error('Usage: node gen-full.mjs <path of the upstream clone> <output directory>')
  process.exit(2)
}

const INPUT = '@import "tailwindcss";'

const CANDIDATES = String.raw`
container mx-auto block inline-block inline flex inline-flex grid inline-grid hidden contents table flow-root sr-only not-sr-only
static relative absolute fixed sticky inset-0 inset-x-0 inset-y-0 top-0 right-0 bottom-0 left-0 top-4 -top-1 left-1/2 top-full z-0 z-10 z-50 -z-10 isolate
visible invisible overflow-hidden overflow-auto overflow-x-auto overflow-y-scroll overscroll-contain float-left clear-both box-border object-cover object-center
flex-row flex-col flex-wrap flex-nowrap flex-1 flex-auto flex-none grow grow-0 shrink shrink-0 basis-1/2 basis-full order-first order-last order-2
items-start items-center items-end items-baseline items-stretch justify-start justify-center justify-end justify-between justify-around justify-evenly self-start self-center self-end content-center place-items-center place-content-center
grid-cols-1 grid-cols-2 grid-cols-3 grid-cols-4 grid-cols-12 grid-rows-3 col-span-2 col-span-full col-start-2 row-span-2 grid-flow-col auto-cols-fr gap-2 gap-4 gap-6 gap-x-4 gap-y-8 grid-cols-[200px_1fr] grid-cols-subgrid
p-0 p-2 p-4 p-6 p-8 px-2 px-4 px-6 py-1 py-2 py-3 py-12 pt-4 pr-2 pb-8 pl-3 ps-4 pe-4 p-px p-[3px]
m-0 m-2 m-4 m-auto mx-auto mx-4 my-2 my-8 mt-1 mt-4 mt-8 mr-2 mb-4 mb-6 ml-auto -mt-2 -mx-4 ms-2 me-2 space-x-2 space-x-4 space-y-2 space-y-4 space-y-6 -space-x-1 space-x-reverse
w-4 w-8 w-12 w-16 w-64 w-1/2 w-1/3 w-2/3 w-full w-screen w-auto w-fit w-min w-max w-px w-[calc(100%-2rem)] min-w-0 min-w-full max-w-xs max-w-sm max-w-md max-w-lg max-w-xl max-w-2xl max-w-7xl max-w-prose max-w-none max-w-screen-lg
h-4 h-8 h-12 h-16 h-64 h-full h-screen h-dvh h-auto h-px min-h-0 min-h-screen min-h-full max-h-96 max-h-screen size-4 size-6 size-10 size-full aspect-square aspect-video aspect-[4/3]
font-sans font-serif font-mono text-xs text-sm text-base text-lg text-xl text-2xl text-3xl text-4xl text-6xl text-sm/6 text-base/7 font-thin font-normal font-medium font-semibold font-bold font-extrabold italic not-italic
leading-none leading-tight leading-normal leading-relaxed leading-6 tracking-tight tracking-wide tracking-widest text-left text-center text-right text-justify uppercase lowercase capitalize normal-case
underline line-through no-underline underline-offset-4 decoration-2 decoration-dotted decoration-sky-500 truncate text-ellipsis whitespace-nowrap whitespace-pre-wrap break-words break-all line-clamp-2 line-clamp-3 text-balance text-pretty text-wrap antialiased tabular-nums list-disc list-decimal list-inside align-middle
text-white text-black text-transparent text-current text-gray-500 text-gray-700 text-gray-900 text-slate-600 text-zinc-400 text-red-500 text-red-600 text-green-600 text-blue-500 text-blue-600 text-indigo-600 text-white/80 text-black/50 text-[#1da1f2]
bg-white bg-black bg-transparent bg-current bg-gray-50 bg-gray-100 bg-gray-200 bg-gray-800 bg-gray-900 bg-slate-900 bg-zinc-950 bg-red-500 bg-green-500 bg-blue-500 bg-blue-600 bg-indigo-600 bg-yellow-400 bg-white/10 bg-black/50 bg-blue-500/75 bg-[#0f172a] bg-(--brand)
bg-linear-to-r bg-linear-to-br bg-gradient-to-r bg-radial bg-conic from-blue-500 from-indigo-500 via-purple-500 to-pink-500 to-transparent from-10% to-90% bg-cover bg-center bg-no-repeat bg-fixed bg-clip-text bg-none bg-[url(/hero.png)]
border border-0 border-2 border-4 border-t border-b border-l-4 border-x border-y border-solid border-dashed border-none border-gray-200 border-gray-300 border-transparent border-red-500 border-blue-500/50 border-collapse
divide-x divide-y divide-gray-200 divide-y-reverse rounded rounded-none rounded-sm rounded-md rounded-lg rounded-xl rounded-2xl rounded-full rounded-t-lg rounded-b-none rounded-l-md rounded-tl-xl
outline outline-none outline-hidden outline-2 outline-offset-2 outline-blue-500 ring ring-1 ring-2 ring-4 ring-inset ring-blue-500 ring-gray-300 ring-black/5 ring-offset-2 ring-offset-white inset-ring inset-ring-gray-950/5
shadow shadow-xs shadow-sm shadow-md shadow-lg shadow-xl shadow-2xl shadow-none shadow-inner shadow-blue-500/50 shadow-black/10 inset-shadow-sm inset-shadow-xs drop-shadow drop-shadow-md drop-shadow-lg text-shadow-sm
opacity-0 opacity-25 opacity-50 opacity-75 opacity-100 mix-blend-multiply blur blur-sm blur-lg brightness-50 brightness-110 contrast-125 grayscale invert saturate-150 sepia filter backdrop-blur backdrop-blur-sm backdrop-blur-md backdrop-brightness-50 backdrop-saturate-150 backdrop-filter
transform transform-gpu transform-none scale-95 scale-100 scale-105 scale-x-50 -scale-x-100 rotate-0 rotate-45 rotate-90 rotate-180 -rotate-6 translate-x-0 translate-x-4 translate-x-full -translate-x-1/2 translate-y-0 translate-y-1 -translate-y-1/2 -translate-y-full skew-x-12 origin-center origin-top-left perspective-distant rotate-x-12 transform-3d
transition transition-all transition-colors transition-opacity transition-transform transition-shadow transition-none duration-75 duration-150 duration-200 duration-300 duration-500 duration-700 ease-in ease-out ease-in-out ease-linear delay-100 delay-150
animate-spin animate-ping animate-pulse animate-bounce animate-none will-change-transform
cursor-pointer cursor-default cursor-not-allowed cursor-wait pointer-events-none pointer-events-auto select-none select-text select-all resize-none resize-y scroll-smooth scroll-mt-4 snap-x snap-mandatory snap-start snap-center touch-none touch-pan-y appearance-none accent-blue-500 caret-blue-500 field-sizing-content scheme-dark
fill-current fill-none fill-blue-500 stroke-current stroke-2 stroke-gray-400 table-auto table-fixed columns-2 break-inside-avoid content-[''] content-none
hover:bg-gray-100 hover:bg-blue-700 hover:text-white hover:text-gray-900 hover:underline hover:opacity-80 hover:shadow-lg hover:scale-105 hover:border-gray-400 hover:-translate-y-1
focus:outline-none focus:outline-hidden focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 focus:border-blue-500 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-indigo-600 focus-visible:ring-2 focus-within:ring-2
active:bg-blue-800 active:scale-95 disabled:opacity-50 disabled:cursor-not-allowed disabled:pointer-events-none enabled:hover:bg-blue-700 checked:bg-blue-600 invalid:border-red-500 required:border-red-300 read-only:bg-gray-100 placeholder:text-gray-400 placeholder-shown:border-gray-300
first:mt-0 last:mb-0 first:rounded-t-lg last:border-b-0 odd:bg-white even:bg-gray-50 only:mx-auto empty:hidden nth-3:bg-red-500 visited:text-purple-600 open:rotate-180
before:absolute before:inset-0 before:content-[''] after:block after:content-['*'] after:ml-0.5 after:text-red-500 file:mr-4 file:rounded-full file:border-0 marker:text-gray-400 selection:bg-blue-200 first-letter:text-4xl first-line:uppercase backdrop:bg-black/50
dark:bg-gray-900 dark:bg-gray-800 dark:text-white dark:text-gray-100 dark:text-gray-400 dark:border-gray-700 dark:hover:bg-gray-700 dark:ring-white/10 dark:shadow-none dark:divide-gray-700 dark:placeholder:text-gray-500 dark:focus:ring-blue-400
sm:block sm:flex sm:hidden sm:grid-cols-2 sm:px-6 sm:text-sm sm:max-w-md sm:rounded-lg sm:flex-row md:flex md:hidden md:grid-cols-3 md:w-1/2 md:px-8 md:text-base md:col-span-2 lg:flex lg:hidden lg:grid-cols-4 lg:px-8 lg:text-lg lg:max-w-4xl lg:col-span-3 xl:grid-cols-6 xl:px-12 2xl:max-w-7xl
max-sm:hidden max-md:flex-col max-lg:px-4 min-[900px]:flex max-[600px]:hidden sm:max-lg:bg-red-500 md:hover:bg-blue-600 lg:dark:bg-gray-950 sm:first:mt-0 print:hidden portrait:hidden landscape:flex
motion-safe:animate-spin motion-reduce:animate-none motion-reduce:transition-none contrast-more:border-black forced-colors:outline rtl:text-right ltr:ml-2 rtl:mr-2 starting:opacity-0 pointer-coarse:p-4
group group/item peer peer/email group-hover:opacity-100 group-hover:text-blue-600 group-hover:visible group-focus:ring-2 group-hover/item:visible group-data-[state=open]:rotate-180 group-has-[a]:underline group-aria-expanded:rotate-90 group-[.is-active]:bg-blue-500
peer-checked:bg-blue-600 peer-checked:translate-x-4 peer-focus:ring-2 peer-invalid:visible peer-disabled:opacity-50 peer-placeholder-shown:top-2 peer-checked/email:text-blue-600 peer-hover:text-gray-900
has-checked:bg-indigo-50 has-[:focus]:ring-2 has-[>img]:p-0 not-first:mt-4 not-last:border-b not-hover:opacity-75 in-data-open:block
aria-selected:bg-blue-100 aria-expanded:rotate-180 aria-disabled:opacity-50 aria-[current=page]:text-blue-600 data-active:bg-gray-100 data-[state=open]:bg-gray-100 data-[state=closed]:hidden data-[side=top]:mb-2 data-disabled:opacity-50
@container @container/main @sm:flex @md:grid-cols-2 @lg:grid-cols-3 @xl:px-8 @max-md:hidden @min-[400px]:flex @md/main:flex-row @[500px]:block
supports-[display:grid]:grid supports-backdrop-filter:bg-white/60 *:p-2 *:rounded-full **:data-slot:px-2 [&>*]:mb-2 [&_p]:mt-4 [&:nth-child(3)]:underline [&_svg]:size-4 [@media(hover:hover)]:opacity-100
!flex flex! !mt-0 hover:bg-red-500! md:!hidden [mask-type:luminance] [--scroll-offset:56px] lg:[--scroll-offset:44px] w-(--sidebar-width) text-(--muted) fill-(--icon) bg-[color:var(--bg)] mask-b-from-80% mask-linear-to-r
`
  .trim()
  .split(/\s+/)

let up = await loadUpstream(clone)
let options = { loadStylesheet: up.loadStylesheet }

async function entry(candidates) {
  let compiler = await up.compile(INPUT, options)
  let css = compiler.build(candidates)
  return {
    input: INPUT,
    candidates,
    css,
    optimized: up.optimize(css).code,
    minified: up.optimize(css, { minify: true }).code,
  }
}

let candidates = [...new Set(CANDIDATES)]
let entries = [await entry(candidates), await entry([])]
writeGzip(path.join(outDir, 'full.json.gz'), JSON.stringify(entries))
console.log(`full.json.gz: ${entries.length} entries, ${candidates.length} candidates in the first entry`)

let cssDir = path.join(outDir, 'css')
fs.mkdirSync(cssDir, { recursive: true })
for (let name of ['index.css', 'theme.css', 'preflight.css', 'utilities.css']) {
  fs.copyFileSync(path.join(up.packageDir, name), path.join(cssDir, name))
}
console.log('css/: 4 files')
