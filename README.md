# Minesweeper
A simple minesweeper implementation in Vanilla JS.

## Versions

| Path  | Description |
|-------|-------------|
| `/v1` | The original 2017 Vanilla JS implementation, served as is from [`v1/`](v1/). |

## Deploying on Vercel

The repository deploys as a static site on Vercel's free (Hobby) plan. Routing lives in [`vercel.json`](vercel.json).

1. Sign in at [vercel.com](https://vercel.com) with GitHub.
2. Click **Add New… → Project** and import this repository.
3. Leave **Framework Preset** as **Other** and keep the default build settings, then click **Deploy**.

After that, every push to `master` deploys to production and every other branch gets its own preview URL.

## Running locally

Serve the repository root with any static file server and open `/v1/base.html`:

```sh
python3 -m http.server 8000
# http://localhost:8000/v1/base.html
```
