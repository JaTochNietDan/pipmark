# Pipmark website

A buildless landing page served by Cloudflare Worker `pipmark-site` at
https://pipmark.io. No GitHub Pages deployment is used.

Preview: `python3 -m http.server 8137 --directory site` from the repository root.

Deploy: `python3 scripts/deploy_site.py` with `CLOUDFLARE_API_TOKEN` in the
environment. The existing ops mise environment provides this credential locally;
run the script by absolute path from that checkout. The deploy script embeds
static assets in one Worker module, with no runtime dependencies or third-party
requests. The download link points to the latest normal GitHub release.

The custom domain and Cloudflare-managed DNS/certificate are tracked in the ops
repository at `terraform/zones/pipmark.io/main.tf`. The code is managed here.
