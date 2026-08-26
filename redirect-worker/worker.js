const encoder = new TextEncoder();

function hex(buffer) {
  return [...new Uint8Array(buffer)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

function encodeRfc3986(value) {
  return encodeURIComponent(value).replace(/[!'()*]/g, (c) => `%${c.charCodeAt(0).toString(16).toUpperCase()}`);
}

async function hmac(key, value, output = "raw") {
  const cryptoKey = await crypto.subtle.importKey(
    "raw",
    typeof key === "string" ? encoder.encode(key) : key,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const signature = await crypto.subtle.sign("HMAC", cryptoKey, encoder.encode(value));
  return output === "hex" ? hex(signature) : signature;
}

async function sha256(value) {
  return hex(await crypto.subtle.digest("SHA-256", encoder.encode(value)));
}

async function signingKey(secret, date, region) {
  const kDate = await hmac(`AWS4${secret}`, date);
  const kRegion = await hmac(kDate, region);
  const kService = await hmac(kRegion, "s3");
  return hmac(kService, "aws4_request");
}

function yyyymmdd(date) {
  return date.toISOString().slice(0, 10).replaceAll("-", "");
}

function amzDate(date) {
  return `${yyyymmdd(date)}T${date.toISOString().slice(11, 19).replaceAll(":", "")}Z`;
}

async function signedB2Url(env, key) {
  const now = new Date();
  const date = yyyymmdd(now);
  const timestamp = amzDate(now);
  const host = new URL(env.B2_ENDPOINT).host;
  const region = env.B2_REGION || "us-west-004";
  const expires = String(Math.min(Number(env.LINK_EXPIRES_SECONDS || 86400), 604800));
  const credential = `${env.B2_KEY_ID}/${date}/${region}/s3/aws4_request`;
  const encodedKey = key.split("/").map(encodeRfc3986).join("/");
  const canonicalUri = `/${env.B2_BUCKET}/${encodedKey}`;

  const params = new URLSearchParams({
    "X-Amz-Algorithm": "AWS4-HMAC-SHA256",
    "X-Amz-Credential": credential,
    "X-Amz-Date": timestamp,
    "X-Amz-Expires": expires,
    "X-Amz-SignedHeaders": "host",
  });
  const canonicalQuery = [...params.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([k, v]) => `${encodeRfc3986(k)}=${encodeRfc3986(v)}`)
    .join("&");
  const canonicalRequest = [
    "GET",
    canonicalUri,
    canonicalQuery,
    `host:${host}\n`,
    "host",
    "UNSIGNED-PAYLOAD",
  ].join("\n");
  const stringToSign = [
    "AWS4-HMAC-SHA256",
    timestamp,
    `${date}/${region}/s3/aws4_request`,
    await sha256(canonicalRequest),
  ].join("\n");
  const signature = await hmac(await signingKey(env.B2_SECRET_KEY, date, region), stringToSign, "hex");
  return `${env.B2_ENDPOINT.replace(/\/$/, "")}${canonicalUri}?${canonicalQuery}&X-Amz-Signature=${signature}`;
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const rawPath = decodeURIComponent(url.pathname.replace(/^\/+/, ""));
    if (!rawPath || rawPath === "health") {
      return new Response("533clip redirect online", { status: 200 });
    }

    let key = rawPath;
    if (!key.includes("/")) {
      key = `c/${key.replace(/\.mp4$/i, "")}.mp4`;
    }
    if (!/^(c\/[a-zA-Z0-9_-]+\.mp4|clips\/[a-zA-Z0-9_-]+\/[^/]+\.mp4)$/.test(key)) {
      return new Response("bad clip id", { status: 400 });
    }

    return Response.redirect(await signedB2Url(env, key), 302);
  },
};
