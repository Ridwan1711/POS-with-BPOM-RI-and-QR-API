import type { NextRequest } from "next/server";

/**
 * Same-origin proxy to the Rust API. The browser only talks to this Next.js origin, so the
 * httpOnly session cookie is first-party, and no CORS setup is needed in the browser.
 *
 * BACKEND_URL is read at runtime (not baked in at build time), so one image runs anywhere.
 */
export const dynamic = "force-dynamic";

const FORWARDED_REQUEST_HEADERS = ["cookie", "content-type", "accept", "x-store-id", "user-agent"];
const FORWARDED_RESPONSE_HEADERS = ["content-type", "cache-control", "retry-after"];

function backendBase(): URL {
  const base = process.env.BACKEND_URL ?? "http://localhost:8080";
  return new URL(base.endsWith("/") ? base : `${base}/`);
}

async function proxy(request: NextRequest, context: { params: Promise<{ path: string[] }> }): Promise<Response> {
  const { path } = await context.params;
  const target = new URL(`api/v1/${path.map(encodeURIComponent).join("/")}`, backendBase());
  target.search = request.nextUrl.search;

  const headers = new Headers();
  for (const name of FORWARDED_REQUEST_HEADERS) {
    const value = request.headers.get(name);
    if (value) headers.set(name, value);
  }
  const hasBody = request.method !== "GET" && request.method !== "HEAD";
  const body = hasBody ? await request.text() : undefined;

  let upstream: Response;
  try {
    upstream = await fetch(target, {
      method: request.method,
      headers,
      body,
      cache: "no-store",
      redirect: "manual",
    });
  } catch {
    return Response.json(
      { error: { code: "bad_gateway", message: "The API is not reachable right now. Try again shortly.", details: null } },
      { status: 502 },
    );
  }

  const response = new Response(upstream.status === 204 ? null : upstream.body, { status: upstream.status });
  for (const name of FORWARDED_RESPONSE_HEADERS) {
    const value = upstream.headers.get(name);
    if (value) response.headers.set(name, value);
  }
  for (const cookie of upstream.headers.getSetCookie()) {
    response.headers.append("set-cookie", cookie);
  }
  return response;
}

export { proxy as GET, proxy as POST, proxy as PATCH, proxy as DELETE };
