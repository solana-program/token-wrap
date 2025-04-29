# @solana-program/token-wrap

TypeScript client for the Solana Token Wrap program.

---

## 📦 Publishing to NPM

This package uses Changesets and GitHub Actions to manage versioning and publishing.

### 🚀 Publishing Steps

1. Ensure you have the [changesets CLI](https://github.com/changesets/changesets) installed.
2. Create a Changeset

Run the following to document your changes and select a version bump:

```bash
pnpm changeset
```

3. Version the release (will bump the local package version and add to the changelog)

```bash
pnpm changeset version
```

4. Create a PR with the changes and merge! The github action will detect the difference in versions and publish
   automatically.
