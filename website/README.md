# website

Static landing page for omarchy-kids.com. No build step — plain HTML/CSS.

- `index.html` — redirects to `/en/` or `/de/` based on browser language.
- `en/`, `de/` — the two language versions.
- `assets/` — shared CSS and favicon.
- `CNAME` — custom domain for GitHub Pages (omarchy-kids.com).

Layout is inspired by [omarchy.org](https://omarchy.org) (dark terminal theme, JetBrains Mono, centered nav buttons).

## Deployment

Deployed via GitHub Pages, built by `.github/workflows/deploy-pages.yml` on every push to `main` that touches `website/`. One-time setup still needed:

1. Repo → Settings → Pages → Source: "GitHub Actions".
2. At the domain registrar for omarchy-kids.com, point it at GitHub Pages: an `ALIAS`/`ANAME` (or `A` records to GitHub's Pages IPs) for the apex domain, matching the `CNAME` file in this folder.

**TODO before going live:** add an Impressum (German legal notice) — required once the site is publicly reachable. Deliberately left out for now.
