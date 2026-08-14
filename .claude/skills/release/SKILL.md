---
name: release
description: Use when the user wants to release, publish, or ship the AnyLeft app — phrases like "release", "发版", "发布", "上线", "推送新版本", "跑一遍安装", "一条龙", or updating the installed app to the latest version. Triggers on /release. Handles committing pending work, version bump, tag, push, rebuild, and install in one run.
---

# Release AnyLeft

One-shot release pipeline for this repo (mirrors `AGENTS.md`'s checklist, automated).

## Procedure

1. **Commit pending work first.** Run `git status --porcelain`. If there are uncommitted changes, commit them with a conventional-commit message (`feat:`/`fix:`/`chore:`…) that describes the change — this message also drives the automatic version bump.
2. **Run the pipeline script**, passing the commit message if step 1 was done inside the script:
   ```bash
   bash .claude/skills/release/scripts/release.sh --message "feat(providers): add opencode-go usage"
   ```
   If the working tree is already clean, run it with no `--message`.
3. **Version choice.** By default the script infers the semver bump from commit subjects since the last tag: `feat` → minor, `BREAKING CHANGE`/`!:` → major, anything else → patch. Override with `--patch`/`--minor`/`--major` or an explicit version (`release.sh 0.3.0`). It refuses when the tree is dirty without `--message`, the branch is not `main`, or no commits exist since the last tag.
4. **What it does** (in order): bumps `package.json` + `src-tauri/Cargo.toml` + `src-tauri/tauri.conf.json`, commits `chore(release): bump version to X.Y.Z`, pushes annotated tag `vX.Y.Z` and `main` to `origin`, runs `pnpm app:install --latest` (full rebuild → install to `/Applications`), and launches the app.
5. **Verify and report.** Confirm the push and tag succeeded (script output ends with the push results), then check the app process is alive: `pgrep -x anyleft`. Report the new version, the tag, and the install result to the user. If any step fails, fix and re-run — the script is safe to re-run before the push; after a successful push, re-running would create a duplicate tag, so resolve the underlying problem instead of blindly retrying.

## Notes

- The script aborts before doing anything destructive on: dirty tree without `--message`, non-`main` branch, missing `jq`, or no commits since the last tag.
- Install requires macOS; `scripts/install.sh` strips Gatekeeper quarantine so the ad-hoc-signed bundle launches.
- Don't skip the launch check — a build can succeed while the bundle fails to start.
