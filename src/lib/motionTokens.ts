/**
 * UX-009: the framer-motion-side counterpart to `tailwind.config.ts`'s `transitionDuration`
 * tokens — framer-motion's `transition` prop takes seconds, not a Tailwind class, so it can't
 * read `duration-fast`/`duration-standard` directly. Kept as the same two named durations
 * (`fast` for opacity/color micro-interactions, `standard` for structural motion) so a
 * framer-motion transition and a CSS transition doing conceptually the same kind of motion use
 * the same value on purpose, not by coincidence.
 */
export const MOTION_FAST_SECONDS = 0.15;
export const MOTION_STANDARD_SECONDS = 0.2;
