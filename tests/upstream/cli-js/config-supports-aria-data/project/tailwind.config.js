module.exports = {
  theme: {
    extend: {
      supports: {
        grid: 'display: grid',
        blur2: 'backdrop-filter: blur(1px)',
        'not-ok': 'not (display: grid)',
      },
      aria: {
        asc: 'sort="ascending"',
        current2: 'current="page"',
      },
      data: {
        open2: 'state="open"',
        checked2: 'ui~="checked"',
        active: 'active',
      },
      containers: {
        card: '22rem',
      },
    },
  },
}
