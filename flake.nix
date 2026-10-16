{
  description = "Wan Shi Tong development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      crane,
      ...
    }:
    let
      # Not x86_64-darwin: this nixpkgs has dropped it, and importing it for
      # that system throws, which breaks `nix flake show` for everyone.
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
    in
    {
      nixosModules.default = import ./nix/module.nix self;
    }
    // flake-utils.lib.eachSystem systems (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # The release binaries' targets: static, so they run on any Linux.
        releaseTargets = [
          "x86_64-unknown-linux-musl"
          "aarch64-unknown-linux-musl"
        ];
        # What wanshitong-web's build script needs, in a shell.
        webBuildEnv = {
          # Compiles the stylesheet with this instead of downloading a
          # Tailwind release
          TAILWIND_CLI = pkgs.lib.getExe pkgs.tailwindcss_4;
          # ...and the TypeScript with this
          TSC = pkgs.lib.getExe pkgs.typescript;
        };
      in
      {
        packages.default = pkgs.callPackage ./nix/package.nix { inherit craneLib; };

        checks = pkgs.lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
          module = import ./nix/test.nix { inherit self pkgs; };
        };

        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            # Rust toolchain (from rust-toolchain.toml)
            rustToolchain

            # Database
            sqlx-cli
            sqlite

            # Runtime dependency (ffprobe)
            ffmpeg-headless

            nixfmt
          ];

          env = webBuildEnv // {
            # Fetch git dependencies with the git CLI so SSH `insteadOf`
            # rewrites and ssh-agent work (libgit2 can't handle them)
            CARGO_NET_GIT_FETCH_WITH_CLI = "true";
          };
        };

        # Builds the release binaries, as the release workflow does:
        #
        #   nix develop .#release -c cargo zigbuild --locked --release -p wst \
        #     --target x86_64-unknown-linux-musl --target aarch64-unknown-linux-musl
        #
        # cargo-zigbuild links for either target from here, with zig, so no
        # cross toolchain is needed. Queries are checked against the committed
        # sqlx cache.
        devShells.release = pkgs.mkShell {
          packages = with pkgs; [
            (rustToolchain.override { targets = releaseTargets; })
            cargo-zigbuild
            zig
          ];
          env = webBuildEnv // {
            SQLX_OFFLINE = "true";
          };
        };

        formatter = pkgs.nixfmt;
      }
    );
}
