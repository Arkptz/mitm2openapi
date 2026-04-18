{
  description = "mitm2openapi — convert mitmproxy/HAR captures to OpenAPI 3.0 specs";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      ...
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs {
        inherit system;
        overlays = [ (import rust-overlay) ];
      };
      toolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [
          "rust-src"
          "rust-analyzer"
        ];
      };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = [
          toolchain
          pkgs.pkg-config
          pkgs.openssl
          pkgs.python3
          pkgs.python3Packages.mitmproxy
        ];
      };
    };
}
