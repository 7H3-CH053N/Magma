/** @type {import('tailwindcss').Config} */
export default {
  darkMode: "class",
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Driven by CSS variables (see src/styles/index.css) so the palette is
        // live-customizable from Settings, in both light and dark.
        magma: {
          bg: "var(--magma-bg)",
          panel: "var(--magma-panel)",
          ink: "var(--magma-ink)",
          muted: "var(--magma-muted)",
          accent: "var(--magma-accent)",
          ai: "var(--magma-ai)",
        },
      },
      fontFamily: {
        sans: "var(--magma-font-ui)",
        mono: "var(--magma-font-mono)",
      },
    },
  },
  plugins: [],
};
