# Contributing

Tracewise is open source under the [GNU GPL v3 or later](LICENSE): changes you distribute must stay open source under the same license.

## Commits

Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary`.

| Type | Use for | In the changelog |
|---|---|---|
| `feat` | a new feature | Added |
| `fix` | a bug fix | Fixed |
| `perf`, `refactor` | speed, internal changes | Changed |
| `docs` | documentation | Documentation |
| `test`, `ci`, `build`, `chore`, `style` | everything else | left out |

Add `!` after the type (`feat!: ...`) for a breaking change. CI checks every commit of a pull request.

## Before a pull request

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

## Documentation site

The site at https://cmdworks.github.io/Tracewise/ is built from `docs/` and a few root files (`UI.md`, `CHANGELOG.md`, `CONTRIBUTING.md`). Edit those; never edit `site/src`, which is generated. The home page and the look are in `site/`.

```bash
cd site && pnpm install && pnpm dev     # live preview
pnpm build                              # fails on a broken link
```

Changes to the docs on `main` are published by the `Docs site` workflow.

## Releasing (maintainers)

```bash
scripts/release.sh 0.3.0                      # bumps versions, writes the CHANGELOG from commits, commits and tags
git push origin main && git push origin v0.3.0
```

The tag runs the release workflow. It refuses a version that is not higher than the newest release or does not match `Cargo.toml`.
