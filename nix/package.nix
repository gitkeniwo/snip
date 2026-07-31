# A frozen nixpkgs-submission draft, pinned to one release. The flake does not
# build this file: flake.nix overrides version, src, and cargoDeps (from
# Cargo.lock), so nothing here affects `nix run` / `nix profile install`.
# Refresh the version and both hashes once, right before opening the nixpkgs PR.
{
  lib,
  rustPlatform,
  fetchFromGitHub,
  gitMinimal,
  installShellFiles,
  stdenv,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "sniplab";
  version = "0.3.0";

  src = fetchFromGitHub {
    owner = "gitkeniwo";
    repo = "snip";
    rev = "v${finalAttrs.version}";
    hash = "sha256-hWnPkcJyTH4y5mlz8I1erIN1yarlwAfzYnk+pVHlJJ0=";
  };

  cargoHash = "sha256-0fKjBqaR+Sq0B24rqBJNYdyPKQ4pKut9vH9MuY2qYpA=";

  nativeBuildInputs = [ installShellFiles ];
  nativeCheckInputs = [ gitMinimal ];

  # `tui` is currently the crate's only feature, so this is equivalent to
  # Cargo's `--all-features` while using buildRustPackage's feature interface.
  buildFeatures = [ "tui" ];

  doCheck = true;

  preCheck = ''
    export HOME=$(mktemp -d)
  '';

  postInstall =
    lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
      installShellCompletion --cmd snip \
        --bash <($out/bin/snip completion bash) \
        --zsh  <($out/bin/snip completion zsh) \
        --fish <($out/bin/snip completion fish)
    ''
    + ''
      installManPage man/*.1
      install -Dm644 README.md LICENSE -t $out/share/doc/sniplab
    '';

  meta = {
    description = "Filesystem-native snippet library and agent-friendly CLI";
    homepage = "https://github.com/gitkeniwo/snip";
    changelog = "https://github.com/gitkeniwo/snip/blob/v${finalAttrs.version}/CHANGELOG.md";
    license = lib.licenses.mit;
    mainProgram = "snip";
    # Add the submitter's nixpkgs maintainer handle before opening the PR.
    maintainers = with lib.maintainers; [ ];
    platforms = lib.platforms.unix;
  };
})
