/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./templates/**/*.{html,tera}",
    "./static/js/**/*.js"
  ],
  darkMode: 'class',
  safelist: [
    // Colors used in metric_card.html.tera and other dynamic templates
    {
      pattern: /^(bg|border|text|hover:border)-(blue|green|purple|indigo|yellow|teal|red|gray)-(400|500|600)(\/10|\/50)?$/,
    },
  ],
  theme: {
    extend: {
      colors: {
        'dark-bg': '#1e1e1e',
        'dark-surface': '#2d2d2d',
        'dark-border': '#3d3d3d',
      }
    }
  },
  plugins: [],
}
