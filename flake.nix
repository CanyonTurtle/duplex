{
  description = "duplex - rubik's cube LL/ZBLL alg tooling (wasm frontend + native CLI)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ "wasm32-unknown-unknown" ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          NIX_CONFIG = "experimental-features = nix-command flakes";

          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          packages = with pkgs; [
            rustToolchain
            cargo-watch
            cargo-edit

            # frontend (webpack/yarn build for the wasm UI)
            nodejs
            yarn

            # handy for poking at the generated candidate/solution csv/json
            jq
          ];
        };
      });
}
