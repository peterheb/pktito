/* pktito.net. The pages and their files are the static assets build.py
   writes to site/public, which Cloudflare serves before this runs; what
   reaches it is everything else, and of that it answers the installers,
   from the R2 bucket bound as RELEASES:

     /download/macos, /download/windows, /download/linux-deb, /download/linux-appimage
         a redirect to the current release's file, as latest.json names it
     /download/<version>/<file>
         the file, which never changes once posted: cached for a year

   The bucket holds latest.json at its top,
     {"version": "0.1.0", "files": {"macos": "pktito_0.1.0_aarch64.dmg", ...}}
   and each release's files under <version>/. A system it names no file
   for goes to /soon; anything else gets the 404 page.

   A download that starts adds one to its file's count for the day, in
   the D1 database bound as STATS (migrations/): a count and nothing
   else, no address and nothing that tells one visitor from another, as
   the About page says. */

const IMMUTABLE = "public, max-age=31536000, immutable";

export default {
  async fetch(request, env, ctx) {
    let parts;
    try {
      parts = new URL(request.url).pathname.split("/").filter(Boolean).map(decodeURIComponent);
    } catch {
      return env.ASSETS.fetch(request); // a malformed %-escape
    }
    if (parts[0] === "download" && (request.method === "GET" || request.method === "HEAD")) {
      if (parts.length === 2) return latest(request, env, parts[1]);
      if (parts.length === 3) return file(request, env, ctx, parts[1], parts[2]);
    }
    return env.ASSETS.fetch(request);
  },
};

/* A release's file for one system, by way of latest.json, so the site's
   links stay put from release to release. */
async function latest(request, env, system) {
  const index = await env.RELEASES.get("latest.json");
  const release = index ? await index.json() : null;
  const name = release?.files?.[system];
  if (!name) {
    // before the first release, and a system a release leaves out
    if (!["macos", "windows", "linux-deb", "linux-appimage"].includes(system)) return env.ASSETS.fetch(request);
    return new Response(null, { status: 302, headers: { location: "/soon", "cache-control": "no-store" } });
  }
  return new Response(null, {
    status: 302,
    headers: { location: `/download/${release.version}/${encodeURIComponent(name)}`, "cache-control": "public, max-age=300" },
  });
}

/* The file itself, resumable: R2 reads the Range and If-* headers as they
   come. */
async function file(request, env, ctx, version, name) {
  const key = `${version}/${name}`;
  const head = request.method === "HEAD";
  const obj = head ? await env.RELEASES.head(key)
    : await env.RELEASES.get(key, { range: request.headers, onlyIf: request.headers });
  if (!obj) return env.ASSETS.fetch(request);
  const headers = new Headers();
  obj.writeHttpMetadata(headers);
  headers.set("etag", obj.httpEtag);
  headers.set("cache-control", IMMUTABLE);
  headers.set("accept-ranges", "bytes");
  headers.set("content-disposition", `attachment; filename="${name.replace(/["\\]/g, "")}"`);
  if (head) {
    headers.set("content-length", String(obj.size));
    return new Response(null, { headers });
  }
  if (!("body" in obj)) {
    // a precondition held back the body: unchanged since the copy the client has, or not the one it asked for
    return new Response(null, { status: request.headers.has("if-none-match") || request.headers.has("if-modified-since") ? 304 : 412, headers });
  }
  if (starts(request)) ctx.waitUntil(count(env, version, name));
  if (request.headers.has("range") && obj.range) {
    // R2 says which bytes it sent as an offset and length, or a suffix (the last n); the others undefined
    const r = obj.range;
    const offset = r.suffix !== undefined ? obj.size - r.suffix : (r.offset ?? 0);
    const length = r.suffix !== undefined ? r.suffix : (r.length ?? obj.size - offset);
    headers.set("content-range", `bytes ${offset}-${offset + length - 1}/${obj.size}`);
    headers.set("content-length", String(length));
    return new Response(obj.body, { status: 206, headers });
  }
  headers.set("content-length", String(obj.size));
  return new Response(obj.body, { headers });
}

/* A download starting: the whole file, or all of it from the first byte
   as a download manager asks. A resumed download, a probe of a few bytes,
   a HEAD and a crawler's fetch are not counted. */
function starts(request) {
  const range = request.headers.get("range");
  if (range && !/^bytes=0-$/.test(range.trim())) return false;
  return !/bot|crawl|spider|slurp|preview/i.test(request.headers.get("user-agent") || "");
}

/* One more download of version/name today (UTC). The response does not
   wait for it, and a count that fails is a count lost, not a download. */
async function count(env, version, name) {
  if (!env.STATS) return;
  const day = new Date().toISOString().slice(0, 10);
  try {
    await env.STATS.prepare(
      "INSERT INTO downloads (day, version, file, n) VALUES (?1, ?2, ?3, 1) " +
      "ON CONFLICT (day, version, file) DO UPDATE SET n = n + 1",
    ).bind(day, version, name).run();
  } catch {
    // the download goes on
  }
}
