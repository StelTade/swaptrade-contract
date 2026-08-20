import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./test/setup.ts'],
    // `e2e/` is Playwright's; vitest must not try to run it.
    include: ['test/**/*.test.tsx', 'test/**/*.test.ts'],
  },
});
