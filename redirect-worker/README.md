# 533clip B2 Redirect Worker

Short public links for private Backblaze B2 uploads.

533clip uploads private clips as:

```txt
c/<clipid>.mp4
```

This Worker turns:

```txt
https://your-worker.your-subdomain.workers.dev/c/<clipid>.mp4
```

into a temporary signed Backblaze B2 URL.

## Cloudflare Worker Setup

1. Create a Cloudflare account.
2. Go to **Workers & Pages**.
3. Create a new Worker.
4. Paste `worker.js`.
5. Open **Settings > Variables**.
6. Add these environment variables:

```txt
B2_ENDPOINT=https://s3.eu-central-003.backblazeb2.com
B2_REGION=eu-central-003
B2_BUCKET=533clip
B2_KEY_ID=your_backblaze_key_id
B2_SECRET_KEY=your_backblaze_secret_key
LINK_EXPIRES_SECONDS=86400
```

Use **Secret** type for `B2_SECRET_KEY`.

## 533clip Settings

In `Settings > Sharing`:

```txt
Provider: Backblaze B2
Endpoint: https://s3.eu-central-003.backblazeb2.com
Region: eu-central-003
Bucket: 533clip
Public URL: https://your-worker.your-subdomain.workers.dev
```

Now Discord gets clean links like:

```txt
https://your-worker.your-subdomain.workers.dev/c/a1b2c3d4e5.mp4
```

