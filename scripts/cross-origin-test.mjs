// Checks a running RepoMemo server for the web app on another address (cross-origin mode, V0.4.17). Nothing is
// opened in a browser: the requests are the ones a browser makes from the web app's origin and from a third one.
//
//   node scripts/cross-origin-test.mjs http://127.0.0.1:3020 http://app.localtest.me:8080 http://evil.localtest.me
//
// The server must be new (the first account to register becomes its administrator), started with
// REPOMEMO_ALLOWED_ORIGIN=<the web origin> and REPOMEMO_COOKIE_SAMESITE=lax (or none, behind https).
const [api, webOrigin, otherOrigin] = process.argv.slice(2).map((value) => value?.replace(/\/$/, ""));
if (!api || !webOrigin || !otherOrigin) {
  console.error("usage: node scripts/cross-origin-test.mjs <api> <web origin> <third origin>");
  process.exit(2);
}

let failed = 0;
const check = (name, ok, detail = "") => {
  if (!ok) failed += 1;
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? ` - ${detail}` : ""}`);
};
const call = (path, { origin, cookie, token, method = "POST", body, headers = {} } = {}) =>
  fetch(api + path, {
    method,
    redirect: "manual",
    headers: {
      ...(body === undefined ? {} : { "content-type": "application/json" }),
      "x-repomemo-client": "web",
      ...(origin ? { origin } : {}),
      ...(cookie ? { cookie } : {}),
      ...(token ? { authorization: `Bearer ${token}` } : {}),
      ...headers,
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });

const registered = await call("/v1/auth/register", {
  origin: webOrigin,
  body: { email: "cross-origin@example.com", display_name: "Cross", password: "a-long-enough-password-1" },
});
check("the web origin can register", registered.status === 201, `status ${registered.status}`);
const session = await registered.json().catch(() => ({}));
const setCookie = registered.headers.getSetCookie?.() ?? [];
const cookieLine = setCookie.find((line) => /repomemo_refresh=/.test(line)) ?? "";
check("the refresh token is a cookie and not in the body", Boolean(cookieLine) && session.refresh_token === "");
check("the cookie is HttpOnly", /;\s*HttpOnly/i.test(cookieLine));
const sameSite = /SameSite=(\w+)/i.exec(cookieLine)?.[1];
check("the cookie is not SameSite=Strict", sameSite === "Lax" || sameSite === "None", `SameSite=${sameSite}`);
const cookie = cookieLine.split(";")[0];

const refreshed = await call("/v1/auth/refresh", { origin: webOrigin, cookie, body: {} });
check("the web origin can refresh with the cookie", refreshed.status === 200, `status ${refreshed.status}`);
const renewed = await refreshed.json().catch(() => ({}));

const third = await call("/v1/auth/refresh", { origin: otherOrigin, cookie, body: {} });
check("a third origin presenting the cookie is refused", third.status === 403, `status ${third.status}`);

const allowed = await call("/v1/auth/refresh", {
  method: "OPTIONS",
  origin: webOrigin,
  headers: { "access-control-request-method": "POST", "access-control-request-headers": "content-type,x-repomemo-client" },
});
check("the preflight from the web origin names it", allowed.headers.get("access-control-allow-origin") === webOrigin);
check("the preflight allows credentials", allowed.headers.get("access-control-allow-credentials") === "true");
const refused = await call("/v1/auth/refresh", {
  method: "OPTIONS",
  origin: otherOrigin,
  headers: { "access-control-request-method": "POST", "access-control-request-headers": "content-type,x-repomemo-client" },
});
check("the preflight from a third origin gets no permission", !refused.headers.get("access-control-allow-origin"));

const events = await call("/v1/system/events", { method: "GET", origin: webOrigin, token: renewed.access_token ?? session.access_token });
check("the live event stream opens for the web origin", events.status === 200 && /text\/event-stream/.test(events.headers.get("content-type") ?? ""), `status ${events.status}`);
await events.body?.cancel();

console.log(failed === 0 ? "Cross-origin mode works." : `${failed} check${failed === 1 ? "" : "s"} failed.`);
process.exit(failed === 0 ? 0 : 1);
