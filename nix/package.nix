{
  lib,
  craneLib,
  ffmpeg-headless,
  tailwindcss_4,
  typescript,
}:
let
  root = ../.;

  # Shared by the dependency-only build and the real build. Anything that
  # changes how dependencies compile (features, profile, the packages built,
  # environment their build scripts read) must be identical in both, or the
  # real build silently recompiles the affected dependencies.
  commonArgs = {
    pname = "wanshitong";
    version = (lib.importTOML ../Cargo.toml).workspace.package.version;

    src = lib.fileset.toSource {
      inherit root;
      fileset = lib.fileset.unions [
        ../Cargo.toml
        ../Cargo.lock
        ../ffprobe-types
        ../wanshitong-api
        ../wanshitong-app
        ../wanshitong-client
        ../wanshitong-core
        ../wanshitong-web
        ../wst
      ];
    };
    strictDeps = true;

    # The whole workspace, for both building and testing, so tests reuse the
    # dependencies exactly as built.
    cargoExtraArgs = "--locked --workspace";

    env = {
      # Check sqlx queries against the committed .sqlx cache, not a live database
      SQLX_OFFLINE = "true";
    };
  };

  # Rebuilt only when dependency-related parts of the manifests or Cargo.lock
  # change; crane stubs out the workspace's own sources for it.
  cargoArtifacts = craneLib.buildDepsOnly commonArgs;
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;

    # Only wanshitong-web's own build script reads these, and crane stubs it out
    # in the dependency build, so they stay out of commonArgs: updating either
    # tool doesn't rebuild the dependencies. The sandbox couldn't download them
    # during the build anyway.
    env = commonArgs.env // {
      TAILWIND_CLI = lib.getExe tailwindcss_4;
      TSC = lib.getExe typescript;
    };

    # The ingest tests shell out to ffprobe
    nativeCheckInputs = [ ffmpeg-headless ];

    meta = {
      description = "Self-hosted media knowledge system";
      mainProgram = "wst";
      platforms = lib.platforms.linux;
    };
  }
)
