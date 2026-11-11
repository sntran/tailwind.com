// `require` in a config file finds the package in node_modules, as in
// each program: only the name in `@plugin` gives the copy of the program.
module.exports = {
  plugins: [require('@tailwindcss/aspect-ratio/marker')],
}
