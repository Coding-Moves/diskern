# diskern-site

Marketing/landing page for [Diskern](https://github.com/Coding-Moves/diskern), deployed to GitHub Pages.

## Develop

```sh
npm install
npm run dev
```

## Build

```sh
npm run build   # outputs to dist/
npm run preview # serve the production build locally
```

## Deploy

Pushing to `main` with changes under `site/` triggers
`.github/workflows/deploy-pages.yml`, which builds this project and
publishes `dist/` to GitHub Pages. See the root `docs/` for more.

The Vite `base` in `vite.config.js` is set to `/diskern/` to match this
repo's GitHub Pages URL (`coding-moves.github.io/diskern/` or your
configured custom domain). If the repo is ever renamed, update `base`
to match.
