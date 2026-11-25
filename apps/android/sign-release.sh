#!/usr/bin/env bash
# Signs the unsigned release APK and checks it carries the expected certificate. Only this step sees the
# signing secrets; the Gradle build before it runs without them (see .github/workflows/android-release.yml).
#
# Usage: sign-release.sh <app-release-unsigned.apk> <signed.apk>
# Environment: ANDROID_HOME, QUEST_RELEASE_KEYSTORE_BASE64 (the .jks, base64), QUEST_RELEASE_STORE_PASSWORD,
# QUEST_RELEASE_KEY_ALIAS, QUEST_RELEASE_KEY_PASSWORD, QUEST_RELEASE_CERT_SHA256 (the certificate's SHA-256,
# hex, colons allowed).
set -euo pipefail

unsigned="$1"
signed="$2"

for name in QUEST_RELEASE_KEYSTORE_BASE64 QUEST_RELEASE_STORE_PASSWORD QUEST_RELEASE_KEY_ALIAS \
    QUEST_RELEASE_KEY_PASSWORD QUEST_RELEASE_CERT_SHA256; do
    if [ -z "${!name:-}" ]; then
        echo "error: $name is not set; the release must be signed with the known key." >&2
        exit 1
    fi
done

build_tools="$(dirname "$(find "$ANDROID_HOME/build-tools" -name apksigner -type f | sort -V | tail -1)")"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

echo "$QUEST_RELEASE_KEYSTORE_BASE64" | base64 --decode > "$work/release.jks"
# 16 KB page alignment for the uncompressed native library (Android 15+ devices with 16 KB pages).
"$build_tools/zipalign" -P 16 -f 4 "$unsigned" "$work/aligned.apk"
"$build_tools/apksigner" sign --ks "$work/release.jks" --ks-key-alias "$QUEST_RELEASE_KEY_ALIAS" \
    --ks-pass env:QUEST_RELEASE_STORE_PASSWORD --key-pass env:QUEST_RELEASE_KEY_PASSWORD \
    --v4-signing-enabled false --out "$signed" "$work/aligned.apk"

certificates="$("$build_tools/apksigner" verify --print-certs "$signed")"
actual="$(sed -n 's/^Signer #1 certificate SHA-256 digest: //p' <<< "$certificates")"
expected="$(tr -d ':' <<< "$QUEST_RELEASE_CERT_SHA256" | tr '[:upper:]' '[:lower:]')"
if [ "$(grep -c '^Signer #[0-9]* certificate DN:' <<< "$certificates")" -ne 1 ] || [ "$actual" != "$expected" ]; then
    echo "error: the APK is signed with certificate $actual, expected $expected (QUEST_RELEASE_CERT_SHA256)." >&2
    echo "Check the keystore secret; Obtainium refuses updates signed with another key." >&2
    echo "apksigner ($build_tools) reported:" >&2
    echo "$certificates" >&2
    rm -f "$signed"
    exit 1
fi
echo "Signed with the expected certificate $actual."
