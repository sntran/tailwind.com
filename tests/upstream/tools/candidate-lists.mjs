// Hand-made candidate lists for gen-classes.mjs.

// Arbitrary values, bare values, fractions and CSS variable shorthands.
// These go into the random candidates as class roots.
export const ARBITRARY_FORMS = String.raw`
w-[13px] w-[calc(100%-theme(spacing.4))] w-[calc(100%/3)] w-(--my-width) w-13 w-3/7 w-[3/7] w-[50%] w-[min(100%,20rem)] w-[--spacing(4)]
h-[37px] h-[100dvh] h-17 h-5/9 h-(--h) h-[calc(100vh-var(--header))] size-[18px] size-11.5 size-2/3 size-(--s)
min-w-[200px] min-w-13 max-w-[65ch] max-w-(--mw) min-h-[50vh] max-h-[calc(100dvh-4rem)] max-h-97
inline-[10px] block-[2em] min-inline-[3rem] max-block-[9rem]
p-[3px] p-(--my-var) p-2.5 p-13 px-[1.5rem] py-(--py) pt-[max(1rem,env(safe-area-inset-top))] pb-0.5 ps-[2ch] pe-17 pbs-[3px] pbe-7
m-[7px] m-(--m) mt-2.5 mx-[auto] mb-[-1px] ms-13 me-[5%] mbs-3 mbe-[2px] -mt-[3px] -mx-13 -m-(--m)
gap-[5px] gap-x-13 gap-y-(--gy) space-x-[3px] space-y-13 -space-x-2.5
inset-[3px] inset-x-13 inset-y-(--iy) top-[10%] top-1/7 left-(--l) right-[calc(50%-1rem)] bottom-2.5 start-[1ch] end-13 inset-s-[2px] inset-e-3 inset-bs-[4px] inset-be-5
z-99 z-[1000] z-(--z) -z-5 order-13 order-[2] -order-3 order-(--o)
basis-[30%] basis-1/7 basis-13 basis-(--b) flex-[2_1_0%] flex-13 flex-1/3 flex-(--f) grow-3 grow-[2] shrink-5 shrink-[0.5]
grid-cols-[repeat(3,minmax(0,1fr))] grid-cols-17 grid-cols-(--cols) grid-cols-[200px_1fr] grid-rows-[auto_1fr_auto] grid-rows-9
col-span-13 col-span-[2] col-start-15 col-end-(--ce) row-span-9 row-start-[3] -col-start-2 col-[1/3] row-[span_2/span_3]
auto-cols-[minmax(0,2fr)] auto-rows-[3rem] columns-[10rem] columns-7 columns-(--c)
aspect-4/3 aspect-[16/10] aspect-(--ar) aspect-[1.5] aspect-21/9
bg-[#0088cc] bg-[#0088cc]/50 bg-[#08c]/[0.25] bg-(--my-bg) bg-(color:--c)/[35%] bg-[color:var(--x)] bg-[rgb(0_128_255/0.5)] bg-[oklch(0.7_0.1_200)]/30 bg-[url(/img_1.png)] bg-[url('/a_b.png')] bg-[image:var(--img)] bg-(image:--img) bg-[length:200px_100px] bg-[size:50%] bg-[position:top_left] bg-[center_top_1rem] bg-[linear-gradient(to_right,red,blue)] bg-[var(--x)] bg-red-500/[0.37] bg-red-500/(--opacity) bg-red-500/37 bg-current/50 bg-inherit bg-transparent/50
bg-linear-to-r/oklch bg-linear-to-br/srgb bg-linear-45 bg-linear-45/oklab -bg-linear-45 bg-linear-[25deg,red_5%,yellow_60%] bg-linear-(--g) bg-linear-to-t/[in_hsl_longer_hue] bg-linear-to-r/shorter bg-radial-[at_25%_25%] bg-radial/oklch bg-radial-(--r) bg-conic-180 bg-conic/[in_hsl_longer_hue] bg-conic-[from_45deg] -bg-conic-30
from-[#ff0000] from-[10%] from-15% from-(--from) from-red-500/40 via-[rgb(0,255,0)] via-33% via-(--via) to-[blue]/50 to-95% to-[position:90%] from-[color:var(--c)] from-[percentage:var(--p)]
text-[13px] text-[length:var(--x)] text-[color:var(--x)] text-[#123456] text-[#123456]/75 text-(--t) text-(length:--t) text-(color:--t)/50 text-sm/6 text-sm/[17px] text-lg/(--lh) text-[14px]/[20px] text-base/loose text-xl/none text-[clamp(1rem,2vw,2rem)] text-red-500/[.5]
font-[weight:--x] font-[650] font-(--fw) font-[family-name:var(--f)] font-(family-name:--f) font-['Inter',sans-serif] font-[Open_Sans] font-550
leading-[1.15] leading-13 leading-(--lh) tracking-[0.02em] tracking-(--tr) -tracking-[1px] indent-[2em] indent-13 -indent-2.5 line-clamp-7 line-clamp-[9] line-clamp-(--lc)
decoration-[3px] decoration-[#f00] decoration-(--d) decoration-[color:var(--d)] decoration-[length:var(--d)] decoration-3 underline-offset-[5px] underline-offset-3 -underline-offset-2 underline-offset-(--uo)
content-['a_b\_c'] content-['hello'] content-[attr(data-x)] content-(--content) content-["\201C"] list-[upper-roman] list-(--l) list-image-[url(/dot.png)]
align-[4px] align-(--va) tab-4 tab-[13] wrap-anywhere
border-[3px] border-[#abc] border-[#abc]/50 border-(--bw) border-(color:--bc) border-(length:--bw) border-13 border-x-[5px] border-t-[color:red] border-s-3 border-be-[2px] border-[length:var(--w)] border-[color:var(--w)] border-red-500/15 divide-x-[3px] divide-y-13 divide-[#00f]/20 divide-(--dc)
rounded-[7px] rounded-(--r) rounded-t-[1rem] rounded-tl-[50%] rounded-ss-[3px] rounded-[10px_20px] rounded-s-(--rs) rounded-4xl
outline-[3px] outline-[#f0f] outline-13 outline-(--ow) outline-(color:--oc) outline-offset-[3px] outline-offset-13 -outline-offset-2 outline-red-500/50
ring-[3px] ring-[#f0f]/50 ring-13 ring-(--rw) ring-(color:--rc)/30 ring-offset-[3px] ring-offset-13 ring-offset-[#fff] inset-ring-[3px] inset-ring-5 inset-ring-[#000]/10
shadow-[0_0_10px_red] shadow-[0_1px_2px_rgb(0_0_0/0.5),0_2px_4px_#000] shadow-(--sh) shadow-[#f00] shadow-[#f00]/50 shadow-(color:--sc)/40 shadow-red-500/30 shadow-lg/50 shadow-xl/[25%] shadow-[0_0_10px_red]/50 shadow-current/20
inset-shadow-[0_2px_4px_#000] inset-shadow-sm/50 inset-shadow-red-500/25 inset-shadow-(--is) drop-shadow-[0_0_5px_red] drop-shadow-lg/50 drop-shadow-red-500/60 drop-shadow-(--ds) drop-shadow-[#00f] text-shadow-[0_1px_0_#fff] text-shadow-lg/30 text-shadow-red-500/40 text-shadow-(--ts)
opacity-37 opacity-[0.37] opacity-(--op) opacity-[37%] opacity-2.5 mix-blend-[plus-lighter]
blur-[3px] blur-(--blur) brightness-115 brightness-[1.15] contrast-115 contrast-(--c) grayscale-35 grayscale-[35%] hue-rotate-37 -hue-rotate-37 hue-rotate-[0.3turn] invert-35 saturate-175 saturate-[1.75] sepia-35 filter-[blur(2px)_brightness(1.1)] filter-(--filter)
backdrop-blur-[3px] backdrop-brightness-115 backdrop-contrast-[1.2] backdrop-grayscale-35 backdrop-hue-rotate-37 -backdrop-hue-rotate-[30deg] backdrop-invert-35 backdrop-opacity-37 backdrop-saturate-175 backdrop-sepia-[.35] backdrop-filter-[blur(4px)]
rotate-17 rotate-[17deg] rotate-(--rot) -rotate-17 rotate-x-33 rotate-y-[1.2rad] -rotate-z-12 rotate-[x_45deg] rotate-[1_1_0_30deg]
scale-103 scale-[1.03] scale-(--sc) -scale-103 scale-x-97 scale-y-[0.5] scale-z-110 scale-[1.1_0.9] scale-3d
translate-x-13 translate-x-[13px] translate-y-1/7 translate-(--t) -translate-x-13 -translate-y-[50%] translate-z-13 translate-[10px_20px] translate-full -translate-1/2
skew-17 skew-x-[17deg] -skew-y-17 skew-(--sk) origin-[10%_20%] origin-(--o) perspective-[500px] perspective-(--p) perspective-origin-[25%_75%] transform-[rotate(3deg)_scale(1.1)] transform-(--tf)
transition-[opacity,transform] transition-(--tr) duration-333 duration-[333ms] duration-(--d) duration-[1.5s] delay-175 delay-[175ms] ease-[cubic-bezier(0.4,0,0.2,1)] ease-(--e) animate-[spin_3s_linear_infinite] animate-(--a) animate-[wiggle_1s_ease-in-out]
will-change-[top,left] will-change-(--wc) cursor-[url(/c.cur),pointer] cursor-(--cur) caret-[#f00] caret-(--cc) caret-red-500/50 accent-[#f00]/50 accent-(--ac)
scroll-m-13 scroll-mt-[3px] -scroll-mx-2.5 scroll-p-13 scroll-pt-(--sp) scroll-pbs-3 scroll-mbe-[1px]
fill-[#f00] fill-(--f) fill-red-500/50 stroke-[#f00] stroke-[3px] stroke-3 stroke-(--s) stroke-(length:--s) stroke-[length:var(--s)] stroke-red-500/50
mask-[url(/m.svg)] mask-(--m) mask-linear-45 -mask-linear-45 mask-linear-from-13% mask-linear-to-[80%] mask-t-from-13 mask-b-to-[5rem] mask-x-from-(--mx) mask-radial-[50%_50%] mask-radial-from-25% mask-radial-at-[25%_25%] mask-conic-33 mask-conic-from-13% mask-l-from-red-500/50 mask-r-to-[#000] mask-size-[50%_50%] mask-position-[center_top]
object-[25%_75%] object-(--op) overscroll-contain field-sizing-content
container-[size] @container/sidebar @container-[inline-size]/main
[color:red] [color:red]/50 [--my-var:13px] [mask-type:luminance] [--x:theme(colors.red.500)] [inset:0_auto] [content:'a_b'] [grid-area:1/2/3/4] [-webkit-line-clamp:3] [color:var(--x,_red)] [background:url(https://example.com/a_b.png)] [@apply:flex] [transition:opacity_.2s,transform_.2s] [width:calc(100%-theme(spacing.4))] [margin:--spacing(4)] [color:--alpha(red/50%)]
`
  .trim()
  .split(/\s+/)

// Variants for the random stacks. gen-classes.mjs adds the names from getVariants() to this list.
export const RANDOM_VARIANTS = String.raw`
[&>*] [&_p] [&:nth-child(3)] [&.active] [.dark_&] [@media(min-width:300px)] [@supports(display:grid)] [@container(min-width:20rem)] [&:is(a,button)] [@media_print]
min-[300px] max-[900px] max-md min-lg min-[40rem] max-[calc(100vw-2rem)] @md @max-lg @min-[300px] @[500px] @lg/sidebar @max-[40rem]/main
supports-[display:grid] supports-[backdrop-filter] supports-grid supports-(--x) not-supports-[display:grid]
data-[x=y] data-[state=open] data-active data-[size~=lg] data-(--d) aria-[sort=ascending] aria-checked aria-[current=page] aria-invalid
nth-3 nth-[2n+1] nth-last-2 nth-of-type-4 nth-last-of-type-[odd] nth-[3n_of_.foo]
* ** has-[>img] has-checked has-[:focus] not-[.foo] not-hover not-first not-[@media_print] not-data-active in-[.sidebar] in-data-open
group-hover group-focus group-[.is-open] group-hover/item group-data-[state=open] group-aria-expanded/nav group-has-[a]
peer-checked peer-focus peer-[:nth-of-type(3)_&] peer-invalid/email peer-data-active peer-has-checked
hover focus focus-visible active disabled first last odd even before after placeholder file marker selection first-letter backdrop
sm md lg xl 2xl dark print portrait motion-safe motion-reduce contrast-more rtl ltr open starting forced-colors pointer-coarse noscript
`
  .trim()
  .split(/\s+/)

export const RANDOM_MODIFIERS = [
  '50', '5', '100', '37', '[35%]', '[0.3]', '(--o)', '[var(--o)]', 'oklch', 'srgb', '2', '[1.5]', 'name', '0', '101', '2.5', '[--o]', 'tight',
]

// Invalid candidates and edge cases. Each item is one candidate.
export const EDGE_CANDIDATES = [
  // Empty or incomplete parts.
  'w-', '-flex', 'bg-red-500/', 'bg-[#fff]/[]', '[color:red]/50x', 'hover:', ':flex', 'w-[calc(1+]', 'text-[var(--x',
  'bg-red-500/101', 'bg-red-500/0.5', 'w-1/0', 'w-1.3', '!flex!', 'group-[]:flex', '[&]', '[]', '[--x]', 'w-__',
  '', ' ', '-', '--', '---', '!', '!!', '!!flex', 'flex!!', '/', '//', ':', '::', 'flex:', 'hover::flex', ':hover:flex', 'hover:focus:',
  '-w-4', '-bg-red-500', '-p-4', '-text-sm', '--flex', '-!mt-4', '!-mt-4', '-mt-4!', 'hover:-mt-4', '-hover:mt-4', 'mt--4', 'mt-4-',
  'w-[', 'w-]', 'w-[]', 'w-[ ]', 'w-[_]', 'w-()', 'w-(', 'w-)', 'w-(--)', 'w-(-x)', 'w-(x)', 'w-(--x', 'w-[--x]', 'w-(--x)]', 'w-[[1px]]',
  'w-[1px', 'w-1px]', 'w-[1px]]', 'w-[[1px]', 'w-[(1px]', 'w-[1px)]', 'w-[calc(1px+2px]', 'w-[calc(1px+2px))]', 'w-[{1px}]', 'w-[}]',
  'w-[1px;]', 'w-[1px;color:red]', 'w-[1px}]', 'w-[a:b]', 'w-[length:]', 'w-[:1px]', 'w-[length:1px]', 'w-[color:red]',
  'bg-[#fff]/', 'bg-[#fff]/[', 'bg-[#fff]/]', 'bg-[#fff]/[ ]', 'bg-[#fff]/()', 'bg-[#fff]/(--)', 'bg-[#fff]/50/50', 'bg-red-500/50/50',
  'bg-red-500/-50', 'bg-red-500/50%', 'bg-red-500/1e2', 'bg-red-500/050', 'bg-red-500/5.5', 'bg-red-500/99.25', 'bg-red-500/100.5',
  'bg-red-500/abc', 'bg-red-500/[abc]', 'bg-red-500/[50]', 'bg-red-500/[50%]', 'bg-red-500/(--x)', 'bg-red-500/[--x]', 'bg-red-500/0', 'bg-red-500/100',
  'bg-red-500/2.5', 'bg-red-500/.5', 'bg-red-500/5.', 'bg-red-500/+5', 'bg-red-500/1/2',
  'bg-red', 'bg-red-', 'bg-red-5000', 'bg-red-50-0', 'bg-red-500-', 'bg--red-500', 'bg-RED-500', 'BG-red-500', 'Bg-Red-500', 'bg-red_500', 'bg red 500',
  'bg-[#ggg]', 'bg-[#12]', 'bg-[#12345]', 'bg-[#1234567]', 'bg-[#123456789]', 'bg-[red]', 'bg-[RED]', 'bg-[Red]/50', 'bg-[currentColor]', 'bg-[currentcolor]/50',
  'bg-[transparent]', 'bg-[inherit]', 'bg-[var(--x)]/50', 'bg-[--x]', 'bg-(--x)/50', 'bg-(--x,red)', 'bg-(--x,#fff)/50', 'bg-(color:--x)', 'bg-(color:)', 'bg-(:--x)', 'bg-(unknown:--x)',
  'bg-[url(/img_1.png)]', 'bg-[url(/img\\_1.png)]', "bg-[url('/img_1.png')]", 'bg-[url("/img 1.png")]', 'bg-[url(_/img.png_)]', 'bg-[url(data:image/png;base64,AAAA)]',
  'bg-[image:url(/a.png)]', 'bg-[length:10px]', 'bg-[position:10px]', 'bg-[size:10px]', 'bg-[10px]', 'bg-[10px_20px]', 'bg-[50%]', 'bg-[top]', 'bg-[left_top]',
  // Fractions and numbers.
  'w-1/0', 'w-0/1', 'w-0/0', 'w-1/1', 'w-01/2', 'w-1/02', 'w-1.5/2', 'w-1/2.5', 'w--1/2', 'w-1/-2', '-w-1/2', 'w-1/2/3', 'w-1/', 'w-/2', 'w-a/b', 'w-1/b',
  'w-1/[2]', 'w-[1]/2', 'w-1.3', 'w-1.25', 'w-1.5', 'w-1.75', 'w-1.0', 'w-1.', 'w-.5', 'w-0.5', 'w-00', 'w-01', 'w-1e3', 'w-0x10', 'w-+1', 'w-1_000', 'w-1,5',
  'w-999999', 'w-99999999999999999999', 'w-0', 'w-px', 'w-full', 'w-screen', 'w-auto', 'w-fit', 'w-xs', 'w-prose', 'w-unknown',
  'aspect-1/0', 'aspect-0/1', 'aspect-1.5/2', 'aspect-16/9/2', 'aspect-16', 'aspect-[16_/_9]', 'aspect--16/9', '-aspect-16/9',
  'z-1.5', 'z--1', '-z--1', 'z-[--z]', 'z-1/2', 'z-auto', '-z-auto', 'z-010', 'order-1.5', 'order-first', '-order-first', 'order-none',
  'opacity-101', 'opacity-100', 'opacity-1.5', 'opacity-2.25', 'opacity-2.5', 'opacity-2.75', 'opacity--5', '-opacity-50', 'opacity-50/50', 'opacity-[101]', 'opacity-[1.5]',
  'grid-cols-0', 'grid-cols-1.5', 'grid-cols--1', '-grid-cols-1', 'grid-cols-none', 'grid-cols-subgrid', 'grid-cols-[]', 'col-span-0', 'col-span-full', '-col-span-2',
  'duration-1.5', 'duration--100', '-duration-100', 'duration-initial', 'delay-0', 'rotate-1.5', 'rotate-45.5', 'rotate--45', '--rotate-45', 'scale-1.5', 'scale-0', 'skew-0.5',
  'leading-1.5', 'leading-0', 'leading-none', 'leading-[--lh]', 'tracking-1', 'line-clamp-0', 'line-clamp-1.5', 'line-clamp-none',
  'mt-2.5', 'mt-2.25', 'mt-2.75', 'mt-2.50', 'mt-2.05', 'mt-0.25', 'mt-.25', 'mt-1/2', 'mt-px', '-mt-px', 'mt-auto', '-mt-auto', 'mt-[-1px]', '-mt-[-1px]', '-mt-0',
  'text-sm/', 'text-sm/[', 'text-sm/[]', 'text-sm/6.5', 'text-sm/6.25', 'text-sm/0', 'text-sm/-1', 'text-sm/tight', 'text-sm/unknown', 'text-sm/6/7', 'text-[13px]/', 'text-red-500/6', 'text-red-500/tight',
  'text-[var(--x)]', 'text-[--x]', 'text-(--x)', 'text-[13]', 'text-[1.5]', 'text-[large]', 'text-[x-large]', 'text-[red]', 'text-[length:red]', 'text-[color:13px]', 'text-[unknown:13px]',
  // Variants.
  'hover', 'hover:hover', 'hover:hover:flex', 'hoverr:flex', 'HOVER:flex', 'Hover:flex', 'hover-:flex', '-hover:flex', 'hover/:flex', 'hover/name:flex', 'hover:flex:focus',
  'group-[]:flex', 'group-[ ]:flex', 'group-[&]:flex', 'group-:flex', 'group:flex', 'group/:flex', 'group-hover/:flex', 'group-hover/name:flex', 'group-hover/[name]:flex', 'group-hover/(--n):flex',
  'group-hover/name/other:flex', 'group-group-hover:flex', 'group-peer-hover:flex', 'group-not-hover:flex', 'group-sm:flex', 'group-dark:flex', 'group-before:flex', 'group-*:flex', 'group-[@media_print]:flex',
  'peer-[]:flex', 'peer:flex', 'peer-sm:flex', 'peer-after:flex', 'peer-hover/n:flex', 'not-[]:flex', 'not-:flex', 'not:flex', 'not-sm:flex', 'not-dark:flex', 'not-before:flex', 'not-not-hover:flex', 'not-group-hover:flex',
  'not-[&_p]:flex', 'not-[@media(min-width:300px)]:flex', 'not-[@supports(display:grid)]:flex', 'not-supports-[display:grid]:flex', 'not-print:flex', 'not-motion-safe:flex', 'not-*:flex', 'not-hover/name:flex',
  'has-[]:flex', 'has-:flex', 'has:flex', 'has-sm:flex', 'has-[&>img]:flex', 'has-not-hover:flex', 'has-has-hover:flex', 'has-[@media_print]:flex', 'in-[]:flex', 'in:flex', 'in-sm:flex', 'in-[@media_print]:flex',
  '[&]:flex', '[]:flex', '[&:flex', '&]:flex', '[&_p:flex', '[&_p]]:flex', '[[&_p]]:flex', '[p]:flex', '[.foo]:flex', '[:hover]:flex', '[&:hover]:flex', '[&:hover]/name:flex', '[@media]:flex', '[@]:flex',
  '[@media(min-width:300px)]:flex', '[@media_(min-width:300px)]:flex', '[@media(min-width:300px){&:hover}]:flex', '[&{color:red}]:flex', '[&;]:flex', '[&_p,&_a]:flex', '[&>*]:[&>*]:flex',
  '[--x]:flex', '[--x:1]:flex', '[color:red]:flex', '[@unknown_foo]:flex', '[&_[data-x]]:flex', '[&[data-x=a_b]]:flex', "[&[data-x='a_b']]:flex", '[&[data-x=a\\_b]]:flex',
  'min-[]:flex', 'min-:flex', 'min:flex', 'min-[300]:flex', 'min-[300px]:max-[300px]:flex', 'min-300px:flex', 'min-[--bp]:flex', 'min-(--bp):flex', 'max-[var(--bp)]:flex', 'max-unknown:flex', 'min-sm:max-sm:flex',
  'sm:md:lg:xl:2xl:flex', 'sm:max-md:flex', 'max-md:sm:flex', 'md:sm:flex', '3xl:flex', 'xs:flex', 'sm/name:flex', 'sm-:flex',
  '@:flex', '@-md:flex', '@md:flex', '@md/:flex', '@md/name:flex', '@/name:flex', '@[]:flex', '@[300px]:flex', '@[300]:flex', '@min-[]:flex', '@max-md/name:flex', '@container:flex', '@unknown:flex', '@md/a/b:flex',
  'supports-[]:flex', 'supports-:flex', 'supports:flex', 'supports-[display]:flex', 'supports-[display:grid]:flex', 'supports-[not_(display:grid)]:flex', 'supports-[selector(:has(a))]:flex', 'supports-[--x]:flex', 'supports-grid/name:flex',
  'data-[]:flex', 'data-:flex', 'data:flex', 'data-[x]:flex', 'data-[x=]:flex', 'data-[=y]:flex', 'data-[x=y_z]:flex', "data-[x='y_z']:flex", 'data-[x="y"]:flex', 'data-[x=y_i]:flex', 'data-[x~=y]:flex', 'data-x:flex', 'data-X:flex', 'data-x-y:flex', 'data-x/name:flex', 'data-(--x):flex',
  'aria-[]:flex', 'aria-:flex', 'aria:flex', 'aria-unknown:flex', 'aria-checked:flex', 'aria-[checked]:flex', 'aria-[sort=ascending]:flex', 'aria-checked/name:flex',
  'nth-0:flex', 'nth-3:flex', 'nth--3:flex', 'nth-1.5:flex', 'nth-[]:flex', 'nth-[2n+1]:flex', 'nth-[2n_+_1]:flex', 'nth-odd:flex', 'nth-[odd]:flex', 'nth-:flex', 'nth:flex', 'nth-3/name:flex', 'nth-last-[3n]:flex', 'nth-of-type-0:flex',
  '*:flex', '**:flex', '*:*:flex', '**:*:flex', '***:flex', '*/name:flex', 'hover:*:flex', '*:hover:flex', '*-foo:flex',
  'dark:dark:flex', 'dark/name:flex', 'starting:flex', 'starting:hover:flex', 'hover:starting:flex', 'before:after:flex', 'before:hover:flex', 'hover:before:flex', 'marker:flex', 'file:hover:flex', 'placeholder:first:flex', 'selection:dark:flex',
  // Important flag.
  '!flex', 'flex!', '!flex!', 'hover:!flex', 'hover:flex!', '!hover:flex', 'hover!:flex', '!!flex', 'flex!!', '!w-[13px]', 'w-[13px]!', 'w-[13px!]', 'w-[13px_!important]', '![color:red]', '[color:red]!', '[color:red!important]', '!-mt-4', '-mt-4!', '!bg-red-500/50', 'bg-red-500/50!', 'bg-red-500!/50',
  // Arbitrary properties.
  '[color:red]', '[color:red]/50', '[color:red]/50x', '[color:red]/[50%]', '[color:red]/(--o)', '[color:#fff]/50', '[color:var(--x)]/50', '[color:]', '[:red]', '[color]', '[color:red', 'color:red]', '[color:red;]', '[color:red;background:blue]', '[color:red}]',
  '[Color:red]', '[COLOR:red]', '[-color:red]', '[--x:1]', '[--X:1]', '[--:1]', '[-:1]', '[--x:]', '[--x:_]', '[--x:a_b]', '[--x:a\\_b]', '[--x:{a:b}]', '[--x:calc(1px+2px)]', '[--x:theme(spacing.4)]', '[--x:--spacing(4)]',
  '[color:red_blue]', '[color:theme(colors.red.500)]', '[color:theme(colors.red.500/50%)]', '[color:theme(colors.nope)]', '[color:theme(--color-red-500)]', '[color:--theme(--color-red-500)]', '[color:--alpha(var(--color-red-500)/50%)]',
  '[width:calc(100%-1px)]', '[width:calc(100%+1px)]', '[width:calc(100%*2)]', '[width:calc(100%/3)]', '[width:calc(-1*1px)]', '[width:calc(var(--a)-var(--b))]', '[width:clamp(1px,2px,3px)]', '[width:min(1px,2px)]',
  '[@media:print]', '[&:hover]', '[1x:red]', '[a b:red]', '[a_b:red]', '[content:"a:b"]', "[content:'a]b']", '[content:"\\""]', '[background:url(a:b)]', '[grid-template-areas:"a_b"_"c_d"]',
  // Arbitrary values with special text.
  "content-['a_b\\_c']", "content-['a_b']", "content-['a\\_b']", "content-['a\\\\_b']", "content-['a b']", 'content-["a_b"]', 'content-[a_b]', "content-['']", 'content-[]', "content-['\\'']", "content-['[']", "content-[']']", "content-['[]']", "content-['(']", "content-['{}']", "content-[':']", "content-['/']", "content-['a/b']/50",
  'w-[calc(100%/3)]', 'w-[calc(100%-1rem)]', 'w-[calc(100%+1rem)]', 'w-[calc(100%_-_1rem)]', 'w-[calc(100%*3)]', 'w-[calc(1px-2px-3px)]', 'w-[calc(-1px)]', 'w-[calc(1px+-2px)]', 'w-[calc(var(--a)-1px)]', 'w-[calc(var(--a-b)-1px)]', 'w-[calc(1px-var(--a-b))]',
  'w-[calc(theme(spacing.4)-1px)]', 'w-[calc(100%-theme(spacing.4))]', 'w-[calc(100%-theme(spacing[4]))]', 'w-[calc(100%-theme(spacing.nope))]', 'w-[calc(100%-theme(spacing.nope,1px))]', 'w-[theme(spacing.4)]', 'w-[theme(--spacing)]', 'w-[--spacing(4)]', 'w-[--spacing(4.5)]', 'w-[--spacing()]', 'w-[--theme(--breakpoint-md)]', 'w-[--theme(--nope,1px)]', 'w-[--theme(--nope)]', 'w-[--alpha(red/50%)]',
  'w-[min(1px,2px)]', 'w-[max(10px,min(20px,30px))]', 'w-[clamp(1px,calc(2px+3vw),4px)]', 'w-[round(up,1px,2px)]', 'w-[anchor-size(width)]', 'w-[env(safe-area-inset-left)]', 'w-[var(--a,var(--b,1px))]', 'w-[var(--a,_1px)]', 'w-[var(--a_b)]', 'w-[var(--a\\_b)]', 'w-[1px_2px]', 'w-[1px,2px]',
  'w-[100PX]', 'w-[1E3px]', 'w-[+1px]', 'w-[-1px]', 'w-[.5px]', 'w-[1.px]', 'w-[1px/2]', 'w-[0]', 'w-[00]', 'w-[1]', 'w-[auto]', 'w-[AUTO]', 'w-[fit-content(10px)]', 'w-[\\31 px]', 'w-[1px\\]', 'w-[\\]]', 'w-[\\[]',
  // Uppercase, unicode and other characters.
  'FLEX', 'Flex', 'fLex', 'flex ', ' flex', 'fl ex', 'flex\t', 'flex\n', 'flex,block', 'flex;', 'flex.', '.flex', '#flex', 'flex#', 'flex@', '@flex', 'flex$', 'flex%', 'flex^', 'flex&', 'flex*', 'flex+', 'flex=', 'flex~', 'flex`', 'flex|', 'flex\\', 'flex?', 'flex<', 'flex>', "flex'", 'flex"',
  'fléx', 'flex-ö', 'w-[13px]é', '日本語', 'bg-日本', '😀', 'hover:😀', '😀:flex', "content-['日本語']", "content-['😀']", 'w-[１３px]', '[color:réd]', 'data-[x=日本]:flex', '[&_.日本]:flex', 'w- ', 'flex​',
  'w-__', 'w-_', '_w-4', 'w_4', 'w-4_', 'w-[_13px_]', 'w-[__]', 'w-[\\_]', 'bg-[_#fff_]', '[color:_red_]', '[_color:red]', 'p-(_--x_)', 'p-(--x_y)', 'p-(--x\\_y)',
  'flex-', 'flex--', 'flex-[]', 'flex-()', 'flex/50', 'flex/[50]', 'flex/name', 'block/', 'hidden-', 'underline-', 'underline/50', 'sr-only/2', 'container/name', 'container-', 'truncate-', 'antialiased!',
  'p-4/50', 'p-4/[50]', 'p-[4px]/50', 'm-auto/50', 'rounded/50', 'rounded-/50', 'rounded-lg-', 'rounded-xl2', 'shadow-', 'shadow/50', 'shadow-none/50', 'shadow-lg/', 'shadow-lg/abc', 'shadow-lg/101', 'shadow-[0_0_#000]/abc',
  'ring-', 'ring/50', 'ring-1/50', 'ring-1.5', 'ring-red', 'ring-[#fff]/abc', 'border-', 'border-1.5', 'border-x-', 'border-red-500/abc', 'border-[1.5px]', 'border-[thin]', 'border-[red_blue]', 'outline-1.5', 'divide-x-1.5',
  'bg-linear-to-r/', 'bg-linear-to-r/[', 'bg-linear-to-r/oklch/50', 'bg-linear-to-r/unknown', 'bg-linear-to-r/[in_oklch]', 'bg-linear-to-r/50', 'bg-linear-to-x', 'bg-linear-45deg', 'bg-linear-45.5', 'bg-linear--45', 'bg-linear-[45]', 'bg-linear-[]', 'bg-linear-', 'bg-linear', 'bg-gradient-to-r', 'bg-gradient-to-r/oklch', 'bg-conic-1.5', 'bg-radial-', 'bg-radial-[]',
  'from-', 'from-5', 'from-5%', 'from-5.5%', 'from-101%', 'from--5%', 'from-[5]', 'from-red-500/abc', 'via-none', 'via-none/50', 'to-%', 'to-100%/50',
  'font-', 'font-[]', 'font-[weight:]', 'font-[weight:bold]', 'font-[family-name:Inter]', 'font-[bold]', 'font-[700]', 'font-700', 'font-1000', 'font-0', 'font-sans/50', 'font-bold/50', 'font-(weight:--x)', 'font-(--x)',
  'translate-x-', 'translate-x-1.3', 'translate-x-full/2', '-translate-x-full', 'translate-x--4', 'translate-1/0', 'rotate-', 'rotate-[45]', 'rotate-45deg', 'scale-', 'scale-[1.5_2_3_4]', 'skew-x-', 'origin-', 'origin-top-left-', 'perspective-',
  'animate-', 'animate-none/50', 'animate-unknown', 'transition-', 'transition-unknown', 'ease-', 'ease-unknown', 'duration-', 'delay-', 'delay-abc',
  'mask-', 'mask-linear-', 'mask-linear-from-', 'mask-linear-from-5', 'mask-t-from-101%', 'mask-radial-from-[]', 'mask-conic-1.5',
  'inset-shadow-', 'inset-ring-', 'drop-shadow-', 'drop-shadow-lg/abc', 'text-shadow-', 'text-shadow-lg/101', 'blur-', 'blur-1.5', 'blur-[]', 'brightness-', 'brightness-1.5', 'backdrop-blur-', 'backdrop-opacity-101',
  '--color-red-500', '--spacing', '--unknown', '--', '--font-sans', '[--color-red-500]', 'theme(spacing.4)', 'var(--x)', '@apply', '@media', '{flex}', '<div>', 'class="flex"', 'flex:hover', 'tw:flex', 'tw-flex', 'lg:hover:!-mt-[calc(100%-1px)]/50',
]
