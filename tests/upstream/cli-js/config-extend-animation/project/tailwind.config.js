module.exports = {
  theme: {
    extend: {
      keyframes: {
        wiggle: {
          '0%, 100%': { transform: 'rotate(-3deg)' },
          '50%': { transform: 'rotate(3deg)' },
        },
        'fade-in': {
          from: { opacity: '0' },
          to: { opacity: '1' },
        },
        unused: {
          to: { color: 'red' },
        },
        'slide-up': {
          '0%': { transform: 'translateY(100%)', opacity: 0 },
          '60%': { transform: 'translateY(-4px)' },
          '100%': { transform: 'translateY(0)', opacity: 1 },
        },
      },
      animation: {
        wiggle: 'wiggle 1s ease-in-out infinite',
        'fade-in': 'fade-in 200ms ease-out both',
        combo: 'fade-in 200ms ease-out, slide-up 400ms cubic-bezier(0.16, 1, 0.3, 1)',
        external: 'not-defined 1s linear',
      },
      transitionTimingFunction: { bounce2: 'cubic-bezier(0.68, -0.55, 0.27, 1.55)' },
      transitionDuration: { 2000: '2000ms' },
    },
  },
}
