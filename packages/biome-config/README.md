# @gaming-cafe/biome-config

Shared Biome ^2.4.15 lint + format rules per [ADR-0006](../../docs/adr/0006-tooling-biome.md).

The root [`biome.json`](../../biome.json) extends this preset and adds project-wide `vcs` + `files` includes/excludes. Individual workspaces only need their own `biome.json` when they require local overrides (e.g. kiosk allowing `console.log` during Tauri dev).

## Usage in a workspace

Add a devDependency and extend the preset:

```jsonc
{
  "extends": ["@gaming-cafe/biome-config"]
}
```

The preset exports via the `biome` and `default` package conditions so Biome resolves it from `node_modules`.

## House rules (summary)

- Formatter: 2-space indent, 100-column width, single quotes, trailing commas `all`, semicolons `always`
- Lint: recommended rules plus `noUnusedImports`, `noUnusedVariables`, `useExhaustiveDependencies`, `useHookAtTopLevel`, `noConsole` (warn), `noExplicitAny` (warn)

The preset omits `vcs` and `files.includes` so consumers can add their own ignore patterns without conflicting with the shared base.
