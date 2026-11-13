// Sandbox-only vitest config, kept at the worktree root so Vite bundles it into
// a writable dir (web/node_modules is read-only in the sandbox). Not a test: it
// is only passed to `npx vitest run --config` from web/ and deleted afterwards.
export default { root: process.cwd() };
