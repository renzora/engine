/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    // Broad match pattern to include all relevant files within the htdocs directory
    './web/**/*.{html,js,css,php}', // Watches all HTML, JS, CSS, and PHP files in the htdocs folder
  ],
  theme: {
    extend: {},
  },
  plugins: [],
}