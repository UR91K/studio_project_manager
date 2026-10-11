import { afterEach, describe, expect, it, vi } from "vitest";
import { Api } from "./api";

/** A stand-in daemon list route: `total` rows, paged by limit/offset, answering at most
 *  `cap` rows a request whatever limit is asked for. Records each request's URL. */
function serve(key: string, total: number, cap = Infinity) {
  const urls: URL[] = [];
  vi.stubGlobal("fetch", async (input: string) => {
    const url = new URL(input);
    urls.push(url);
    const limit = Math.min(Number(url.searchParams.get("limit") ?? 1000), cap);
    const offset = Number(url.searchParams.get("offset") ?? 0);
    const rows = Array.from({ length: Math.max(0, Math.min(limit, total - offset)) }, (_, i) => ({ id: String(offset + i) }));
    return new Response(JSON.stringify({ [key]: rows, total_count: total }));
  });
  return urls;
}

afterEach(() => { vi.unstubAllGlobals(); });

describe("reading a whole list", () => {
  // The bug: search asked for limit=200 and samples for limit=10000, and the screen showed
  // what came back as if it were everything.
  it("reads past one page, to the daemon's total", async () => {
    const urls = serve("samples", 18243);
    const rows = await new Api("http://daemon").samples();
    expect(rows).toHaveLength(18243);
    expect(new Set(rows.map((r) => r.id)).size).toBe(18243);
    expect(urls.map((u) => u.searchParams.get("offset"))).toEqual(["0", "5000", "10000", "15000"]);
  });

  it("keeps a search's query when it adds the paging", async () => {
    const urls = serve("projects", 1184);
    expect(await new Api("http://daemon").search('plugin:"Pro-Q 3"')).toHaveLength(1184);
    expect(urls[0].searchParams.get("query")).toBe('plugin:"Pro-Q 3"');
  });

  it("throws rather than pass a capped list off as whole", async () => {
    serve("projects", 1184, 200);
    await expect(new Api("http://daemon").search("plugin:x")).rejects.toThrow("read 200 rows, the daemon counts 1184");
  });

  it("makes one request for an empty list", async () => {
    const urls = serve("collections", 0);
    expect(await new Api("http://daemon").collections()).toEqual([]);
    expect(urls).toHaveLength(1);
  });
});

describe("failed requests", () => {
  it("show the daemon's own error text", async () => {
    vi.stubGlobal("fetch", async () => new Response(JSON.stringify({ error: "not a folder: /bad" }), { status: 400 }));
    await expect(new Api("http://daemon").addProjectPath("/bad")).rejects.toThrow("not a folder: /bad");
  });

  it("keep the request line when the body is not the daemon's error", async () => {
    vi.stubGlobal("fetch", async () => new Response("oops", { status: 500 }));
    await expect(new Api("http://daemon").shutdownDaemon()).rejects.toThrow("POST /api/v1/system/shutdown: 500 oops");
  });
});
