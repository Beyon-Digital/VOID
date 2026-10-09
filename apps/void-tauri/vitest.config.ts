import { defineConfig } from 'vitest/config';

// Screen-model tests run in node — pure logic only (component renders are
// covered by the testing lane / shell runs, not this harness).
export default defineConfig({
  test: {
    include: ['src/**/*.test.{ts,tsx}'],
    environment: 'node',
  },
});
