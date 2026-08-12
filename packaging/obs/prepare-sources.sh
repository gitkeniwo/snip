#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 1 ]; then
  echo "usage: $0 OUTPUT_DIRECTORY" >&2
  exit 2
fi

script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(CDPATH= cd -- "$script_dir/../.." && pwd)"
output_dir="$1"
version="${VERSION:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo_root/Cargo.toml" | head -n 1)}"
cargo_bin="${CARGO_BIN:-cargo}"

test -n "$version"
install -d "$output_dir"

work_dir="$(mktemp -d)"
cleanup() {
  rm -rf -- "$work_dir"
}
trap cleanup EXIT

source_root="$work_dir/sniplab-$version-obs"
git -C "$repo_root" archive --format=tar \
  --prefix="sniplab-$version-obs/" HEAD \
  | tar -C "$work_dir" -xf -

install -d "$source_root/.cargo"
(
  cd "$source_root"
  "$cargo_bin" vendor --quiet --locked vendor > .cargo/config.toml
)

# RPM builds keep the upstream source and vendor archives separate. The Debian
# transform uses one combined archive so its offline build has the same inputs.
git -C "$repo_root" archive --format=tar --prefix="snip-$version/" HEAD \
  | gzip -n > "$output_dir/sniplab-$version.tar.gz"
tar -C "$source_root" -czf "$output_dir/sniplab-$version-vendor.tar.gz" vendor
tar -C "$work_dir" -czf "$output_dir/sniplab-$version-obs.tar.gz" \
  "sniplab-$version-obs"

rpm_date="$(LC_ALL=C date -u '+%a %b %d %Y')"
changelog_date="$(LC_ALL=C date -R)"
sed \
  -e "s/@VERSION@/$version/g" \
  -e "s/@DATE@/$rpm_date/g" \
  "$repo_root/packaging/copr/sniplab.spec.in" \
  > "$output_dir/sniplab.spec"
sed \
  -e "s/@VERSION@/$version/g" \
  "$script_dir/sniplab.dsc.in" \
  > "$output_dir/sniplab.dsc"
sed \
  -e "s/@VERSION@/$version/g" \
  -e "s/@DATE@/$changelog_date/g" \
  "$script_dir/debian.changelog.in" \
  > "$output_dir/debian.changelog"

install -m 0644 "$script_dir/debian.control" "$output_dir/debian.control"
install -m 0644 "$script_dir/debian.copyright" "$output_dir/debian.copyright"
install -m 0755 "$script_dir/debian.rules" "$output_dir/debian.rules"
