# Release checklist

1. Make sure all feature changes are committed and the repo is clean.
2. Pick the next version (semver patch/minor/major) and bump it in:
   - `package.json`
   - `src-tauri/Cargo.toml`
   - `src-tauri/tauri.conf.json`
3. Commit the version bump:
   ```
   git add -A
   git commit -m "chore(release): bump version to X.Y.Z"
   ```
4. Create and push a signed tag:
   ```
   git tag -a vX.Y.Z -m "vX.Y.Z"
   git push origin main vX.Y.Z
   ```
5. Build and install to `/Applications`:
   ```
   pnpm app:install --latest
   ```
6. Verify the installed app launches: `open /Applications/AnyLeft.app`.

If only a local install is needed without a release, run step 5 directly.
