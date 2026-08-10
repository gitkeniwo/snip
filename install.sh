#!/bin/sh
set -eu

REPO="gitkeniwo/snip"
bin_dir="${HOME}/.local/bin"
requested_version=""
version_was_set=false
force=false
verify_signature=false
uninstall=false

usage() {
  cat <<'EOF'
Usage: install.sh [OPTIONS]

Install the latest snip release to ~/.local/bin.

Options:
  --version VERSION     Install a specific release
  --bin-dir DIRECTORY   Install to a different binary directory
  --force               Replace another installation or allow a downgrade
  --verify-signature    Verify the SHA256SUMS OpenPGP signature with gpg
  --uninstall           Remove the installed binary
  --help                Show this help
EOF
}

die() {
  printf '%s\n' "$1" >&2
  exit 1
}

version_is_valid() {
  version_value=$1
  version_major=${version_value%%.*}
  version_rest=${version_value#*.}
  version_minor=${version_rest%%.*}
  version_patch=${version_rest#*.}

  [ "$version_value" != "$version_rest" ] || return 1
  [ "$version_rest" != "$version_patch" ] || return 1
  case "$version_patch" in
    *.*) return 1 ;;
  esac
  for version_part in "$version_major" "$version_minor" "$version_patch"; do
    case "$version_part" in
      ''|*[!0-9]*) return 1 ;;
    esac
  done
}

compare_versions() {
  compare_left=$1
  compare_right=$2
  compare_left_major=${compare_left%%.*}
  compare_left_rest=${compare_left#*.}
  compare_left_minor=${compare_left_rest%%.*}
  compare_left_patch=${compare_left_rest#*.}
  compare_right_major=${compare_right%%.*}
  compare_right_rest=${compare_right#*.}
  compare_right_minor=${compare_right_rest%%.*}
  compare_right_patch=${compare_right_rest#*.}

  if [ "$compare_left_major" -lt "$compare_right_major" ]; then
    printf '%s\n' -1
  elif [ "$compare_left_major" -gt "$compare_right_major" ]; then
    printf '%s\n' 1
  elif [ "$compare_left_minor" -lt "$compare_right_minor" ]; then
    printf '%s\n' -1
  elif [ "$compare_left_minor" -gt "$compare_right_minor" ]; then
    printf '%s\n' 1
  elif [ "$compare_left_patch" -lt "$compare_right_patch" ]; then
    printf '%s\n' -1
  elif [ "$compare_left_patch" -gt "$compare_right_patch" ]; then
    printf '%s\n' 1
  else
    printf '%s\n' 0
  fi
}

upgrade_command_for() {
  upgrade_path=$1
  case "$upgrade_path" in
    /opt/homebrew/*)
      printf '%s\n' 'brew upgrade snip'
      ;;
    /usr/local/*)
      if [ "$os" = Darwin ]; then
        printf '%s\n' 'brew upgrade snip'
      else
        printf '%s\n' 'the method that installed it'
      fi
      ;;
    /usr/bin/*|/usr/sbin/*)
      printf '%s\n' 'apt reinstall <downloaded-package.deb> or dnf reinstall <downloaded-package.rpm>'
      ;;
    /nix/store/*|*.nix-profile/*)
      printf '%s\n' 'nix profile upgrade'
      ;;
    "$HOME/.cargo/bin"/*)
      printf '%s\n' 'cargo binstall sniplab'
      ;;
    *)
      printf '%s\n' 'the package manager that installed it'
      ;;
  esac
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --version)
      [ "$#" -ge 2 ] || die '--version requires a value'
      requested_version=$2
      version_was_set=true
      shift 2
      ;;
    --version=*)
      requested_version=${1#*=}
      version_was_set=true
      shift
      ;;
    --bin-dir)
      [ "$#" -ge 2 ] || die '--bin-dir requires a value'
      bin_dir=$2
      shift 2
      ;;
    --bin-dir=*)
      bin_dir=${1#*=}
      shift
      ;;
    --force)
      force=true
      shift
      ;;
    --verify-signature)
      verify_signature=true
      shift
      ;;
    --uninstall)
      uninstall=true
      shift
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      die "unknown option: $1"
      ;;
  esac
done

[ -n "$bin_dir" ] || die '--bin-dir cannot be empty'
bin_path="$bin_dir/snip"

if [ "$uninstall" = true ]; then
  if [ ! -e "$bin_path" ]; then
    printf 'snip is not installed at %s\n' "$bin_path"
    exit 0
  fi
  # `snip man install --prefix` may put pages elsewhere; only the default
  # location can be identified without reading snip's own install manifest.
  man_page="$HOME/.local/share/man/man1/snip.1"
  if [ -e "$man_page" ]; then
    die "run \`snip man uninstall\` before uninstalling the binary, then run this script again"
  fi
  rm -f "$bin_path"
  printf 'removed %s\n' "$bin_path"
  exit 0
fi

os="$(uname -s)"
arch="$(uname -m)"

# Under Rosetta 2, `uname -m` reports x86_64 on Apple Silicon. Prefer the
# native arm64 build so we do not silently install the slower Intel binary.
if [ "$os" = Darwin ] && [ "$arch" = x86_64 ]; then
  if [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
    arch=arm64
  fi
fi

case "$os" in
  MINGW*|MSYS*|CYGWIN*)
    die 'this script installs the Linux binary and cannot be used from Git Bash, MSYS, or Cygwin; install with scoop: scoop install snip'
    ;;
esac

case "$os/$arch" in
  Darwin/arm64) target='aarch64-apple-darwin' ;;
  Darwin/x86_64) target='x86_64-apple-darwin' ;;
  Linux/x86_64|Linux/amd64) target='x86_64-unknown-linux-musl' ;;
  Linux/aarch64|Linux/arm64) target='aarch64-unknown-linux-musl' ;;
  *) die "no prebuilt binary for $os/$arch; install with: cargo install sniplab" ;;
esac

if command -v curl >/dev/null 2>&1; then
  transfer_tool=curl
elif command -v wget >/dev/null 2>&1; then
  transfer_tool=wget
else
  die 'neither curl nor wget is available'
fi

download() {
  download_url=$1
  download_destination=$2
  download_error=$3
  if [ "$transfer_tool" = curl ]; then
    if ! curl -fsSL "$download_url" -o "$download_destination"; then
      die "$download_error"
    fi
  else
    if ! wget -q "$download_url" -O "$download_destination"; then
      die "$download_error"
    fi
  fi
}

if [ "$version_was_set" = true ]; then
  version=${requested_version#v}
  if ! version_is_valid "$version"; then
    die "invalid version: $requested_version; expected X.Y.Z"
  fi
else
  latest_url="https://github.com/$REPO/releases/latest"
  if [ "$transfer_tool" = curl ]; then
    if ! url="$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$latest_url")"; then
      die 'could not determine the latest version from the GitHub redirect'
    fi
  else
    response="$(wget --server-response --spider "$latest_url" 2>&1 || true)"
    url="$(printf '%s\n' "$response" | awk 'tolower($1) == "location:" { url=$2 } END { gsub("\\r", "", url); print url }')"
  fi
  version="${url##*/v}"
  if ! version_is_valid "$version"; then
    die 'could not determine the latest version from the GitHub redirect'
  fi
fi

if installed_path="$(command -v snip 2>/dev/null)"; then
  installed_dir="$(dirname "$installed_path")"
  if [ "$installed_dir" != "$bin_dir" ] && [ "$force" != true ]; then
    upgrade_command="$(upgrade_command_for "$installed_path")"
    die "snip is already installed at $installed_path; upgrade it with $upgrade_command, or pass --force to replace it with this script's copy"
  fi
fi

if [ -e "$bin_path" ]; then
  installed_version=""
  if installed_output="$("$bin_path" --version 2>/dev/null)"; then
    case "$installed_output" in
      *' '*) installed_version=${installed_output#* } ;;
    esac
  fi

  if version_is_valid "$installed_version"; then
    version_order="$(compare_versions "$installed_version" "$version")"
    if [ "$version_order" -eq 0 ] && [ "$force" != true ]; then
      printf 'snip %s is already installed\n' "$version"
      exit 0
    elif [ "$version_order" -gt 0 ] && [ "$force" != true ]; then
      die "refusing to downgrade snip $installed_version to $version; pass --force to do it anyway"
    elif [ "$version_order" -lt 0 ]; then
      printf 'upgrading snip %s -> %s\n' "$installed_version" "$version"
    fi
  fi
fi

asset="snip-$target.tar.gz"
release_url="https://github.com/$REPO/releases/download/v$version"
tmp="$(mktemp -d)"
install_tmp="$bin_dir/.snip.tmp.$$"
trap 'rm -rf "$tmp"; rm -f "$install_tmp"' EXIT INT TERM

download "$release_url/SHA256SUMS" "$tmp/SHA256SUMS" \
  "failed to download SHA256SUMS for snip $version; this release may predate checksum support"
download "$release_url/$asset" "$tmp/$asset" \
  "failed to download $asset for snip $version"

if [ "$verify_signature" = true ]; then
  command -v gpg >/dev/null 2>&1 || die '--verify-signature requires gpg'
  download "$release_url/SHA256SUMS.asc" "$tmp/SHA256SUMS.asc" \
    "failed to download SHA256SUMS.asc for snip $version"
  gpg --verify "$tmp/SHA256SUMS.asc" "$tmp/SHA256SUMS"
fi

expected="$(grep " $asset\$" "$tmp/SHA256SUMS" | cut -d' ' -f1)"
if [ -z "$expected" ]; then
  die "$asset is not listed in SHA256SUMS; refusing to install an unverified binary"
fi

if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/$asset" | cut -d' ' -f1)"
elif command -v shasum >/dev/null 2>&1; then
  actual="$(shasum -a 256 "$tmp/$asset" | cut -d' ' -f1)"
else
  die 'neither sha256sum nor shasum is available'
fi

if [ "$actual" != "$expected" ]; then
  die "checksum mismatch for $asset; the download was not what the release published"
fi

tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$bin_dir"
install -m 755 "$tmp/snip" "$install_tmp"
mv -f "$install_tmp" "$bin_path"

printf 'installed snip %s to %s\n' "$version" "$bin_path"

case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *)
    shell_name=${SHELL-}
    if [ "${shell_name##*/}" = fish ]; then
      if [ "$bin_dir" = "$HOME/.local/bin" ]; then
        printf '%s\n' 'fish_add_path ~/.local/bin'
      else
        printf "fish_add_path '%s'\n" "$bin_dir"
      fi
    elif [ "$bin_dir" = "$HOME/.local/bin" ]; then
      dollar='$'
      printf 'export PATH="%sHOME/.local/bin:%sPATH"\n' "$dollar" "$dollar"
    else
      dollar='$'
      printf 'export PATH="%s:%sPATH"\n' "$bin_dir" "$dollar"
    fi
    ;;
esac

printf '%s\n' 'manual pages: snip man install'
printf '%s\n' 'shell completions: snip completion bash|zsh|fish'
