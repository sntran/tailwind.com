# Third-party files in `cli-js/`

The cases in this directory contain files from other projects. This file lists them.
The files are not changed, with one exception: the Phoenix templates are rendered (see below).
All the other files in this directory are part of this repository.

## npm packages

Each package is in the `project/node_modules` directory of the cases in the last column.
`npm install` made the directories. Each package directory contains its license file or its `package.json` with the license name.

| Package | Version | Source | License | Cases |
| --- | --- | --- | --- | --- |
| `@tailwindcss/typography` | 0.5.20 | https://github.com/tailwindlabs/tailwindcss-typography | MIT | `typography`, `typography-config` |
| `postcss-selector-parser` | 6.0.10 | https://github.com/postcss/postcss-selector-parser | MIT | `typography`, `typography-config` |
| `cssesc` | 3.0.0 | https://github.com/mathiasbynens/cssesc | MIT | `typography`, `typography-config` |
| `util-deprecate` | 1.0.2 | https://github.com/TooTallNate/util-deprecate | MIT | `typography`, `typography-config` |
| `@tailwindcss/forms` | 0.5.11 | https://github.com/tailwindlabs/tailwindcss-forms | MIT | `forms`, `forms-class` |
| `mini-svg-data-uri` | 1.4.4 | https://github.com/tigt/mini-svg-data-uri | MIT | `forms`, `forms-class` |
| `tailwindcss-animate` | 1.0.7 | https://github.com/jamiebuilds/tailwindcss-animate | MIT | `animate` |
| `@tailwindcss/container-queries` | 0.1.1 | https://github.com/tailwindlabs/tailwindcss-container-queries | MIT | `container-queries` |

The registry is https://registry.npmjs.org/.

The cases `embedded-typography`, `embedded-forms` and `embedded-aspect-ratio` have no `node_modules` directory:
the program that runs the case has the plugins in its file.
`embedded-wins/project/node_modules/@tailwindcss/aspect-ratio` is not the npm package: its three files are part of this repository.

## daisyUI

| Item | Value |
| --- | --- |
| Files | `phoenix/project/assets/vendor/daisyui.js`, `phoenix/project/assets/vendor/daisyui-theme.js`, the same two files in `phoenix-minify/`, and `daisyui-bundle/project/vendor/daisyui.js` |
| Version | 5.0.35 |
| Source | The Phoenix installer templates (see below). The project is https://github.com/saadeghi/daisyui. |
| License | MIT. Copyright (c) 2020 Pouya Saadeghi. The license text is at the end of each file. |

| Item | Value |
| --- | --- |
| Files | `phoenix-deps/project/deps/daisyui/packages/bundle/`: `daisyui.js`, `daisyui.mjs`, `daisyui-theme.js`, `daisyui-theme.mjs` and `.gitignore` |
| Version | 5.5.20 |
| Source | https://github.com/saadeghi/daisyui/tree/v5.5.20/packages/bundle: the files of the sparse checkout that Mix makes for the dependency `daisyui` of a Phoenix 1.8.9 project |
| License | MIT. Copyright (c) 2020 Pouya Saadeghi. The license text is at the start of each file. |

## Phoenix

| Item | Value |
| --- | --- |
| Files in `phoenix/project/` and `phoenix-minify/project/` | `assets/css/app.css`, `assets/js/app.js`, `assets/vendor/heroicons.js`, `lib/my_app_web/components/core_components.ex`, `lib/my_app_web/components/layouts.ex`, `lib/my_app_web/components/layouts/root.html.heex`, `lib/my_app_web/controllers/page_html/home.html.heex`, and the `.gitignore` file (a part of the template) |
| Version | 1.8.5 |
| Source | https://github.com/phoenixframework/phoenix/tree/v1.8.5/installer/templates (`phx_assets/` and `phx_web/`) |
| License | MIT. Copyright (c) 2014 Chris McCord. https://github.com/phoenixframework/phoenix/blob/v1.8.5/LICENSE.md |

The files are EEx templates in the Phoenix repository. The files here are the result for the application `my_app`
(module `MyApp`, web module `MyAppWeb`) with LiveView, gettext, esbuild, Tailwind CSS and daisyUI.
The files `lib/my_app_web/live/page_live.ex` and `lib/my_app_web/live/page_live.html.heex` are not from Phoenix.

| Item | Value |
| --- | --- |
| Files in `phoenix-deps/project/` | `mix.exs`, `.gitignore`, `config/config.exs`, `assets/css/app.css`, `assets/js/app.js`, `assets/vendor/heroicons.js`, and `lib/my_app_web/` except the two files below |
| Version | 1.8.9 |
| Source | https://github.com/phoenixframework/phoenix/tree/v1.8.9/installer: the output of `mix phx.new my_app --no-install --no-ecto --no-mailer --no-dashboard` with Elixir 1.20.5 |
| License | MIT. Copyright (c) 2014 Chris McCord. https://github.com/phoenixframework/phoenix/blob/v1.8.9/LICENSE.md |

The files `lib/my_app_web/colocated_css.ex` and `lib/my_app_web/components/note.ex` of `phoenix-deps` are not from Phoenix.
The first one is the example of the documentation of `Phoenix.LiveView.ColocatedCSS` (Phoenix LiveView 1.2.9, MIT License).
The directory `_build/dev/phoenix-colocated/my_app/` has the two files that Phoenix LiveView 1.2 makes for the `<style>` tag of `note.ex`
when the project compiles: the manifest `colocated.css`, and the CSS file with the line of the tag and the hash of its text in its name.
They are written by hand, by the rules of `lib/phoenix_live_view/colocated_css.ex`.

## topbar

| Item | Value |
| --- | --- |
| Files | `phoenix/project/assets/vendor/topbar.js`, the same file in `phoenix-minify/`, and `phoenix-deps/project/assets/vendor/topbar.js` |
| Version | 3.0.0 |
| Source | The Phoenix installer templates, versions 1.8.5 and 1.8.9. The project is https://github.com/buunguyen/topbar. |
| License | MIT. Copyright (c) 2024 Buu Nguyen. |

## heroicons

| Item | Value |
| --- | --- |
| Files | `phoenix/project/deps/heroicons/optimized/**/*.svg` (27 icons), `phoenix/project/deps/heroicons/LICENSE`, and the same files in `phoenix-minify/` and in `phoenix-deps/` |
| Version | 2.2.0 |
| Source | https://github.com/tailwindlabs/heroicons/tree/v2.2.0/optimized |
| License | MIT. Copyright (c) Tailwind Labs, Inc. |

The icons are: `arrow-path`, `bars-3`, `check`, `exclamation-circle`, `information-circle` and `x-mark` in `24/outline`, `24/solid`, `20/solid` and `16/solid`,
and `computer-desktop`, `moon` and `sun` in `16/solid`.
