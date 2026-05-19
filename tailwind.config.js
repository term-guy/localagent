/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{vue,js,ts,jsx,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        primary: {
          50: "#f0f5fa",
          100: "#e3edf6",
          200: "#c6d9ec",
          300: "#93b8db",
          400: "#5e94c9",
          500: "#3e7dbb",
          600: "#336699",
          700: "#29527a",
          800: "#1e3d5c",
          900: "#172e45",
          950: "#0e1c2a",
        },
      },
      fontFamily: {
        sans: [
          "Inter",
          "ui-sans-serif",
          "system-ui",
          "-apple-system",
          "BlinkMacSystemFont",
          "Segoe UI",
          "sans-serif",
        ],
      },
    },
  },
  plugins: [],
};
