#!/usr/bin/env bash
# ==============================================================================
# generate-release-notes.sh
# Computes the next SemVer version from git tags and conventional commits (or
# uses an explicit --version), and writes RELEASE_NOTES.md.
# Compatible with both macOS (bash 3.2 / BSD) and Linux (GNU).
# ==============================================================================

set -euo pipefail

VERSION=""
OUTPUT="RELEASE_NOTES.md"
GITHUB_OUTPUT_FILE=""

while [ "$#" -gt 0 ]; do
    case "$1" in
        --version)
            VERSION="${2:-}"
            shift 2
            ;;
        --output)
            OUTPUT="${2:-}"
            shift 2
            ;;
        --github-output)
            GITHUB_OUTPUT_FILE="${2:-}"
            shift 2
            ;;
        *)
            echo "ERROR: Unknown option: $1" >&2
            exit 1
            ;;
    esac
done

LAST_TAG="$(git describe --tags --abbrev=0 --match 'v*' 2>/dev/null || true)"

if [ -n "${LAST_TAG}" ]; then
    LOG_RANGE="${LAST_TAG}..HEAD"
else
    LOG_RANGE="HEAD"
fi

if [ -z "${VERSION}" ]; then
    if [ -z "${LAST_TAG}" ]; then
        VERSION="1.0.0"
    else
        CLEAN_TAG="${LAST_TAG#v}"
        IFS='.' read -r MAJOR MINOR PATCH <<EOF
${CLEAN_TAG}
EOF
        MAJOR="${MAJOR:-1}"
        MINOR="${MINOR:-0}"
        PATCH="${PATCH:-0}"

        BUMP="patch"
        while IFS= read -r HASH; do
            [ -z "${HASH}" ] && continue
            SUBJECT="$(git show -s --format='%s' "${HASH}")"
            case "${SUBJECT}" in
                "chore(release):"*) continue ;;
            esac
            BODY="$(git show -s --format='%b' "${HASH}")"
            if printf '%s\n%s\n' "${SUBJECT}" "${BODY}" | grep -q "BREAKING CHANGE" || \
               printf '%s\n' "${SUBJECT}" | grep -qE '^[a-z]+(\([^)]+\))?!:'; then
                BUMP="major"
                break
            elif printf '%s\n' "${SUBJECT}" | grep -qE '^feat(\([^)]+\))?:'; then
                BUMP="minor"
            fi
        done <<EOF
$(git rev-list --no-merges "${LOG_RANGE}" 2>/dev/null || true)
EOF

        case "${BUMP}" in
            major)
                MAJOR=$((MAJOR + 1))
                MINOR=0
                PATCH=0
                ;;
            minor)
                MINOR=$((MINOR + 1))
                PATCH=0
                ;;
            patch)
                PATCH=$((PATCH + 1))
                ;;
        esac
        VERSION="${MAJOR}.${MINOR}.${PATCH}"
    fi
fi

VERSION="${VERSION#v}"
if ! printf '%s\n' "${VERSION}" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "ERROR: Invalid SemVer version '${VERSION}' (expected X.Y.Z)." >&2
    exit 1
fi

TAG="v${VERSION}"

FEATURES=""
FIXES=""
PERF=""
REFACTOR=""
OTHER=""

if [ -n "${LAST_TAG}" ]; then
    COMMIT_LINES="$(git log --no-merges --pretty=format:'%s (%h)' "${LOG_RANGE}" 2>/dev/null || true)"
else
    COMMIT_LINES="$(git log --no-merges -n 30 --pretty=format:'%s (%h)' "${LOG_RANGE}" 2>/dev/null || true)"
fi

while IFS= read -r LINE; do
    [ -z "${LINE}" ] && continue
    case "${LINE}" in
        "chore(release):"*) continue ;;
    esac

    if printf '%s\n' "${LINE}" | grep -qE '^feat(\([^)]+\))?!?:'; then
        FEATURES="${FEATURES}- ${LINE}
"
    elif printf '%s\n' "${LINE}" | grep -qE '^fix(\([^)]+\))?!?:'; then
        FIXES="${FIXES}- ${LINE}
"
    elif printf '%s\n' "${LINE}" | grep -qE '^perf(\([^)]+\))?!?:'; then
        PERF="${PERF}- ${LINE}
"
    elif printf '%s\n' "${LINE}" | grep -qE '^refactor(\([^)]+\))?!?:'; then
        REFACTOR="${REFACTOR}- ${LINE}
"
    else
        OTHER="${OTHER}- ${LINE}
"
    fi
done <<EOF
${COMMIT_LINES}
EOF

{
    printf '# Equaform-8 %s\n\n' "${TAG}"
    printf '## Changes\n\n'

    HAS_ANY=0
    if [ -n "${FEATURES}" ]; then
        printf '### Features\n\n%s\n' "${FEATURES}"
        HAS_ANY=1
    fi
    if [ -n "${FIXES}" ]; then
        printf '### Fixes\n\n%s\n' "${FIXES}"
        HAS_ANY=1
    fi
    if [ -n "${PERF}" ]; then
        printf '### Performance\n\n%s\n' "${PERF}"
        HAS_ANY=1
    fi
    if [ -n "${REFACTOR}" ]; then
        printf '### Refactoring\n\n%s\n' "${REFACTOR}"
        HAS_ANY=1
    fi
    if [ -n "${OTHER}" ]; then
        printf '### Other Changes\n\n%s\n' "${OTHER}"
        HAS_ANY=1
    fi
    if [ "${HAS_ANY}" -eq 0 ]; then
        printf -- '- Maintenance release.\n\n'
    fi

    cat <<EOF
## Installation

| File | Description |
|------|-------------|
| \`Equaform-8-v${VERSION}.dmg\` | macOS installer DMG (AUv2, VST3, CLAP plugins + \`Equaform-8.app\` standalone + 15 factory presets, Apple Silicon) |
| \`Equaform-8-v${VERSION}.dmg.sha256\` | SHA-256 checksum for macOS DMG |
| \`Equaform-8-Plugins-Windows-v${VERSION}.exe\` | Windows x64 installer (VST3, CLAP plugins + 15 factory presets) |
| \`Equaform-8-Plugins-Windows-v${VERSION}.exe.sha256\` | SHA-256 checksum for Windows installer |

**Requirements:** macOS 12.0+ (Apple Silicon) or Windows 10/11 (x64).

\`\`\`bash
# Verify macOS download integrity
shasum -a 256 -c Equaform-8-v${VERSION}.dmg.sha256
\`\`\`
EOF
} > "${OUTPUT}"

if [ -n "${GITHUB_OUTPUT_FILE}" ]; then
    {
        printf 'version=%s\n' "${VERSION}"
        printf 'tag=%s\n' "${TAG}"
    } >> "${GITHUB_OUTPUT_FILE}"
fi

echo "Generated ${OUTPUT} for ${TAG}"
