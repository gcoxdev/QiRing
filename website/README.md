# QiRing product website

A standalone Astro static site: product tour, nine searchable Help topics, security and privacy information, and downloads from the official GitHub releases. Fonts and screenshots are self-hosted. There is no account service, analytics, or runtime API dependency.

## Work locally

Use Node.js 24 LTS and npm. From the repository root:

```sh
cd website
npm ci
npm run dev
```

Open the local URL printed by Astro. To check the production output:

```sh
npm run check:help
npm run build
npx playwright install chromium
npm test
npm run preview
```

The build writes `website/dist/` and checks every generated page's local links, anchors, and image references. Playwright covers desktop and mobile layouts, navigation, Help search, downloads, and automated WCAG accessibility checks. Automated checks do not replace manual keyboard, screen-reader, or visual review.

## Keep the Help section in sync

The source is the nine Help articles in `apps/desktop/index.html`. After editing them, run from `website/`:

```sh
npm run sync:help
npm run check:help
```

Commit the resulting `src/data/help.json` alongside the app edit. The script preserves article content, promotes headings for web pages, and shows both Windows/Linux and macOS keyboard modifiers. Website introductions and installation notes live separately in `src/pages/` and `src/data/site.js`.

The Website workflow checks for drift when either the website or app Help changes. Normal production builds read the checked-in JSON and need nothing outside `website/`.

## Update downloads

`src/data/release.json` is an explicit snapshot of the published **v0.1.3 prerelease** and its 12 assets. The page links directly to GitHub-hosted installers, portable archives, checksums, and SBOMs. It also links to all releases. It deliberately avoids `/releases/latest`, which excludes prereleases.

After publishing a new release:

1. Inspect the public release and its actual assets at <https://github.com/gcoxdev/qiring/releases>.
2. Update the tag, release URL, prerelease flag, publication date, signing status, and each asset's name, browser download URL, and byte size in `src/data/release.json`.
3. Review `src/pages/downloads.astro`, `security.astro`, and home-page copy for signing, architecture, and maturity changes. The current macOS download is Apple Silicon only. Do not claim a signed build without verifying the release.
4. Run the build and browser tests, then redeploy. Test the GitHub links after publishing; local tests check them against the snapshot, not the remote file contents.

This makes deploys reproducible without GitHub API tokens, rate limits, or runtime requests. A draft release must be published before its downloads are public.

## Screenshots: fictional data only

All four PNGs in `public/screenshots/` show the real app frontend at 1280 × 860 with an **in-memory mocked Tauri bridge**. Every entry is invented: `alex@example.com`, reserved `.example` domains, and disposable demo passwords. No real Ring is opened, and no Rust/Tauri backend runs. The capture context blocks external network requests. Visible health results are illustrative fixtures, not an audit of any real passwords.

To regenerate after a UI change, install the desktop frontend dependencies, then run:

```sh
# From the repository root
npm --prefix apps/desktop ci
cd website
npx playwright install chromium
npm run screenshots
npm run build
npm test
```

The command starts and stops its own Vite preview on port 1432. Inspect the images before committing. Never replace them with captures of a personal Ring. The fictional records and mocked commands are reviewable in `scripts/demo-data.mjs`; capture actions are in `scripts/capture-app.mjs`. To use an existing Chromium installation locally, set `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` to its executable path.

## Files and licenses

- `src/pages/`: product pages and static guide routes.
- `src/styles/global.css`: responsive styles and locally loaded fonts.
- `src/data/`: shared links, mirrored Help, release snapshot.
- `public/`: app mark, demo screenshots, IBM Plex fonts and their OFL licenses.
- `scripts/`: Help synchronization, screenshot generation, and output validation.
- `tests/`: browser interaction and accessibility checks.

Website source follows the repository's AGPL-3.0 license. The QiRing mark comes from the desktop app; IBM Plex fonts retain their bundled SIL Open Font License notices.
