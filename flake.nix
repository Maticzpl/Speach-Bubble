# Run with `nix develop .#default` assuming this file is in your pwd
{
  description = "Speachbubble flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-24.11";
  };

  outputs = { self, nixpkgs, ... }:
    let
      system = "x86_64-linux";
    in
    {
      devShells."${system}".default =
        let
          pkgs = import nixpkgs {
            inherit system;
          };
        in
        pkgs.mkShell {
          packages = with pkgs; [
            rustfmt
            bacon
            pkg-config
            openssl
            # cargo
            cargo-watch
          ];

          shellHook = /*bash*/ ''

      '';
        };
    };
}
