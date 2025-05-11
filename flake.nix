{
  description = "Build a cargo project without extra checks";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane.url = "github:ipetkov/crane";

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    crane,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = nixpkgs.legacyPackages.${system};

      craneLib = crane.mkLib pkgs;

      # Common arguments can be set here to avoid repeating them later
      # Note: changes here will rebuild all dependency crates
      commonArgs = {
        src = craneLib.cleanCargoSource ./.;
        strictDeps = true;

        buildInputs =
          [
            # Add additional build inputs here
          ]
          ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
            # Additional darwin specific inputs can be set here
            pkgs.libiconv
          ];
      };

      hello = craneLib.buildPackage (commonArgs
        // {
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          # Additional environment variables or build phases/hooks can be set
          # here *without* rebuilding all dependency crates
          # MY_CUSTOM_VAR = "some value";
        });

      debian-img = pkgs.dockerTools.pullImage {
        imageName = "debian";
        imageDigest = "sha256:b1211f6d19afd012477bd34fdcabb6b663d680e0f4b0537da6e6b0fd057a3ec3";
        sha256 = "sha256-0t1jLRz8KMk5KegY53kinEkduqUhDOeIUNmb8nUbBL8=";
      };
    in {
      checks = {
        inherit hello;
      };

      packages.default = hello;

      apps.default = flake-utils.lib.mkApp {
        drv = hello;
      };

      packages.docker = pkgs.dockerTools.streamLayeredImage {
        name = "hello";
        tag = "latest";
        # fromImage = debian-img;

        contents = [
          hello
          pkgs.htop
          pkgs.bash
          pkgs.busybox
        ];

        config = {
          Cmd = ["${hello}/bin/hello"];
        };
      };

      devShells.default = craneLib.devShell {
        # Inherit inputs from checks.
        checks = self.checks.${system};

        # Additional dev-shell environment variables can be set directly
        # MY_CUSTOM_DEVELOPMENT_VAR = "something else";

        # Extra inputs can be added here; cargo and rustc are provided by default.
        packages = [
          # pkgs.ripgrep
        ];
      };
    });
}
