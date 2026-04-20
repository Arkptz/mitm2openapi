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

      oasdiff = pkgs.buildGoModule rec {
        pname = "oasdiff";
        version = "1.11.10";

        src = pkgs.fetchFromGitHub {
          owner = "oasdiff";
          repo = "oasdiff";
          rev = "v${version}";
          hash = "sha256-/Pk2mKzdYKl51RvEkm5yRDMHz2vISgoHlnel+llDJus=";
        };

        vendorHash = "sha256-ZKs9Ai8Q9Yj4V9GIufYRh9cl3ZUKnSehwpaodyGXtfg=";
        subPackages = [ "." ];

        ldflags = [
          "-s"
          "-w"
        ];
      };
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = [
          toolchain
          pkgs.pkg-config
          pkgs.openssl

          # capture
          pkgs.python3
          pkgs.python3Packages.mitmproxy

          # integration tests
          oasdiff
          pkgs.nodejs
          pkgs.shfmt
          pkgs.actionlint
          pkgs.yq-go
          pkgs.prettier

          # demo GIF pipeline
          pkgs.act
          pkgs.vhs
          pkgs.ffmpeg
          pkgs.gifski
          pkgs.gifsicle
        ];
      };
    };
}
