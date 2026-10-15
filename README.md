# gaming-cafe

Monorepo for the Arena360 gaming-cafe platform.

> Phase 8 will replace this with a full quickstart. For now see `docs/`.

- Requirements: [docs/REQUIREMENTS.md](docs/REQUIREMENTS.md)
- Architecture: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- Contributing: [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md)
- ADRs: [docs/adr/](docs/adr/)
- Migration log: [docs/MIGRATION.md](docs/MIGRATION.md)

## Coding agent setup

| Component | Project setup |
|---|---|
| Codex CLI | Primary coding agent |
| Ponytail | Project-local skill, `full` level by default |
| Agent Skills | Built-in Codex support; shared skills live in `.agents/skills/` |
| Code Review Graph | Keep the existing MCP; use the graph before reading source |
| Graphify | Optional |
| Browser automation MCP | Optional for UI tasks |

Install [Codex CLI](https://developers.openai.com/codex/cli) once on your machine
if needed, then launch it from the repository root:

```sh
npm install -g @openai/codex
codex
```

Codex reads [AGENTS.md](AGENTS.md) and discovers
[project-local skills](https://developers.openai.com/codex/skills#where-to-save-skills)
in `.agents/skills/`. In Codex, use `/skills` to verify discovery or explicitly
invoke `$ponytail`. The skill is available on the next turn; restart Codex if it
does not appear. `AGENTS.md` applies Ponytail to coding tasks by default.

The checked-in [Ponytail skill](.agents/skills/ponytail/SKILL.md) comes from
[DietrichGebert/ponytail 5.2.0](https://github.com/DietrichGebert/ponytail/tree/84740e3d344b69b4a133201fa519eb37a62a2b2a/skills/ponytail)
and includes its [MIT license](.agents/skills/ponytail/LICENSE).

Keep your existing Code Review Graph MCP registration. Run `codex mcp list` in
this repository, or `/mcp` inside Codex, to verify `code-review-graph` is enabled.
MCP registrations are machine-specific; a new checkout does not install the server.

Graphify and browser automation MCP are optional and are not installed by this
setup. Add them when a task needs them.
