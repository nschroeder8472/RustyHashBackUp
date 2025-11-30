/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./templates/**/*.{html,tera}",
    "./static/js/**/*.js"
  ],
  darkMode: 'class',
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
