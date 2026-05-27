# @gaming-cafe/tsconfig

Shared TypeScript configuration presets consumed by every workspace. Pick the preset that matches the runtime:

| Preset | Use for |
|--------|---------|
| `base.json` | Plain TS libraries (`api-types`, `contracts`, `utils`) |
| `node.json` | Node-only tooling and scripts |
| `react.json` | Browser/React apps (Vite admin) and React component libraries |
| `nest.json` | Legacy NestJS decorator options (no current consumer; kept for parity) |

`react.json` disables `exactOptionalPropertyTypes` for migrated SPA/library code that passes optional props with explicit `undefined`. Plain libraries on `base.json` keep the full strict profile.

`base.json` is the canonical source of shared `compilerOptions`. Root `tsconfig.base.json` extends it for repo-level tooling.

## Usage

Add a workspace devDependency:

```json
"devDependencies": {
  "@gaming-cafe/tsconfig": "workspace:*"
}
```

Then extend the matching preset (only set app-specific overrides such as `outDir`, `noEmit`, or `include`):

```jsonc
{
  "extends": "@gaming-cafe/tsconfig/react.json",
  "compilerOptions": {
    "outDir": "dist",
    "rootDir": "src"
  },
  "include": ["src"]
}
```
