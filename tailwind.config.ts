import type { Config } from "tailwindcss";

const config = {
  darkMode: ["class"],
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        border: "hsl(var(--border))",
        input: "hsl(var(--input))",
        ring: "hsl(var(--ring))",
        background: "hsl(var(--background))",
        foreground: "hsl(var(--foreground))",
        primary: {
          DEFAULT: "hsl(var(--primary))",
          foreground: "hsl(var(--primary-foreground))",
        },
        secondary: {
          DEFAULT: "hsl(var(--secondary))",
          foreground: "hsl(var(--secondary-foreground))",
        },
        muted: {
          DEFAULT: "hsl(var(--muted))",
          foreground: "hsl(var(--muted-foreground))",
        },
        accent: {
          DEFAULT: "hsl(var(--accent))",
          foreground: "hsl(var(--accent-foreground))",
        },
        destructive: {
          DEFAULT: "hsl(var(--destructive))",
          foreground: "hsl(var(--destructive-foreground))",
        },
        warning: {
          DEFAULT: "hsl(var(--warning))",
          foreground: "hsl(var(--warning-foreground))",
        },
        card: {
          DEFAULT: "hsl(var(--card))",
          foreground: "hsl(var(--card-foreground))",
        },
        popover: {
          DEFAULT: "hsl(var(--popover))",
          foreground: "hsl(var(--popover-foreground))",
        },
      },
      borderRadius: {
        lg: "var(--radius)",
        md: "calc(var(--radius) - 2px)",
        sm: "calc(var(--radius) - 4px)",
      },
      fontFamily: {
        sans: ["var(--font-ui)"],
        mono: ["var(--font-code)"],
      },
      fontSize: {
        caption: ["var(--text-caption)", { lineHeight: "1rem", fontWeight: "400" }],
        metadata: ["var(--text-metadata)", { lineHeight: "1.125rem", fontWeight: "400" }],
        body: ["var(--text-body)", { lineHeight: "1.3125rem", fontWeight: "400" }],
        emphasis: ["var(--text-emphasis)", { lineHeight: "1.5rem", fontWeight: "500" }],
        section: ["var(--text-section)", { lineHeight: "1.5rem", fontWeight: "600" }],
        view: ["var(--text-view)", { lineHeight: "1.875rem", fontWeight: "600" }],
      },
      boxShadow: {
        surface: "var(--shadow-surface)",
        elevated: "var(--shadow-elevated)",
      },
      // UX-009: named motion tokens — before this, transitions used a mix of duration-150,
      // duration-200, and (in framer-motion `transition` props, which don't read this Tailwind
      // config) hardcoded 0.14/0.15/0.18 second literals for what were conceptually the same two
      // kinds of motion. `fast` is for color/opacity micro-interactions (hover states, menu/
      // backdrop fades); `standard` is for structural transitions (drawer slides, panel width,
      // progress bars). See `src/lib/motionTokens.ts` for the framer-motion-side equivalents.
      transitionDuration: {
        fast: "150ms",
        standard: "200ms",
      },
    },
  },
  plugins: [],
} satisfies Config;

export default config;
