#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source_dir="${repo_root}/distribution/itch-html5"
output_dir="${repo_root}/dist/itch"
archive_path="${output_dir}/mir2-platinum-176-web-alpha-html5.zip"
index_path="${source_dir}/index.html"

if [[ ! -f "${index_path}" ]]; then
  echo "Missing itch HTML5 entry point: ${index_path}" >&2
  exit 1
fi

file_count=0
unpacked_bytes=0
largest_file_bytes=0
longest_path_chars=0

while IFS= read -r -d '' file_path; do
  relative_path="${file_path#${source_dir}/}"
  file_bytes="$(wc -c < "${file_path}" | tr -d ' ')"
  path_chars="${#relative_path}"

  file_count=$((file_count + 1))
  unpacked_bytes=$((unpacked_bytes + file_bytes))
  if (( file_bytes > largest_file_bytes )); then
    largest_file_bytes="${file_bytes}"
  fi
  if (( path_chars > longest_path_chars )); then
    longest_path_chars="${path_chars}"
  fi
done < <(find "${source_dir}" -type f -print0)

if (( file_count > 1000 )); then
  echo "itch HTML5 limit exceeded: ${file_count} files (maximum 1000)." >&2
  exit 1
fi

if (( unpacked_bytes > 500 * 1024 * 1024 )); then
  echo "itch HTML5 limit exceeded: ${unpacked_bytes} unpacked bytes (maximum 500 MiB)." >&2
  exit 1
fi

if (( largest_file_bytes > 200 * 1024 * 1024 )); then
  echo "itch HTML5 limit exceeded: largest file is ${largest_file_bytes} bytes (maximum 200 MiB)." >&2
  exit 1
fi

if (( longest_path_chars > 240 )); then
  echo "itch HTML5 limit exceeded: longest path is ${longest_path_chars} characters (maximum 240)." >&2
  exit 1
fi

mkdir -p "${output_dir}"
rm -f "${archive_path}"

(
  cd "${source_dir}"
  zip -q -X -9 "${archive_path}" index.html
)

unzip -tq "${archive_path}"

archive_bytes="$(wc -c < "${archive_path}" | tr -d ' ')"
echo "Built itch HTML5 archive: ${archive_path}"
echo "Source files: ${file_count}; unpacked bytes: ${unpacked_bytes}; archive bytes: ${archive_bytes}"
