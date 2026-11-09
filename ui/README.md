# Tracewise desktop app (preview)

Tauri 2 and Svelte 5. The app runs the `trail` program for every read and action; it holds no data of its own.

```bash
pnpm install
pnpm tauri dev            # the native app
pnpm check && pnpm test   # type-check and unit tests
```

## In a browser, against real history (development only)

`dev/trail-bridge.js` lets `vite dev` answer the app's calls with the real binary, using the same allow-list as the desktop app. It is off unless `TRAIL_UI_BRIDGE=1` and is never part of a build.

```bash
eval "$(dev/make-demo.sh /tmp/trail-ui-demo)"   # a disposable repository with changes by a person and three agents
TRAIL_UI_BRIDGE=1 pnpm dev                      # then open http://127.0.0.1:1420/
```
