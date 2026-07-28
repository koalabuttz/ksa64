importScripts("/precache-manifest.js");

const PRECACHE_VERSION = typeof self.__KSA64_PRECACHE_VERSION__ === "string"
  ? self.__KSA64_PRECACHE_VERSION__
  : "invalid";
const PRECACHE = Array.isArray(self.__KSA64_PRECACHE__)
  ? self.__KSA64_PRECACHE__.filter((route) => typeof route === "string" && route.startsWith("/") && route !== "/runtime-config.js")
  : [];
const CACHE_PREFIX = "ksa64-shell-";
const CACHE_NAME = CACHE_PREFIX + PRECACHE_VERSION;

self.addEventListener("install", (event) => {
  event.waitUntil(
    PRECACHE_VERSION === "invalid" || PRECACHE.length === 0
      ? Promise.reject(new Error("KSA64 precache manifest is invalid"))
      : caches.open(CACHE_NAME).then((cache) => cache.addAll(PRECACHE)),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((key) => key.startsWith(CACHE_PREFIX) && key !== CACHE_NAME).map((key) => caches.delete(key))))
      .then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;

  const url = new URL(request.url);
  if (url.origin !== self.location.origin || url.pathname.startsWith("/session/") || url.pathname === "/runtime-config.js") return;

  event.respondWith(
    caches.match(request).then((cached) => {
      const fresh = fetch(request)
        .then((response) => {
          const cacheControl = response.headers.get("cache-control") ?? "";
          if (response.ok && !cacheControl.toLowerCase().includes("no-store")) {
            const copy = response.clone();
            void caches.open(CACHE_NAME).then((cache) => cache.put(request, copy));
          }
          return response;
        })
        .catch(() => cached ?? caches.match("/index.html"));
      return cached ?? fresh;
    }),
  );
});
