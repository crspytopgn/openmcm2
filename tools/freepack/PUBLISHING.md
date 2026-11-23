# Publishing the Free Pack

The Free Pack downloads from two places:

- **The launcher** downloads it from a GitHub release in `crspytopgn/openmcm2` with the tag `freepack`.
- **The browser version** downloads it from a Cloudflare R2 bucket. GitHub release files can't be fetched by a web page, because GitHub doesn't send the CORS headers that would allow it.

Both places hold the same `freepack.json`. That file lists every file in the pack with its SHA-256. An update only downloads what changed: the launcher fetches just the part zips that hold a changed file, and the browser fetches just the changed files.

## 1. Pack it

First give the pack its menus, so it plays with no MM2 files. Their layout comes from a copy of the game's `UI` folder; the words and the Pro Circuit's files are ours, in `freepack/menu/`:

    cargo run --release -p mm2-tools -- free-menus <MM2>/UI freepack/menu/text.txt <pack folder>/menus
    python3 freepack/menu/no_text.py <pack folder>
    python3 tools/freepack/make_ui_sounds.py <pack folder>

Then turn its sounds into Ogg Vorbis (needs ffmpeg with libvorbis). The sources stay WAVs; this writes a `.ogg` beside each `.wav`, checks it is the same length to the sample, and removes the `.wav`. The sound part goes from about 20 MB to about 3 MB. OpenMCM2 0.7.7 and later play `.ogg`, so publish a pack made this way only after that release is out (its `freepack.json` says `"format": 2`, which older launchers refuse rather than download sounds they can't play):

    python3 tools/freepack/ogg_sounds.py <pack folder>

Then give the scenery a little more colour (the publish workflow does this with its `colour_lift` input, 0.20 by default). It lifts the colour of the ground, skies, plants and trackside objects by 20 %, keeping brightness and contrast, and leaves bikes, riders, menus and the HUD alone. Run it once only: each run lifts again. Free tracks bake the ground into their files, so a change to the amount also needs `MADE` in `crates/mm2-course/src/free.rs` bumped, so players' tracks are built again:

    python3 tools/freepack/saturate.py <pack folder> 0.20

Then run this on the finished pack folder, after `share_textures.py`:

    python3 tools/freepack/pack.py <pack folder> out 2026.11.20

That makes:

- `out/github/`: the part zips and `freepack.json`, for the GitHub release.
- `out/r2/`: `freepack.json` and `files/<sha256>`, for R2.

## 2. GitHub release (launcher)

The first time only, create the release. Mark it a **pre-release**, so the launcher's own update check (`releases/latest`) never mistakes it for a new version of OpenMCM2:

    gh release create freepack --repo crspytopgn/openmcm2 --prerelease \
      --title "Free Pack" --notes "Art and sound to play without MM2. The launcher downloads this for you." out/github/*

For each new version, upload the new parts and the new manifest, then delete the parts the new manifest no longer names:

    gh release upload freepack --repo crspytopgn/openmcm2 --clobber out/github/*

The parts' names contain a hash of what they hold. That means unchanged parts keep their names, and nobody downloads them again.

## 3. Cloudflare R2 (browser)

One-time setup in the Cloudflare dashboard, on the project's account:

1. **R2 Object Storage → Create bucket**: name it `openmcm2-freepack`, location Automatic.
2. In the bucket, go to **Settings → Public access** and do one of these:
   - Connect a custom domain if you have one on Cloudflare (for example `freepack.<your domain>`). This is better: it's cached at the edge and has no rate limit.
   - Or allow the **R2.dev subdomain**. It's fine for testing, but rate-limited.
3. In the bucket, go to **Settings → CORS policy** and paste:

       [{"AllowedOrigins": ["https://crspytopgn.github.io", "http://localhost:8000"],
         "AllowedMethods": ["GET", "HEAD"], "AllowedHeaders": ["*"], "MaxAgeSeconds": 86400}]

4. **R2 → Manage R2 API tokens → Create API token**: choose Object Read & Write, limited to `openmcm2-freepack`. Keep the Access Key ID, Secret Access Key and the account's S3 endpoint `https://<account id>.r2.cloudflarestorage.com`. Don't put them in the repo or in chat.

Upload with rclone (`brew install rclone`, then `rclone config`, choosing type `s3`, provider `Cloudflare`, with the key and endpoint above, named `r2`):

    rclone copy out/r2/files r2:openmcm2-freepack/files --header-upload "Cache-Control: public, max-age=31536000, immutable"
    rclone copyto out/r2/freepack.json r2:openmcm2-freepack/freepack.json --header-upload "Cache-Control: no-cache"

Always upload the files first and the manifest last, so a player never sees a manifest that names files which aren't there yet. Old files can stay. To clean them up now and then, remove files the manifest doesn't list: `rclone sync out/r2/files r2:openmcm2-freepack/files`.

Then put the bucket's public address in `PACK_URL` in `web/index.html`, ending with `/`.

## Cost

R2 charges nothing for downloads (egress). Storage is free up to 10 GB a month, then $0.015 per GB. Reads are free up to 10 million a month. The pack is under 2 GB, so it should cost nothing.
