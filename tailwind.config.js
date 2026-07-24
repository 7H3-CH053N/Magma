/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Magma's warm, minimal palette — one good default, no theme-hunting.
        magma: {
          bg: "#faf9f7",
          panel: "#f3f1ee",
          ink: "#1c1a17",
          muted: "#6b6b6b",
          accent: "#e0533d",
          ai: "#7c5cff",
        },
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "-apple-system", "sans-serif"],
        mono: ["JetBrains Mono", "ui-monospace", "monospace"],
      },
    },
  },
  plugins: [],
};
