# Vendored Scalar bundle

`scalar-standalone-<version>.js` is the standalone browser build of
[`@scalar/api-reference`](https://www.npmjs.com/package/@scalar/api-reference),
which `src/api_docs.rs` serves as the API reference at `/api/docs`. It's
vendored, rather than loaded from a CDN, so the reference works offline.

To bump the version:

```sh
version=1.66.1   # the release to pin
curl -sSL -o "scalar-standalone-${version}.js" \
  "https://cdn.jsdelivr.net/npm/@scalar/api-reference@${version}/dist/browser/standalone.js"
# The .map isn't vendored, so drop the comment pointing devtools at it.
sed -i '/^\/\/# sourceMappingURL=/d' "scalar-standalone-${version}.js"
```

Then replace the old file, and the version in `SCALAR_JS_PATH` and `SCALAR_JS`
in `src/api_docs.rs`. The version is in the served file name, which is what
makes its immutable `Cache-Control` safe.

The bundle is over jj's 1 MiB `snapshot.max-new-file-size`, so jj leaves it
untracked without saying so. Track it explicitly:

```sh
jj --config snapshot.max-new-file-size=4MiB file track "scalar-standalone-${version}.js"
```
